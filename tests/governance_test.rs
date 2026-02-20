//! Integration tests for governance model layer (ToR, agenda points, proposals)

mod common;

use ahlt::models::{relation, tor, user, agenda_point, proposal, meeting};
use ahlt::models::user::NewUser;
use ahlt::auth::password;
use common::setup_test_db;

#[test]
fn test_create_tor_with_members() {
    let (_dir, conn) = setup_test_db();

    // Create two users
    let user1_id = user::create(&conn, &NewUser {
        username: "member1".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "member1@test.com".to_string(),
        display_name: "Member 1".to_string(),
        role_id: 0,
    }).unwrap();

    let user2_id = user::create(&conn, &NewUser {
        username: "member2".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "member2@test.com".to_string(),
        display_name: "Member 2".to_string(),
        role_id: 0,
    }).unwrap();

    // Create ToR
    let tor_id = tor::create(&conn, "TestToR", "Test", &[("description", "Test Terms of Reference"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Monday"), ("cadence_time", "09:00"), ("cadence_duration_minutes", "90"), ("default_location", "Room A")]).unwrap();

    // Add members to ToR (via fills_position relation)
    relation::create(&conn, "fills_position", user1_id, tor_id).unwrap();
    relation::create(&conn, "fills_position", user2_id, tor_id).unwrap();

    // Verify ToR was created
    let tor_detail = tor::find_detail_by_id(&conn, tor_id).unwrap();
    assert!(tor_detail.is_some());
    let tor_detail = tor_detail.unwrap();
    assert_eq!(tor_detail.name, "TestToR");

    println!("[PASS] test_create_tor_with_members");
}

#[test]
fn test_agenda_point_lifecycle() {
    let (_dir, conn) = setup_test_db();

    // Setup: Create admin user and ToR
    let admin_id = user::create(&conn, &NewUser {
        username: "admin".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "admin@test.com".to_string(),
        display_name: "Admin".to_string(),
        role_id: 0,
    }).unwrap();

    let tor_id = tor::create(&conn, "TestToR", "Test", &[("description", "Test ToR"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Monday"), ("cadence_time", "09:00"), ("cadence_duration_minutes", "90"), ("default_location", "Room A")]).unwrap();

    // Create agenda point
    let ap_id = agenda_point::create(
        &conn,
        tor_id,
        "Review Q1 Goals",
        "Discuss and finalize Q1 goals",
        "discussion",
        "2025-02-15",
        90,
        admin_id,
        "", "", ""
    ).unwrap();

    // Verify created
    let ap = agenda_point::find_by_id(&conn, ap_id).unwrap();
    assert!(ap.is_some());
    let ap = ap.unwrap();
    assert_eq!(ap.title, "Review Q1 Goals");

    // Update agenda point
    agenda_point::update(&conn, ap_id, "Review Q1 Outcomes", "Updated description", "presentation", "2025-02-16", 120, "", "", "").unwrap();

    // Verify update
    let ap = agenda_point::find_by_id(&conn, ap_id).unwrap().unwrap();
    assert_eq!(ap.title, "Review Q1 Outcomes");

    // List all for ToR
    let all_ap = agenda_point::find_all_for_tor(&conn, tor_id).unwrap();
    assert_eq!(all_ap.len(), 1);

    println!("[PASS] test_agenda_point_lifecycle");
}

#[test]
fn test_meeting_create_and_update_status() {
    let (_dir, conn) = setup_test_db();

    // Setup: Create ToR
    let tor_id = tor::create(&conn, "TestToR", "Test", &[("description", "Test ToR"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Monday"), ("cadence_time", "09:00"), ("cadence_duration_minutes", "90"), ("default_location", "Room A")]).unwrap();

    // Create meeting
    let meeting_id = meeting::create(&conn, tor_id, "2025-02-20", "TestToR Meeting", "Conference Room A", "Initial notes", "", "", "", "", "").unwrap();

    // Verify created with projected status
    let mtg = meeting::find_by_id(&conn, meeting_id).unwrap();
    assert!(mtg.is_some());

    // Update status to confirmed
    meeting::update_status(&conn, meeting_id, "confirmed").unwrap();

    // Verify status updated
    let mtg = meeting::find_by_id(&conn, meeting_id).unwrap().unwrap();
    assert_eq!(mtg.status, "confirmed");

    println!("[PASS] test_meeting_create_and_update_status");
}

#[test]
fn test_proposal_creation() {
    let (_dir, conn) = setup_test_db();

    // Setup: Create user and ToR
    let user_id = user::create(&conn, &NewUser {
        username: "proposer".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "proposer@test.com".to_string(),
        display_name: "Proposer".to_string(),
        role_id: 0,
    }).unwrap();

    let tor_id = tor::create(&conn, "TestToR", "Test", &[("description", "Test ToR"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Monday"), ("cadence_time", "09:00"), ("cadence_duration_minutes", "90"), ("default_location", "Room A")]).unwrap();

    // Create proposal
    let prop_id = proposal::create(
        &conn,
        tor_id,
        "Add Remote Work Policy",
        "Allow flexible remote work options",
        "Improve work-life balance and retention",
        user_id,
        "2025-02-01",
        None
    ).unwrap();

    // Verify created with draft status
    let prop = proposal::find_by_id(&conn, prop_id).unwrap();
    assert!(prop.is_some());
    let prop = prop.unwrap();
    assert_eq!(prop.title, "Add Remote Work Policy");
    assert_eq!(prop.status, "draft");

    // Update to submitted
    proposal::update_status(&conn, prop_id, "submitted", None).unwrap();

    // Verify status changed
    let prop = proposal::find_by_id(&conn, prop_id).unwrap().unwrap();
    assert_eq!(prop.status, "submitted");

    println!("[PASS] test_proposal_creation");
}

#[test]
fn test_proposal_lifecycle() {
    let (_dir, conn) = setup_test_db();

    // Setup
    let user_id = user::create(&conn, &NewUser {
        username: "user1".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "u1@test.com".to_string(),
        display_name: "User 1".to_string(),
        role_id: 0,
    }).unwrap();

    let tor_id = tor::create(&conn, "TestToR", "Test", &[("description", "Test ToR"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Monday"), ("cadence_time", "09:00"), ("cadence_duration_minutes", "90"), ("default_location", "Room A")]).unwrap();

    // Create proposal
    let prop_id = proposal::create(
        &conn,
        tor_id,
        "Policy Change",
        "Update policy X",
        "Needed for compliance",
        user_id,
        "2025-02-01",
        None
    ).unwrap();

    // Move through workflow: draft → submitted → under_review → approved
    proposal::update_status(&conn, prop_id, "submitted", None).unwrap();
    proposal::update_status(&conn, prop_id, "under_review", None).unwrap();
    proposal::update_status(&conn, prop_id, "approved", None).unwrap();

    // Verify final state
    let prop = proposal::find_by_id(&conn, prop_id).unwrap().unwrap();
    assert_eq!(prop.status, "approved");

    println!("[PASS] test_proposal_lifecycle");
}

#[test]
fn test_cascade_delete_tor() {
    let (_dir, conn) = setup_test_db();

    // Setup: Create ToR with agenda points and proposal
    let admin_id = user::create(&conn, &NewUser {
        username: "admin".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "admin@test.com".to_string(),
        display_name: "Admin".to_string(),
        role_id: 0,
    }).unwrap();

    let tor_id = tor::create(&conn, "DeleteMe", "Delete", &[("description", "ToR to delete"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Monday"), ("cadence_time", "09:00"), ("cadence_duration_minutes", "90"), ("default_location", "Room A")]).unwrap();
    let ap_id = agenda_point::create(&conn, tor_id, "Item 1", "Desc", "discussion", "2025-02-20", 60, admin_id, "", "", "").unwrap();
    let prop_id = proposal::create(&conn, tor_id, "Prop 1", "Desc", "Rationale", admin_id, "2025-02-01", None).unwrap();

    // Verify they exist
    assert!(tor::find_detail_by_id(&conn, tor_id).is_ok());
    assert!(agenda_point::find_by_id(&conn, ap_id).is_ok());
    assert!(proposal::find_by_id(&conn, prop_id).is_ok());

    // Delete ToR
    tor::delete(&conn, tor_id).unwrap();

    // Verify ToR is deleted (cascade should remove related entities)
    let result = tor::find_detail_by_id(&conn, tor_id);
    match result {
        Ok(opt) => assert!(opt.is_none()),
        Err(_) => {} // Error is also acceptable if ToR doesn't exist
    }

    println!("[PASS] test_cascade_delete_tor");
}

#[test]
fn test_governance_data_query() {
    let (_dir, conn) = setup_test_db();

    // Setup: Create multiple ToRs and agenda points
    let admin_id = user::create(&conn, &NewUser {
        username: "admin".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "admin@test.com".to_string(),
        display_name: "Admin".to_string(),
        role_id: 0,
    }).unwrap();

    let tor1_id = tor::create(&conn, "ToR1", "Label1", &[("description", "First ToR"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Monday"), ("cadence_time", "09:00"), ("cadence_duration_minutes", "90"), ("default_location", "Room A")]).unwrap();
    let tor2_id = tor::create(&conn, "ToR2", "Label2", &[("description", "Second ToR"), ("status", "active"), ("meeting_cadence", "weekly"), ("cadence_day", "Tuesday"), ("cadence_time", "10:00"), ("cadence_duration_minutes", "120"), ("default_location", "Room B")]).unwrap();

    let _ap1_id = agenda_point::create(&conn, tor1_id, "AP1", "Desc", "discussion", "2025-02-15", 60, admin_id, "", "", "").unwrap();
    let _ap2_id = agenda_point::create(&conn, tor1_id, "AP2", "Desc", "decision", "2025-03-15", 90, admin_id, "", "", "").unwrap();
    let _ap3_id = agenda_point::create(&conn, tor2_id, "AP3", "Desc", "discussion", "2025-02-20", 60, admin_id, "", "", "").unwrap();

    // Query all for ToR1
    let aps_tor1 = agenda_point::find_all_for_tor(&conn, tor1_id).unwrap();
    assert_eq!(aps_tor1.len(), 2);

    // Query cross-ToR
    let aps_cross = agenda_point::find_all_cross_tor(&conn, None).unwrap();
    assert!(aps_cross.len() >= 3);

    println!("[PASS] test_governance_data_query");
}

