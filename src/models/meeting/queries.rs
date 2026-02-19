use rusqlite::{params, Connection};

use super::types::*;

/// Create a new meeting entity linked to a ToR.
///
/// Inserts an entity with `entity_type='meeting'`, sets `status` to `"projected"`,
/// stores `meeting_date`, and creates a `belongs_to_tor` relation to the given ToR.
/// Empty `location`/`notes` are skipped (not stored as properties).
pub fn create(
    conn: &Connection,
    tor_id: i64,
    meeting_date: &str,
    tor_name: &str,
    location: &str,
    notes: &str,
) -> rusqlite::Result<i64> {
    let name = format!(
        "{}-{}",
        tor_name.to_lowercase().replace(' ', "-"),
        meeting_date
    );
    let label = format!("{} \u{2014} {}", tor_name, meeting_date);

    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('meeting', ?1, ?2)",
        params![name, label],
    )?;
    let meeting_id = conn.last_insert_rowid();

    // Always store status and meeting_date; skip empty location/notes.
    let props: Vec<(&str, &str)> = vec![
        ("meeting_date", meeting_date),
        ("status", "projected"),
        ("location", location),
        ("notes", notes),
    ];
    for (key, value) in props {
        if !value.is_empty() || key == "status" || key == "meeting_date" {
            conn.execute(
                "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
                params![meeting_id, key, value],
            )?;
        }
    }

    // Create belongs_to_tor relation (meeting -> tor).
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor'), ?1, ?2)",
        params![meeting_id, tor_id],
    )?;

    Ok(meeting_id)
}

/// Base SELECT columns for meeting list queries (with inline subqueries for agenda_count and has_minutes).
const MEETING_LIST_SELECT: &str = "\
SELECT e.id, e.name, e.label, \
       COALESCE(p_date.value, '') AS meeting_date, \
       COALESCE(p_status.value, 'projected') AS status, \
       tor.id AS tor_id, tor.name AS tor_name, tor.label AS tor_label, \
       (SELECT COUNT(*) FROM relations r_agenda \
        WHERE r_agenda.target_id = e.id \
          AND r_agenda.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'scheduled_for_meeting') \
       ) AS agenda_count, \
       EXISTS(SELECT 1 FROM relations r_min \
        WHERE r_min.source_id = e.id \
          AND r_min.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'minutes_of') \
       ) AS has_minutes \
FROM entities e \
LEFT JOIN entity_properties p_date ON e.id = p_date.entity_id AND p_date.key = 'meeting_date' \
LEFT JOIN entity_properties p_status ON e.id = p_status.entity_id AND p_status.key = 'status' \
JOIN relations r_tor ON e.id = r_tor.source_id \
    AND r_tor.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
JOIN entities tor ON r_tor.target_id = tor.id \
WHERE e.entity_type = 'meeting'";

fn map_meeting_list_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MeetingListItem> {
    Ok(MeetingListItem {
        id: row.get("id")?,
        name: row.get("name")?,
        label: row.get("label")?,
        meeting_date: row.get("meeting_date")?,
        status: row.get("status")?,
        tor_id: row.get("tor_id")?,
        tor_name: row.get("tor_name")?,
        tor_label: row.get("tor_label")?,
        agenda_count: row.get("agenda_count")?,
        has_minutes: row.get("has_minutes")?,
    })
}

/// Find all meetings belonging to a specific ToR, ordered by date DESCENDING (newest first).
pub fn find_by_tor(conn: &Connection, tor_id: i64) -> rusqlite::Result<Vec<MeetingListItem>> {
    let sql = format!(
        "{} AND tor.id = ?1 ORDER BY p_date.value DESC",
        MEETING_LIST_SELECT
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![tor_id], map_meeting_list_row)?;
    rows.collect()
}

/// Find all upcoming meetings across all ToRs from a date cutoff, ordered by date ASCENDING (soonest first).
pub fn find_upcoming_all(
    conn: &Connection,
    from_date: &str,
) -> rusqlite::Result<Vec<MeetingListItem>> {
    let sql = format!(
        "{} AND p_date.value >= ?1 ORDER BY p_date.value ASC",
        MEETING_LIST_SELECT
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![from_date], map_meeting_list_row)?;
    rows.collect()
}

/// Find a meeting by its entity ID. Returns full detail including ToR info.
pub fn find_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<MeetingDetail>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label, \
                COALESCE(p_date.value, '') AS meeting_date, \
                COALESCE(p_status.value, 'projected') AS status, \
                COALESCE(p_loc.value, '') AS location, \
                COALESCE(p_notes.value, '') AS notes, \
                COALESCE(tor.id, 0) AS tor_id, \
                COALESCE(tor.name, '') AS tor_name, \
                COALESCE(tor.label, '') AS tor_label \
         FROM entities e \
         LEFT JOIN entity_properties p_date ON e.id = p_date.entity_id AND p_date.key = 'meeting_date' \
         LEFT JOIN entity_properties p_status ON e.id = p_status.entity_id AND p_status.key = 'status' \
         LEFT JOIN entity_properties p_loc ON e.id = p_loc.entity_id AND p_loc.key = 'location' \
         LEFT JOIN entity_properties p_notes ON e.id = p_notes.entity_id AND p_notes.key = 'notes' \
         LEFT JOIN relations r_tor ON e.id = r_tor.source_id \
             AND r_tor.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
         LEFT JOIN entities tor ON r_tor.target_id = tor.id \
         WHERE e.id = ?1 AND e.entity_type = 'meeting'",
    )?;

    let mut rows = stmt.query_map(params![id], |row| {
        Ok(MeetingDetail {
            id: row.get("id")?,
            name: row.get("name")?,
            label: row.get("label")?,
            meeting_date: row.get("meeting_date")?,
            status: row.get("status")?,
            location: row.get("location")?,
            notes: row.get("notes")?,
            tor_id: row.get("tor_id")?,
            tor_name: row.get("tor_name")?,
            tor_label: row.get("tor_label")?,
        })
    })?;

    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}
