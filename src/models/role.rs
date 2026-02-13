use rusqlite::{Connection, params};

/// For use in templates (dropdowns, display).
#[derive(Debug, Clone)]
pub struct RoleDisplay {
    pub id: i64,
    pub name: String,
    pub label: String,
}

/// Find all roles for display (dropdowns, lists).
pub fn find_all_display(conn: &Connection) -> rusqlite::Result<Vec<RoleDisplay>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, label FROM entities WHERE entity_type = 'role' ORDER BY sort_order, id"
    )?;
    let roles = stmt.query_map([], |row| {
        Ok(RoleDisplay {
            id: row.get("id")?,
            name: row.get("name")?,
            label: row.get("label")?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(roles)
}

/// Find a role entity by id.
pub fn find_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<RoleDisplay>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, label FROM entities WHERE id = ?1 AND entity_type = 'role'"
    )?;
    let mut rows = stmt.query_map(params![id], |row| {
        Ok(RoleDisplay {
            id: row.get("id")?,
            name: row.get("name")?,
            label: row.get("label")?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}
