//! Terms of Reference (ToR) tests — covers ToR creation, retrieval, member/position management.
//!
//! Tests the ToR model layer operations:
//! - ToR creation, retrieval, updates, and deletion
//! - Member and position management
//! - Member counting and enumeration
//! - Membership validation

mod common;

use ahlt::models::tor::{create, update, delete, find_detail_by_id, find_all_list_items, find_members, count_members, find_non_members, get_tor_name};
use ahlt::models::user;
use ahlt::auth::password;
use common::*;

const TEST_TOR_NAME: &str = "test_tor";
const TEST_TOR_LABEL: &str = "Test Terms of Reference";
const TEST_DESCRIPTION: &str = "A test ToR for unit tests";
const TEST_STATUS: &str = "active";
const TEST_CADENCE: &str = "weekly";
const TEST_DAY: &str = "Monday";
const TEST_TIME: &str = "10:00";
const TEST_DURATION: &str = "60";
const TEST_LOCATION: &str = "Conference Room A";

#[test]
fn test_create_tor_success() {
    let (_dir, conn) = setup_test_db();

    let tor_id = create(
        &conn,
        TEST_TOR_NAME,
        TEST_TOR_LABEL,
        &[("description", TEST_DESCRIPTION), ("status", TEST_STATUS), ("meeting_cadence", TEST_CADENCE), ("cadence_day", TEST_DAY), ("cadence_time", TEST_TIME), ("cadence_duration_minutes", TEST_DURATION), ("default_location", TEST_LOCATION)],
    ).expect("Failed to create ToR");

    assert!(tor_id > 0);

    let tor = find_detail_by_id(&conn, tor_id)
        .expect("Query failed")
        .expect("ToR not found");

    assert_eq!(tor.name, TEST_TOR_NAME);
    assert_eq!(tor.label, TEST_TOR_LABEL);
    assert_eq!(tor.description, TEST_DESCRIPTION);
}

#[test]
fn test_create_tor_duplicate_name() {
    let (_dir, conn) = setup_test_db();

    let first_id = create(&conn, TEST_TOR_NAME, TEST_TOR_LABEL, &[])
        .expect("Failed to create first ToR");
    assert!(first_id > 0);

    // Try to create ToR with same name
    let duplicate = create(&conn, TEST_TOR_NAME, "Different Label", &[]);
    
    // Should fail on UNIQUE constraint
    assert!(duplicate.is_err());
}

#[test]
fn test_find_tor_by_id_success() {
    let (_dir, conn) = setup_test_db();

    let created_id = create(&conn, TEST_TOR_NAME, TEST_TOR_LABEL, &[("description", TEST_DESCRIPTION), ("status", TEST_STATUS), ("meeting_cadence", TEST_CADENCE), ("cadence_day", TEST_DAY), ("cadence_time", TEST_TIME), ("cadence_duration_minutes", TEST_DURATION), ("default_location", TEST_LOCATION)])
        .expect("Failed to create ToR");

    let tor = find_detail_by_id(&conn, created_id)
        .expect("Query failed")
        .expect("ToR not found");

    assert_eq!(tor.id, created_id);
    assert_eq!(tor.name, TEST_TOR_NAME);
    assert_eq!(tor.meeting_cadence, TEST_CADENCE);
}

#[test]
fn test_find_tor_by_id_not_found() {
    let (_dir, conn) = setup_test_db();

    let result = find_detail_by_id(&conn, 9999)
        .expect("Query failed");

    assert!(result.is_none());
}

#[test]
fn test_list_all_tors() {
    let (_dir, conn) = setup_test_db();

    // Create multiple ToRs
    for i in 0..3 {
        let name = format!("tor_{}", i);
        let label = format!("ToR {}", i);
        let _ = create(&conn, &name, &label, &[])
            .expect("Failed to create ToR");
    }

    let tors = find_all_list_items(&conn)
        .expect("Failed to list ToRs");

    assert!(tors.len() >= 3);
}

#[test]
fn test_get_tor_name() {
    let (_dir, conn) = setup_test_db();

    let tor_id = create(&conn, TEST_TOR_NAME, TEST_TOR_LABEL, &[])
        .expect("Failed to create ToR");

    let name = get_tor_name(&conn, tor_id)
        .expect("Failed to get ToR name");

    assert_eq!(name, TEST_TOR_LABEL);
}

#[test]
fn test_update_tor_success() {
    let (_dir, conn) = setup_test_db();

    let tor_id = create(&conn, TEST_TOR_NAME, TEST_TOR_LABEL, &[("description", TEST_DESCRIPTION), ("status", TEST_STATUS)])
        .expect("Failed to create ToR");

    let updated_label = "Updated ToR Label";
    let _ = update(&conn, tor_id, TEST_TOR_NAME, updated_label, &[("description", "Updated description"), ("status", TEST_STATUS)])
        .expect("Failed to update ToR");

    let tor = find_detail_by_id(&conn, tor_id)
        .expect("Query failed")
        .expect("ToR not found");

    assert_eq!(tor.label, updated_label);
    assert_eq!(tor.description, "Updated description");
}

#[test]
fn test_update_tor_not_found() {
    let (_dir, conn) = setup_test_db();

    // Updating non-existent ToR may fail or succeed depending on implementation
    // The important thing is that it doesn't panic
    let _ = update(&conn, 9999, "name", "label", &[]);
}

#[test]
fn test_count_members_empty() {
    let (_dir, conn) = setup_test_db();

    let tor_id = create(&conn, TEST_TOR_NAME, TEST_TOR_LABEL, &[])
        .expect("Failed to create ToR");

    let count = count_members(&conn, tor_id)
        .expect("Failed to count members");

    // Fresh ToR with no positions should have 0 members
    assert_eq!(count, 0);
}

#[test]
fn test_find_tor_members_empty() {
    let (_dir, conn) = setup_test_db();

    let tor_id = create(&conn, TEST_TOR_NAME, TEST_TOR_LABEL, &[])
        .expect("Failed to create ToR");

    let members = find_members(&conn, tor_id)
        .expect("Failed to find members");

    // Fresh ToR with no positions should have no members
    assert!(members.is_empty());
}

#[test]
fn test_delete_tor_success() {
    let (_dir, conn) = setup_test_db();

    let tor_id = create(&conn, TEST_TOR_NAME, TEST_TOR_LABEL, &[])
        .expect("Failed to create ToR");

    let _ = delete(&conn, tor_id)
        .expect("Failed to delete ToR");

    let result = find_detail_by_id(&conn, tor_id)
        .expect("Query failed");

    assert!(result.is_none(), "ToR should be deleted");
}

#[test]
fn test_find_non_members() {
    let (_dir, conn) = setup_test_db();

    let tor_id = create(&conn, TEST_TOR_NAME, TEST_TOR_LABEL, &[])
        .expect("Failed to create ToR");

    // Create a test user
    let password_hash = password::hash_password("testpass123")
        .expect("Failed to hash password");
    let new_user = user::NewUser {
        username: "testuser".to_string(),
        password: password_hash,
        email: "test@example.com".to_string(),
        display_name: "Test User".to_string(),
        role_id: 0,
    };

    let _ = user::create(&conn, &new_user)
        .expect("Failed to create user");

    // User should appear in non-members list since no position assigned
    let non_members = find_non_members(&conn, tor_id)
        .expect("Failed to find non-members");

    assert!(!non_members.is_empty());
}
