//! Integration tests for the suggestion model layer

mod common;

use ahlt::auth::password;
use ahlt::models::user::NewUser;
use ahlt::models::{suggestion, tor, user};
use common::setup_test_db;

/// Helper: create a test user, returning user id.
async fn create_test_user(pool: &sqlx::PgPool, suffix: &str) -> i64 {
    user::create(
        pool,
        &NewUser {
            username: format!("sugtest_{}", suffix),
            password: password::hash_password("pass").unwrap(),
            email: format!("sugtest_{}@test.com", suffix),
            display_name: format!("Suggestion Tester {}", suffix),
        },
    )
    .await
    .unwrap()
}

/// Helper: create a test ToR with minimal required properties, returning ToR id.
async fn create_test_tor(pool: &sqlx::PgPool, name: &str) -> i64 {
    tor::create(pool, name, name, &[("status", "active")])
        .await
        .unwrap()
}

#[tokio::test]
async fn test_create_suggestion() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let user_id = create_test_user(pool, "create").await;
    let tor_id = create_test_tor(pool, "tor_create_sug").await;

    let sug_id = suggestion::create(
        pool,
        tor_id,
        "We should improve onboarding documentation",
        user_id,
        "2025-06-01",
    )
    .await
    .unwrap();

    assert!(sug_id > 0);

    // Verify via find_by_id
    let detail = suggestion::find_by_id(pool, sug_id).await.unwrap();
    assert!(detail.is_some());

    let detail = detail.unwrap();
    assert_eq!(detail.id, sug_id);
    assert_eq!(detail.description, "We should improve onboarding documentation");
    assert_eq!(detail.status, "open");
    assert_eq!(detail.submitted_by_id, user_id);
    assert_eq!(detail.submitted_date, "2025-06-01");
    assert!(detail.rejection_reason.is_none());
    assert!(detail.spawned_proposal_id.is_none());
}

#[tokio::test]
async fn test_find_suggestions_for_tor() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let user_id = create_test_user(pool, "list").await;
    let tor_id = create_test_tor(pool, "tor_list_sug").await;

    // Create two suggestions with different dates to avoid UNIQUE name collision
    let sug1 = suggestion::create(
        pool,
        tor_id,
        "First suggestion for the team",
        user_id,
        "2025-07-01",
    )
    .await
    .unwrap();

    let sug2 = suggestion::create(
        pool,
        tor_id,
        "Second suggestion about process",
        user_id,
        "2025-07-02",
    )
    .await
    .unwrap();

    let items = suggestion::find_all_for_tor(pool, tor_id).await.unwrap();
    assert_eq!(items.len(), 2);

    // Results are ordered by submitted_date DESC, so sug2 comes first
    let ids: Vec<i64> = items.iter().map(|i| i.id).collect();
    assert!(ids.contains(&sug1));
    assert!(ids.contains(&sug2));

    // Verify descriptions are present
    let descs: Vec<&str> = items.iter().map(|i| i.description.as_str()).collect();
    assert!(descs.contains(&"First suggestion for the team"));
    assert!(descs.contains(&"Second suggestion about process"));
}

#[tokio::test]
async fn test_update_status_to_accepted() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let user_id = create_test_user(pool, "accept").await;
    let tor_id = create_test_tor(pool, "tor_accept_sug").await;

    let sug_id = suggestion::create(
        pool,
        tor_id,
        "Suggestion that will be accepted",
        user_id,
        "2025-08-01",
    )
    .await
    .unwrap();

    // Verify initial status
    let detail = suggestion::find_by_id(pool, sug_id).await.unwrap().unwrap();
    assert_eq!(detail.status, "open");

    // Update to accepted
    suggestion::update_status(pool, sug_id, "accepted", None)
        .await
        .unwrap();

    // Verify updated status
    let detail = suggestion::find_by_id(pool, sug_id).await.unwrap().unwrap();
    assert_eq!(detail.status, "accepted");
    assert!(detail.rejection_reason.is_none());
}

#[tokio::test]
async fn test_update_status_to_rejected() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let user_id = create_test_user(pool, "reject").await;
    let tor_id = create_test_tor(pool, "tor_reject_sug").await;

    let sug_id = suggestion::create(
        pool,
        tor_id,
        "Suggestion that will be rejected",
        user_id,
        "2025-09-01",
    )
    .await
    .unwrap();

    // Reject with a reason
    suggestion::update_status(
        pool,
        sug_id,
        "rejected",
        Some("Out of scope for this term of reference"),
    )
    .await
    .unwrap();

    // Verify rejected status and reason
    let detail = suggestion::find_by_id(pool, sug_id).await.unwrap().unwrap();
    assert_eq!(detail.status, "rejected");
    assert_eq!(
        detail.rejection_reason.as_deref(),
        Some("Out of scope for this term of reference")
    );
}

#[tokio::test]
async fn test_find_by_id_nonexistent() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Use a large bogus ID that won't exist
    let result = suggestion::find_by_id(pool, 999_999).await.unwrap();
    assert!(result.is_none());
}
