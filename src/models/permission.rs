use rusqlite::{Connection, params};

/// Get all permission codes for a given role entity id.
/// Traverses: role --[has_permission]--> permission entities, returns their names (codes).
pub fn find_codes_by_role_id(conn: &Connection, role_id: i64) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT perm.name AS code \
         FROM relations r \
         JOIN entities perm ON r.target_id = perm.id \
         WHERE r.source_id = ?1 \
           AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission') \
         ORDER BY perm.name"
    )?;
    let codes = stmt
        .query_map(params![role_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(codes)
}
