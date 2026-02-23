//! Integration tests for the opinion model layer.
//!
//! Tests cover: record_opinion, find_opinions_for_agenda_point,
//! find_opinion_by_user_and_agenda_point, update_opinion, record_decision.

mod common;

use ahlt::auth::password;
use ahlt::models::user::NewUser;
use ahlt::models::{agenda_point, coa, opinion, relation, tor, user};
use common::setup_test_db;

/// Helper: create a user with a unique username for the given test.
async fn create_test_user(pool: &sqlx::PgPool, suffix: &str) -> i64 {
    user::create(
        pool,
        &NewUser {
            username: format!("optest_user_{}", suffix),
            password: password::hash_password("pass").unwrap(),
            email: format!("{}@test.com", suffix),
            display_name: format!("User {}", suffix),
        },
    )
    .await
    .unwrap()
}

/// Helper: create a ToR, agenda point, and two COAs linked to the agenda point.
/// Returns (tor_id, agenda_point_id, coa1_id, coa2_id).
async fn create_ap_with_coas(
    pool: &sqlx::PgPool,
    prefix: &str,
    user_id: i64,
) -> (i64, i64, i64, i64) {
    let tor_id = tor::create(
        pool,
        &format!("{}_tor", prefix),
        &format!("{} ToR", prefix),
        &[("status", "active")],
    )
    .await
    .unwrap();

    let ap_id = agenda_point::create(
        pool,
        tor_id,
        &format!("{} Agenda Point", prefix),
        "Test agenda point description",
        "decision",
        "2026-03-01",
        30,
        user_id,
        "",  // presenter
        "",  // priority
        "",  // pre_read_url
    )
    .await
    .unwrap();

    let coa1_id = coa::create(
        pool,
        &format!("{} COA Alpha", prefix),
        "First course of action",
        "simple",
        user_id,
    )
    .await
    .unwrap();

    let coa2_id = coa::create(
        pool,
        &format!("{} COA Beta", prefix),
        "Second course of action",
        "simple",
        user_id,
    )
    .await
    .unwrap();

    // Link COAs to the agenda point via considers_coa relation
    relation::create(pool, "considers_coa", ap_id, coa1_id)
        .await
        .unwrap();
    relation::create(pool, "considers_coa", ap_id, coa2_id)
        .await
        .unwrap();

    (tor_id, ap_id, coa1_id, coa2_id)
}

#[tokio::test]
async fn test_record_opinion() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let user_id = create_test_user(pool, "rec_op").await;
    let (_tor_id, ap_id, coa1_id, _coa2_id) =
        create_ap_with_coas(pool, "rec_op", user_id).await;

    // Record an opinion
    let opinion_id = opinion::record_opinion(
        pool,
        ap_id,
        user_id,
        coa1_id,
        "I strongly prefer COA Alpha",
    )
    .await
    .unwrap();

    assert!(opinion_id > 0, "opinion id should be positive");

    // Verify via find_opinion_by_id
    let detail = opinion::find_opinion_by_id(pool, opinion_id)
        .await
        .unwrap();
    assert!(detail.is_some(), "opinion should be found by id");

    let detail = detail.unwrap();
    assert_eq!(detail.id, opinion_id);
    assert_eq!(detail.agenda_point_id, ap_id);
    assert_eq!(detail.recorded_by, user_id);
    assert_eq!(detail.recorded_by_name, "User rec_op");
    assert_eq!(detail.preferred_coa_id, coa1_id);
    assert_eq!(detail.commentary, "I strongly prefer COA Alpha");
    assert!(!detail.created_date.is_empty(), "created_date should be set");

    println!("[PASS] test_record_opinion");
}

#[tokio::test]
async fn test_find_opinions_for_agenda_point() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let user1_id = create_test_user(pool, "find_ops_u1").await;
    let user2_id = create_test_user(pool, "find_ops_u2").await;
    let (_tor_id, ap_id, coa1_id, coa2_id) =
        create_ap_with_coas(pool, "find_ops", user1_id).await;

    // Two users record opinions on the same agenda point
    opinion::record_opinion(pool, ap_id, user1_id, coa1_id, "User1 prefers Alpha")
        .await
        .unwrap();
    opinion::record_opinion(pool, ap_id, user2_id, coa2_id, "User2 prefers Beta")
        .await
        .unwrap();

    // Fetch all opinions for this agenda point
    let opinions = opinion::find_opinions_for_agenda_point(pool, ap_id)
        .await
        .unwrap();
    assert_eq!(opinions.len(), 2, "should find exactly 2 opinions");

    // Verify user1's opinion
    let op1 = opinions
        .iter()
        .find(|o| o.recorded_by == user1_id);
    assert!(op1.is_some(), "user1 opinion should be present");
    let op1 = op1.unwrap();
    assert_eq!(op1.preferred_coa_id, coa1_id);
    assert_eq!(op1.commentary, "User1 prefers Alpha");

    // Verify user2's opinion
    let op2 = opinions
        .iter()
        .find(|o| o.recorded_by == user2_id);
    assert!(op2.is_some(), "user2 opinion should be present");
    let op2 = op2.unwrap();
    assert_eq!(op2.preferred_coa_id, coa2_id);
    assert_eq!(op2.commentary, "User2 prefers Beta");

    println!("[PASS] test_find_opinions_for_agenda_point");
}

#[tokio::test]
async fn test_find_opinion_by_user_and_agenda_point() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let user1_id = create_test_user(pool, "lookup_u1").await;
    let user2_id = create_test_user(pool, "lookup_u2").await;
    let (_tor_id, ap_id, coa1_id, _coa2_id) =
        create_ap_with_coas(pool, "lookup", user1_id).await;

    // User1 records an opinion
    let opinion_id = opinion::record_opinion(
        pool,
        ap_id,
        user1_id,
        coa1_id,
        "Lookup test commentary",
    )
    .await
    .unwrap();

    // User1 should be found
    let found = opinion::find_opinion_by_user_and_agenda_point(pool, user1_id, ap_id)
        .await
        .unwrap();
    assert_eq!(found, Some(opinion_id), "should find user1's opinion id");

    // User2 has NOT recorded an opinion — should return None
    let not_found = opinion::find_opinion_by_user_and_agenda_point(pool, user2_id, ap_id)
        .await
        .unwrap();
    assert_eq!(not_found, None, "user2 has no opinion, should be None");

    println!("[PASS] test_find_opinion_by_user_and_agenda_point");
}

#[tokio::test]
async fn test_update_opinion() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let user_id = create_test_user(pool, "upd_op").await;
    let (_tor_id, ap_id, coa1_id, coa2_id) =
        create_ap_with_coas(pool, "upd_op", user_id).await;

    // Record initial opinion preferring COA Alpha
    let opinion_id = opinion::record_opinion(
        pool,
        ap_id,
        user_id,
        coa1_id,
        "Original commentary",
    )
    .await
    .unwrap();

    // Verify initial state
    let before = opinion::find_opinion_by_id(pool, opinion_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before.preferred_coa_id, coa1_id);
    assert_eq!(before.commentary, "Original commentary");

    // Update to prefer COA Beta with new commentary
    opinion::update_opinion(pool, opinion_id, coa2_id, "Revised commentary")
        .await
        .unwrap();

    // Verify the update persisted
    let after = opinion::find_opinion_by_id(pool, opinion_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.preferred_coa_id, coa2_id, "preferred COA should be updated");
    assert_eq!(after.commentary, "Revised commentary", "commentary should be updated");

    // Other fields should remain unchanged
    assert_eq!(after.id, opinion_id);
    assert_eq!(after.agenda_point_id, ap_id);
    assert_eq!(after.recorded_by, user_id);

    println!("[PASS] test_update_opinion");
}

#[tokio::test]
async fn test_record_decision() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let user_id = create_test_user(pool, "dec").await;
    let (_tor_id, ap_id, coa1_id, _coa2_id) =
        create_ap_with_coas(pool, "dec", user_id).await;

    // Record a decision on the agenda point
    let decision_id = opinion::record_decision(
        pool,
        ap_id,
        user_id,
        coa1_id,
        "COA Alpha is the most viable option",
    )
    .await
    .unwrap();

    assert!(decision_id > 0, "decision id should be positive");

    // Verify the agenda point status was updated to "voted"
    let ap = agenda_point::find_by_id(pool, ap_id).await.unwrap();
    assert!(ap.is_some(), "agenda point should still exist");
    let ap = ap.unwrap();
    assert_eq!(ap.status, "voted", "agenda point status should be 'voted' after decision");

    println!("[PASS] test_record_decision");
}
