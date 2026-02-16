use rusqlite::{Connection, params};

#[derive(Debug, Clone, serde::Serialize)]
pub struct NavItemPreview {
    pub id: i64,
    pub label: String,
    pub path: String,
    pub module_name: String,
}

pub fn find_accessible_nav_items(
    conn: &Connection,
    permission_ids: &[i64],
) -> rusqlite::Result<Vec<NavItemPreview>> {
    if permission_ids.is_empty() {
        return Ok(Vec::new());
    }

    // Build permission code lookup
    let mut stmt = conn.prepare(
        "SELECT name FROM entities WHERE id IN (SELECT value FROM json_each(?1))"
    )?;
    let permission_codes: Vec<String> = stmt.query_map(
        params![serde_json::to_string(&permission_ids).unwrap()],
        |row| row.get(0),
    )?.collect::<Result<Vec<_>, _>>()?;

    if permission_codes.is_empty() {
        return Ok(Vec::new());
    }

    // Find nav items where permission_required matches any permission code
    let placeholders = permission_codes.iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");

    let query = format!(
        "SELECT DISTINCT ni.id, ni.label,
                COALESCE(p_path.value, '') as path,
                COALESCE(m.label, 'General') as module_name
         FROM entities ni
         LEFT JOIN entity_properties p_path ON p_path.entity_id = ni.id AND p_path.key = 'path'
         LEFT JOIN entity_properties p_perm ON p_perm.entity_id = ni.id AND p_perm.key = 'permission_required'
         LEFT JOIN relations r_mod ON r_mod.source_id = ni.id
         LEFT JOIN entities rt_mod ON rt_mod.id = r_mod.relation_type_id AND rt_mod.name = 'in_module'
         LEFT JOIN entities m ON m.id = r_mod.target_id
         WHERE ni.entity_type = 'nav_item'
           AND (p_perm.value IS NULL OR p_perm.value IN ({}))
         ORDER BY m.label, ni.label",
        placeholders
    );

    let mut stmt = conn.prepare(&query)?;
    let params_refs: Vec<&dyn rusqlite::types::ToSql> = permission_codes.iter()
        .map(|s| s as &dyn rusqlite::types::ToSql)
        .collect();

    let items = stmt.query_map(params_refs.as_slice(), |row| {
        Ok(NavItemPreview {
            id: row.get("id")?,
            label: row.get("label")?,
            path: row.get("path")?,
            module_name: row.get("module_name")?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;

    Ok(items)
}
