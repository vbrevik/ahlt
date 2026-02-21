mod common;
use common::*;

use std::collections::HashSet;
use regex::Regex;

// ============================================================================
// SHARED TEST HELPERS
// ============================================================================

async fn get_permissions_for_user(pool: &sqlx::PgPool, user_id: i64) -> HashSet<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT p.name FROM entities p
         WHERE p.entity_type = 'permission'
         AND EXISTS (
             SELECT 1 FROM relations r1
             WHERE r1.relation_type_id = (
                 SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission'
             )
             AND r1.target_id = p.id
             AND r1.source_id IN (
                 SELECT r2.target_id FROM relations r2
                 WHERE r2.relation_type_id = (
                     SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role'
                 )
                 AND r2.source_id = $1
             )
         )"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .expect("Failed to query permissions");

    rows.into_iter().map(|(name,)| name).collect()
}

fn extract_csrf_token(html: &str) -> String {
    let re = Regex::new(r#"name="csrf_token"\s+value="([^"]+)""#)
        .expect("Failed to compile regex");
    re.captures(html)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "invalid_token".to_string())
}

// ============================================================================
// INFRASTRUCTURE TESTS
// ============================================================================

#[tokio::test]
async fn test_infrastructure_compiled() {
    let db = setup_test_db().await;
    let pool = db.pool();
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM entities")
        .fetch_one(pool)
        .await
        .unwrap();
    // Fresh database has seeded relation types + default role, not zero
    assert!(count.0 > 0, "Database should have seeded entities");
}

#[tokio::test]
async fn test_csrf_extraction() {
    let html = r#"<form><input type="hidden" name="csrf_token" value="test_token_12345" /></form>"#;
    assert_eq!(extract_csrf_token(html), "test_token_12345");
}

#[tokio::test]
async fn test_csrf_extraction_missing() {
    let html = r#"<form><input type="text" name="username" /></form>"#;
    assert_eq!(extract_csrf_token(html), "invalid_token");
}

// ============================================================================
// AUTHENTICATION TESTS
// ============================================================================

#[tokio::test]
async fn test_auth_user_lookup() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let user_id = insert_entity(pool, "user", "testuser", "Test User").await;
    insert_prop(pool, user_id, "password", "hashed_password_123").await;

    let found: (i64, String) = sqlx::query_as(
        "SELECT id, name FROM entities WHERE entity_type = 'user' AND name = 'testuser'"
    )
    .fetch_one(pool)
    .await
    .expect("User should be found");

    assert_eq!(found.0, user_id);
    assert_eq!(found.1, "testuser");

    let password: (String,) = sqlx::query_as(
        "SELECT value FROM entity_properties WHERE entity_id = $1 AND key = 'password'"
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .expect("Password should be retrievable");
    assert_eq!(password.0, "hashed_password_123");
}

#[tokio::test]
async fn test_auth_nonexistent_user() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let found: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM entities WHERE entity_type = 'user' AND name = 'nonexistent'"
    )
    .fetch_optional(pool)
    .await
    .unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn test_auth_permission_assignment() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // The common module seeds has_role and has_permission relation types already.
    // We create our own to get known IDs for the raw query below.
    let has_role_id = insert_entity(pool, "relation_type", "test_has_role", "Test Has Role").await;
    let has_permission_id = insert_entity(pool, "relation_type", "test_has_permission", "Test Has Permission").await;

    let user_id = insert_entity(pool, "user", "alice", "Alice").await;
    let admin_role_id = insert_entity(pool, "role", "admin", "Administrator").await;
    let perm_id = insert_entity(pool, "permission", "users.create", "Create Users").await;

    insert_relation(pool, has_role_id, user_id, admin_role_id).await;
    insert_relation(pool, has_permission_id, admin_role_id, perm_id).await;

    let has_perm: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM entities p
         WHERE p.entity_type = 'permission' AND p.name = 'users.create'
         AND EXISTS (
             SELECT 1 FROM relations r1
             WHERE r1.relation_type_id = $1 AND r1.target_id = p.id
             AND r1.source_id IN (
                 SELECT r2.target_id FROM relations r2
                 WHERE r2.relation_type_id = $2 AND r2.source_id = $3
             )
         )"
    )
    .bind(has_permission_id)
    .bind(has_role_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap();

    assert!(has_perm.0 > 0, "User should have users.create through role");
}

// ============================================================================
// USER MANAGEMENT TESTS
// ============================================================================

#[tokio::test]
async fn test_user_create_and_retrieve() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let user_id = insert_entity(pool, "user", "newuser", "New User").await;
    insert_prop(pool, user_id, "email", "newuser@example.com").await;
    insert_prop(pool, user_id, "password", "hashed_pass").await;

    let row: (i64, String, String) = sqlx::query_as(
        "SELECT id, name, label FROM entities WHERE entity_type = 'user' AND id = $1"
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .expect("User should be retrievable");

    assert_eq!(row.0, user_id);
    assert_eq!(row.1, "newuser");
    assert_eq!(row.2, "New User");

    let email: (String,) = sqlx::query_as(
        "SELECT value FROM entity_properties WHERE entity_id = $1 AND key = 'email'"
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(email.0, "newuser@example.com");
}

#[tokio::test]
async fn test_user_update_properties() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let user_id = insert_entity(pool, "user", "updatetest", "Update Test").await;
    insert_prop(pool, user_id, "email", "old@example.com").await;
    insert_prop(pool, user_id, "email", "new@example.com").await;

    let email: (String,) = sqlx::query_as(
        "SELECT value FROM entity_properties WHERE entity_id = $1 AND key = 'email'"
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(email.0, "new@example.com");
}

// ============================================================================
// PERMISSION ENFORCEMENT TESTS
// ============================================================================

#[tokio::test]
async fn test_permission_admin_has_all() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Use the seeded relation types from common module
    let has_role_id: (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role'"
    ).fetch_one(pool).await.unwrap();
    let has_perm_id: (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission'"
    ).fetch_one(pool).await.unwrap();

    let admin_role_id = insert_entity(pool, "role", "admin", "Administrator").await;

    let p1 = insert_entity(pool, "permission", "users.list", "List Users").await;
    let p2 = insert_entity(pool, "permission", "users.create", "Create Users").await;
    let p3 = insert_entity(pool, "permission", "roles.manage", "Manage Roles").await;

    insert_relation(pool, has_perm_id.0, admin_role_id, p1).await;
    insert_relation(pool, has_perm_id.0, admin_role_id, p2).await;
    insert_relation(pool, has_perm_id.0, admin_role_id, p3).await;

    let admin_id = insert_entity(pool, "user", "admin", "Administrator User").await;
    insert_relation(pool, has_role_id.0, admin_id, admin_role_id).await;

    let perms = get_permissions_for_user(pool, admin_id).await;
    assert_eq!(perms.len(), 3);
    assert!(perms.contains("users.list"));
    assert!(perms.contains("users.create"));
    assert!(perms.contains("roles.manage"));
}

#[tokio::test]
async fn test_permission_viewer_limited() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let has_role_id: (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role'"
    ).fetch_one(pool).await.unwrap();
    let has_perm_id: (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission'"
    ).fetch_one(pool).await.unwrap();

    let viewer_role_id = insert_entity(pool, "role", "viewer", "Viewer").await;

    let p_list = insert_entity(pool, "permission", "users.list", "List Users").await;
    let _p_create = insert_entity(pool, "permission", "users.create", "Create Users").await;

    insert_relation(pool, has_perm_id.0, viewer_role_id, p_list).await;

    let viewer_id = insert_entity(pool, "user", "bob", "Bob Viewer").await;
    insert_relation(pool, has_role_id.0, viewer_id, viewer_role_id).await;

    let perms = get_permissions_for_user(pool, viewer_id).await;
    assert_eq!(perms.len(), 1);
    assert!(perms.contains("users.list"));
    assert!(!perms.contains("users.create"));
}

// ============================================================================
// USER DELETE & CASCADE TESTS
// ============================================================================

#[tokio::test]
async fn test_user_delete_cascades_properties_and_relations() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let has_role_id: (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role'"
    ).fetch_one(pool).await.unwrap();

    let role_id = insert_entity(pool, "role", "editor", "Editor").await;
    let user_id = insert_entity(pool, "user", "deleteme", "Delete Me").await;
    insert_prop(pool, user_id, "email", "delete@example.com").await;
    insert_prop(pool, user_id, "password", "hashed_pass").await;
    insert_relation(pool, has_role_id.0, user_id, role_id).await;

    // Verify data exists before delete
    let prop_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM entity_properties WHERE entity_id = $1"
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(prop_count.0, 2, "Should have 2 properties before delete");

    let rel_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM relations WHERE source_id = $1"
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(rel_count.0, 1, "Should have 1 relation before delete");

    // Delete user entity
    sqlx::query("DELETE FROM entities WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await
        .unwrap();

    // Properties should be cascade-deleted
    let prop_count_after: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM entity_properties WHERE entity_id = $1"
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(prop_count_after.0, 0, "Properties should be cascade-deleted");

    // Relations should be cascade-deleted
    let rel_count_after: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM relations WHERE source_id = $1"
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(rel_count_after.0, 0, "Relations should be cascade-deleted");

    // Role should still exist (not cascade-deleted)
    let role_exists: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM entities WHERE id = $1"
    )
    .bind(role_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert!(role_exists.0 > 0, "Role should still exist after user deletion");
}

// ============================================================================
// USER LIST & SEARCH TESTS
// ============================================================================

#[tokio::test]
async fn test_user_list_with_search() {
    let db = setup_test_db().await;
    let pool = db.pool();

    insert_entity(pool, "user", "alice", "Alice Smith").await;
    insert_entity(pool, "user", "bob", "Bob Jones").await;
    insert_entity(pool, "user", "charlie", "Charlie Smith").await;
    // Non-user entity should not appear in user searches
    insert_entity(pool, "role", "admin", "Administrator").await;

    // Search by name (username)
    let results: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT e.id, e.name, e.label FROM entities e
         WHERE e.entity_type = 'user' AND (e.name LIKE $1 OR e.label LIKE $1)
         ORDER BY e.name"
    )
    .bind("%alice%")
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1, "alice");

    // Search by label (display name) - should find both Smiths
    let results: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT e.id, e.name, e.label FROM entities e
         WHERE e.entity_type = 'user' AND (e.name LIKE $1 OR e.label LIKE $1)
         ORDER BY e.name"
    )
    .bind("%Smith%")
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].1, "alice");
    assert_eq!(results[1].1, "charlie");

    // Pagination: LIMIT/OFFSET pattern
    let page1: Vec<(String,)> = sqlx::query_as(
        "SELECT e.name FROM entities e
         WHERE e.entity_type = 'user'
         ORDER BY e.name LIMIT 2 OFFSET 0"
    )
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(page1.len(), 2);
    assert_eq!(page1[0].0, "alice");
    assert_eq!(page1[1].0, "bob");

    let page2: Vec<(String,)> = sqlx::query_as(
        "SELECT e.name FROM entities e
         WHERE e.entity_type = 'user'
         ORDER BY e.name LIMIT 2 OFFSET 2"
    )
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(page2.len(), 1);
    assert_eq!(page2[0].0, "charlie");
}

// ============================================================================
// ENTITY UNIQUENESS CONSTRAINT TESTS
// ============================================================================

#[tokio::test]
async fn test_entity_uniqueness_constraint() {
    let db = setup_test_db().await;
    let pool = db.pool();

    insert_entity(pool, "user", "alice", "Alice").await;

    // Same entity_type + name should fail
    let result = sqlx::query(
        "INSERT INTO entities (entity_type, name, label) VALUES ('user', 'alice', 'Alice Duplicate')"
    )
    .execute(pool)
    .await;
    assert!(result.is_err(), "Duplicate (entity_type, name) should be rejected");

    // Same name but different entity_type should succeed
    let result = sqlx::query(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', 'alice', 'Alice Role')"
    )
    .execute(pool)
    .await;
    assert!(result.is_ok(), "Same name with different entity_type should be allowed");
}

// ============================================================================
// ROLE PERMISSION LIFECYCLE TESTS
// ============================================================================

#[tokio::test]
async fn test_role_permission_lifecycle() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Use seeded relation types
    let has_role_id: (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role'"
    ).fetch_one(pool).await.unwrap();
    let has_perm_id: (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission'"
    ).fetch_one(pool).await.unwrap();

    // Create role with initial permissions
    let role_id = insert_entity(pool, "role", "editor", "Editor").await;
    let p_list = insert_entity(pool, "permission", "users.list", "List Users").await;
    let p_edit = insert_entity(pool, "permission", "users.edit", "Edit Users").await;
    let p_create = insert_entity(pool, "permission", "users.create", "Create Users").await;

    insert_relation(pool, has_perm_id.0, role_id, p_list).await;
    insert_relation(pool, has_perm_id.0, role_id, p_edit).await;

    // Assign user to role
    let user_id = insert_entity(pool, "user", "editor_user", "Editor User").await;
    insert_relation(pool, has_role_id.0, user_id, role_id).await;

    // Verify initial permissions (list + edit)
    let perms = get_permissions_for_user(pool, user_id).await;
    assert_eq!(perms.len(), 2);
    assert!(perms.contains("users.list"));
    assert!(perms.contains("users.edit"));
    assert!(!perms.contains("users.create"));

    // Grant additional permission to role
    insert_relation(pool, has_perm_id.0, role_id, p_create).await;

    // User should now have 3 permissions (inherited through role)
    let perms = get_permissions_for_user(pool, user_id).await;
    assert_eq!(perms.len(), 3);
    assert!(perms.contains("users.create"));

    // Revoke a permission from role
    sqlx::query(
        "DELETE FROM relations WHERE relation_type_id = $1 AND source_id = $2 AND target_id = $3"
    )
    .bind(has_perm_id.0)
    .bind(role_id)
    .bind(p_edit)
    .execute(pool)
    .await
    .unwrap();

    // User should now have 2 permissions (list + create, not edit)
    let perms = get_permissions_for_user(pool, user_id).await;
    assert_eq!(perms.len(), 2);
    assert!(perms.contains("users.list"));
    assert!(perms.contains("users.create"));
    assert!(!perms.contains("users.edit"), "Revoked permission should be gone");
}

// ============================================================================
// PERMISSION EDGE CASE TESTS
// ============================================================================

#[tokio::test]
async fn test_user_no_role_has_no_permissions() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Seeded relation types exist (has_role, has_permission) from common module

    // Create permissions and a role with permissions
    let has_perm_id: (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission'"
    ).fetch_one(pool).await.unwrap();

    let role_id = insert_entity(pool, "role", "admin", "Administrator").await;
    let perm_id = insert_entity(pool, "permission", "users.list", "List Users").await;
    insert_relation(pool, has_perm_id.0, role_id, perm_id).await;

    // Create user WITHOUT assigning any role
    let user_id = insert_entity(pool, "user", "norole", "No Role User").await;

    let perms = get_permissions_for_user(pool, user_id).await;
    assert!(perms.is_empty(), "User without role should have no permissions");
}

// ============================================================================
// RELATION UNIQUENESS CONSTRAINT TESTS
// ============================================================================

#[tokio::test]
async fn test_relation_uniqueness_prevents_duplicate_role_assignment() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let has_role_id: (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role'"
    ).fetch_one(pool).await.unwrap();

    let role_id = insert_entity(pool, "role", "admin", "Administrator").await;
    let user_id = insert_entity(pool, "user", "alice", "Alice").await;

    // First assignment should succeed
    insert_relation(pool, has_role_id.0, user_id, role_id).await;

    // Duplicate assignment should fail (UNIQUE constraint on relation_type_id, source_id, target_id)
    let result = sqlx::query(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES ($1, $2, $3)"
    )
    .bind(has_role_id.0)
    .bind(user_id)
    .bind(role_id)
    .execute(pool)
    .await;
    assert!(result.is_err(), "Duplicate relation should be rejected by UNIQUE constraint");

    // Different relation type with same source/target should succeed
    let has_perm_id: (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission'"
    ).fetch_one(pool).await.unwrap();

    let result = sqlx::query(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES ($1, $2, $3)"
    )
    .bind(has_perm_id.0)
    .bind(user_id)
    .bind(role_id)
    .execute(pool)
    .await;
    assert!(result.is_ok(), "Different relation type with same entities should be allowed");
}

// ============================================================================
// NAV ITEM PERMISSION GATING TESTS
// ============================================================================

#[tokio::test]
async fn test_nav_item_permission_gating() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let has_role_id: (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role'"
    ).fetch_one(pool).await.unwrap();
    let has_perm_id: (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission'"
    ).fetch_one(pool).await.unwrap();
    let requires_perm_id: (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'requires_permission'"
    ).fetch_one(pool).await.unwrap();

    // Create permissions
    let p_users = insert_entity(pool, "permission", "users.list", "List Users").await;
    let p_roles = insert_entity(pool, "permission", "roles.manage", "Manage Roles").await;

    // Create nav items with permission requirements
    let nav_users = insert_entity(pool, "nav_item", "admin.users", "Users").await;
    insert_prop(pool, nav_users, "url", "/users").await;
    insert_relation(pool, requires_perm_id.0, nav_users, p_users).await;

    let nav_roles = insert_entity(pool, "nav_item", "admin.roles", "Roles").await;
    insert_prop(pool, nav_roles, "url", "/roles").await;
    insert_relation(pool, requires_perm_id.0, nav_roles, p_roles).await;

    // Create viewer role (only users.list permission)
    let viewer_role = insert_entity(pool, "role", "viewer", "Viewer").await;
    insert_relation(pool, has_perm_id.0, viewer_role, p_users).await;

    let viewer_id = insert_entity(pool, "user", "viewer_user", "Viewer User").await;
    insert_relation(pool, has_role_id.0, viewer_id, viewer_role).await;

    let user_perms = get_permissions_for_user(pool, viewer_id).await;

    // Query nav items and check which ones user has access to
    let nav_items: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT ni.name, ni.label, COALESCE(ep.value, '') as url, p.name as required_perm
         FROM entities ni
         JOIN relations r ON r.source_id = ni.id AND r.relation_type_id = $1
         JOIN entities p ON p.id = r.target_id
         LEFT JOIN entity_properties ep ON ep.entity_id = ni.id AND ep.key = 'url'
         WHERE ni.entity_type = 'nav_item'
         ORDER BY ni.name"
    )
    .bind(requires_perm_id.0)
    .fetch_all(pool)
    .await
    .unwrap();

    assert_eq!(nav_items.len(), 2, "Should have 2 nav items with permission requirements");

    // Filter nav items by user's permissions
    let accessible: Vec<&(String, String, String, String)> = nav_items.iter()
        .filter(|(_, _, _, required_perm)| user_perms.contains(required_perm))
        .collect();

    assert_eq!(accessible.len(), 1, "Viewer should only access 1 nav item");
    assert_eq!(accessible[0].0, "admin.users", "Viewer should access Users nav item");

    let inaccessible: Vec<&(String, String, String, String)> = nav_items.iter()
        .filter(|(_, _, _, required_perm)| !user_perms.contains(required_perm))
        .collect();

    assert_eq!(inaccessible.len(), 1);
    assert_eq!(inaccessible[0].0, "admin.roles", "Viewer should NOT access Roles nav item");
}
