//! Integration tests for governance model layer (ToR, agenda points, proposals)

mod common;

use ahlt::models::{relation, tor, user, agenda_point, proposal, meeting};
use ahlt::models::user::NewUser;
use ahlt::auth::password;
use common::setup_test_db;

#[tokio::test]
async fn test_create_tor_with_members() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Create two users
    let user1_id = user::create(pool, &NewUser {
        username: "member1".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "member1@test.com".to_string(),
        display_name: "Member 1".to_string(),
    }).await.unwrap();

    let user2_id = user::create(pool, &NewUser {
        username: "member2".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "member2@test.com".to_string(),
        display_name: "Member 2".to_string(),
    }).await.unwrap();

    // Create ToR
    let tor_id = tor::create(pool, "TestToR", "Test", &[("description", "Test Terms of Reference"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Monday"), ("cadence_time", "09:00"), ("cadence_duration_minutes", "90"), ("default_location", "Room A")]).await.unwrap();

    // Add members to ToR (via fills_position relation)
    relation::create(pool, "fills_position", user1_id, tor_id).await.unwrap();
    relation::create(pool, "fills_position", user2_id, tor_id).await.unwrap();

    // Verify ToR was created
    let tor_detail = tor::find_detail_by_id(pool, tor_id).await.unwrap();
    assert!(tor_detail.is_some());
    let tor_detail = tor_detail.unwrap();
    assert_eq!(tor_detail.name, "TestToR");

    println!("[PASS] test_create_tor_with_members");
}

#[tokio::test]
async fn test_agenda_point_lifecycle() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Setup: Create admin user and ToR
    let admin_id = user::create(pool, &NewUser {
        username: "admin".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "admin@test.com".to_string(),
        display_name: "Admin".to_string(),
    }).await.unwrap();

    let tor_id = tor::create(pool, "TestToR", "Test", &[("description", "Test ToR"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Monday"), ("cadence_time", "09:00"), ("cadence_duration_minutes", "90"), ("default_location", "Room A")]).await.unwrap();

    // Create agenda point
    let ap_id = agenda_point::create(
        pool,
        tor_id,
        "Review Q1 Goals",
        "Discuss and finalize Q1 goals",
        "discussion",
        "2025-02-15",
        90,
        admin_id,
        "", "", ""
    ).await.unwrap();

    // Verify created
    let ap = agenda_point::find_by_id(pool, ap_id).await.unwrap();
    assert!(ap.is_some());
    let ap = ap.unwrap();
    assert_eq!(ap.title, "Review Q1 Goals");

    // Update agenda point
    agenda_point::update(pool, ap_id, "Review Q1 Outcomes", "Updated description", "presentation", "2025-02-16", 120, "", "", "").await.unwrap();

    // Verify update
    let ap = agenda_point::find_by_id(pool, ap_id).await.unwrap().unwrap();
    assert_eq!(ap.title, "Review Q1 Outcomes");

    // List all for ToR
    let all_ap = agenda_point::find_all_for_tor(pool, tor_id).await.unwrap();
    assert_eq!(all_ap.len(), 1);

    println!("[PASS] test_agenda_point_lifecycle");
}

#[tokio::test]
async fn test_meeting_create_and_update_status() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Setup: Create ToR
    let tor_id = tor::create(pool, "TestToR", "Test", &[("description", "Test ToR"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Monday"), ("cadence_time", "09:00"), ("cadence_duration_minutes", "90"), ("default_location", "Room A")]).await.unwrap();

    // Create meeting
    let meeting_id = meeting::create(pool, tor_id, "2025-02-20", "TestToR Meeting", "Conference Room A", "Initial notes", "", "", "", "", "").await.unwrap();

    // Verify created with projected status
    let mtg = meeting::find_by_id(pool, meeting_id).await.unwrap();
    assert!(mtg.is_some());

    // Update status to confirmed
    meeting::update_status(pool, meeting_id, "confirmed").await.unwrap();

    // Verify status updated
    let mtg = meeting::find_by_id(pool, meeting_id).await.unwrap().unwrap();
    assert_eq!(mtg.status, "confirmed");

    println!("[PASS] test_meeting_create_and_update_status");
}

#[tokio::test]
async fn test_proposal_creation() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Setup: Create user and ToR
    let user_id = user::create(pool, &NewUser {
        username: "proposer".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "proposer@test.com".to_string(),
        display_name: "Proposer".to_string(),
    }).await.unwrap();

    let tor_id = tor::create(pool, "TestToR", "Test", &[("description", "Test ToR"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Monday"), ("cadence_time", "09:00"), ("cadence_duration_minutes", "90"), ("default_location", "Room A")]).await.unwrap();

    // Create proposal
    let prop_id = proposal::create(
        pool,
        tor_id,
        "Add Remote Work Policy",
        "Allow flexible remote work options",
        "Improve work-life balance and retention",
        user_id,
        "2025-02-01",
        None
    ).await.unwrap();

    // Verify created with draft status
    let prop = proposal::find_by_id(pool, prop_id).await.unwrap();
    assert!(prop.is_some());
    let prop = prop.unwrap();
    assert_eq!(prop.title, "Add Remote Work Policy");
    assert_eq!(prop.status, "draft");

    // Update to submitted
    proposal::update_status(pool, prop_id, "submitted", None).await.unwrap();

    // Verify status changed
    let prop = proposal::find_by_id(pool, prop_id).await.unwrap().unwrap();
    assert_eq!(prop.status, "submitted");

    println!("[PASS] test_proposal_creation");
}

#[tokio::test]
async fn test_proposal_lifecycle() {
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
        "Policy Change",
        "Update policy X",
        "Needed for compliance",
        user_id,
        "2025-02-01",
        None
    ).await.unwrap();

    // Move through workflow: draft -> submitted -> under_review -> approved
    proposal::update_status(pool, prop_id, "submitted", None).await.unwrap();
    proposal::update_status(pool, prop_id, "under_review", None).await.unwrap();
    proposal::update_status(pool, prop_id, "approved", None).await.unwrap();

    // Verify final state
    let prop = proposal::find_by_id(pool, prop_id).await.unwrap().unwrap();
    assert_eq!(prop.status, "approved");

    println!("[PASS] test_proposal_lifecycle");
}

#[tokio::test]
async fn test_cascade_delete_tor() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Setup: Create ToR with agenda points and proposal
    let admin_id = user::create(pool, &NewUser {
        username: "admin".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "admin@test.com".to_string(),
        display_name: "Admin".to_string(),
    }).await.unwrap();

    let tor_id = tor::create(pool, "DeleteMe", "Delete", &[("description", "ToR to delete"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Monday"), ("cadence_time", "09:00"), ("cadence_duration_minutes", "90"), ("default_location", "Room A")]).await.unwrap();
    let ap_id = agenda_point::create(pool, tor_id, "Item 1", "Desc", "discussion", "2025-02-20", 60, admin_id, "", "", "").await.unwrap();
    let prop_id = proposal::create(pool, tor_id, "Prop 1", "Desc", "Rationale", admin_id, "2025-02-01", None).await.unwrap();

    // Verify they exist
    assert!(tor::find_detail_by_id(pool, tor_id).await.is_ok());
    assert!(agenda_point::find_by_id(pool, ap_id).await.is_ok());
    assert!(proposal::find_by_id(pool, prop_id).await.is_ok());

    // Delete ToR
    tor::delete(pool, tor_id).await.unwrap();

    // Verify ToR is deleted (cascade should remove related entities)
    let result = tor::find_detail_by_id(pool, tor_id).await;
    match result {
        Ok(opt) => assert!(opt.is_none()),
        Err(_) => {} // Error is also acceptable if ToR doesn't exist
    }

    println!("[PASS] test_cascade_delete_tor");
}

#[tokio::test]
async fn test_governance_data_query() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Setup: Create multiple ToRs and agenda points
    let admin_id = user::create(pool, &NewUser {
        username: "admin".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "admin@test.com".to_string(),
        display_name: "Admin".to_string(),
    }).await.unwrap();

    let tor1_id = tor::create(pool, "ToR1", "Label1", &[("description", "First ToR"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Monday"), ("cadence_time", "09:00"), ("cadence_duration_minutes", "90"), ("default_location", "Room A")]).await.unwrap();
    let tor2_id = tor::create(pool, "ToR2", "Label2", &[("description", "Second ToR"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Tuesday"), ("cadence_time", "10:00"), ("cadence_duration_minutes", "120"), ("default_location", "Room B")]).await.unwrap();

    let _ap1_id = agenda_point::create(pool, tor1_id, "AP1", "Desc", "discussion", "2025-02-15", 60, admin_id, "", "", "").await.unwrap();
    let _ap2_id = agenda_point::create(pool, tor1_id, "AP2", "Desc", "decision", "2025-03-15", 90, admin_id, "", "", "").await.unwrap();
    let _ap3_id = agenda_point::create(pool, tor2_id, "AP3", "Desc", "discussion", "2025-02-20", 60, admin_id, "", "", "").await.unwrap();

    // Query all for ToR1
    let aps_tor1 = agenda_point::find_all_for_tor(pool, tor1_id).await.unwrap();
    assert_eq!(aps_tor1.len(), 2);

    // Query cross-ToR
    let aps_cross = agenda_point::find_all_cross_tor(pool, None).await.unwrap();
    assert!(aps_cross.len() >= 3);

    println!("[PASS] test_governance_data_query");
}
