use rusqlite::{Connection, params};
use super::types::*;

pub fn find_steps_for_tor(conn: &Connection, tor_id: i64) -> rusqlite::Result<Vec<ProtocolStep>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label, \
                COALESCE(p_type.value, 'procedural') AS step_type, \
                CAST(COALESCE(p_order.value, '0') AS INTEGER) AS sequence_order, \
                CASE WHEN p_dur.value IS NOT NULL THEN CAST(p_dur.value AS INTEGER) ELSE NULL END AS duration, \
                COALESCE(p_desc.value, '') AS description, \
                COALESCE(p_req.value, 'true') AS is_required \
         FROM entities e \
         JOIN relations r ON e.id = r.source_id \
         LEFT JOIN entity_properties p_type ON e.id = p_type.entity_id AND p_type.key = 'step_type' \
         LEFT JOIN entity_properties p_order ON e.id = p_order.entity_id AND p_order.key = 'sequence_order' \
         LEFT JOIN entity_properties p_dur ON e.id = p_dur.entity_id AND p_dur.key = 'default_duration_minutes' \
         LEFT JOIN entity_properties p_desc ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
         LEFT JOIN entity_properties p_req ON e.id = p_req.entity_id AND p_req.key = 'is_required' \
         WHERE r.target_id = ?1 \
           AND r.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'protocol_of') \
           AND e.entity_type = 'protocol_step' \
         ORDER BY CAST(COALESCE(p_order.value, '0') AS INTEGER)",
    )?;

    let steps = stmt
        .query_map(params![tor_id], |row| {
            Ok(ProtocolStep {
                id: row.get("id")?,
                name: row.get("name")?,
                label: row.get("label")?,
                step_type: row.get("step_type")?,
                sequence_order: row.get("sequence_order")?,
                default_duration_minutes: row.get("duration")?,
                description: row.get("description")?,
                is_required: row.get::<_, String>("is_required")? == "true",
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(steps)
}

pub fn create_step(
    conn: &Connection,
    tor_id: i64,
    name: &str,
    label: &str,
    step_type: &str,
    sequence_order: i64,
    default_duration_minutes: Option<i64>,
    description: &str,
    is_required: bool,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('protocol_step', ?1, ?2)",
        params![name, label],
    )?;
    let step_id = conn.last_insert_rowid();

    let props: Vec<(&str, String)> = vec![
        ("step_type", step_type.to_string()),
        ("sequence_order", sequence_order.to_string()),
        ("description", description.to_string()),
        ("is_required", if is_required { "true" } else { "false" }.to_string()),
    ];

    for (key, value) in &props {
        if !value.is_empty() {
            conn.execute(
                "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
                params![step_id, key, value],
            )?;
        }
    }

    if let Some(dur) = default_duration_minutes {
        conn.execute(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'default_duration_minutes', ?2)",
            params![step_id, dur.to_string()],
        )?;
    }

    // Link to ToR via protocol_of relation
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'protocol_of'), ?1, ?2)",
        params![step_id, tor_id],
    )?;

    Ok(step_id)
}

pub fn delete_step(conn: &Connection, step_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM entities WHERE id = ?1 AND entity_type = 'protocol_step'",
        params![step_id],
    )?;
    Ok(())
}

/// Swap sequence_order of two steps.
pub fn reorder_steps(conn: &Connection, step_a_id: i64, step_b_id: i64) -> rusqlite::Result<()> {
    let order_a: String = conn.query_row(
        "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = 'sequence_order'",
        params![step_a_id],
        |row| row.get(0),
    )?;
    let order_b: String = conn.query_row(
        "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = 'sequence_order'",
        params![step_b_id],
        |row| row.get(0),
    )?;

    conn.execute(
        "UPDATE entity_properties SET value = ?1 WHERE entity_id = ?2 AND key = 'sequence_order'",
        params![order_b, step_a_id],
    )?;
    conn.execute(
        "UPDATE entity_properties SET value = ?1 WHERE entity_id = ?2 AND key = 'sequence_order'",
        params![order_a, step_b_id],
    )?;

    Ok(())
}
