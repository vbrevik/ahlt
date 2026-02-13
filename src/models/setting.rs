use rusqlite::{Connection, params};

/// A setting for display and editing.
#[derive(Debug, Clone)]
pub struct SettingDisplay {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub value: String,
    pub description: String,
    pub setting_type: String, // "text", "number", "boolean"
}

/// Find all active settings, ordered by sort_order.
pub fn find_all(conn: &Connection) -> rusqlite::Result<Vec<SettingDisplay>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label, \
                COALESCE(p_val.value, '') AS value, \
                COALESCE(p_desc.value, '') AS description, \
                COALESCE(p_type.value, 'text') AS setting_type \
         FROM entities e \
         LEFT JOIN entity_properties p_val ON e.id = p_val.entity_id AND p_val.key = 'value' \
         LEFT JOIN entity_properties p_desc ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
         LEFT JOIN entity_properties p_type ON e.id = p_type.entity_id AND p_type.key = 'setting_type' \
         WHERE e.entity_type = 'setting' AND e.is_active = 1 \
         ORDER BY e.sort_order, e.id"
    )?;
    let settings = stmt.query_map([], |row| {
        Ok(SettingDisplay {
            id: row.get("id")?,
            name: row.get("name")?,
            label: row.get("label")?,
            value: row.get("value")?,
            description: row.get("description")?,
            setting_type: row.get("setting_type")?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(settings)
}

/// Get a single setting's value by name, returning a default if not found.
pub fn get_value(conn: &Connection, name: &str, default: &str) -> String {
    conn.query_row(
        "SELECT COALESCE(p.value, ?2) \
         FROM entities e \
         LEFT JOIN entity_properties p ON e.id = p.entity_id AND p.key = 'value' \
         WHERE e.entity_type = 'setting' AND e.name = ?1",
        params![name, default],
        |row| row.get(0),
    ).unwrap_or_else(|_| default.to_string())
}

/// Update a single setting's value by entity id (upsert on entity_properties).
pub fn update_value(conn: &Connection, id: i64, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'value', ?2) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value",
        params![id, value],
    )?;
    conn.execute(
        "UPDATE entities SET updated_at = strftime('%Y-%m-%dT%H:%M:%S','now') WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}
