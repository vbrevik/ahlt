//! Integration tests for warnings model layer

mod common;

use ahlt::models::user::NewUser;
use ahlt::models::user;
use ahlt::warnings;
use ahlt::auth::password;
use common::setup_test_db;

#[test]
fn test_create_warning() {
    let (_dir, conn) = setup_test_db();

    // Create a warning
    let warning_id = warnings::create_warning(
        &conn,
        "high",
        "membership",
        "position.vacant",
        "Position has been vacant for 30 days",
        "position_id=123",
        "tor",
    ).unwrap();

    assert!(warning_id > 0);

    // Verify the warning exists
    let detail = warnings::queries::get_warning_detail(&conn, warning_id).unwrap();
    assert!(detail.is_some());
    let detail = detail.unwrap();
    assert_eq!(detail.severity, "high");
    assert_eq!(detail.category, "membership");
    assert_eq!(detail.message, "Position has been vacant for 30 days");

    println!("[PASS] test_create_warning");
}


#[test]
fn test_warning_dedup_same_source() {
    let (_dir, conn) = setup_test_db();

    // Create first warning
    let _warning1_id = warnings::create_warning(
        &conn,
        "high",
        "membership",
        "position.vacant",
        "Position X is vacant",
        "position_id=123",
        "tor",
    ).unwrap();

    // Check if exists (dedup)
    let exists = warnings::warning_exists(&conn, "position.vacant", "position_id=123");
    assert!(exists);

    // Try to create another with same source_action and dedup key
    // The warning_exists check should prevent creation in real code
    let exists2 = warnings::warning_exists(&conn, "position.vacant", "position_id=123");
    assert_eq!(exists, exists2);

    println!("[PASS] test_warning_dedup_same_source");
}

#[test]
fn test_warning_dedup_requires_key() {
    let (_dir, conn) = setup_test_db();

    // Create warning with specific dedup key in details
    let warning_id = warnings::create_warning(
        &conn,
        "high",
        "membership",
        "position.vacant",
        "Position is vacant",
        "position_id=123|action=filled",
        "tor",
    ).unwrap();

    assert!(warning_id > 0);

    // Dedup should find it with the key
    let exists = warnings::warning_exists(&conn, "position.vacant", "position_id=123");
    assert!(exists);

    // Dedup should NOT find it with a different key
    let not_exists = warnings::warning_exists(&conn, "position.vacant", "position_id=999");
    assert!(!not_exists);

    // Dedup should NOT find it with different source_action
    let not_exists2 = warnings::warning_exists(&conn, "position.filled", "position_id=123");
    assert!(!not_exists2);

    println!("[PASS] test_warning_dedup_requires_key");
}

#[test]
fn test_resolve_warning() {
    let (_dir, conn) = setup_test_db();

    // Create users
    let admin_id = user::create(&conn, &NewUser {
        username: "admin".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "admin@test.com".to_string(),
        display_name: "Admin".to_string(),
        role_id: 0,
    }).unwrap();

    let user_id = user::create(&conn, &NewUser {
        username: "user1".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "u1@test.com".to_string(),
        display_name: "User 1".to_string(),
        role_id: 0,
    }).unwrap();

    // Create warning and receipt
    let warning_id = warnings::create_warning(
        &conn,
        "high",
        "test",
        "test.action",
        "Test warning",
        "test_key=1",
        "test",
    ).unwrap();

    let _receipt_ids = warnings::create_receipts(&conn, warning_id, &[user_id]).unwrap();

    // Verify warning exists
    let detail = warnings::queries::get_warning_detail(&conn, warning_id).unwrap();
    assert!(detail.is_some());

    // Resolve the warning
    warnings::resolve_warning(&conn, warning_id, admin_id).unwrap();

    // Verify warning status changed to resolved
    let detail = warnings::queries::get_warning_detail(&conn, warning_id).unwrap();
    assert!(detail.is_some());
    let detail = detail.unwrap();
    assert_eq!(detail.status, "resolved");

    println!("[PASS] test_resolve_warning");
}

#[test]
fn test_find_receipt_for_user() {
    let (_dir, conn) = setup_test_db();

    // Create user
    let user_id = user::create(&conn, &NewUser {
        username: "user1".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "u1@test.com".to_string(),
        display_name: "User 1".to_string(),
        role_id: 0,
    }).unwrap();

    // Create warning and receipt
    let warning_id = warnings::create_warning(
        &conn,
        "medium",
        "test",
        "test.action",
        "Test warning",
        "test_key=1",
        "test",
    ).unwrap();

    let receipt_ids = warnings::create_receipts(&conn, warning_id, &[user_id]).unwrap();
    let expected_receipt_id = receipt_ids[0];

    // Find receipt for this user
    let found_receipt_id = warnings::queries::find_receipt_for_user(&conn, warning_id, user_id).unwrap();
    assert_eq!(found_receipt_id, Some(expected_receipt_id));

    // Try to find receipt for non-existent user
    let not_found = warnings::queries::find_receipt_for_user(&conn, warning_id, user_id + 999).unwrap();
    assert!(not_found.is_none());

    println!("[PASS] test_find_receipt_for_user");
}

