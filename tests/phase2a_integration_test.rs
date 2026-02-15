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
