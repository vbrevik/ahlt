//! Integration tests for warnings model layer

mod common;

use ahlt::models::user::NewUser;
use ahlt::models::user;
use ahlt::warnings;
use ahlt::auth::password;
use common::setup_test_db;

#[tokio::test]
async fn test_create_warning() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Create a warning
    let warning_id = warnings::create_warning(
        pool,
        "high",
        "membership",
        "position.vacant",
        "Position has been vacant for 30 days",
        "position_id=123",
        "tor",
    ).await.unwrap();

    assert!(warning_id > 0);

    // Verify the warning exists
    let detail = warnings::queries::get_warning_detail(pool, warning_id).await.unwrap();
    assert!(detail.is_some());
    let detail = detail.unwrap();
    assert_eq!(detail.severity, "high");
    assert_eq!(detail.category, "membership");
    assert_eq!(detail.message, "Position has been vacant for 30 days");
}


#[tokio::test]
async fn test_warning_dedup_same_source() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Create first warning
    let _warning1_id = warnings::create_warning(
        pool,
        "high",
        "membership",
        "position.vacant",
        "Position X is vacant",
        "position_id=123",
        "tor",
    ).await.unwrap();

    // Check if exists (dedup)
    let exists = warnings::warning_exists(pool, "position.vacant", "position_id=123").await;
    assert!(exists);

    // Try to create another with same source_action and dedup key
    // The warning_exists check should prevent creation in real code
    let exists2 = warnings::warning_exists(pool, "position.vacant", "position_id=123").await;
    assert_eq!(exists, exists2);
}

#[tokio::test]
async fn test_warning_dedup_requires_key() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Create warning with specific dedup key in details
    let warning_id = warnings::create_warning(
        pool,
        "high",
        "membership",
        "position.vacant",
        "Position is vacant",
        "position_id=123|action=filled",
        "tor",
    ).await.unwrap();

    assert!(warning_id > 0);

    // Dedup should find it with the key
    let exists = warnings::warning_exists(pool, "position.vacant", "position_id=123").await;
    assert!(exists);

    // Dedup should NOT find it with a different key
    let not_exists = warnings::warning_exists(pool, "position.vacant", "position_id=999").await;
    assert!(!not_exists);

    // Dedup should NOT find it with different source_action
    let not_exists2 = warnings::warning_exists(pool, "position.filled", "position_id=123").await;
    assert!(!not_exists2);
}

#[tokio::test]
async fn test_resolve_warning() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Create users
    let admin_id = user::create(pool, &NewUser {
        username: "admin".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "admin@test.com".to_string(),
        display_name: "Admin".to_string(),
    }).await.unwrap();

    let user_id = user::create(pool, &NewUser {
        username: "user1".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "u1@test.com".to_string(),
        display_name: "User 1".to_string(),
    }).await.unwrap();

    // Create warning and receipt
    let warning_id = warnings::create_warning(
        pool,
        "high",
        "test",
        "test.action",
        "Test warning",
        "test_key=1",
        "test",
    ).await.unwrap();

    let _receipt_ids = warnings::create_receipts(pool, warning_id, &[user_id]).await.unwrap();

    // Verify warning exists
    let detail = warnings::queries::get_warning_detail(pool, warning_id).await.unwrap();
    assert!(detail.is_some());

    // Resolve the warning
    warnings::resolve_warning(pool, warning_id, admin_id).await.unwrap();

    // Verify warning status changed to resolved
    let detail = warnings::queries::get_warning_detail(pool, warning_id).await.unwrap();
    assert!(detail.is_some());
    let detail = detail.unwrap();
    assert_eq!(detail.status, "resolved");
}

#[tokio::test]
async fn test_find_receipt_for_user() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Create user
    let user_id = user::create(pool, &NewUser {
        username: "user1".to_string(),
        password: password::hash_password("pass").unwrap(),
        email: "u1@test.com".to_string(),
        display_name: "User 1".to_string(),
    }).await.unwrap();

    // Create warning and receipt
    let warning_id = warnings::create_warning(
        pool,
        "medium",
        "test",
        "test.action",
        "Test warning",
        "test_key=1",
        "test",
    ).await.unwrap();

    let receipt_ids = warnings::create_receipts(pool, warning_id, &[user_id]).await.unwrap();
    let expected_receipt_id = receipt_ids[0];

    // Find receipt for this user
    let found_receipt_id = warnings::queries::find_receipt_for_user(pool, warning_id, user_id).await.unwrap();
    assert_eq!(found_receipt_id, Some(expected_receipt_id));

    // Try to find receipt for non-existent user
    let not_found = warnings::queries::find_receipt_for_user(pool, warning_id, user_id + 999).await.unwrap();
    assert!(not_found.is_none());
}
