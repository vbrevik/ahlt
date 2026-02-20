use std::collections::HashSet;
use rusqlite::{Connection, params};

/// Permission info for the matrix display.
pub struct PermissionInfo {
    pub id: i64,
    pub code: String,
    pub label: String,
    pub group_name: String,
}

/// Get all permissions with their group_name property, ordered by group then name.
pub fn find_all_with_groups(conn: &Connection) -> rusqlite::Result<Vec<PermissionInfo>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label, COALESCE(ep.value, 'Other') AS group_name \
         FROM entities e \
         LEFT JOIN entity_properties ep ON e.id = ep.entity_id AND ep.key = 'group_name' \
         WHERE e.entity_type = 'permission' AND e.is_active = 1 \
         ORDER BY group_name, e.name"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(PermissionInfo {
            id: row.get(0)?,
            code: row.get(1)?,
            label: row.get(2)?,
            group_name: row.get(3)?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Get all (role_id, permission_id) pairs that have has_permission relations.
pub fn find_all_role_grants(conn: &Connection) -> rusqlite::Result<HashSet<(i64, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT r.source_id, r.target_id \
         FROM relations r \
         WHERE r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission')"
    )?;
    let pairs = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
    })?.collect::<Result<HashSet<_>, _>>()?;
    Ok(pairs)
}

/// Add a has_permission relation between a role and permission.
pub fn grant_permission(conn: &Connection, role_id: i64, permission_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission'), ?1, ?2)",
        params![role_id, permission_id],
    )?;
    Ok(())
}

/// Remove a has_permission relation between a role and permission.
pub fn revoke_permission(conn: &Connection, role_id: i64, permission_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM relations WHERE relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission') \
         AND source_id = ?1 AND target_id = ?2",
        params![role_id, permission_id],
    )?;
    Ok(())
}

/// Get all permission codes for a user across ALL assigned roles (multi-role union).
/// Traverses: user --[has_role]--> role --[has_permission]--> permission entities.
/// Returns sorted, deduplicated permission codes.
pub fn find_codes_by_user_id(conn: &Connection, user_id: i64) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT perm.name AS code \
         FROM relations r_role \
         JOIN relations r_perm ON r_perm.source_id = r_role.target_id \
         JOIN entities perm ON r_perm.target_id = perm.id \
         WHERE r_role.source_id = ?1 \
           AND r_role.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
           AND r_perm.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission') \
           AND perm.entity_type = 'permission' \
         ORDER BY perm.name"
    )?;
    let codes = stmt
        .query_map(params![user_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(codes)
}

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
