mod common;

use common::setup_test_db;
use ahlt::models::{entity, relation, permission, role, user};

#[tokio::test]
async fn test_find_codes_by_user_id_multi_role() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Create two roles
    let role_a = entity::create(pool, "role", "role_a", "Role A").await.unwrap();
    let role_b = entity::create(pool, "role", "role_b", "Role B").await.unwrap();

    // Create permissions
    let perm1 = entity::create(pool, "permission", "users.list", "List Users").await.unwrap();
    let perm2 = entity::create(pool, "permission", "users.create", "Create Users").await.unwrap();
    let perm3 = entity::create(pool, "permission", "tor.view", "View ToR").await.unwrap();

    // Assign permissions to roles
    relation::create(pool, "has_permission", role_a, perm1).await.unwrap();
    relation::create(pool, "has_permission", role_a, perm2).await.unwrap();
    relation::create(pool, "has_permission", role_b, perm3).await.unwrap();
    relation::create(pool, "has_permission", role_b, perm1).await.unwrap(); // overlap: users.list on both roles

    // Create user with both roles
    let user_id = entity::create(pool, "user", "testuser", "Test User").await.unwrap();
    relation::create(pool, "has_role", user_id, role_a).await.unwrap();
    relation::create(pool, "has_role", user_id, role_b).await.unwrap();

    let codes = permission::find_codes_by_user_id(pool, user_id).await.unwrap();
    assert_eq!(codes, vec!["tor.view", "users.create", "users.list"]);
}

#[tokio::test]
async fn test_find_codes_by_user_id_no_roles() {
    let db = setup_test_db().await;
    let pool = db.pool();
    let user_id = entity::create(pool, "user", "norole", "No Role").await.unwrap();
    let codes = permission::find_codes_by_user_id(pool, user_id).await.unwrap();
    assert!(codes.is_empty());
}

#[tokio::test]
async fn test_find_codes_by_user_id_single_role() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let role = entity::create(pool, "role", "viewer", "Viewer").await.unwrap();
    let perm = entity::create(pool, "permission", "dashboard.view", "View Dashboard").await.unwrap();
    relation::create(pool, "has_permission", role, perm).await.unwrap();

    let user_id = entity::create(pool, "user", "single", "Single Role").await.unwrap();
    relation::create(pool, "has_role", user_id, role).await.unwrap();

    let codes = permission::find_codes_by_user_id(pool, user_id).await.unwrap();
    assert_eq!(codes, vec!["dashboard.view"]);
}

// ============================================================================
// Role assignment query tests
// ============================================================================

#[tokio::test]
async fn test_find_users_by_role() {
    let db = setup_test_db().await;
    let pool = db.pool();
    let role_id = entity::create(pool, "role", "editor", "Editor").await.unwrap();
    let user1 = entity::create(pool, "user", "alice", "Alice").await.unwrap();
    let user2 = entity::create(pool, "user", "bob", "Bob").await.unwrap();
    relation::create(pool, "has_role", user1, role_id).await.unwrap();
    relation::create(pool, "has_role", user2, role_id).await.unwrap();

    let members = role::find_users_by_role(pool, role_id).await.unwrap();
    assert_eq!(members.len(), 2);
    // Verify sorted by label (display_name) then name
    assert_eq!(members[0].display_name, "Alice");
    assert_eq!(members[1].display_name, "Bob");
    // Verify no passwords exposed — struct only has user_id, username, display_name
    assert_eq!(members[0].user_id, user1);
    assert_eq!(members[0].username, "alice");
}

#[tokio::test]
async fn test_find_users_by_role_empty() {
    let db = setup_test_db().await;
    let pool = db.pool();
    let role_id = entity::create(pool, "role", "empty_role", "Empty Role").await.unwrap();

    let members = role::find_users_by_role(pool, role_id).await.unwrap();
    assert!(members.is_empty());
}

#[tokio::test]
async fn test_find_users_not_in_role() {
    let db = setup_test_db().await;
    let pool = db.pool();
    let role_id = entity::create(pool, "role", "editor", "Editor").await.unwrap();
    let user1 = entity::create(pool, "user", "alice", "Alice").await.unwrap();
    let _user2 = entity::create(pool, "user", "bob", "Bob").await.unwrap();
    relation::create(pool, "has_role", user1, role_id).await.unwrap();

    let not_in_role = role::find_users_not_in_role(pool, role_id).await.unwrap();
    assert_eq!(not_in_role.len(), 1);
    assert_eq!(not_in_role[0].username, "bob");
}

#[tokio::test]
async fn test_find_users_not_in_role_all_assigned() {
    let db = setup_test_db().await;
    let pool = db.pool();
    let role_id = entity::create(pool, "role", "editor", "Editor").await.unwrap();
    let user1 = entity::create(pool, "user", "alice", "Alice").await.unwrap();
    relation::create(pool, "has_role", user1, role_id).await.unwrap();

    let not_in_role = role::find_users_not_in_role(pool, role_id).await.unwrap();
    assert!(not_in_role.is_empty());
}

#[tokio::test]
async fn test_find_all_with_roles() {
    let db = setup_test_db().await;
    let pool = db.pool();
    let role_a = entity::create(pool, "role", "role_a", "Role A").await.unwrap();
    let role_b = entity::create(pool, "role", "role_b", "Role B").await.unwrap();
    let user1 = entity::create(pool, "user", "alice", "Alice").await.unwrap();
    let _user2 = entity::create(pool, "user", "bob", "Bob").await.unwrap();
    relation::create(pool, "has_role", user1, role_a).await.unwrap();
    relation::create(pool, "has_role", user1, role_b).await.unwrap();

    let users = user::find_all_with_roles(pool).await.unwrap();
    let alice = users.iter().find(|u| u.username == "alice").unwrap();
    assert_eq!(alice.roles.len(), 2);
    let bob = users.iter().find(|u| u.username == "bob").unwrap();
    assert_eq!(bob.roles.len(), 0);
}
