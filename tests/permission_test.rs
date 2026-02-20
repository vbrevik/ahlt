mod common;

use common::setup_test_db;
use ahlt::models::{entity, relation, permission};

#[test]
fn test_find_codes_by_user_id_multi_role() {
    let (_dir, conn) = setup_test_db();

    // Create two roles
    let role_a = entity::create(&conn, "role", "role_a", "Role A").unwrap();
    let role_b = entity::create(&conn, "role", "role_b", "Role B").unwrap();

    // Create permissions
    let perm1 = entity::create(&conn, "permission", "users.list", "List Users").unwrap();
    let perm2 = entity::create(&conn, "permission", "users.create", "Create Users").unwrap();
    let perm3 = entity::create(&conn, "permission", "tor.view", "View ToR").unwrap();

    // Assign permissions to roles
    relation::create(&conn, "has_permission", role_a, perm1).unwrap();
    relation::create(&conn, "has_permission", role_a, perm2).unwrap();
    relation::create(&conn, "has_permission", role_b, perm3).unwrap();
    relation::create(&conn, "has_permission", role_b, perm1).unwrap(); // overlap: users.list on both roles

    // Create user with both roles
    let user_id = entity::create(&conn, "user", "testuser", "Test User").unwrap();
    relation::create(&conn, "has_role", user_id, role_a).unwrap();
    relation::create(&conn, "has_role", user_id, role_b).unwrap();

    let codes = permission::find_codes_by_user_id(&conn, user_id).unwrap();
    assert_eq!(codes, vec!["tor.view", "users.create", "users.list"]);
}

#[test]
fn test_find_codes_by_user_id_no_roles() {
    let (_dir, conn) = setup_test_db();
    let user_id = entity::create(&conn, "user", "norole", "No Role").unwrap();
    let codes = permission::find_codes_by_user_id(&conn, user_id).unwrap();
    assert!(codes.is_empty());
}

#[test]
fn test_find_codes_by_user_id_single_role() {
    let (_dir, conn) = setup_test_db();

    let role = entity::create(&conn, "role", "viewer", "Viewer").unwrap();
    let perm = entity::create(&conn, "permission", "dashboard.view", "View Dashboard").unwrap();
    relation::create(&conn, "has_permission", role, perm).unwrap();

    let user_id = entity::create(&conn, "user", "single", "Single Role").unwrap();
    relation::create(&conn, "has_role", user_id, role).unwrap();

    let codes = permission::find_codes_by_user_id(&conn, user_id).unwrap();
    assert_eq!(codes, vec!["dashboard.view"]);
}
