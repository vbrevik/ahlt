//! Meeting minutes tests — covers minutes scaffold generation, content updates, and status management.
//!
//! Tests the minutes model layer operations:
//! - Minutes scaffold generation with auto-generated sections
//! - Minutes retrieval by ID and by meeting
//! - Section enumeration and content
//! - Content updates and status transitions
//! - Auto-generated attendance and protocol sections

mod common;

use ahlt::models::minutes::*;
use ahlt::models::tor;
use common::*;

const TEST_MEETING_NAME: &str = "Board Meeting";
const TEST_TOR_NAME: &str = "board_of_directors";
const TEST_TOR_LABEL: &str = "Board of Directors";

#[test]
fn test_generate_scaffold_success() {
    let (_dir, conn) = setup_test_db();

    // Create a ToR first
    let tor_id = tor::create(&conn, TEST_TOR_NAME, TEST_TOR_LABEL, "", "", "", "", "", "", "", "", "")
        .expect("Failed to create ToR");

    // Create a meeting entity (minutes_of relation requires source_id to be a meeting)
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('meeting', 'test_meeting', 'Test Meeting')",
        [],
    ).expect("Failed to create meeting");
    let meeting_id = conn.last_insert_rowid();

    // Generate scaffold
    let minutes_id = generate_scaffold(&conn, meeting_id, tor_id, TEST_MEETING_NAME)
        .expect("Failed to generate scaffold");

    assert!(minutes_id > 0);

    // Verify minutes was created
    let minutes = find_by_id(&conn, minutes_id)
        .expect("Query failed")
        .expect("Minutes not found");

    assert_eq!(minutes.status, "draft");
    assert_eq!(minutes.meeting_id, meeting_id);
}

#[test]
fn test_find_minutes_by_meeting() {
    let (_dir, conn) = setup_test_db();

    let tor_id = tor::create(&conn, TEST_TOR_NAME, TEST_TOR_LABEL, "", "", "", "", "", "", "", "", "")
        .expect("Failed to create ToR");

    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('meeting', 'test_meeting', 'Test Meeting')",
        [],
    ).expect("Failed to create meeting");
    let meeting_id = conn.last_insert_rowid();

    let minutes_id = generate_scaffold(&conn, meeting_id, tor_id, TEST_MEETING_NAME)
        .expect("Failed to generate scaffold");

    let found = find_by_meeting(&conn, meeting_id)
        .expect("Query failed")
        .expect("Minutes not found");

    assert_eq!(found.id, minutes_id);
    assert_eq!(found.meeting_id, meeting_id);
}

#[test]
fn test_find_minutes_by_id_success() {
    let (_dir, conn) = setup_test_db();

    let tor_id = tor::create(&conn, TEST_TOR_NAME, TEST_TOR_LABEL, "", "", "", "", "", "", "", "", "")
        .expect("Failed to create ToR");

    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('meeting', 'test_meeting', 'Test Meeting')",
        [],
    ).expect("Failed to create meeting");
    let meeting_id = conn.last_insert_rowid();

    let minutes_id = generate_scaffold(&conn, meeting_id, tor_id, TEST_MEETING_NAME)
        .expect("Failed to generate scaffold");

    let found = find_by_id(&conn, minutes_id)
        .expect("Query failed")
        .expect("Minutes not found");

    assert_eq!(found.id, minutes_id);
    assert_eq!(found.label, format!("Minutes — {}", TEST_MEETING_NAME));
}

#[test]
fn test_find_minutes_by_id_not_found() {
    let (_dir, conn) = setup_test_db();

    let result = find_by_id(&conn, 9999)
        .expect("Query failed");

    assert!(result.is_none());
}

#[test]
fn test_find_sections_of_minutes() {
    let (_dir, conn) = setup_test_db();

    let tor_id = tor::create(&conn, TEST_TOR_NAME, TEST_TOR_LABEL, "", "", "", "", "", "", "", "", "")
        .expect("Failed to create ToR");

    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('meeting', 'test_meeting', 'Test Meeting')",
        [],
    ).expect("Failed to create meeting");
    let meeting_id = conn.last_insert_rowid();

    let minutes_id = generate_scaffold(&conn, meeting_id, tor_id, TEST_MEETING_NAME)
        .expect("Failed to generate scaffold");

    let sections = find_sections(&conn, minutes_id)
        .expect("Failed to find sections");

    // Should have 5 auto-generated sections
    assert_eq!(sections.len(), 5);
    assert_eq!(sections[0].section_type, "attendance");
    assert_eq!(sections[1].section_type, "protocol");
    assert_eq!(sections[2].section_type, "agenda_items");
    assert_eq!(sections[3].section_type, "decisions");
    assert_eq!(sections[4].section_type, "action_items");
}

#[test]
fn test_update_section_content() {
    let (_dir, conn) = setup_test_db();

    let tor_id = tor::create(&conn, TEST_TOR_NAME, TEST_TOR_LABEL, "", "", "", "", "", "", "", "", "")
        .expect("Failed to create ToR");

    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('meeting', 'test_meeting', 'Test Meeting')",
        [],
    ).expect("Failed to create meeting");
    let meeting_id = conn.last_insert_rowid();

    let minutes_id = generate_scaffold(&conn, meeting_id, tor_id, TEST_MEETING_NAME)
        .expect("Failed to generate scaffold");

    let sections = find_sections(&conn, minutes_id)
        .expect("Failed to find sections");
    let agenda_section_id = sections.iter()
        .find(|s| s.section_type == "agenda_items")
        .expect("Agenda section not found")
        .id;

    let new_content = "1. Financial Review\n2. Strategic Planning\n3. Q&A";
    let _ = update_section_content(&conn, agenda_section_id, new_content)
        .expect("Failed to update section content");

    let updated_sections = find_sections(&conn, minutes_id)
        .expect("Failed to find sections");
    let updated = updated_sections.iter()
        .find(|s| s.id == agenda_section_id)
        .expect("Section not found");

    assert_eq!(updated.content, new_content);
}

#[test]
fn test_update_minutes_status() {
    let (_dir, conn) = setup_test_db();

    let tor_id = tor::create(&conn, TEST_TOR_NAME, TEST_TOR_LABEL, "", "", "", "", "", "", "", "", "")
        .expect("Failed to create ToR");

    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('meeting', 'test_meeting', 'Test Meeting')",
        [],
    ).expect("Failed to create meeting");
    let meeting_id = conn.last_insert_rowid();

    let minutes_id = generate_scaffold(&conn, meeting_id, tor_id, TEST_MEETING_NAME)
        .expect("Failed to generate scaffold");

    let _ = update_status(&conn, minutes_id, "approved")
        .expect("Failed to update status");

    let updated = find_by_id(&conn, minutes_id)
        .expect("Query failed")
        .expect("Minutes not found");

    assert_eq!(updated.status, "approved");
}

#[test]
fn test_auto_generated_attendance_section() {
    let (_dir, conn) = setup_test_db();

    let tor_id = tor::create(&conn, TEST_TOR_NAME, TEST_TOR_LABEL, "", "", "", "", "", "", "", "", "")
        .expect("Failed to create ToR");

    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('meeting', 'test_meeting', 'Test Meeting')",
        [],
    ).expect("Failed to create meeting");
    let meeting_id = conn.last_insert_rowid();

    let minutes_id = generate_scaffold(&conn, meeting_id, tor_id, TEST_MEETING_NAME)
        .expect("Failed to generate scaffold");

    let sections = find_sections(&conn, minutes_id)
        .expect("Failed to find sections");
    let attendance = sections.iter()
        .find(|s| s.section_type == "attendance")
        .expect("Attendance section not found");

    // Auto-generated attendance should have "## Attendance" header
    assert!(attendance.content.contains("Attendance"));
    assert!(attendance.is_auto_generated);
}
