use rusqlite::{Connection, params};
use super::types::{RoleDisplay, RoleListItem, RoleDetail, PermissionCheckbox, RoleMember};

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

/// Find all roles with user count and permission count for the list page.
pub fn find_all_list_items(conn: &Connection) -> rusqlite::Result<Vec<RoleListItem>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label, \
                COALESCE(p_desc.value, '') AS description, \
                (SELECT COUNT(*) FROM relations r_user \
                 WHERE r_user.target_id = e.id \
                   AND r_user.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
                ) AS user_count, \
                (SELECT COUNT(*) FROM relations r_perm \
                 WHERE r_perm.source_id = e.id \
                   AND r_perm.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission') \
                ) AS permission_count \
         FROM entities e \
         LEFT JOIN entity_properties p_desc ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
         WHERE e.entity_type = 'role' \
         ORDER BY e.sort_order, e.id"
    )?;
    let roles = stmt.query_map([], |row| {
        Ok(RoleListItem {
            id: row.get("id")?,
            name: row.get("name")?,
            label: row.get("label")?,
            description: row.get("description")?,
            user_count: row.get("user_count")?,
            permission_count: row.get("permission_count")?,
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

/// Find a role with its description for editing.
pub fn find_detail_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<RoleDetail>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label, COALESCE(p_desc.value, '') AS description \
         FROM entities e \
         LEFT JOIN entity_properties p_desc ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
         WHERE e.id = ?1 AND e.entity_type = 'role'"
    )?;
    let mut rows = stmt.query_map(params![id], |row| {
        Ok(RoleDetail {
            id: row.get("id")?,
            name: row.get("name")?,
            label: row.get("label")?,
            description: row.get("description")?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Get all permissions as checkboxes, with `checked` set for those assigned to the given role.
pub fn find_permission_checkboxes(conn: &Connection, role_id: i64) -> rusqlite::Result<Vec<PermissionCheckbox>> {
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name AS code, p.label, \
                COALESCE(pg.value, '') AS group_name, \
                CASE WHEN r.id IS NOT NULL THEN 1 ELSE 0 END AS checked \
         FROM entities p \
         LEFT JOIN entity_properties pg ON p.id = pg.entity_id AND pg.key = 'group_name' \
         LEFT JOIN relations r ON r.source_id = ?1 AND r.target_id = p.id \
             AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission') \
         WHERE p.entity_type = 'permission' \
         ORDER BY group_name, p.name"
    )?;
    let perms = stmt.query_map(params![role_id], |row| {
        Ok(PermissionCheckbox {
            id: row.get("id")?,
            code: row.get("code")?,
            label: row.get("label")?,
            group_name: row.get("group_name")?,
            checked: row.get::<_, i64>("checked")? == 1,
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(perms)
}

/// Create a new role entity with description and permission relations.
pub fn create(conn: &Connection, name: &str, label: &str, description: &str, permission_ids: &[i64]) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', ?1, ?2)",
        params![name, label],
    )?;
    let role_id = conn.last_insert_rowid();

    if !description.is_empty() {
        conn.execute(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'description', ?2)",
            params![role_id, description],
        )?;
    }

    let has_perm_id = get_has_permission_id(conn)?;
    for perm_id in permission_ids {
        conn.execute(
            "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
            params![has_perm_id, role_id, perm_id],
        )?;
    }

    Ok(role_id)
}

/// Update a role's name, label, description, and permission relations.
pub fn update(conn: &Connection, id: i64, name: &str, label: &str, description: &str, permission_ids: &[i64]) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE entities SET name = ?1, label = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%S','now') WHERE id = ?3",
        params![name, label, id],
    )?;

    // Upsert description
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'description', ?2) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value",
        params![id, description],
    )?;

    // Replace permission relations: delete all, re-insert selected
    let has_perm_id = get_has_permission_id(conn)?;
    conn.execute(
        "DELETE FROM relations WHERE source_id = ?1 AND relation_type_id = ?2",
        params![id, has_perm_id],
    )?;
    for perm_id in permission_ids {
        conn.execute(
            "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
            params![has_perm_id, id, perm_id],
        )?;
    }

    Ok(())
}

/// Delete a role entity (cascades to properties and relations via FK).
pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM entities WHERE id = ?1 AND entity_type = 'role'", params![id])?;
    Ok(())
}

/// Count users assigned to a role.
pub fn count_users(conn: &Connection, role_id: i64) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM relations \
         WHERE relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
         AND target_id = ?1",
        params![role_id],
        |row| row.get(0),
    )
}

/// Find all users assigned to a specific role.
pub fn find_users_by_role(conn: &Connection, role_id: i64) -> rusqlite::Result<Vec<RoleMember>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name AS username, e.label AS display_name \
         FROM entities e \
         JOIN relations r ON r.source_id = e.id AND r.target_id = ?1 \
           AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
         WHERE e.entity_type = 'user' \
         ORDER BY e.label, e.name"
    )?;
    let members = stmt.query_map(params![role_id], |row| {
        Ok(RoleMember {
            user_id: row.get("id")?,
            username: row.get("username")?,
            display_name: row.get("display_name")?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(members)
}

/// Find users NOT assigned to a specific role (for "Add User" dropdown).
pub fn find_users_not_in_role(conn: &Connection, role_id: i64) -> rusqlite::Result<Vec<RoleMember>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name AS username, e.label AS display_name \
         FROM entities e \
         WHERE e.entity_type = 'user' \
           AND e.id NOT IN ( \
               SELECT r.source_id FROM relations r \
               WHERE r.target_id = ?1 \
                 AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
           ) \
         ORDER BY e.label, e.name"
    )?;
    let members = stmt.query_map(params![role_id], |row| {
        Ok(RoleMember {
            user_id: row.get("id")?,
            username: row.get("username")?,
            display_name: row.get("display_name")?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(members)
}

/// Helper to get the has_permission relation type id.
fn get_has_permission_id(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission'",
        [],
        |row| row.get(0),
    )
}
