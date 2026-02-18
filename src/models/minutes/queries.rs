use rusqlite::{Connection, params};
use super::types::*;

/// Find minutes for a specific meeting.
pub fn find_by_meeting(conn: &Connection, meeting_id: i64) -> rusqlite::Result<Option<Minutes>> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.name, m.label, \
                COALESCE(p_status.value, 'draft') AS status, \
                COALESCE(p_date.value, '') AS generated_date, \
                r.source_id AS meeting_id_check, \
                COALESCE(mtg.name, '') AS meeting_name \
         FROM entities m \
         JOIN relations r ON m.id = r.target_id \
         JOIN entities mtg ON r.source_id = mtg.id \
         LEFT JOIN entity_properties p_status ON m.id = p_status.entity_id AND p_status.key = 'status' \
         LEFT JOIN entity_properties p_date ON m.id = p_date.entity_id AND p_date.key = 'generated_date' \
         WHERE r.source_id = ?1 \
           AND r.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'minutes_of') \
           AND m.entity_type = 'minutes'",
    )?;

    let mut rows = stmt.query_map(params![meeting_id], |row| {
        Ok(Minutes {
            id: row.get("id")?,
            name: row.get("name")?,
            label: row.get("label")?,
            status: row.get("status")?,
            generated_date: row.get("generated_date")?,
            meeting_id: row.get("meeting_id_check")?,
            meeting_name: row.get("meeting_name")?,
        })
    })?;

    match rows.next() {
        Some(Ok(m)) => Ok(Some(m)),
        Some(Err(e)) => Err(e),
        None => Ok(None),
    }
}

/// Find minutes by ID.
pub fn find_by_id(conn: &Connection, minutes_id: i64) -> rusqlite::Result<Option<Minutes>> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.name, m.label, \
                COALESCE(p_status.value, 'draft') AS status, \
                COALESCE(p_date.value, '') AS generated_date, \
                r.source_id AS meeting_id, \
                COALESCE(mtg.name, '') AS meeting_name \
         FROM entities m \
         JOIN relations r ON m.id = r.target_id \
         JOIN entities mtg ON r.source_id = mtg.id \
         LEFT JOIN entity_properties p_status ON m.id = p_status.entity_id AND p_status.key = 'status' \
         LEFT JOIN entity_properties p_date ON m.id = p_date.entity_id AND p_date.key = 'generated_date' \
         WHERE m.id = ?1 \
           AND r.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'minutes_of') \
           AND m.entity_type = 'minutes'",
    )?;

    let mut rows = stmt.query_map(params![minutes_id], |row| {
        Ok(Minutes {
            id: row.get("id")?,
            name: row.get("name")?,
            label: row.get("label")?,
            status: row.get("status")?,
            generated_date: row.get("generated_date")?,
            meeting_id: row.get("meeting_id")?,
            meeting_name: row.get("meeting_name")?,
        })
    })?;

    match rows.next() {
        Some(Ok(m)) => Ok(Some(m)),
        Some(Err(e)) => Err(e),
        None => Ok(None),
    }
}

/// Find all sections of a minutes document, ordered by sequence.
pub fn find_sections(conn: &Connection, minutes_id: i64) -> rusqlite::Result<Vec<MinutesSection>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.name, s.label, \
                COALESCE(p_type.value, '') AS section_type, \
                CAST(COALESCE(p_order.value, '0') AS INTEGER) AS sequence_order, \
                COALESCE(p_content.value, '') AS content, \
                COALESCE(p_auto.value, 'false') AS is_auto_generated \
         FROM entities s \
         JOIN relations r ON s.id = r.source_id \
         LEFT JOIN entity_properties p_type ON s.id = p_type.entity_id AND p_type.key = 'section_type' \
         LEFT JOIN entity_properties p_order ON s.id = p_order.entity_id AND p_order.key = 'sequence_order' \
         LEFT JOIN entity_properties p_content ON s.id = p_content.entity_id AND p_content.key = 'content' \
         LEFT JOIN entity_properties p_auto ON s.id = p_auto.entity_id AND p_auto.key = 'is_auto_generated' \
         WHERE r.target_id = ?1 \
           AND r.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'section_of') \
           AND s.entity_type = 'minutes_section' \
         ORDER BY CAST(COALESCE(p_order.value, '0') AS INTEGER)",
    )?;

    let sections = stmt
        .query_map(params![minutes_id], |row| {
            Ok(MinutesSection {
                id: row.get("id")?,
                name: row.get("name")?,
                label: row.get("label")?,
                section_type: row.get("section_type")?,
                sequence_order: row.get("sequence_order")?,
                content: row.get("content")?,
                is_auto_generated: row.get::<_, String>("is_auto_generated")? == "true",
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(sections)
}

/// Generate a minutes scaffold for a meeting.
/// Creates the minutes entity and auto-generated sections.
pub fn generate_scaffold(
    conn: &Connection,
    meeting_id: i64,
    tor_id: i64,
    meeting_name: &str,
) -> rusqlite::Result<i64> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let minutes_name = format!("minutes_{}", meeting_name.to_lowercase().replace(' ', "_"));
    let minutes_label = format!("Minutes \u{2014} {}", meeting_name);

    // Create minutes entity
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('minutes', ?1, ?2)",
        params![minutes_name, minutes_label],
    )?;
    let minutes_id = conn.last_insert_rowid();

    // Set properties
    let props = vec![
        ("status", "draft"),
        ("generated_date", &today),
    ];
    for (key, value) in props {
        conn.execute(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
            params![minutes_id, key, value],
        )?;
    }

    // Link to meeting via minutes_of relation (meeting -> minutes)
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'minutes_of'), ?1, ?2)",
        params![meeting_id, minutes_id],
    )?;

    // Generate sections
    let sections = [
        ("attendance", "Attendance", generate_attendance_content(conn, tor_id)?),
        ("protocol", "Meeting Protocol", generate_protocol_content(conn, tor_id)?),
        ("agenda_items", "Agenda Items", "No agenda items recorded.".to_string()),
        ("decisions", "Decisions", "No decisions recorded.".to_string()),
        ("action_items", "Action Items", "No action items recorded.".to_string()),
    ];

    for (i, (section_type, label, content)) in sections.iter().enumerate() {
        let section_name = format!("{}_{}", section_type, minutes_id);
        conn.execute(
            "INSERT INTO entities (entity_type, name, label) VALUES ('minutes_section', ?1, ?2)",
            params![section_name, label],
        )?;
        let section_id = conn.last_insert_rowid();

        let section_props = [
            ("section_type", section_type.to_string()),
            ("sequence_order", (i + 1).to_string()),
            ("content", content.to_string()),
            ("is_auto_generated", "true".to_string()),
        ];
        for (key, value) in &section_props {
            conn.execute(
                "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
                params![section_id, key, value],
            )?;
        }

        // Link section to minutes via section_of (section -> minutes)
        conn.execute(
            "INSERT INTO relations (relation_type_id, source_id, target_id) \
             VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'section_of'), ?1, ?2)",
            params![section_id, minutes_id],
        )?;
    }

    Ok(minutes_id)
}

/// Generate attendance content showing positions and their holders.
fn generate_attendance_content(conn: &Connection, tor_id: i64) -> rusqlite::Result<String> {
    use crate::models::tor;
    let members = tor::find_members(conn, tor_id)?;
    let mut lines = Vec::new();
    lines.push("## Attendance\n".to_string());

    for m in &members {
        let holder = match &m.holder_label {
            Some(label) => format!("**{}**", label),
            None => {
                if m.membership_type == "mandatory" {
                    "**VACANT \u{2014} mandatory position**".to_string()
                } else {
                    "_Vacant_".to_string()
                }
            }
        };
        let mt_badge = if m.membership_type == "mandatory" { " [M]" } else { "" };
        lines.push(format!("- {} \u{2014} {}{}", m.position_label, holder, mt_badge));
    }

    Ok(lines.join("\n"))
}

/// Generate protocol content from ToR protocol steps.
fn generate_protocol_content(conn: &Connection, tor_id: i64) -> rusqlite::Result<String> {
    use crate::models::protocol;
    let steps = protocol::find_steps_for_tor(conn, tor_id)?;
    let mut lines = Vec::new();
    lines.push("## Meeting Protocol\n".to_string());

    for step in &steps {
        let duration = match step.default_duration_minutes {
            Some(d) => format!(" ({} min)", d),
            None => String::new(),
        };
        let required = if step.is_required { " *" } else { "" };
        lines.push(format!("{}. {}{}{}", step.sequence_order, step.label, duration, required));
    }

    Ok(lines.join("\n"))
}

/// Update a section's content.
pub fn update_section_content(conn: &Connection, section_id: i64, content: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE entity_properties SET value = ?1 WHERE entity_id = ?2 AND key = 'content'",
        params![content, section_id],
    )?;
    Ok(())
}

/// Update minutes status.
pub fn update_status(conn: &Connection, minutes_id: i64, new_status: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE entity_properties SET value = ?1 WHERE entity_id = ?2 AND key = 'status'",
        params![new_status, minutes_id],
    )?;
    Ok(())
}
