use rusqlite::{Connection, params};
use super::entity::Entity;

/// Find all target entities related to source via a named relation type.
/// e.g. find_targets(conn, user_id, "has_role") → [role entity]
pub fn find_targets(conn: &Connection, source_id: i64, relation_type_name: &str) -> rusqlite::Result<Vec<Entity>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.entity_type, t.name, t.label, t.sort_order, t.is_active, t.created_at, t.updated_at \
         FROM relations r \
         JOIN entities t ON r.target_id = t.id \
         WHERE r.source_id = ?1 \
           AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = ?2) \
         ORDER BY t.sort_order, t.id"
    )?;
    let rows = stmt.query_map(params![source_id, relation_type_name], |row| {
        Ok(Entity {
            id: row.get("id")?,
            entity_type: row.get("entity_type")?,
            name: row.get("name")?,
            label: row.get("label")?,
            sort_order: row.get("sort_order")?,
            is_active: row.get::<_, i64>("is_active")? != 0,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Find all source entities related to target via a named relation type.
/// e.g. find_sources(conn, role_id, "has_role") → [user entities with that role]
pub fn find_sources(conn: &Connection, target_id: i64, relation_type_name: &str) -> rusqlite::Result<Vec<Entity>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.entity_type, s.name, s.label, s.sort_order, s.is_active, s.created_at, s.updated_at \
         FROM relations r \
         JOIN entities s ON r.source_id = s.id \
         WHERE r.target_id = ?1 \
           AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = ?2) \
         ORDER BY s.sort_order, s.id"
    )?;
    let rows = stmt.query_map(params![target_id, relation_type_name], |row| {
        Ok(Entity {
            id: row.get("id")?,
            entity_type: row.get("entity_type")?,
            name: row.get("name")?,
            label: row.get("label")?,
            sort_order: row.get("sort_order")?,
            is_active: row.get::<_, i64>("is_active")? != 0,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Create a relation between two entities.
pub fn create(conn: &Connection, relation_type_name: &str, source_id: i64, target_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = ?1), ?2, ?3)",
        params![relation_type_name, source_id, target_id],
    )?;
    Ok(())
}

/// Delete a specific relation.
pub fn delete(conn: &Connection, relation_type_name: &str, source_id: i64, target_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM relations WHERE relation_type_id = \
         (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = ?1) \
         AND source_id = ?2 AND target_id = ?3",
        params![relation_type_name, source_id, target_id],
    )?;
    Ok(())
}

/// Delete all relations of a given type from a source entity.
/// e.g. delete_all_from_source(conn, user_id, "has_role") removes all role assignments.
pub fn delete_all_from_source(conn: &Connection, source_id: i64, relation_type_name: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM relations WHERE source_id = ?1 AND relation_type_id = \
         (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = ?2)",
        params![source_id, relation_type_name],
    )?;
    Ok(())
}
