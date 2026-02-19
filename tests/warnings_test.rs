use tempfile::TempDir;
use rusqlite::params;

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

fn insert_entity(conn: &rusqlite::Connection, entity_type: &str, name: &str, label: &str) -> i64 {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES (?, ?, ?)",
        [entity_type, name, label],
    ).expect("Failed to insert entity");
    conn.last_insert_rowid()
}

fn insert_prop(conn: &rusqlite::Connection, entity_id: i64, key: &str, value: &str) {
    conn.execute(
        "INSERT OR REPLACE INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
        params![entity_id, key, value],
    ).expect("Failed to insert property");
}

fn seed_warning_types(conn: &rusqlite::Connection) {
    // Seed the relation types needed by the warning system
    insert_entity(conn, "relation_type", "targets_user", "Targets User");
    insert_entity(conn, "relation_type", "for_warning", "For Warning");
    insert_entity(conn, "relation_type", "for_user", "For User");
    insert_entity(conn, "relation_type", "on_receipt", "On Receipt");
    insert_entity(conn, "relation_type", "forwarded_to_user", "Forwarded To User");
}

fn seed_users(conn: &rusqlite::Connection) -> (i64, i64) {
    let user1 = insert_entity(conn, "user", "alice", "Alice");
    insert_prop(conn, user1, "email", "alice@test.com");
    let user2 = insert_entity(conn, "user", "bob", "Bob");
    insert_prop(conn, user2, "email", "bob@test.com");
    (user1, user2)
}

fn seed_tor_types(conn: &rusqlite::Connection) {
    insert_entity(conn, "relation_type", "belongs_to_tor", "Belongs To ToR");
    insert_entity(conn, "relation_type", "fills_position", "Fills Position");
    insert_entity(conn, "relation_type", "has_role", "Has Role");
    insert_entity(conn, "relation_type", "has_permission", "Has Permission");
}

// --- Tests ---

#[test]
fn test_create_warning_and_receipts() {
    let (_dir, conn) = setup_test_db();
    seed_warning_types(&conn);
    let (user1, user2) = seed_users(&conn);

    let warning_id = ahlt::warnings::create_warning(
        &conn, "high", "security", "test.action", "Test warning message", "details", "system",
    ).expect("Failed to create warning");

    // Verify warning entity exists
    let entity_type: String = conn.query_row(
        "SELECT entity_type FROM entities WHERE id = ?1",
        params![warning_id], |row| row.get(0),
    ).unwrap();
    assert_eq!(entity_type, "warning");

    // Verify properties
    let severity: String = conn.query_row(
        "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = 'severity'",
        params![warning_id], |row| row.get(0),
    ).unwrap();
    assert_eq!(severity, "high");

    // Create receipts for both users
    let receipt_ids = ahlt::warnings::create_receipts(&conn, warning_id, &[user1, user2])
        .expect("Failed to create receipts");
    assert_eq!(receipt_ids.len(), 2);

    // Verify receipt entities exist
    for receipt_id in &receipt_ids {
        let rt: String = conn.query_row(
            "SELECT entity_type FROM entities WHERE id = ?1",
            params![receipt_id], |row| row.get(0),
        ).unwrap();
        assert_eq!(rt, "warning_receipt");
    }
}

#[test]
fn test_count_unread() {
    let (_dir, conn) = setup_test_db();
    seed_warning_types(&conn);
    let (user1, user2) = seed_users(&conn);

    // Initially zero
    assert_eq!(ahlt::warnings::queries::count_unread(&conn, user1), 0);

    // Create a warning with receipt for user1
    let w1 = ahlt::warnings::create_warning(
        &conn, "info", "system", "test.1", "Warning 1", "", "system",
    ).unwrap();
    ahlt::warnings::create_receipts(&conn, w1, &[user1]).unwrap();

    assert_eq!(ahlt::warnings::queries::count_unread(&conn, user1), 1);
    assert_eq!(ahlt::warnings::queries::count_unread(&conn, user2), 0);

    // Create another warning for both
    let w2 = ahlt::warnings::create_warning(
        &conn, "medium", "governance", "test.2", "Warning 2", "", "system",
    ).unwrap();
    ahlt::warnings::create_receipts(&conn, w2, &[user1, user2]).unwrap();

    assert_eq!(ahlt::warnings::queries::count_unread(&conn, user1), 2);
    assert_eq!(ahlt::warnings::queries::count_unread(&conn, user2), 1);
}

#[test]
fn test_mark_read_updates_receipt() {
    let (_dir, conn) = setup_test_db();
    seed_warning_types(&conn);
    let (user1, _user2) = seed_users(&conn);

    let w = ahlt::warnings::create_warning(
        &conn, "low", "system", "test.read", "Read test", "", "system",
    ).unwrap();
    ahlt::warnings::create_receipts(&conn, w, &[user1]).unwrap();

    assert_eq!(ahlt::warnings::queries::count_unread(&conn, user1), 1);

    // Mark as read
    let receipt_id = ahlt::warnings::queries::find_receipt_for_user(&conn, w, user1)
        .unwrap().unwrap();
    ahlt::warnings::update_receipt_status(&conn, receipt_id, "read", user1).unwrap();

    assert_eq!(ahlt::warnings::queries::count_unread(&conn, user1), 0);

    // Verify status property
    let status: String = conn.query_row(
        "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = 'status'",
        params![receipt_id], |row| row.get(0),
    ).unwrap();
    assert_eq!(status, "read");
}

#[test]
fn test_warning_deduplication() {
    let (_dir, conn) = setup_test_db();
    seed_warning_types(&conn);

    let source_action = "scheduled.test_dedup";
    let dedup_key = "test_dedup";

    // First check — should not exist
    assert!(!ahlt::warnings::warning_exists(&conn, source_action, dedup_key));

    // Create warning — dedup_key must appear in details for warning_exists LIKE check
    ahlt::warnings::create_warning(
        &conn, "info", "system", source_action, "First warning", dedup_key, "system",
    ).unwrap();

    // Now should exist
    assert!(ahlt::warnings::warning_exists(&conn, source_action, dedup_key));
}

#[test]
fn test_find_for_user_pagination() {
    let (_dir, conn) = setup_test_db();
    seed_warning_types(&conn);
    let (user1, _) = seed_users(&conn);

    // Create 5 warnings
    for i in 0..5 {
        let w = ahlt::warnings::create_warning(
            &conn, "info", "system", &format!("test.page.{}", i),
            &format!("Warning {}", i), "", "system",
        ).unwrap();
        ahlt::warnings::create_receipts(&conn, w, &[user1]).unwrap();
    }

    // Get page 1 with per_page=2
    let page = ahlt::warnings::queries::find_for_user(
        &conn, user1, 1, 2, None, None, false, false,
    ).unwrap();
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.total_count, 5);
    assert_eq!(page.total_pages, 3);

    // Get page 3 (last page, 1 item)
    let page3 = ahlt::warnings::queries::find_for_user(
        &conn, user1, 3, 2, None, None, false, false,
    ).unwrap();
    assert_eq!(page3.items.len(), 1);
}

#[test]
fn test_warning_detail() {
    let (_dir, conn) = setup_test_db();
    seed_warning_types(&conn);
    let (user1, _) = seed_users(&conn);

    let w = ahlt::warnings::create_warning(
        &conn, "critical", "security", "test.detail",
        "Critical security issue", "{\"ip\":\"1.2.3.4\"}", "system",
    ).unwrap();
    ahlt::warnings::create_receipts(&conn, w, &[user1]).unwrap();

    let detail = ahlt::warnings::queries::get_warning_detail(&conn, w)
        .unwrap().expect("Should find warning detail");

    assert_eq!(detail.severity, "critical");
    assert_eq!(detail.category, "security");
    assert_eq!(detail.message, "Critical security issue");
    assert_eq!(detail.source_action, "test.detail");
    assert!(detail.details.contains("1.2.3.4"));

    // Verify recipients
    let recipients = ahlt::warnings::queries::get_recipients(&conn, w).unwrap();
    assert_eq!(recipients.len(), 1);
    assert_eq!(recipients[0].username, "alice");
    assert_eq!(recipients[0].status, "unread");
}

#[test]
fn test_event_timeline() {
    let (_dir, conn) = setup_test_db();
    seed_warning_types(&conn);
    let (user1, _) = seed_users(&conn);

    let w = ahlt::warnings::create_warning(
        &conn, "low", "system", "test.timeline", "Timeline test", "", "system",
    ).unwrap();
    ahlt::warnings::create_receipts(&conn, w, &[user1]).unwrap();

    let receipt_id = ahlt::warnings::queries::find_receipt_for_user(&conn, w, user1)
        .unwrap().unwrap();

    // Receipt creation should have generated a "created" event
    let timeline = ahlt::warnings::queries::get_receipt_timeline(&conn, receipt_id).unwrap();
    assert!(!timeline.is_empty());
    assert_eq!(timeline[0].action, "created");

    // Update status and check new event
    ahlt::warnings::update_receipt_status(&conn, receipt_id, "read", user1).unwrap();
    let timeline = ahlt::warnings::queries::get_receipt_timeline(&conn, receipt_id).unwrap();
    assert!(timeline.len() >= 2);
}

#[test]
fn test_tor_vacancy_generator_creates_warning() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let pool = ahlt::db::init_pool(db_path.to_str().unwrap());
    let conn = pool.get().unwrap();
    conn.execute_batch(MIGRATIONS).unwrap();
    seed_warning_types(&conn);
    seed_tor_types(&conn);

    let conn_map = ahlt::handlers::warning_handlers::ws::new_connection_map();

    // Create an admin user with tor.manage_members permission
    let admin = insert_entity(&conn, "user", "admin", "Admin");
    let role = insert_entity(&conn, "role", "admin_role", "Admin Role");
    let perm = insert_entity(&conn, "permission", "tor.manage_members", "Manage ToR Members");
    // has_role: admin -> admin_role
    let has_role_rt: i64 = conn.query_row(
        "SELECT id FROM entities WHERE name = 'has_role'", [], |r| r.get(0),
    ).unwrap();
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
        params![has_role_rt, admin, role],
    ).unwrap();
    // has_permission: admin_role -> tor.manage_members
    let has_perm_rt: i64 = conn.query_row(
        "SELECT id FROM entities WHERE name = 'has_permission'", [], |r| r.get(0),
    ).unwrap();
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
        params![has_perm_rt, role, perm],
    ).unwrap();

    // Create a ToR with status=active
    let tor = insert_entity(&conn, "tor", "test_tor", "Test Committee");
    insert_prop(&conn, tor, "status", "active");

    // Create a mandatory position linked to the ToR (vacant — no fills_position)
    let pos = insert_entity(&conn, "tor_function", "chair", "Chair");
    insert_prop(&conn, pos, "membership_type", "mandatory");
    let belongs_to_rt: i64 = conn.query_row(
        "SELECT id FROM entities WHERE name = 'belongs_to_tor'", [], |r| r.get(0),
    ).unwrap();
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
        params![belongs_to_rt, pos, tor],
    ).unwrap();

    // Run the generator
    ahlt::warnings::generators::check_tor_vacancies(&conn, &conn_map, &pool);

    // Verify a warning was created
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entities e
         JOIN entity_properties sa ON sa.entity_id = e.id AND sa.key = 'source_action'
         WHERE e.entity_type = 'warning' AND sa.value = 'scheduled.tor_vacancy'",
        [], |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 1, "Expected one vacancy warning");

    // Verify the warning message references the position
    let message: String = conn.query_row(
        "SELECT ep.value FROM entities e
         JOIN entity_properties ep ON ep.entity_id = e.id AND ep.key = 'message'
         JOIN entity_properties sa ON sa.entity_id = e.id AND sa.key = 'source_action'
         WHERE e.entity_type = 'warning' AND sa.value = 'scheduled.tor_vacancy'",
        [], |r| r.get(0),
    ).unwrap();
    assert!(message.contains("Chair"), "Warning should mention the vacant position");
    assert!(message.contains("Test Committee"), "Warning should mention the ToR");

    // Verify receipt was created for admin
    let receipt_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entities WHERE entity_type = 'warning_receipt'",
        [], |r| r.get(0),
    ).unwrap();
    assert_eq!(receipt_count, 1, "Expected one receipt for the admin user");

    // Run again — dedup should prevent a second warning
    ahlt::warnings::generators::check_tor_vacancies(&conn, &conn_map, &pool);
    let count2: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entities e
         JOIN entity_properties sa ON sa.entity_id = e.id AND sa.key = 'source_action'
         WHERE e.entity_type = 'warning' AND sa.value = 'scheduled.tor_vacancy'",
        [], |r| r.get(0),
    ).unwrap();
    assert_eq!(count2, 1, "Dedup should prevent second warning");
}

#[test]
fn test_tor_vacancy_auto_resolves_when_filled() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let pool = ahlt::db::init_pool(db_path.to_str().unwrap());
    let conn = pool.get().unwrap();
    conn.execute_batch(MIGRATIONS).unwrap();
    seed_warning_types(&conn);
    seed_tor_types(&conn);

    let conn_map = ahlt::handlers::warning_handlers::ws::new_connection_map();

    // Minimal setup: admin with permission
    let admin = insert_entity(&conn, "user", "admin", "Admin");
    let role = insert_entity(&conn, "role", "admin_role", "Admin Role");
    let perm = insert_entity(&conn, "permission", "tor.manage_members", "Manage Members");
    let has_role_rt: i64 = conn.query_row(
        "SELECT id FROM entities WHERE name = 'has_role'", [], |r| r.get(0),
    ).unwrap();
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
        params![has_role_rt, admin, role],
    ).unwrap();
    let has_perm_rt: i64 = conn.query_row(
        "SELECT id FROM entities WHERE name = 'has_permission'", [], |r| r.get(0),
    ).unwrap();
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
        params![has_perm_rt, role, perm],
    ).unwrap();

    // Create ToR + vacant mandatory position
    let tor = insert_entity(&conn, "tor", "test_tor", "Test Committee");
    insert_prop(&conn, tor, "status", "active");
    let pos = insert_entity(&conn, "tor_function", "chair", "Chair");
    insert_prop(&conn, pos, "membership_type", "mandatory");
    let belongs_to_rt: i64 = conn.query_row(
        "SELECT id FROM entities WHERE name = 'belongs_to_tor'", [], |r| r.get(0),
    ).unwrap();
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
        params![belongs_to_rt, pos, tor],
    ).unwrap();

    // Run generator — should create warning
    ahlt::warnings::generators::check_tor_vacancies(&conn, &conn_map, &pool);
    let status: String = conn.query_row(
        "SELECT ep.value FROM entities e
         JOIN entity_properties ep ON ep.entity_id = e.id AND ep.key = 'status'
         JOIN entity_properties sa ON sa.entity_id = e.id AND sa.key = 'source_action'
         WHERE e.entity_type = 'warning' AND sa.value = 'scheduled.tor_vacancy'",
        [], |r| r.get(0),
    ).unwrap();
    assert_eq!(status, "active");

    // Fill the position: create fills_position relation
    let fills_rt: i64 = conn.query_row(
        "SELECT id FROM entities WHERE name = 'fills_position'", [], |r| r.get(0),
    ).unwrap();
    let filler = insert_entity(&conn, "user", "bob", "Bob");
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
        params![fills_rt, filler, pos],
    ).unwrap();

    // Run generator again — should auto-resolve the warning
    ahlt::warnings::generators::check_tor_vacancies(&conn, &conn_map, &pool);
    let status2: String = conn.query_row(
        "SELECT ep.value FROM entities e
         JOIN entity_properties ep ON ep.entity_id = e.id AND ep.key = 'status'
         JOIN entity_properties sa ON sa.entity_id = e.id AND sa.key = 'source_action'
         WHERE e.entity_type = 'warning' AND sa.value = 'scheduled.tor_vacancy'",
        [], |r| r.get(0),
    ).unwrap();
    assert_eq!(status2, "resolved", "Warning should auto-resolve when vacancy is filled");
}
