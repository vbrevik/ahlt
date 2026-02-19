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
    )
    .expect("Failed to insert entity");
    conn.last_insert_rowid()
}

fn insert_prop(conn: &rusqlite::Connection, entity_id: i64, key: &str, value: &str) {
    conn.execute(
        "INSERT OR REPLACE INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
        params![entity_id, key, value],
    )
    .expect("Failed to insert property");
}

#[allow(dead_code)]
fn insert_relation(
    conn: &rusqlite::Connection,
    relation_type_id: i64,
    source_id: i64,
    target_id: i64,
) -> i64 {
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
        params![relation_type_id, source_id, target_id],
    )
    .expect("Failed to insert relation");
    conn.last_insert_rowid()
}

/// Sets up a ToR entity and the relation types needed for meetings.
/// Returns (tor_id, belongs_to_tor_rt_id, scheduled_for_meeting_rt_id).
fn setup_tor_with_relation_types(conn: &rusqlite::Connection) -> (i64, i64, i64) {
    let belongs_to_tor_rt =
        insert_entity(conn, "relation_type", "belongs_to_tor", "Belongs to ToR");
    let scheduled_for_meeting_rt = insert_entity(
        conn,
        "relation_type",
        "scheduled_for_meeting",
        "Scheduled For Meeting",
    );
    let tor_id = insert_entity(conn, "tor", "test-tor", "Test ToR");
    insert_prop(conn, tor_id, "meeting_cadence", "weekly");
    insert_prop(conn, tor_id, "status", "active");
    (tor_id, belongs_to_tor_rt, scheduled_for_meeting_rt)
}

// --- Tests ---

#[test]
fn test_create_meeting() {
    let (_dir, conn) = setup_test_db();
    let (tor_id, belongs_to_tor_rt, _) = setup_tor_with_relation_types(&conn);

    let meeting_id = ahlt::models::meeting::create(&conn, tor_id, "2026-03-01", "Test ToR", "", "")
        .expect("Failed to create meeting");

    assert!(meeting_id > 0);

    // Verify entity type
    let entity_type: String = conn
        .query_row(
            "SELECT entity_type FROM entities WHERE id = ?1",
            [meeting_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(entity_type, "meeting");

    // Verify status property
    let status: String = conn
        .query_row(
            "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = 'status'",
            [meeting_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(status, "projected");

    // Verify meeting_date property
    let date: String = conn
        .query_row(
            "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = 'meeting_date'",
            [meeting_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(date, "2026-03-01");

    // Verify belongs_to_tor relation
    let rel_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM relations WHERE source_id = ?1 AND target_id = ?2 AND relation_type_id = ?3",
            params![meeting_id, tor_id, belongs_to_tor_rt],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(rel_count, 1);
}

#[test]
fn test_find_meeting_by_id() {
    let (_dir, conn) = setup_test_db();
    let (tor_id, _, _) = setup_tor_with_relation_types(&conn);

    let meeting_id =
        ahlt::models::meeting::create(&conn, tor_id, "2026-03-15", "Test ToR", "Room A", "Discussion notes")
            .expect("Failed to create meeting");

    let detail = ahlt::models::meeting::find_by_id(&conn, meeting_id)
        .expect("Query failed")
        .expect("Meeting not found");

    assert_eq!(detail.id, meeting_id);
    assert_eq!(detail.meeting_date, "2026-03-15");
    assert_eq!(detail.status, "projected");
    assert_eq!(detail.location, "Room A");
    assert_eq!(detail.notes, "Discussion notes");
    assert_eq!(detail.tor_id, tor_id);
}

#[test]
fn test_find_meeting_by_id_not_found() {
    let (_dir, conn) = setup_test_db();

    let result = ahlt::models::meeting::find_by_id(&conn, 99999).expect("Query failed");
    assert!(result.is_none());
}

#[test]
fn test_find_meetings_by_tor() {
    let (_dir, conn) = setup_test_db();
    let (tor_id, _, _) = setup_tor_with_relation_types(&conn);
    ahlt::models::meeting::create(&conn, tor_id, "2026-04-01", "Test ToR", "", "").unwrap();
    ahlt::models::meeting::create(&conn, tor_id, "2026-04-08", "Test ToR", "", "").unwrap();
    ahlt::models::meeting::create(&conn, tor_id, "2025-01-01", "Test ToR", "", "").unwrap();
    let meetings = ahlt::models::meeting::find_by_tor(&conn, tor_id).expect("Query failed");
    assert_eq!(meetings.len(), 3);
    assert_eq!(meetings[0].meeting_date, "2026-04-08");
    assert_eq!(meetings[1].meeting_date, "2026-04-01");
    assert_eq!(meetings[2].meeting_date, "2025-01-01");
}

#[test]
fn test_find_upcoming_all_cross_tor() {
    let (_dir, conn) = setup_test_db();
    let (tor_id1, _, _) = setup_tor_with_relation_types(&conn);
    let tor_id2 = insert_entity(&conn, "tor", "test-tor-2", "Test ToR 2");
    insert_prop(&conn, tor_id2, "status", "active");
    ahlt::models::meeting::create(&conn, tor_id1, "2026-04-01", "Test ToR", "", "").unwrap();
    ahlt::models::meeting::create(&conn, tor_id2, "2026-04-02", "Test ToR 2", "", "").unwrap();
    ahlt::models::meeting::create(&conn, tor_id1, "2025-01-01", "Test ToR", "", "").unwrap();
    let upcoming = ahlt::models::meeting::find_upcoming_all(&conn, "2026-03-01").expect("Query failed");
    assert_eq!(upcoming.len(), 2);
    assert_eq!(upcoming[0].meeting_date, "2026-04-01");
    assert_eq!(upcoming[1].meeting_date, "2026-04-02");
}
