use std::collections::HashSet;
use regex::Regex;
use tempfile::TempDir;

// ============================================================================
// SHARED TEST INFRASTRUCTURE
// ============================================================================

/// Real schema from src/schema.sql — single source of truth
const MIGRATIONS: &str = include_str!("../src/schema.sql");

fn setup_test_db() -> (TempDir, rusqlite::Connection) {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = dir.path().join("test.db");
    let conn = rusqlite::Connection::open(&db_path).expect("Failed to open test DB");
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;")
        .expect("Failed to set pragmas");
    conn.execute_batch(MIGRATIONS)
        .expect("Failed to run migrations");
    (dir, conn)
}

fn insert_entity(
    conn: &rusqlite::Connection,
    entity_type: &str,
    name: &str,
    label: &str,
) -> i64 {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES (?, ?, ?)",
        [entity_type, name, label],
    ).expect("Failed to insert entity");
    conn.last_insert_rowid()
}

fn insert_prop(conn: &rusqlite::Connection, entity_id: i64, key: &str, value: &str) {
    conn.execute(
        "INSERT OR REPLACE INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
        rusqlite::params![entity_id, key, value],
    ).expect("Failed to insert property");
}

fn insert_relation(
    conn: &rusqlite::Connection,
    relation_type_id: i64,
    source_id: i64,
    target_id: i64,
) -> i64 {
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
        rusqlite::params![relation_type_id, source_id, target_id],
    ).expect("Failed to insert relation");
    conn.last_insert_rowid()
}

fn get_permissions_for_user(conn: &rusqlite::Connection, user_id: i64) -> HashSet<String> {
    let mut stmt = conn.prepare(
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
                 AND r2.source_id = ?1
             )
         )"
    ).expect("Failed to prepare permissions query");

    stmt.query_map([user_id], |row| row.get(0))
        .expect("Failed to query permissions")
        .filter_map(Result::ok)
        .collect()
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

#[test]
fn test_infrastructure_compiled() {
    let (_dir, conn) = setup_test_db();
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM entities", [], |row| row.get(0)).unwrap();
    assert_eq!(count, 0, "Fresh database should have zero entities");
}

#[test]
fn test_csrf_extraction() {
    let html = r#"<form><input type="hidden" name="csrf_token" value="test_token_12345" /></form>"#;
    assert_eq!(extract_csrf_token(html), "test_token_12345");
}

#[test]
fn test_csrf_extraction_missing() {
    let html = r#"<form><input type="text" name="username" /></form>"#;
    assert_eq!(extract_csrf_token(html), "invalid_token");
}

// ============================================================================
// AUTHENTICATION TESTS
// ============================================================================

#[test]
fn test_auth_user_lookup() {
    let (_dir, conn) = setup_test_db();

    let user_id = insert_entity(&conn, "user", "testuser", "Test User");
    insert_prop(&conn, user_id, "password", "hashed_password_123");

    let (found_id, found_name): (i64, String) = conn.query_row(
        "SELECT id, name FROM entities WHERE entity_type = 'user' AND name = 'testuser'",
        [], |row| Ok((row.get(0)?, row.get(1)?)),
    ).expect("User should be found");

    assert_eq!(found_id, user_id);
    assert_eq!(found_name, "testuser");

    let password: String = conn.query_row(
        "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = 'password'",
        [user_id], |row| row.get(0),
    ).expect("Password should be retrievable");
    assert_eq!(password, "hashed_password_123");
}

#[test]
fn test_auth_nonexistent_user() {
    let (_dir, conn) = setup_test_db();

    let found: Option<i64> = conn.query_row(
        "SELECT id FROM entities WHERE entity_type = 'user' AND name = 'nonexistent'",
        [], |row| row.get(0),
    ).ok();
    assert!(found.is_none());
}

#[test]
fn test_auth_permission_assignment() {
    let (_dir, conn) = setup_test_db();

    let has_role_id = insert_entity(&conn, "relation_type", "has_role", "Has Role");
    let has_permission_id = insert_entity(&conn, "relation_type", "has_permission", "Has Permission");

    let user_id = insert_entity(&conn, "user", "alice", "Alice");
    let admin_role_id = insert_entity(&conn, "role", "admin", "Administrator");
    let perm_id = insert_entity(&conn, "permission", "users.create", "Create Users");

    insert_relation(&conn, has_role_id, user_id, admin_role_id);
    insert_relation(&conn, has_permission_id, admin_role_id, perm_id);

    let has_perm: bool = conn.query_row(
        "SELECT COUNT(*) FROM entities p
         WHERE p.entity_type = 'permission' AND p.name = 'users.create'
         AND EXISTS (
             SELECT 1 FROM relations r1
             WHERE r1.relation_type_id = ?1 AND r1.target_id = p.id
             AND r1.source_id IN (
                 SELECT r2.target_id FROM relations r2
                 WHERE r2.relation_type_id = ?2 AND r2.source_id = ?3
             )
         )",
        [has_permission_id, has_role_id, user_id],
        |row| row.get::<_, i64>(0).map(|c| c > 0),
    ).unwrap();

    assert!(has_perm, "User should have users.create through role");
}

// ============================================================================
// USER MANAGEMENT TESTS
// ============================================================================

#[test]
fn test_user_create_and_retrieve() {
    let (_dir, conn) = setup_test_db();

    let user_id = insert_entity(&conn, "user", "newuser", "New User");
    insert_prop(&conn, user_id, "email", "newuser@example.com");
    insert_prop(&conn, user_id, "password", "hashed_pass");

    let (id, name, label): (i64, String, String) = conn.query_row(
        "SELECT id, name, label FROM entities WHERE entity_type = 'user' AND id = ?1",
        [user_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).expect("User should be retrievable");

    assert_eq!(id, user_id);
    assert_eq!(name, "newuser");
    assert_eq!(label, "New User");

    let email: String = conn.query_row(
        "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = 'email'",
        [user_id], |row| row.get(0),
    ).unwrap();
    assert_eq!(email, "newuser@example.com");
}

#[test]
fn test_user_update_properties() {
    let (_dir, conn) = setup_test_db();

    let user_id = insert_entity(&conn, "user", "updatetest", "Update Test");
    insert_prop(&conn, user_id, "email", "old@example.com");
    insert_prop(&conn, user_id, "email", "new@example.com");

    let email: String = conn.query_row(
        "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = 'email'",
        [user_id], |row| row.get(0),
    ).unwrap();
    assert_eq!(email, "new@example.com");
}

// ============================================================================
// PERMISSION ENFORCEMENT TESTS
// ============================================================================

#[test]
fn test_permission_admin_has_all() {
    let (_dir, conn) = setup_test_db();

    let has_role_id = insert_entity(&conn, "relation_type", "has_role", "Has Role");
    let has_perm_id = insert_entity(&conn, "relation_type", "has_permission", "Has Permission");

    let admin_role_id = insert_entity(&conn, "role", "admin", "Administrator");

    let p1 = insert_entity(&conn, "permission", "users.list", "List Users");
    let p2 = insert_entity(&conn, "permission", "users.create", "Create Users");
    let p3 = insert_entity(&conn, "permission", "roles.manage", "Manage Roles");

    insert_relation(&conn, has_perm_id, admin_role_id, p1);
    insert_relation(&conn, has_perm_id, admin_role_id, p2);
    insert_relation(&conn, has_perm_id, admin_role_id, p3);

    let admin_id = insert_entity(&conn, "user", "admin", "Administrator");
    insert_relation(&conn, has_role_id, admin_id, admin_role_id);

    let perms = get_permissions_for_user(&conn, admin_id);
    assert_eq!(perms.len(), 3);
    assert!(perms.contains("users.list"));
    assert!(perms.contains("users.create"));
    assert!(perms.contains("roles.manage"));
}

#[test]
fn test_permission_viewer_limited() {
    let (_dir, conn) = setup_test_db();

    let has_role_id = insert_entity(&conn, "relation_type", "has_role", "Has Role");
    let has_perm_id = insert_entity(&conn, "relation_type", "has_permission", "Has Permission");

    let viewer_role_id = insert_entity(&conn, "role", "viewer", "Viewer");

    let p_list = insert_entity(&conn, "permission", "users.list", "List Users");
    let _p_create = insert_entity(&conn, "permission", "users.create", "Create Users");

    insert_relation(&conn, has_perm_id, viewer_role_id, p_list);

    let viewer_id = insert_entity(&conn, "user", "bob", "Bob Viewer");
    insert_relation(&conn, has_role_id, viewer_id, viewer_role_id);

    let perms = get_permissions_for_user(&conn, viewer_id);
    assert_eq!(perms.len(), 1);
    assert!(perms.contains("users.list"));
    assert!(!perms.contains("users.create"));
}

// ============================================================================
// USER DELETE & CASCADE TESTS
// ============================================================================

#[test]
fn test_user_delete_cascades_properties_and_relations() {
    let (_dir, conn) = setup_test_db();

    let has_role_id = insert_entity(&conn, "relation_type", "has_role", "Has Role");
    let role_id = insert_entity(&conn, "role", "editor", "Editor");
    let user_id = insert_entity(&conn, "user", "deleteme", "Delete Me");
    insert_prop(&conn, user_id, "email", "delete@example.com");
    insert_prop(&conn, user_id, "password", "hashed_pass");
    insert_relation(&conn, has_role_id, user_id, role_id);

    // Verify data exists before delete
    let prop_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entity_properties WHERE entity_id = ?1",
        [user_id], |row| row.get(0),
    ).unwrap();
    assert_eq!(prop_count, 2, "Should have 2 properties before delete");

    let rel_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM relations WHERE source_id = ?1",
        [user_id], |row| row.get(0),
    ).unwrap();
    assert_eq!(rel_count, 1, "Should have 1 relation before delete");

    // Delete user entity
    conn.execute("DELETE FROM entities WHERE id = ?1", [user_id]).unwrap();

    // Properties should be cascade-deleted
    let prop_count_after: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entity_properties WHERE entity_id = ?1",
        [user_id], |row| row.get(0),
    ).unwrap();
    assert_eq!(prop_count_after, 0, "Properties should be cascade-deleted");

    // Relations should be cascade-deleted
    let rel_count_after: i64 = conn.query_row(
        "SELECT COUNT(*) FROM relations WHERE source_id = ?1",
        [user_id], |row| row.get(0),
    ).unwrap();
    assert_eq!(rel_count_after, 0, "Relations should be cascade-deleted");

    // Role should still exist (not cascade-deleted)
    let role_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM entities WHERE id = ?1",
        [role_id], |row| row.get::<_, i64>(0).map(|c| c > 0),
    ).unwrap();
    assert!(role_exists, "Role should still exist after user deletion");
}

// ============================================================================
// USER LIST & SEARCH TESTS
// ============================================================================

#[test]
fn test_user_list_with_search() {
    let (_dir, conn) = setup_test_db();

    insert_entity(&conn, "user", "alice", "Alice Smith");
    insert_entity(&conn, "user", "bob", "Bob Jones");
    insert_entity(&conn, "user", "charlie", "Charlie Smith");
    // Non-user entity should not appear in user searches
    insert_entity(&conn, "role", "admin", "Administrator");

    // Search by name (username)
    let results: Vec<(i64, String, String)> = {
        let search = "%alice%";
        let mut stmt = conn.prepare(
            "SELECT e.id, e.name, e.label FROM entities e
             WHERE e.entity_type = 'user' AND (e.name LIKE ?1 OR e.label LIKE ?1)
             ORDER BY e.name"
        ).unwrap();
        stmt.query_map([search], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        }).unwrap().filter_map(Result::ok).collect()
    };
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].1, "alice");

    // Search by label (display name) - should find both Smiths
    let results: Vec<(i64, String, String)> = {
        let search = "%Smith%";
        let mut stmt = conn.prepare(
            "SELECT e.id, e.name, e.label FROM entities e
             WHERE e.entity_type = 'user' AND (e.name LIKE ?1 OR e.label LIKE ?1)
             ORDER BY e.name"
        ).unwrap();
        stmt.query_map([search], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        }).unwrap().filter_map(Result::ok).collect()
    };
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].1, "alice");
    assert_eq!(results[1].1, "charlie");

    // Pagination: LIMIT/OFFSET pattern
    let page1: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT e.name FROM entities e
             WHERE e.entity_type = 'user'
             ORDER BY e.name LIMIT 2 OFFSET 0"
        ).unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap().filter_map(Result::ok).collect()
    };
    assert_eq!(page1.len(), 2);
    assert_eq!(page1[0], "alice");
    assert_eq!(page1[1], "bob");

    let page2: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT e.name FROM entities e
             WHERE e.entity_type = 'user'
             ORDER BY e.name LIMIT 2 OFFSET 2"
        ).unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap().filter_map(Result::ok).collect()
    };
    assert_eq!(page2.len(), 1);
    assert_eq!(page2[0], "charlie");
}

// ============================================================================
// ENTITY UNIQUENESS CONSTRAINT TESTS
// ============================================================================

#[test]
fn test_entity_uniqueness_constraint() {
    let (_dir, conn) = setup_test_db();

    insert_entity(&conn, "user", "alice", "Alice");

    // Same entity_type + name should fail
    let result = conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('user', 'alice', 'Alice Duplicate')",
        [],
    );
    assert!(result.is_err(), "Duplicate (entity_type, name) should be rejected");

    // Same name but different entity_type should succeed
    let result = conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', 'alice', 'Alice Role')",
        [],
    );
    assert!(result.is_ok(), "Same name with different entity_type should be allowed");
}

// ============================================================================
// ROLE PERMISSION LIFECYCLE TESTS
// ============================================================================

#[test]
fn test_role_permission_lifecycle() {
    let (_dir, conn) = setup_test_db();

    let has_role_id = insert_entity(&conn, "relation_type", "has_role", "Has Role");
    let has_perm_id = insert_entity(&conn, "relation_type", "has_permission", "Has Permission");

    // Create role with initial permissions
    let role_id = insert_entity(&conn, "role", "editor", "Editor");
    let p_list = insert_entity(&conn, "permission", "users.list", "List Users");
    let p_edit = insert_entity(&conn, "permission", "users.edit", "Edit Users");
    let p_create = insert_entity(&conn, "permission", "users.create", "Create Users");

    insert_relation(&conn, has_perm_id, role_id, p_list);
    insert_relation(&conn, has_perm_id, role_id, p_edit);

    // Assign user to role
    let user_id = insert_entity(&conn, "user", "editor_user", "Editor User");
    insert_relation(&conn, has_role_id, user_id, role_id);

    // Verify initial permissions (list + edit)
    let perms = get_permissions_for_user(&conn, user_id);
    assert_eq!(perms.len(), 2);
    assert!(perms.contains("users.list"));
    assert!(perms.contains("users.edit"));
    assert!(!perms.contains("users.create"));

    // Grant additional permission to role
    insert_relation(&conn, has_perm_id, role_id, p_create);

    // User should now have 3 permissions (inherited through role)
    let perms = get_permissions_for_user(&conn, user_id);
    assert_eq!(perms.len(), 3);
    assert!(perms.contains("users.create"));

    // Revoke a permission from role
    conn.execute(
        "DELETE FROM relations WHERE relation_type_id = ?1 AND source_id = ?2 AND target_id = ?3",
        rusqlite::params![has_perm_id, role_id, p_edit],
    ).unwrap();

    // User should now have 2 permissions (list + create, not edit)
    let perms = get_permissions_for_user(&conn, user_id);
    assert_eq!(perms.len(), 2);
    assert!(perms.contains("users.list"));
    assert!(perms.contains("users.create"));
    assert!(!perms.contains("users.edit"), "Revoked permission should be gone");
}

// ============================================================================
// PERMISSION EDGE CASE TESTS
// ============================================================================

#[test]
fn test_user_no_role_has_no_permissions() {
    let (_dir, conn) = setup_test_db();

    let _has_role_id = insert_entity(&conn, "relation_type", "has_role", "Has Role");
    let _has_perm_id = insert_entity(&conn, "relation_type", "has_permission", "Has Permission");

    // Create permissions and a role with permissions
    let role_id = insert_entity(&conn, "role", "admin", "Administrator");
    let perm_id = insert_entity(&conn, "permission", "users.list", "List Users");
    insert_relation(&conn, _has_perm_id, role_id, perm_id);

    // Create user WITHOUT assigning any role
    let user_id = insert_entity(&conn, "user", "norole", "No Role User");

    let perms = get_permissions_for_user(&conn, user_id);
    assert!(perms.is_empty(), "User without role should have no permissions");
}

// ============================================================================
// RELATION UNIQUENESS CONSTRAINT TESTS
// ============================================================================

#[test]
fn test_relation_uniqueness_prevents_duplicate_role_assignment() {
    let (_dir, conn) = setup_test_db();

    let has_role_id = insert_entity(&conn, "relation_type", "has_role", "Has Role");
    let role_id = insert_entity(&conn, "role", "admin", "Administrator");
    let user_id = insert_entity(&conn, "user", "alice", "Alice");

    // First assignment should succeed
    insert_relation(&conn, has_role_id, user_id, role_id);

    // Duplicate assignment should fail (UNIQUE constraint on relation_type_id, source_id, target_id)
    let result = conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
        rusqlite::params![has_role_id, user_id, role_id],
    );
    assert!(result.is_err(), "Duplicate relation should be rejected by UNIQUE constraint");

    // Different relation type with same source/target should succeed
    let has_perm_id = insert_entity(&conn, "relation_type", "has_permission", "Has Permission");
    let result = conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
        rusqlite::params![has_perm_id, user_id, role_id],
    );
    assert!(result.is_ok(), "Different relation type with same entities should be allowed");
}

// ============================================================================
// NAV ITEM PERMISSION GATING TESTS
// ============================================================================

#[test]
fn test_nav_item_permission_gating() {
    let (_dir, conn) = setup_test_db();

    let has_role_id = insert_entity(&conn, "relation_type", "has_role", "Has Role");
    let has_perm_id = insert_entity(&conn, "relation_type", "has_permission", "Has Permission");
    let requires_perm_id = insert_entity(&conn, "relation_type", "requires_permission", "Requires Permission");

    // Create permissions
    let p_users = insert_entity(&conn, "permission", "users.list", "List Users");
    let p_roles = insert_entity(&conn, "permission", "roles.manage", "Manage Roles");

    // Create nav items with permission requirements
    let nav_users = insert_entity(&conn, "nav_item", "admin.users", "Users");
    insert_prop(&conn, nav_users, "url", "/users");
    insert_relation(&conn, requires_perm_id, nav_users, p_users);

    let nav_roles = insert_entity(&conn, "nav_item", "admin.roles", "Roles");
    insert_prop(&conn, nav_roles, "url", "/roles");
    insert_relation(&conn, requires_perm_id, nav_roles, p_roles);

    // Create viewer role (only users.list permission)
    let viewer_role = insert_entity(&conn, "role", "viewer", "Viewer");
    insert_relation(&conn, has_perm_id, viewer_role, p_users);

    let viewer_id = insert_entity(&conn, "user", "viewer_user", "Viewer User");
    insert_relation(&conn, has_role_id, viewer_id, viewer_role);

    let user_perms = get_permissions_for_user(&conn, viewer_id);

    // Query nav items and check which ones user has access to
    let mut stmt = conn.prepare(
        "SELECT ni.name, ni.label, COALESCE(ep.value, '') as url, p.name as required_perm
         FROM entities ni
         JOIN relations r ON r.source_id = ni.id AND r.relation_type_id = ?1
         JOIN entities p ON p.id = r.target_id
         LEFT JOIN entity_properties ep ON ep.entity_id = ni.id AND ep.key = 'url'
         WHERE ni.entity_type = 'nav_item'
         ORDER BY ni.name"
    ).unwrap();

    let nav_items: Vec<(String, String, String, String)> = stmt.query_map(
        [requires_perm_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).unwrap().filter_map(Result::ok).collect();

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
