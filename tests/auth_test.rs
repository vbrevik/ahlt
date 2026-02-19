//! Authentication tests — covers password hashing, verification, user creation, and password updates.
//!
//! Tests the authentication layer at the model level:
//! - Password hashing with argon2
//! - Password verification (correct and incorrect)
//! - User creation with hashed passwords
//! - Password updates and re-verification

mod common;

use ahlt::models::user::NewUser;
use ahlt::auth::password;
use ahlt::models::user;
use common::*;

const TEST_USERNAME: &str = "testuser";
const TEST_EMAIL: &str = "test@example.com";
const TEST_PASSWORD: &str = "password123";
const TEST_DISPLAY_NAME: &str = "Test User";

#[test]
fn test_hash_password_success() {
    let hash = password::hash_password(TEST_PASSWORD)
        .expect("Failed to hash password");

    assert!(!hash.is_empty());
    assert!(hash.len() > 20); // Argon2 hashes are long
}

#[test]
fn test_verify_password_correct() {
    let hash = password::hash_password(TEST_PASSWORD)
        .expect("Failed to hash password");

    let verified = password::verify_password(TEST_PASSWORD, &hash)
        .expect("Verification failed");

    assert!(verified);
}

#[test]
fn test_verify_password_incorrect() {
    let hash = password::hash_password(TEST_PASSWORD)
        .expect("Failed to hash password");

    let verified = password::verify_password("wrongpassword", &hash)
        .expect("Verification failed");

    assert!(!verified);
}

#[test]
fn test_hash_password_randomness() {
    let hash1 = password::hash_password(TEST_PASSWORD)
        .expect("Failed to hash first password");
    let hash2 = password::hash_password(TEST_PASSWORD)
        .expect("Failed to hash second password");

    // Same password should produce different hashes (different salts)
    assert_ne!(hash1, hash2);

    // But both hashes should verify with the same password
    assert!(password::verify_password(TEST_PASSWORD, &hash1)
        .expect("Verification 1 failed"));
    assert!(password::verify_password(TEST_PASSWORD, &hash2)
        .expect("Verification 2 failed"));
}

#[test]
fn test_create_user_success() {
    let (_dir, conn) = setup_test_db();

    let password_hash = password::hash_password(TEST_PASSWORD)
        .expect("Failed to hash password");
    let new_user = NewUser {
        username: TEST_USERNAME.to_string(),
        password: password_hash,
        email: TEST_EMAIL.to_string(),
        display_name: TEST_DISPLAY_NAME.to_string(),
        role_id: 0,
    };

    let user_id = user::create(&conn, &new_user)
        .expect("Failed to create user");

    assert!(user_id > 0);
}

#[test]
fn test_find_user_by_username() {
    let (_dir, conn) = setup_test_db();

    let password_hash = password::hash_password(TEST_PASSWORD)
        .expect("Failed to hash password");
    let new_user = NewUser {
        username: TEST_USERNAME.to_string(),
        password: password_hash,
        email: TEST_EMAIL.to_string(),
        display_name: TEST_DISPLAY_NAME.to_string(),
        role_id: 0,
    };

    let created_id = user::create(&conn, &new_user)
        .expect("Failed to create user");

    let found = user::find_by_username(&conn, TEST_USERNAME)
        .expect("Query failed")
        .expect("User not found");

    assert_eq!(found.id, created_id);
    assert_eq!(found.username, TEST_USERNAME);
    assert_eq!(found.email, TEST_EMAIL);
}

#[test]
fn test_find_user_by_username_not_found() {
    let (_dir, conn) = setup_test_db();

    let result = user::find_by_username(&conn, "nonexistent")
        .expect("Query failed");

    assert!(result.is_none());
}

#[test]
fn test_update_password_and_verify() {
    let (_dir, conn) = setup_test_db();

    let old_password = "oldpassword123";
    let new_password = "newpassword456";

    let password_hash = password::hash_password(old_password)
        .expect("Failed to hash old password");
    let new_user = NewUser {
        username: "updateuser".to_string(),
        password: password_hash,
        email: "update@example.com".to_string(),
        display_name: "Update User".to_string(),
        role_id: 0,
    };

    let user_id = user::create(&conn, &new_user)
        .expect("Failed to create user");

    // Update password
    let new_hash = password::hash_password(new_password)
        .expect("Failed to hash new password");
    user::update_password(&conn, user_id, &new_hash)
        .expect("Failed to update password");

    // Verify new password works
    let updated = user::find_by_username(&conn, "updateuser")
        .expect("Query failed")
        .expect("User not found");

    assert!(password::verify_password(new_password, &updated.password)
        .expect("New password verification failed"));
    assert!(!password::verify_password(old_password, &updated.password)
        .expect("Old password verification failed"));
}
