use rusqlite::{params, Connection};

use super::types::*;

/// Create a new meeting entity linked to a ToR.
///
/// Inserts an entity with `entity_type='meeting'`, sets `status` to `"projected"`,
/// stores `meeting_date`, and creates a `belongs_to_tor` relation to the given ToR.
/// Empty optional fields are skipped (not stored as properties).
#[allow(clippy::too_many_arguments)]
pub fn create(
    conn: &Connection,
    tor_id: i64,
    meeting_date: &str,
    tor_name: &str,
    location: &str,
    notes: &str,
    meeting_number: &str,
    classification: &str,
    vtc_details: &str,
    chair_user_id: &str,
    secretary_user_id: &str,
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

    // Always store status and meeting_date; skip empty optional fields.
    let props: Vec<(&str, &str)> = vec![
        ("meeting_date", meeting_date),
        ("status", "projected"),
        ("location", location),
        ("notes", notes),
        ("meeting_number", meeting_number),
        ("classification", classification),
        ("vtc_details", vtc_details),
        ("chair_user_id", chair_user_id),
        ("secretary_user_id", secretary_user_id),
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

/// Find all past meetings across all ToRs before a date cutoff, ordered by date DESCENDING (most recent first).
pub fn find_past_all(
    conn: &Connection,
    before_date: &str,
) -> rusqlite::Result<Vec<MeetingListItem>> {
    let sql = format!(
        "{} AND p_date.value < ?1 ORDER BY p_date.value DESC",
        MEETING_LIST_SELECT
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![before_date], map_meeting_list_row)?;
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
                COALESCE(tor.label, '') AS tor_label, \
                COALESCE(p_meetnum.value, '') AS meeting_number, \
                COALESCE(p_class.value, '') AS classification, \
                COALESCE(p_vtc.value, '') AS vtc_details, \
                COALESCE(p_chair.value, '') AS chair_user_id, \
                COALESCE(p_secretary.value, '') AS secretary_user_id, \
                COALESCE(p_roll.value, '[]') AS roll_call_data \
         FROM entities e \
         LEFT JOIN entity_properties p_date ON e.id = p_date.entity_id AND p_date.key = 'meeting_date' \
         LEFT JOIN entity_properties p_status ON e.id = p_status.entity_id AND p_status.key = 'status' \
         LEFT JOIN entity_properties p_loc ON e.id = p_loc.entity_id AND p_loc.key = 'location' \
         LEFT JOIN entity_properties p_notes ON e.id = p_notes.entity_id AND p_notes.key = 'notes' \
         LEFT JOIN entity_properties p_meetnum ON e.id = p_meetnum.entity_id AND p_meetnum.key = 'meeting_number' \
         LEFT JOIN entity_properties p_class ON e.id = p_class.entity_id AND p_class.key = 'classification' \
         LEFT JOIN entity_properties p_vtc ON e.id = p_vtc.entity_id AND p_vtc.key = 'vtc_details' \
         LEFT JOIN entity_properties p_chair ON e.id = p_chair.entity_id AND p_chair.key = 'chair_user_id' \
         LEFT JOIN entity_properties p_secretary ON e.id = p_secretary.entity_id AND p_secretary.key = 'secretary_user_id' \
         LEFT JOIN entity_properties p_roll ON e.id = p_roll.entity_id AND p_roll.key = 'roll_call_data' \
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
            meeting_number: row.get("meeting_number")?,
            classification: row.get("classification")?,
            vtc_details: row.get("vtc_details")?,
            chair_user_id: row.get("chair_user_id")?,
            secretary_user_id: row.get("secretary_user_id")?,
            roll_call_data: row.get("roll_call_data")?,
        })
    })?;

    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Agenda point associated with a meeting.
#[derive(Debug, Clone)]
pub struct MeetingAgendaPoint {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub item_type: String,
    pub status: String,
}

/// Assign an agenda point to a meeting (idempotent — ignores duplicates).
///
/// Creates a `scheduled_for_meeting` relation: source = agenda_point, target = meeting.
pub fn assign_agenda(
    conn: &Connection,
    meeting_id: i64,
    agenda_point_id: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id) \
         VALUES (\
             (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'scheduled_for_meeting'), \
             ?1, ?2\
         )",
        params![agenda_point_id, meeting_id],
    )?;
    Ok(())
}

/// Remove an agenda point from a meeting.
///
/// Deletes the `scheduled_for_meeting` relation between the agenda point and meeting.
pub fn remove_agenda(
    conn: &Connection,
    meeting_id: i64,
    agenda_point_id: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM relations \
         WHERE source_id = ?1 AND target_id = ?2 \
           AND relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'scheduled_for_meeting')",
        params![agenda_point_id, meeting_id],
    )?;
    Ok(())
}

/// Find all agenda points assigned to a meeting via `scheduled_for_meeting`.
pub fn find_agenda_points(
    conn: &Connection,
    meeting_id: i64,
) -> rusqlite::Result<Vec<MeetingAgendaPoint>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label, \
                COALESCE(p_type.value, '') AS item_type, \
                COALESCE(p_status.value, '') AS status \
         FROM entities e \
         JOIN relations r ON r.source_id = e.id \
             AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'scheduled_for_meeting') \
             AND r.target_id = ?1 \
         LEFT JOIN entity_properties p_type ON e.id = p_type.entity_id AND p_type.key = 'item_type' \
         LEFT JOIN entity_properties p_status ON e.id = p_status.entity_id AND p_status.key = 'status' \
         WHERE e.entity_type = 'agenda_point' \
         ORDER BY e.label ASC",
    )?;
    let rows = stmt.query_map(params![meeting_id], |row| {
        Ok(MeetingAgendaPoint {
            id: row.get("id")?,
            name: row.get("name")?,
            label: row.get("label")?,
            item_type: row.get("item_type")?,
            status: row.get("status")?,
        })
    })?;
    rows.collect()
}

/// Find agenda points belonging to a ToR that are NOT assigned to ANY meeting.
pub fn find_unassigned_agenda_points(
    conn: &Connection,
    tor_id: i64,
) -> rusqlite::Result<Vec<MeetingAgendaPoint>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label, \
                COALESCE(p_type.value, '') AS item_type, \
                COALESCE(p_status.value, '') AS status \
         FROM entities e \
         JOIN relations r_tor ON r_tor.source_id = e.id \
             AND r_tor.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
             AND r_tor.target_id = ?1 \
         LEFT JOIN entity_properties p_type ON e.id = p_type.entity_id AND p_type.key = 'item_type' \
         LEFT JOIN entity_properties p_status ON e.id = p_status.entity_id AND p_status.key = 'status' \
         WHERE e.entity_type = 'agenda_point' \
           AND NOT EXISTS ( \
               SELECT 1 FROM relations r_sched \
               WHERE r_sched.source_id = e.id \
                 AND r_sched.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'scheduled_for_meeting') \
           ) \
         ORDER BY e.label ASC",
    )?;
    let rows = stmt.query_map(params![tor_id], |row| {
        Ok(MeetingAgendaPoint {
            id: row.get("id")?,
            name: row.get("name")?,
            label: row.get("label")?,
            item_type: row.get("item_type")?,
            status: row.get("status")?,
        })
    })?;
    rows.collect()
}

/// Update a meeting's status property (upsert).
pub fn update_status(
    conn: &Connection,
    meeting_id: i64,
    status: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'status', ?2) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value",
        params![meeting_id, status],
    )?;
    Ok(())
}

/// Upsert roll_call_data JSON string for a meeting.
pub fn update_roll_call(conn: &Connection, meeting_id: i64, json: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'roll_call_data', ?2) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value",
        params![meeting_id, json],
    )?;
    Ok(())
}
