use rusqlite::{Connection, params};
use super::types::*;

pub fn find_all_list_items(conn: &Connection) -> rusqlite::Result<Vec<TorListItem>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label, \
                COALESCE(p_desc.value, '') AS description, \
                COALESCE(p_status.value, 'active') AS status, \
                COALESCE(p_cadence.value, 'ad-hoc') AS meeting_cadence, \
                (SELECT COUNT(*) FROM relations r_member \
                 WHERE r_member.target_id = e.id \
                   AND r_member.relation_type_id = (\
                       SELECT id FROM entities \
                       WHERE entity_type = 'relation_type' AND name = 'member_of') \
                ) AS member_count, \
                (SELECT COUNT(*) FROM relations r_func \
                 WHERE r_func.target_id = e.id \
                   AND r_func.relation_type_id = (\
                       SELECT id FROM entities \
                       WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
                ) AS function_count \
         FROM entities e \
         LEFT JOIN entity_properties p_desc \
             ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
         LEFT JOIN entity_properties p_status \
             ON e.id = p_status.entity_id AND p_status.key = 'status' \
         LEFT JOIN entity_properties p_cadence \
             ON e.id = p_cadence.entity_id AND p_cadence.key = 'meeting_cadence' \
         WHERE e.entity_type = 'tor' \
         ORDER BY e.sort_order, e.id",
    )?;

    let items = stmt
        .query_map([], |row| {
            Ok(TorListItem {
                id: row.get("id")?,
                name: row.get("name")?,
                label: row.get("label")?,
                description: row.get("description")?,
                status: row.get("status")?,
                meeting_cadence: row.get("meeting_cadence")?,
                member_count: row.get("member_count")?,
                function_count: row.get("function_count")?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(items)
}

pub fn find_detail_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<TorDetail>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label, \
                COALESCE(p_desc.value, '') AS description, \
                COALESCE(p_status.value, 'active') AS status, \
                COALESCE(p_cadence.value, 'ad-hoc') AS meeting_cadence, \
                COALESCE(p_day.value, '') AS cadence_day, \
                COALESCE(p_time.value, '') AS cadence_time, \
                COALESCE(p_dur.value, '60') AS cadence_duration_minutes, \
                COALESCE(p_loc.value, '') AS default_location, \
                COALESCE(p_remote.value, '') AS remote_url, \
                COALESCE(p_repo.value, '') AS background_repo_url \
         FROM entities e \
         LEFT JOIN entity_properties p_desc \
             ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
         LEFT JOIN entity_properties p_status \
             ON e.id = p_status.entity_id AND p_status.key = 'status' \
         LEFT JOIN entity_properties p_cadence \
             ON e.id = p_cadence.entity_id AND p_cadence.key = 'meeting_cadence' \
         LEFT JOIN entity_properties p_day \
             ON e.id = p_day.entity_id AND p_day.key = 'cadence_day' \
         LEFT JOIN entity_properties p_time \
             ON e.id = p_time.entity_id AND p_time.key = 'cadence_time' \
         LEFT JOIN entity_properties p_dur \
             ON e.id = p_dur.entity_id AND p_dur.key = 'cadence_duration_minutes' \
         LEFT JOIN entity_properties p_loc \
             ON e.id = p_loc.entity_id AND p_loc.key = 'default_location' \
         LEFT JOIN entity_properties p_remote \
             ON e.id = p_remote.entity_id AND p_remote.key = 'remote_url' \
         LEFT JOIN entity_properties p_repo \
             ON e.id = p_repo.entity_id AND p_repo.key = 'background_repo_url' \
         WHERE e.id = ?1 AND e.entity_type = 'tor'",
    )?;

    let mut rows = stmt.query_map(params![id], |row| {
        Ok(TorDetail {
            id: row.get("id")?,
            name: row.get("name")?,
            label: row.get("label")?,
            description: row.get("description")?,
            status: row.get("status")?,
            meeting_cadence: row.get("meeting_cadence")?,
            cadence_day: row.get("cadence_day")?,
            cadence_time: row.get("cadence_time")?,
            cadence_duration_minutes: row.get("cadence_duration_minutes")?,
            default_location: row.get("default_location")?,
            remote_url: row.get("remote_url")?,
            background_repo_url: row.get("background_repo_url")?,
        })
    })?;

    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn create(
    conn: &Connection,
    name: &str,
    label: &str,
    description: &str,
    status: &str,
    meeting_cadence: &str,
    cadence_day: &str,
    cadence_time: &str,
    cadence_duration_minutes: &str,
    default_location: &str,
    remote_url: &str,
    background_repo_url: &str,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('tor', ?1, ?2)",
        params![name, label],
    )?;
    let tor_id = conn.last_insert_rowid();

    let props: Vec<(&str, &str)> = vec![
        ("description", description),
        ("status", status),
        ("meeting_cadence", meeting_cadence),
        ("cadence_day", cadence_day),
        ("cadence_time", cadence_time),
        ("cadence_duration_minutes", cadence_duration_minutes),
        ("default_location", default_location),
        ("remote_url", remote_url),
        ("background_repo_url", background_repo_url),
    ];

    for (key, value) in props {
        if !value.is_empty() {
            conn.execute(
                "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
                params![tor_id, key, value],
            )?;
        }
    }

    Ok(tor_id)
}

#[allow(clippy::too_many_arguments)]
pub fn update(
    conn: &Connection,
    id: i64,
    name: &str,
    label: &str,
    description: &str,
    status: &str,
    meeting_cadence: &str,
    cadence_day: &str,
    cadence_time: &str,
    cadence_duration_minutes: &str,
    default_location: &str,
    remote_url: &str,
    background_repo_url: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE entities SET name = ?1, label = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%S','now') \
         WHERE id = ?3",
        params![name, label, id],
    )?;

    let props: Vec<(&str, &str)> = vec![
        ("description", description),
        ("status", status),
        ("meeting_cadence", meeting_cadence),
        ("cadence_day", cadence_day),
        ("cadence_time", cadence_time),
        ("cadence_duration_minutes", cadence_duration_minutes),
        ("default_location", default_location),
        ("remote_url", remote_url),
        ("background_repo_url", background_repo_url),
    ];

    for (key, value) in props {
        conn.execute(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3) \
             ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value",
            params![id, key, value],
        )?;
    }

    Ok(())
}

pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM entities WHERE id = ?1 AND entity_type = 'tor'",
        params![id],
    )?;
    Ok(())
}

pub fn find_members(conn: &Connection, tor_id: i64) -> rusqlite::Result<Vec<TorMember>> {
    let mut stmt = conn.prepare(
        "SELECT u.id, u.name, u.label \
         FROM relations r \
         JOIN entities u ON r.source_id = u.id \
         WHERE r.target_id = ?1 \
           AND r.relation_type_id = (\
               SELECT id FROM entities \
               WHERE entity_type = 'relation_type' AND name = 'member_of') \
         ORDER BY u.label",
    )?;

    let users: Vec<(i64, String, String)> = stmt
        .query_map(params![tor_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut func_stmt = conn.prepare(
        "SELECT f.id, f.name, f.label \
         FROM relations r_role \
         JOIN entities f ON r_role.target_id = f.id \
         JOIN relations r_tor ON f.id = r_tor.source_id \
         WHERE r_role.source_id = ?1 \
           AND r_role.relation_type_id = (\
               SELECT id FROM entities \
               WHERE entity_type = 'relation_type' AND name = 'has_tor_role') \
           AND r_tor.target_id = ?2 \
           AND r_tor.relation_type_id = (\
               SELECT id FROM entities \
               WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
         ORDER BY f.label",
    )?;

    let mut members = Vec::new();
    for (user_id, user_name, user_label) in users {
        let functions: Vec<TorFunctionRef> = func_stmt
            .query_map(params![user_id, tor_id], |row| {
                Ok(TorFunctionRef {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    label: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        members.push(TorMember {
            user_id,
            user_name,
            user_label,
            functions,
        });
    }

    Ok(members)
}

pub fn find_functions(
    conn: &Connection,
    tor_id: i64,
) -> rusqlite::Result<Vec<TorFunctionListItem>> {
    let mut stmt = conn.prepare(
        "SELECT f.id, f.name, f.label, \
                COALESCE(p_cat.value, '') AS category \
         FROM relations r \
         JOIN entities f ON r.source_id = f.id \
         LEFT JOIN entity_properties p_cat \
             ON f.id = p_cat.entity_id AND p_cat.key = 'category' \
         WHERE r.target_id = ?1 \
           AND r.relation_type_id = (\
               SELECT id FROM entities \
               WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
           AND f.entity_type = 'tor_function' \
         ORDER BY f.sort_order, f.id",
    )?;

    let functions: Vec<(i64, String, String, String)> = stmt
        .query_map(params![tor_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut user_stmt = conn.prepare(
        "SELECT u.label \
         FROM relations r \
         JOIN entities u ON r.source_id = u.id \
         WHERE r.target_id = ?1 \
           AND r.relation_type_id = (\
               SELECT id FROM entities \
               WHERE entity_type = 'relation_type' AND name = 'has_tor_role') \
         ORDER BY u.label",
    )?;

    let mut result = Vec::new();
    for (id, name, label, category) in functions {
        let assigned_to: Vec<String> = user_stmt
            .query_map(params![id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        result.push(TorFunctionListItem {
            id,
            name,
            label,
            category,
            assigned_to,
        });
    }

    Ok(result)
}

pub fn add_member(conn: &Connection, user_id: i64, tor_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id) \
         VALUES (\
             (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'member_of'), \
             ?1, ?2\
         )",
        params![user_id, tor_id],
    )?;
    Ok(())
}

pub fn remove_member(conn: &Connection, user_id: i64, tor_id: i64) -> rusqlite::Result<()> {
    // Remove membership relation
    conn.execute(
        "DELETE FROM relations \
         WHERE source_id = ?1 AND target_id = ?2 \
           AND relation_type_id = (\
               SELECT id FROM entities \
               WHERE entity_type = 'relation_type' AND name = 'member_of')",
        params![user_id, tor_id],
    )?;

    // Remove any tor function assignments for this user within this tor
    conn.execute(
        "DELETE FROM relations \
         WHERE source_id = ?1 \
           AND relation_type_id = (\
               SELECT id FROM entities \
               WHERE entity_type = 'relation_type' AND name = 'has_tor_role') \
           AND target_id IN (\
               SELECT r.source_id FROM relations r \
               WHERE r.target_id = ?2 \
                 AND r.relation_type_id = (\
                     SELECT id FROM entities \
                     WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor'))",
        params![user_id, tor_id],
    )?;

    Ok(())
}

pub fn create_function(
    conn: &Connection,
    tor_id: i64,
    name: &str,
    label: &str,
    description: &str,
    category: &str,
    authority_props: &[(&str, bool)],
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('tor_function', ?1, ?2)",
        params![name, label],
    )?;
    let func_id = conn.last_insert_rowid();

    if !description.is_empty() {
        conn.execute(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'description', ?2)",
            params![func_id, description],
        )?;
    }

    if !category.is_empty() {
        conn.execute(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'category', ?2)",
            params![func_id, category],
        )?;
    }

    for (key, value) in authority_props {
        conn.execute(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
            params![func_id, key, if *value { "true" } else { "false" }],
        )?;
    }

    // Link function to ToR via belongs_to_tor relation
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) \
         VALUES (\
             (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor'), \
             ?1, ?2\
         )",
        params![func_id, tor_id],
    )?;

    Ok(func_id)
}

pub fn assign_function(
    conn: &Connection,
    user_id: i64,
    function_id: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id) \
         VALUES (\
             (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_tor_role'), \
             ?1, ?2\
         )",
        params![user_id, function_id],
    )?;
    Ok(())
}

pub fn unassign_function(
    conn: &Connection,
    user_id: i64,
    function_id: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM relations \
         WHERE source_id = ?1 AND target_id = ?2 \
           AND relation_type_id = (\
               SELECT id FROM entities \
               WHERE entity_type = 'relation_type' AND name = 'has_tor_role')",
        params![user_id, function_id],
    )?;
    Ok(())
}

pub fn count_members(conn: &Connection, tor_id: i64) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM relations \
         WHERE relation_type_id = (\
             SELECT id FROM entities \
             WHERE entity_type = 'relation_type' AND name = 'member_of') \
           AND target_id = ?1",
        params![tor_id],
        |row| row.get(0),
    )
}

pub fn find_non_members(
    conn: &Connection,
    tor_id: i64,
) -> rusqlite::Result<Vec<(i64, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label \
         FROM entities e \
         WHERE e.entity_type = 'user' \
           AND e.is_active = 1 \
           AND e.id NOT IN (\
               SELECT r.source_id FROM relations r \
               WHERE r.target_id = ?1 \
                 AND r.relation_type_id = (\
                     SELECT id FROM entities \
                     WHERE entity_type = 'relation_type' AND name = 'member_of')) \
         ORDER BY e.label",
    )?;

    let users = stmt
        .query_map(params![tor_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(users)
}
