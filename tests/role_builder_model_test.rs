use tempfile::TempDir;
use ahlt::models::role::builder::find_accessible_nav_items;

const MIGRATIONS: &str = include_str!("../src/schema.sql");

fn setup_test_db() -> (TempDir, rusqlite::Connection) {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = dir.path().join("test.db");
    let conn = rusqlite::Connection::open(&db_path).expect("Failed to open test DB");
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;")
        .expect("Failed to set pragmas");
    conn.execute_batch(MIGRATIONS).expect("Failed to run migrations");
    (dir, conn)
}

#[test]
fn test_find_accessible_nav_items() {
    let (_dir, conn) = setup_test_db();

    // Create requires_permission relation type
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (999, 'relation_type', 'requires_permission', 'Requires Permission')",
        [],
    ).unwrap();

    // Create test permission
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (100, 'permission', 'test.view', 'Test View')",
        [],
    ).unwrap();

    // Create parent module (top-level nav_item, no parent)
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label, is_active, sort_order) VALUES (201, 'nav_item', 'test_module', 'Test Module', 1, 1)",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (201, 'url', '/test-module')",
        [],
    ).unwrap();

    // Create child nav item with parent property
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label, is_active, sort_order) VALUES (200, 'nav_item', 'test_page', 'Test Page', 1, 2)",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (200, 'url', '/test')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (200, 'parent', 'test_module')",
        [],
    ).unwrap();

    // Link child nav item to permission via requires_permission relation
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (999, 200, 100)",
        [],
    ).unwrap();

    // Query with permission ID 100
    let items = find_accessible_nav_items(&conn, &[100]).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Test Page");
    assert_eq!(items[0].path, "/test");
    assert_eq!(items[0].module_name, "Test Module");
}

#[test]
fn test_standalone_module_appears_when_permitted() {
    let (_dir, conn) = setup_test_db();

    // Create requires_permission relation type
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (999, 'relation_type', 'requires_permission', 'Requires Permission')",
        [],
    ).unwrap();

    // Create permission
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (100, 'permission', 'dashboard.view', 'View Dashboard')",
        [],
    ).unwrap();

    // Create standalone module (no children) with permission
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label, is_active, sort_order) VALUES (201, 'nav_item', 'dashboard', 'Dashboard', 1, 1)",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (201, 'url', '/dashboard')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (999, 201, 100)",
        [],
    ).unwrap();

    let items = find_accessible_nav_items(&conn, &[100]).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Dashboard");
}

#[test]
fn test_unpermitted_items_excluded() {
    let (_dir, conn) = setup_test_db();

    // Create requires_permission relation type
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (999, 'relation_type', 'requires_permission', 'Requires Permission')",
        [],
    ).unwrap();

    // Create two permissions
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (100, 'permission', 'users.list', 'List Users')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (101, 'permission', 'roles.manage', 'Manage Roles')",
        [],
    ).unwrap();

    // Parent module
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label, is_active, sort_order) VALUES (201, 'nav_item', 'admin', 'Admin', 1, 1)",
        [],
    ).unwrap();

    // Child requiring users.list
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label, is_active, sort_order) VALUES (202, 'nav_item', 'admin.users', 'Users', 1, 2)",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (202, 'parent', 'admin')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (999, 202, 100)",
        [],
    ).unwrap();

    // Child requiring roles.manage
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label, is_active, sort_order) VALUES (203, 'nav_item', 'admin.roles', 'Roles', 1, 3)",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (203, 'parent', 'admin')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (999, 203, 101)",
        [],
    ).unwrap();

    // Only grant users.list — should see Users but not Roles
    let items = find_accessible_nav_items(&conn, &[100]).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Users");
}
