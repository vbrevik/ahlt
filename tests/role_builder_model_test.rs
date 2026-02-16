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

    // Create test permission
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (100, 'permission', 'test.view', 'Test View')",
        [],
    ).unwrap();

    // Create test nav item with permission requirement
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (200, 'nav_item', 'test_page', 'Test Page')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (200, 'path', '/test')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (200, 'permission_required', 'test.view')",
        [],
    ).unwrap();

    // Create nav module
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (201, 'nav_module', 'test_module', 'Test Module')",
        [],
    ).unwrap();

    // Create in_module relation type
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (999, 'relation_type', 'in_module', 'In Module')",
        [],
    ).unwrap();

    // Link nav item to module
    let rt_id: i64 = 999;
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, 200, 201)",
        [rt_id],
    ).unwrap();

    // Query with permission ID 100
    let items = find_accessible_nav_items(&conn, &[100]).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Test Page");
    assert_eq!(items[0].path, "/test");
    assert_eq!(items[0].module_name, "Test Module");
}
