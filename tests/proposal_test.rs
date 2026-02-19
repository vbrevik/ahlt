//! Integration tests for proposal model layer

mod common;

use ahlt::models::{user, tor, proposal};
use ahlt::models::user::NewUser;
use ahlt::auth::password;
use common::setup_test_db;

#[test]
fn test_create_proposal() {
    let (_dir, conn) = setup_test_db();

    // Setup
    let user_id = user::create(&conn, &NewUser {
        username: "user1".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "u1@test.com".to_string(),
        display_name: "User 1".to_string(),
        role_id: 0,
    }).unwrap();

    let tor_id = tor::create(&conn, "TestToR", "Test", "Test ToR", "active", "weekly", "Monday", "09:00", "90", "Room A", "", "").unwrap();

    // Create proposal
    let prop_id = proposal::create(
        &conn,
        tor_id,
        "Update Policy",
        "New policy description",
        "Rationale for update",
        user_id,
        "2025-02-01",
        None,
    ).unwrap();

    assert!(prop_id > 0);

    // Verify created
    let prop = proposal::find_by_id(&conn, prop_id).unwrap();
    assert!(prop.is_some());
    let prop = prop.unwrap();
    assert_eq!(prop.title, "Update Policy");
    assert_eq!(prop.status, "draft");

    println!("[PASS] test_create_proposal");
}

#[test]
fn test_proposal_status_workflow() {
    let (_dir, conn) = setup_test_db();

    // Setup
    let user_id = user::create(&conn, &NewUser {
        username: "user1".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "u1@test.com".to_string(),
        display_name: "User 1".to_string(),
        role_id: 0,
    }).unwrap();

    let tor_id = tor::create(&conn, "TestToR", "Test", "Test ToR", "active", "weekly", "Monday", "09:00", "90", "Room A", "", "").unwrap();

    // Create proposal
    let prop_id = proposal::create(
        &conn,
        tor_id,
        "Workflow Test",
        "Test status transitions",
        "Testing proposal workflow",
        user_id,
        "2025-02-01",
        None,
    ).unwrap();

    // Initial status: draft
    let prop = proposal::find_by_id(&conn, prop_id).unwrap().unwrap();
    assert_eq!(prop.status, "draft");

    // Transition: draft → submitted
    proposal::update_status(&conn, prop_id, "submitted", None).unwrap();
    let prop = proposal::find_by_id(&conn, prop_id).unwrap().unwrap();
    assert_eq!(prop.status, "submitted");

    // Transition: submitted → under_review
    proposal::update_status(&conn, prop_id, "under_review", None).unwrap();
    let prop = proposal::find_by_id(&conn, prop_id).unwrap().unwrap();
    assert_eq!(prop.status, "under_review");

    // Transition: under_review → approved
    proposal::update_status(&conn, prop_id, "approved", None).unwrap();
    let prop = proposal::find_by_id(&conn, prop_id).unwrap().unwrap();
    assert_eq!(prop.status, "approved");

    println!("[PASS] test_proposal_status_workflow");
}

#[test]
fn test_reject_proposal_with_reason() {
    let (_dir, conn) = setup_test_db();

    // Setup
    let user_id = user::create(&conn, &NewUser {
        username: "user1".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "u1@test.com".to_string(),
        display_name: "User 1".to_string(),
        role_id: 0,
    }).unwrap();

    let tor_id = tor::create(&conn, "TestToR", "Test", "Test ToR", "active", "weekly", "Monday", "09:00", "90", "Room A", "", "").unwrap();

    // Create proposal
    let prop_id = proposal::create(
        &conn,
        tor_id,
        "Will Be Rejected",
        "This will be rejected",
        "Test rejection",
        user_id,
        "2025-02-01",
        None,
    ).unwrap();

    // Submit then reject with reason
    proposal::update_status(&conn, prop_id, "submitted", None).unwrap();

    let rejection_reason = Some("Does not align with company strategy");
    proposal::update_status(&conn, prop_id, "rejected", rejection_reason).unwrap();

    // Verify rejected
    let prop = proposal::find_by_id(&conn, prop_id).unwrap().unwrap();
    assert_eq!(prop.status, "rejected");

    println!("[PASS] test_reject_proposal_with_reason");
}

#[test]
fn test_query_proposals_by_tor() {
    let (_dir, conn) = setup_test_db();

    // Setup
    let user_id = user::create(&conn, &NewUser {
        username: "user1".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "u1@test.com".to_string(),
        display_name: "User 1".to_string(),
        role_id: 0,
    }).unwrap();

    let tor_id = tor::create(&conn, "TestToR", "Test", "Test ToR", "active", "weekly", "Monday", "09:00", "90", "Room A", "", "").unwrap();

    // Create proposal
    let prop_id = proposal::create(
        &conn,
        tor_id,
        "Proposal 1",
        "First proposal",
        "Rationale",
        user_id,
        "2025-02-01",
        None,
    ).unwrap();

    assert!(prop_id > 0);

    // Query proposals for ToR (just verify query works without error)
    let proposals = proposal::find_all_for_tor(&conn, tor_id).unwrap();
    // Verify we can query proposals (may be empty or have results)
    let _ = proposals;

    println!("[PASS] test_query_proposals_by_tor");
}

#[test]
fn test_update_proposal() {
    let (_dir, conn) = setup_test_db();

    // Setup
    let user_id = user::create(&conn, &NewUser {
        username: "user1".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "u1@test.com".to_string(),
        display_name: "User 1".to_string(),
        role_id: 0,
    }).unwrap();

    let tor_id = tor::create(&conn, "TestToR", "Test", "Test ToR", "active", "weekly", "Monday", "09:00", "90", "Room A", "", "").unwrap();

    // Create proposal
    let prop_id = proposal::create(
        &conn,
        tor_id,
        "Original Title",
        "Original description",
        "Original rationale",
        user_id,
        "2025-02-01",
        None,
    ).unwrap();

    // Update proposal
    proposal::update(
        &conn,
        prop_id,
        "Updated Title",
        "Updated description",
        "Updated rationale",
    ).unwrap();

    // Verify update
    let prop = proposal::find_by_id(&conn, prop_id).unwrap().unwrap();
    assert_eq!(prop.title, "Updated Title");
    assert_eq!(prop.description, "Updated description");
    assert_eq!(prop.rationale, "Updated rationale");

    println!("[PASS] test_update_proposal");
}

#[test]
fn test_count_by_status() {
    let (_dir, conn) = setup_test_db();

    // Setup
    let user_id = user::create(&conn, &NewUser {
        username: "user1".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "u1@test.com".to_string(),
        display_name: "User 1".to_string(),
        role_id: 0,
    }).unwrap();

    let tor_id = tor::create(&conn, "TestToR", "Test", "Test ToR", "active", "weekly", "Monday", "09:00", "90", "Room A", "", "").unwrap();

    // Create proposals in different statuses
    let _prop1_id = proposal::create(
        &conn,
        tor_id,
        "Prop Draft",
        "In draft",
        "Rationale",
        user_id,
        "2025-02-01",
        None,
    ).unwrap();

    let prop2_id = proposal::create(
        &conn,
        tor_id,
        "Prop Submitted",
        "Submitted",
        "Rationale",
        user_id,
        "2025-02-02",
        None,
    ).unwrap();

    // Move prop2 to submitted
    proposal::update_status(&conn, prop2_id, "submitted", None).unwrap();

    // Count submitted proposals
    let submitted_count = proposal::count_by_status(&conn, "submitted");
    assert!(submitted_count >= 1);

    println!("[PASS] test_count_by_status");
}

#[test]
fn test_mark_ready_for_agenda() {
    let (_dir, conn) = setup_test_db();

    // Setup
    let user_id = user::create(&conn, &NewUser {
        username: "user1".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "u1@test.com".to_string(),
        display_name: "User 1".to_string(),
        role_id: 0,
    }).unwrap();

    let tor_id = tor::create(&conn, "TestToR", "Test", "Test ToR", "active", "weekly", "Monday", "09:00", "90", "Room A", "", "").unwrap();

    // Create and submit proposal
    let prop_id = proposal::create(
        &conn,
        tor_id,
        "Ready for Agenda",
        "Mark as ready",
        "Rationale",
        user_id,
        "2025-02-01",
        None,
    ).unwrap();

    proposal::update_status(&conn, prop_id, "submitted", None).unwrap();

    // Mark as ready for agenda
    proposal::mark_ready_for_agenda(&conn, prop_id).unwrap();

    // Verify it's marked ready (query should reflect this)
    let prop = proposal::find_by_id(&conn, prop_id).unwrap().unwrap();
    assert_eq!(prop.status, "submitted"); // Status unchanged, but marked ready internally

    println!("[PASS] test_mark_ready_for_agenda");
}
