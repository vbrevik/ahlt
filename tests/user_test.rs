//! User CRUD tests — covers user creation, retrieval, search, updates, and deletion.
//!
//! Tests the user model layer operations:
//! - User creation with validation (duplicate username check)
//! - User retrieval by ID and username
//! - User listing and pagination
//! - User updates (full record)
//! - User deletion
//! - Password updates and re-verification

mod common;

use ahlt::models::user::*;
use ahlt::auth::password;
use common::*;

const TEST_USERNAME: &str = "testuser";
const TEST_EMAIL: &str = "test@example.com";
const TEST_PASSWORD: &str = "password123";
const TEST_DISPLAY_NAME: &str = "Test User";

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
    };

    let user_id = create(&conn, &new_user)
        .expect("Failed to create user");

    assert!(user_id > 0);
    
    let found = find_display_by_id(&conn, user_id)
        .expect("Query failed")
        .expect("User not found");
    
    assert_eq!(found.username, TEST_USERNAME);
    assert_eq!(found.email, TEST_EMAIL);
}

#[test]
fn test_create_user_duplicate_username() {
    let (_dir, conn) = setup_test_db();

    let password_hash = password::hash_password(TEST_PASSWORD)
        .expect("Failed to hash password");
    let new_user = NewUser {
        username: TEST_USERNAME.to_string(),
        password: password_hash.clone(),
        email: TEST_EMAIL.to_string(),
        display_name: TEST_DISPLAY_NAME.to_string(),
    };

    // First user succeeds
    let first_id = create(&conn, &new_user)
        .expect("Failed to create first user");
    assert!(first_id > 0);

    // Second user with same username should fail
    let duplicate = NewUser {
        username: TEST_USERNAME.to_string(),
        password: password_hash,
        email: "different@example.com".to_string(),
        display_name: "Different Name".to_string(),
    };

    let result = create(&conn, &duplicate);
    assert!(result.is_err(), "Should fail on duplicate username");
}

#[test]
fn test_find_user_by_id_success() {
    let (_dir, conn) = setup_test_db();

    let password_hash = password::hash_password(TEST_PASSWORD)
        .expect("Failed to hash password");
    let new_user = NewUser {
        username: TEST_USERNAME.to_string(),
        password: password_hash,
        email: TEST_EMAIL.to_string(),
        display_name: TEST_DISPLAY_NAME.to_string(),
    };

    let created_id = create(&conn, &new_user)
        .expect("Failed to create user");

    let found = find_display_by_id(&conn, created_id)
        .expect("Query failed")
        .expect("User not found");

    assert_eq!(found.id, created_id);
    assert_eq!(found.username, TEST_USERNAME);
    assert_eq!(found.email, TEST_EMAIL);
}

#[test]
fn test_find_user_by_id_not_found() {
    let (_dir, conn) = setup_test_db();

    let result = find_display_by_id(&conn, 9999)
        .expect("Query failed");

    assert!(result.is_none());
}

#[test]
fn test_find_user_by_username_success() {
    let (_dir, conn) = setup_test_db();

    let password_hash = password::hash_password(TEST_PASSWORD)
        .expect("Failed to hash password");
    let new_user = NewUser {
        username: TEST_USERNAME.to_string(),
        password: password_hash,
        email: TEST_EMAIL.to_string(),
        display_name: TEST_DISPLAY_NAME.to_string(),
    };

    let created_id = create(&conn, &new_user)
        .expect("Failed to create user");

    let found = find_by_username(&conn, TEST_USERNAME)
        .expect("Query failed")
        .expect("User not found");

    assert_eq!(found.id, created_id);
    assert_eq!(found.username, TEST_USERNAME);
}

#[test]
fn test_find_user_by_username_not_found() {
    let (_dir, conn) = setup_test_db();

    let result = find_by_username(&conn, "nonexistent")
        .expect("Query failed");

    assert!(result.is_none());
}

#[test]
fn test_list_users_paginated() {
    let (_dir, conn) = setup_test_db();

    let password_hash = password::hash_password(TEST_PASSWORD)
        .expect("Failed to hash password");

    // Create multiple users
    for i in 0..3 {
        let new_user = NewUser {
            username: format!("user{}", i),
            password: password_hash.clone(),
            email: format!("user{}@example.com", i),
            display_name: format!("User {}", i),
        };
        let _ = create(&conn, &new_user);
    }

    use ahlt::models::table_filter::{FilterTree, SortSpec};
    let page = find_paginated(&conn, 1, 10, &FilterTree::default(), &SortSpec::default())
        .expect("Failed to list users");

    assert!(page.users.len() >= 3);
    assert_eq!(page.page, 1);
    assert!(page.total_count >= 3);
}

#[test]
fn test_search_users_by_name() {
    let (_dir, conn) = setup_test_db();

    let password_hash = password::hash_password(TEST_PASSWORD)
        .expect("Failed to hash password");
    let new_user = NewUser {
        username: "searchable".to_string(),
        password: password_hash,
        email: TEST_EMAIL.to_string(),
        display_name: "Searchable Display".to_string(),
    };

    let _ = create(&conn, &new_user);

    use ahlt::models::table_filter::{FilterTree, Condition, SortSpec};
    let filter = FilterTree {
        conditions: vec![Condition {
            field: "username".into(),
            op: "contains".into(),
            value: "search".into(),
        }],
        ..Default::default()
    };
    let results = find_paginated(&conn, 1, 10, &filter, &SortSpec::default())
        .expect("Failed to search users");

    assert!(!results.users.is_empty());
    assert!(results.users.iter().any(|u| u.username == "searchable"));
}

#[test]
fn test_update_user_success() {
    let (_dir, conn) = setup_test_db();

    let password_hash = password::hash_password(TEST_PASSWORD)
        .expect("Failed to hash password");
    let new_user = NewUser {
        username: TEST_USERNAME.to_string(),
        password: password_hash.clone(),
        email: TEST_EMAIL.to_string(),
        display_name: TEST_DISPLAY_NAME.to_string(),
    };

    let user_id = create(&conn, &new_user)
        .expect("Failed to create user");

    let updated_display = "Updated Display Name";
    let _ = update(&conn, user_id, TEST_USERNAME, Some(&password_hash),
                   TEST_EMAIL, updated_display)
        .expect("Failed to update user");

    let updated = find_display_by_id(&conn, user_id)
        .expect("Query failed")
        .expect("User not found");

    assert_eq!(updated.display_name, updated_display);
}

#[test]
fn test_update_user_not_found() {
    let (_dir, conn) = setup_test_db();

    // Trying to update a non-existent user — the function handles it gracefully (doesn't panic)
    let _ = update(&conn, 9999, "username", None, "email@test.com", "Name");
}

#[test]
fn test_delete_user_success() {
    let (_dir, conn) = setup_test_db();

    let password_hash = password::hash_password(TEST_PASSWORD)
        .expect("Failed to hash password");
    let new_user = NewUser {
        username: TEST_USERNAME.to_string(),
        password: password_hash,
        email: TEST_EMAIL.to_string(),
        display_name: TEST_DISPLAY_NAME.to_string(),
    };

    let user_id = create(&conn, &new_user)
        .expect("Failed to create user");

    let _ = delete(&conn, user_id)
        .expect("Failed to delete user");

    let result = find_display_by_id(&conn, user_id)
        .expect("Query failed");

    assert!(result.is_none(), "User should be deleted");
}

#[test]
fn test_delete_user_not_found() {
    let (_dir, conn) = setup_test_db();

    let result = delete(&conn, 9999);
    
    // SQLite DELETE doesn't error on non-existent rows, returns Ok(())
    assert!(result.is_ok());
}

#[test]
fn test_count_users() {
    let (_dir, conn) = setup_test_db();

    let password_hash = password::hash_password(TEST_PASSWORD)
        .expect("Failed to hash password");

    let initial_count = count(&conn)
        .expect("Failed to count users");

    let new_user = NewUser {
        username: TEST_USERNAME.to_string(),
        password: password_hash,
        email: TEST_EMAIL.to_string(),
        display_name: TEST_DISPLAY_NAME.to_string(),
    };

    let _ = create(&conn, &new_user)
        .expect("Failed to create user");

    let after_count = count(&conn)
        .expect("Failed to count users");

    assert_eq!(after_count, initial_count + 1);
}

#[test]
fn test_update_password_success() {
    let (_dir, conn) = setup_test_db();

    let password_hash = password::hash_password(TEST_PASSWORD)
        .expect("Failed to hash password");
    let new_user = NewUser {
        username: TEST_USERNAME.to_string(),
        password: password_hash,
        email: TEST_EMAIL.to_string(),
        display_name: TEST_DISPLAY_NAME.to_string(),
    };

    let user_id = create(&conn, &new_user)
        .expect("Failed to create user");

    let new_password = "newpassword456";
    let new_hash = password::hash_password(new_password)
        .expect("Failed to hash new password");

    let _ = update_password(&conn, user_id, &new_hash)
        .expect("Failed to update password");

    // Verify the new password hash is stored
    let found = find_by_username(&conn, TEST_USERNAME)
        .expect("Query failed")
        .expect("User not found");

    assert!(password::verify_password(new_password, &found.password)
        .expect("Verification failed"));
}

#[test]
fn test_find_paginated_sort_by_username_asc() {
    use ahlt::models::user::{create, find_paginated, types::NewUser};
    use ahlt::models::table_filter::{FilterTree, SortSpec, SortDir};
    use common::setup_test_db;

    let (_dir, conn) = setup_test_db();
    // Create two users with known usernames
    for (name, label) in [("beta_user", "Beta"), ("alpha_user", "Alpha")] {
        let _ = create(&conn, &NewUser {
            username: name.into(), password: "hash".into(),
            email: format!("{name}@test.com"), display_name: label.into(),
        });
    }
    let sort = SortSpec { column: "username".into(), dir: SortDir::Asc };
    let page = find_paginated(&conn, 1, 10, &FilterTree::default(), &sort).unwrap();
    let names: Vec<&str> = page.users.iter().map(|u| u.username.as_str()).collect();
    let idx_alpha = names.iter().position(|&n| n == "alpha_user").unwrap();
    let idx_beta = names.iter().position(|&n| n == "beta_user").unwrap();
    assert!(idx_alpha < idx_beta, "alpha should come before beta when sorted ASC");
}

#[test]
fn test_find_paginated_filter_by_role() {
    use ahlt::models::user::{create, find_paginated, types::NewUser};
    use ahlt::models::table_filter::{FilterTree, Condition, SortSpec};
    use ahlt::models::role;
    use ahlt::models::relation;
    use common::setup_test_db;

    let (_dir, conn) = setup_test_db();

    // Find default role id (seeded role)
    let roles = role::queries::find_all_list_items(&conn).unwrap();
    // If no roles found, skip with a basic assertion
    if roles.is_empty() {
        // No roles seeded, just verify filter doesn't panic
        let filter = FilterTree {
            conditions: vec![Condition {
                field: "role".into(), op: "is".into(), value: "nonexistent".into(),
            }],
            ..Default::default()
        };
        let page = find_paginated(&conn, 1, 10, &filter, &SortSpec::default()).unwrap();
        assert_eq!(page.users.len(), 0);
        return;
    }

    let first_role = &roles[0];

    let user_id = create(&conn, &NewUser {
        username: "role_filtered_user".into(),
        password: "hash".into(),
        email: "rf@test.com".into(),
        display_name: "RF".into(),
    }).unwrap();

    // Manually assign role via relation
    relation::create(&conn, "has_role", user_id, first_role.id).unwrap();

    let filter = FilterTree {
        conditions: vec![Condition {
            field: "role".into(), op: "is".into(), value: first_role.name.clone(),
        }],
        ..Default::default()
    };
    let page = find_paginated(&conn, 1, 10, &filter, &SortSpec::default()).unwrap();
    assert!(page.users.iter().any(|u| u.username == "role_filtered_user"),
        "role_filtered_user should appear in results filtered by role");
}
