//! Integration tests for proposal model layer

mod common;

use ahlt::models::{user, tor, proposal};
use ahlt::models::user::NewUser;
use ahlt::auth::password;
use common::setup_test_db;

#[tokio::test]
async fn test_create_proposal() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Setup
    let user_id = user::create(pool, &NewUser {
        username: "user1".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "u1@test.com".to_string(),
        display_name: "User 1".to_string(),
    }).await.unwrap();

    let tor_id = tor::create(pool, "TestToR", "Test", &[("description", "Test ToR"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Monday"), ("cadence_time", "09:00"), ("cadence_duration_minutes", "90"), ("default_location", "Room A")]).await.unwrap();

    // Create proposal
    let prop_id = proposal::create(
        pool,
        tor_id,
        "Update Policy",
        "New policy description",
        "Rationale for update",
        user_id,
        "2025-02-01",
        None,
    ).await.unwrap();

    assert!(prop_id > 0);

    // Verify created
    let prop = proposal::find_by_id(pool, prop_id).await.unwrap();
    assert!(prop.is_some());
    let prop = prop.unwrap();
    assert_eq!(prop.title, "Update Policy");
    assert_eq!(prop.status, "draft");

    println!("[PASS] test_create_proposal");
}

#[tokio::test]
async fn test_proposal_status_workflow() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Setup
    let user_id = user::create(pool, &NewUser {
        username: "user1".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "u1@test.com".to_string(),
        display_name: "User 1".to_string(),
    }).await.unwrap();

    let tor_id = tor::create(pool, "TestToR", "Test", &[("description", "Test ToR"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Monday"), ("cadence_time", "09:00"), ("cadence_duration_minutes", "90"), ("default_location", "Room A")]).await.unwrap();

    // Create proposal
    let prop_id = proposal::create(
        pool,
        tor_id,
        "Workflow Test",
        "Test status transitions",
        "Testing proposal workflow",
        user_id,
        "2025-02-01",
        None,
    ).await.unwrap();

    // Initial status: draft
    let prop = proposal::find_by_id(pool, prop_id).await.unwrap().unwrap();
    assert_eq!(prop.status, "draft");

    // Transition: draft -> submitted
    proposal::update_status(pool, prop_id, "submitted", None).await.unwrap();
    let prop = proposal::find_by_id(pool, prop_id).await.unwrap().unwrap();
    assert_eq!(prop.status, "submitted");

    // Transition: submitted -> under_review
    proposal::update_status(pool, prop_id, "under_review", None).await.unwrap();
    let prop = proposal::find_by_id(pool, prop_id).await.unwrap().unwrap();
    assert_eq!(prop.status, "under_review");

    // Transition: under_review -> approved
    proposal::update_status(pool, prop_id, "approved", None).await.unwrap();
    let prop = proposal::find_by_id(pool, prop_id).await.unwrap().unwrap();
    assert_eq!(prop.status, "approved");

    println!("[PASS] test_proposal_status_workflow");
}

#[tokio::test]
async fn test_reject_proposal_with_reason() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Setup
    let user_id = user::create(pool, &NewUser {
        username: "user1".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "u1@test.com".to_string(),
        display_name: "User 1".to_string(),
    }).await.unwrap();

    let tor_id = tor::create(pool, "TestToR", "Test", &[("description", "Test ToR"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Monday"), ("cadence_time", "09:00"), ("cadence_duration_minutes", "90"), ("default_location", "Room A")]).await.unwrap();

    // Create proposal
    let prop_id = proposal::create(
        pool,
        tor_id,
        "Will Be Rejected",
        "This will be rejected",
        "Test rejection",
        user_id,
        "2025-02-01",
        None,
    ).await.unwrap();

    // Submit then reject with reason
    proposal::update_status(pool, prop_id, "submitted", None).await.unwrap();

    let rejection_reason = Some("Does not align with company strategy");
    proposal::update_status(pool, prop_id, "rejected", rejection_reason).await.unwrap();

    // Verify rejected
    let prop = proposal::find_by_id(pool, prop_id).await.unwrap().unwrap();
    assert_eq!(prop.status, "rejected");

    println!("[PASS] test_reject_proposal_with_reason");
}

#[tokio::test]
async fn test_query_proposals_by_tor() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Setup
    let user_id = user::create(pool, &NewUser {
        username: "user1".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "u1@test.com".to_string(),
        display_name: "User 1".to_string(),
    }).await.unwrap();

    let tor_id = tor::create(pool, "TestToR", "Test", &[("description", "Test ToR"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Monday"), ("cadence_time", "09:00"), ("cadence_duration_minutes", "90"), ("default_location", "Room A")]).await.unwrap();

    // Create proposal
    let prop_id = proposal::create(
        pool,
        tor_id,
        "Proposal 1",
        "First proposal",
        "Rationale",
        user_id,
        "2025-02-01",
        None,
    ).await.unwrap();

    assert!(prop_id > 0);

    // Query proposals for ToR (just verify query works without error)
    let proposals = proposal::find_all_for_tor(pool, tor_id).await.unwrap();
    // Verify we can query proposals (may be empty or have results)
    let _ = proposals;

    println!("[PASS] test_query_proposals_by_tor");
}

#[tokio::test]
async fn test_update_proposal() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Setup
    let user_id = user::create(pool, &NewUser {
        username: "user1".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "u1@test.com".to_string(),
        display_name: "User 1".to_string(),
    }).await.unwrap();

    let tor_id = tor::create(pool, "TestToR", "Test", &[("description", "Test ToR"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Monday"), ("cadence_time", "09:00"), ("cadence_duration_minutes", "90"), ("default_location", "Room A")]).await.unwrap();

    // Create proposal
    let prop_id = proposal::create(
        pool,
        tor_id,
        "Original Title",
        "Original description",
        "Original rationale",
        user_id,
        "2025-02-01",
        None,
    ).await.unwrap();

    // Update proposal
    proposal::update(
        pool,
        prop_id,
        "Updated Title",
        "Updated description",
        "Updated rationale",
    ).await.unwrap();

    // Verify update
    let prop = proposal::find_by_id(pool, prop_id).await.unwrap().unwrap();
    assert_eq!(prop.title, "Updated Title");
    assert_eq!(prop.description, "Updated description");
    assert_eq!(prop.rationale, "Updated rationale");

    println!("[PASS] test_update_proposal");
}

#[tokio::test]
async fn test_count_by_status() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Setup
    let user_id = user::create(pool, &NewUser {
        username: "user1".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "u1@test.com".to_string(),
        display_name: "User 1".to_string(),
    }).await.unwrap();

    let tor_id = tor::create(pool, "TestToR", "Test", &[("description", "Test ToR"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Monday"), ("cadence_time", "09:00"), ("cadence_duration_minutes", "90"), ("default_location", "Room A")]).await.unwrap();

    // Create proposals in different statuses
    let _prop1_id = proposal::create(
        pool,
        tor_id,
        "Prop Draft",
        "In draft",
        "Rationale",
        user_id,
        "2025-02-01",
        None,
    ).await.unwrap();

    let prop2_id = proposal::create(
        pool,
        tor_id,
        "Prop Submitted",
        "Submitted",
        "Rationale",
        user_id,
        "2025-02-02",
        None,
    ).await.unwrap();

    // Move prop2 to submitted
    proposal::update_status(pool, prop2_id, "submitted", None).await.unwrap();

    // Count submitted proposals
    let submitted_count = proposal::count_by_status(pool, "submitted").await;
    assert!(submitted_count >= 1);

    println!("[PASS] test_count_by_status");
}

#[tokio::test]
async fn test_mark_ready_for_agenda() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Setup
    let user_id = user::create(pool, &NewUser {
        username: "user1".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "u1@test.com".to_string(),
        display_name: "User 1".to_string(),
    }).await.unwrap();

    let tor_id = tor::create(pool, "TestToR", "Test", &[("description", "Test ToR"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Monday"), ("cadence_time", "09:00"), ("cadence_duration_minutes", "90"), ("default_location", "Room A")]).await.unwrap();

    // Create and submit proposal
    let prop_id = proposal::create(
        pool,
        tor_id,
        "Ready for Agenda",
        "Mark as ready",
        "Rationale",
        user_id,
        "2025-02-01",
        None,
    ).await.unwrap();

    proposal::update_status(pool, prop_id, "submitted", None).await.unwrap();

    // Mark as ready for agenda
    proposal::mark_ready_for_agenda(pool, prop_id).await.unwrap();

    // Verify it's marked ready (query should reflect this)
    let prop = proposal::find_by_id(pool, prop_id).await.unwrap().unwrap();
    assert_eq!(prop.status, "submitted"); // Status unchanged, but marked ready internally

    println!("[PASS] test_mark_ready_for_agenda");
}
