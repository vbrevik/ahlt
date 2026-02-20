mod common;

use common::setup_test_db;
use ahlt::models::{entity, relation, permission, role, user};

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

// ============================================================================
// Role assignment query tests
// ============================================================================

#[test]
fn test_find_users_by_role() {
    let (_dir, conn) = setup_test_db();
    let role_id = entity::create(&conn, "role", "editor", "Editor").unwrap();
    let user1 = entity::create(&conn, "user", "alice", "Alice").unwrap();
    let user2 = entity::create(&conn, "user", "bob", "Bob").unwrap();
    relation::create(&conn, "has_role", user1, role_id).unwrap();
    relation::create(&conn, "has_role", user2, role_id).unwrap();

    let members = role::find_users_by_role(&conn, role_id).unwrap();
    assert_eq!(members.len(), 2);
    // Verify sorted by label (display_name) then name
    assert_eq!(members[0].display_name, "Alice");
    assert_eq!(members[1].display_name, "Bob");
    // Verify no passwords exposed — struct only has user_id, username, display_name
    assert_eq!(members[0].user_id, user1);
    assert_eq!(members[0].username, "alice");
}

#[test]
fn test_find_users_by_role_empty() {
    let (_dir, conn) = setup_test_db();
    let role_id = entity::create(&conn, "role", "empty_role", "Empty Role").unwrap();

    let members = role::find_users_by_role(&conn, role_id).unwrap();
    assert!(members.is_empty());
}

#[test]
fn test_find_users_not_in_role() {
    let (_dir, conn) = setup_test_db();
    let role_id = entity::create(&conn, "role", "editor", "Editor").unwrap();
    let user1 = entity::create(&conn, "user", "alice", "Alice").unwrap();
    let _user2 = entity::create(&conn, "user", "bob", "Bob").unwrap();
    relation::create(&conn, "has_role", user1, role_id).unwrap();

    let not_in_role = role::find_users_not_in_role(&conn, role_id).unwrap();
    assert_eq!(not_in_role.len(), 1);
    assert_eq!(not_in_role[0].username, "bob");
}

#[test]
fn test_find_users_not_in_role_all_assigned() {
    let (_dir, conn) = setup_test_db();
    let role_id = entity::create(&conn, "role", "editor", "Editor").unwrap();
    let user1 = entity::create(&conn, "user", "alice", "Alice").unwrap();
    relation::create(&conn, "has_role", user1, role_id).unwrap();

    let not_in_role = role::find_users_not_in_role(&conn, role_id).unwrap();
    assert!(not_in_role.is_empty());
}

#[test]
fn test_find_all_with_roles() {
    let (_dir, conn) = setup_test_db();
    let role_a = entity::create(&conn, "role", "role_a", "Role A").unwrap();
    let role_b = entity::create(&conn, "role", "role_b", "Role B").unwrap();
    let user1 = entity::create(&conn, "user", "alice", "Alice").unwrap();
    let _user2 = entity::create(&conn, "user", "bob", "Bob").unwrap();
    relation::create(&conn, "has_role", user1, role_a).unwrap();
    relation::create(&conn, "has_role", user1, role_b).unwrap();

    let users = user::find_all_with_roles(&conn).unwrap();
    let alice = users.iter().find(|u| u.username == "alice").unwrap();
    assert_eq!(alice.roles.len(), 2);
    let bob = users.iter().find(|u| u.username == "bob").unwrap();
    assert_eq!(bob.roles.len(), 0);
}
