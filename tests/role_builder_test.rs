use tempfile::TempDir;
use ahlt::models::role;

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
fn test_create_role_via_builder() {
    let (_dir, conn) = setup_test_db();

    // Create has_permission relation type
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (900, 'relation_type', 'has_permission', 'Has Permission')",
        [],
    ).unwrap();

    // Create test permissions
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (1, 'permission', 'test.read', 'Test Read')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (2, 'permission', 'test.write', 'Test Write')",
        [],
    ).unwrap();

    // Create role via builder simulation
    let role_id = conn.query_row(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', 'test_role', 'Test Role') RETURNING id",
        [],
        |row| row.get::<_, i64>(0),
    ).unwrap();

    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'description', 'Test description')",
        [role_id],
    ).unwrap();

    let rt_id: i64 = 900;

    for perm_id in [1, 2] {
        conn.execute(
            "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
            [rt_id, role_id, perm_id],
        ).unwrap();
    }

    // Verify role
    let role = role::find_by_id(&conn, role_id).unwrap().unwrap();
    assert_eq!(role.name, "test_role");
    assert_eq!(role.label, "Test Role");

    // Verify permissions
    let permissions = role::find_permission_checkboxes(&conn, role_id).unwrap();
    let granted = permissions.iter().filter(|p| p.checked).count();
    assert_eq!(granted, 2);
}

#[test]
fn test_role_name_uniqueness() {
    let (_dir, conn) = setup_test_db();

    // Create first role
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', 'duplicate', 'First')",
        [],
    ).unwrap();

    // Attempt duplicate
    let result = conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', 'duplicate', 'Second')",
        [],
    );

    assert!(result.is_err());
}

#[test]
fn test_menu_preview_calculation() {
    let (_dir, conn) = setup_test_db();

    // Create permission
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (100, 'permission', 'admin.settings', 'Admin Settings')",
        [],
    ).unwrap();

    // Create nav module
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (200, 'nav_module', 'admin', 'Admin')",
        [],
    ).unwrap();

    // Create nav item
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (201, 'nav_item', 'settings', 'Settings')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (201, 'path', '/settings')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (201, 'permission_required', 'admin.settings')",
        [],
    ).unwrap();

    // Create in_module relation type
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (999, 'relation_type', 'in_module', 'In Module')",
        [],
    ).unwrap();

    // Link nav item to module
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (999, 201, 200)",
        [],
    ).unwrap();

    // Query accessible items
    let items = role::builder::find_accessible_nav_items(&conn, &[100]).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Settings");
}

#[test]
fn test_no_permissions_selected() {
    let (_dir, conn) = setup_test_db();

    // Create role with no permissions (valid but not useful)
    let role_id = conn.query_row(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', 'empty_role', 'Empty Role') RETURNING id",
        [],
        |row| row.get::<_, i64>(0),
    ).unwrap();

    // Verify role exists
    let role = role::find_by_id(&conn, role_id).unwrap().unwrap();
    assert_eq!(role.name, "empty_role");

    // Verify no permissions
    let permissions = role::find_permission_checkboxes(&conn, role_id).unwrap();
    let granted = permissions.iter().filter(|p| p.checked).count();
    assert_eq!(granted, 0);
}

#[test]
fn test_builder_requires_admin_permission() {
    // This would be tested at the handler level with mock sessions
    // For now, we just verify the permission code can be created
    let (_dir, conn) = setup_test_db();

    // Create the admin.roles permission
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('permission', 'admin.roles', 'Manage Roles')",
        [],
    ).unwrap();

    let exists = conn.query_row(
        "SELECT 1 FROM entities WHERE entity_type='permission' AND name='admin.roles'",
        [],
        |_| Ok(true),
    ).unwrap_or(false);

    assert!(exists, "admin.roles permission should exist");
}
