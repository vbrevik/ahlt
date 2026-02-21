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

/// Helper to create a meeting entity and return its id.
async fn create_test_meeting(pool: &sqlx::PgPool) -> i64 {
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO entities (entity_type, name, label) VALUES ('meeting', 'test_meeting', 'Test Meeting') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .expect("Failed to create meeting");
    row.0
}

#[tokio::test]
async fn test_generate_scaffold_success() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Create a ToR first
    let tor_id = tor::create(pool, TEST_TOR_NAME, TEST_TOR_LABEL, &[])
        .await
        .expect("Failed to create ToR");

    let meeting_id = create_test_meeting(pool).await;

    // Generate scaffold
    let minutes_id = generate_scaffold(pool, meeting_id, tor_id, TEST_MEETING_NAME)
        .await
        .expect("Failed to generate scaffold");

    assert!(minutes_id > 0);

    // Verify minutes was created
    let minutes = find_by_id(pool, minutes_id)
        .await
        .expect("Query failed")
        .expect("Minutes not found");

    assert_eq!(minutes.status, "draft");
    assert_eq!(minutes.meeting_id, meeting_id);
}

#[tokio::test]
async fn test_find_minutes_by_meeting() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let tor_id = tor::create(pool, TEST_TOR_NAME, TEST_TOR_LABEL, &[])
        .await
        .expect("Failed to create ToR");

    let meeting_id = create_test_meeting(pool).await;

    let minutes_id = generate_scaffold(pool, meeting_id, tor_id, TEST_MEETING_NAME)
        .await
        .expect("Failed to generate scaffold");

    let found = find_by_meeting(pool, meeting_id)
        .await
        .expect("Query failed")
        .expect("Minutes not found");

    assert_eq!(found.id, minutes_id);
    assert_eq!(found.meeting_id, meeting_id);
}

#[tokio::test]
async fn test_find_minutes_by_id_success() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let tor_id = tor::create(pool, TEST_TOR_NAME, TEST_TOR_LABEL, &[])
        .await
        .expect("Failed to create ToR");

    let meeting_id = create_test_meeting(pool).await;

    let minutes_id = generate_scaffold(pool, meeting_id, tor_id, TEST_MEETING_NAME)
        .await
        .expect("Failed to generate scaffold");

    let found = find_by_id(pool, minutes_id)
        .await
        .expect("Query failed")
        .expect("Minutes not found");

    assert_eq!(found.id, minutes_id);
    assert_eq!(found.label, format!("Minutes — {}", TEST_MEETING_NAME));
}

#[tokio::test]
async fn test_find_minutes_by_id_not_found() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let result = find_by_id(pool, 9999)
        .await
        .expect("Query failed");

    assert!(result.is_none());
}

#[tokio::test]
async fn test_find_sections_of_minutes() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let tor_id = tor::create(pool, TEST_TOR_NAME, TEST_TOR_LABEL, &[])
        .await
        .expect("Failed to create ToR");

    let meeting_id = create_test_meeting(pool).await;

    let minutes_id = generate_scaffold(pool, meeting_id, tor_id, TEST_MEETING_NAME)
        .await
        .expect("Failed to generate scaffold");

    let sections = find_sections(pool, minutes_id)
        .await
        .expect("Failed to find sections");

    // Should have 5 auto-generated sections
    assert_eq!(sections.len(), 5);
    assert_eq!(sections[0].section_type, "attendance");
    assert_eq!(sections[1].section_type, "protocol");
    assert_eq!(sections[2].section_type, "agenda_items");
    assert_eq!(sections[3].section_type, "decisions");
    assert_eq!(sections[4].section_type, "action_items");
}

#[tokio::test]
async fn test_update_section_content() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let tor_id = tor::create(pool, TEST_TOR_NAME, TEST_TOR_LABEL, &[])
        .await
        .expect("Failed to create ToR");

    let meeting_id = create_test_meeting(pool).await;

    let minutes_id = generate_scaffold(pool, meeting_id, tor_id, TEST_MEETING_NAME)
        .await
        .expect("Failed to generate scaffold");

    let sections = find_sections(pool, minutes_id)
        .await
        .expect("Failed to find sections");
    let agenda_section_id = sections.iter()
        .find(|s| s.section_type == "agenda_items")
        .expect("Agenda section not found")
        .id;

    let new_content = "1. Financial Review\n2. Strategic Planning\n3. Q&A";
    let _ = update_section_content(pool, agenda_section_id, new_content)
        .await
        .expect("Failed to update section content");

    let updated_sections = find_sections(pool, minutes_id)
        .await
        .expect("Failed to find sections");
    let updated = updated_sections.iter()
        .find(|s| s.id == agenda_section_id)
        .expect("Section not found");

    assert_eq!(updated.content, new_content);
}

#[tokio::test]
async fn test_update_minutes_status() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let tor_id = tor::create(pool, TEST_TOR_NAME, TEST_TOR_LABEL, &[])
        .await
        .expect("Failed to create ToR");

    let meeting_id = create_test_meeting(pool).await;

    let minutes_id = generate_scaffold(pool, meeting_id, tor_id, TEST_MEETING_NAME)
        .await
        .expect("Failed to generate scaffold");

    let _ = update_status(pool, minutes_id, "approved")
        .await
        .expect("Failed to update status");

    let updated = find_by_id(pool, minutes_id)
        .await
        .expect("Query failed")
        .expect("Minutes not found");

    assert_eq!(updated.status, "approved");
}

#[tokio::test]
async fn test_auto_generated_attendance_section() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let tor_id = tor::create(pool, TEST_TOR_NAME, TEST_TOR_LABEL, &[])
        .await
        .expect("Failed to create ToR");

    let meeting_id = create_test_meeting(pool).await;

    let minutes_id = generate_scaffold(pool, meeting_id, tor_id, TEST_MEETING_NAME)
        .await
        .expect("Failed to generate scaffold");

    let sections = find_sections(pool, minutes_id)
        .await
        .expect("Failed to find sections");
    let attendance = sections.iter()
        .find(|s| s.section_type == "attendance")
        .expect("Attendance section not found");

    // Auto-generated attendance should have "## Attendance" header
    assert!(attendance.content.contains("Attendance"));
    assert!(attendance.is_auto_generated);
}
