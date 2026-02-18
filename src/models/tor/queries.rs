use rusqlite::{Connection, params};
use crate::errors::AppError;
use super::types::*;

pub fn find_all_list_items(conn: &Connection) -> rusqlite::Result<Vec<TorListItem>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label, \
                COALESCE(p_desc.value, '') AS description, \
                COALESCE(p_status.value, 'active') AS status, \
                COALESCE(p_cadence.value, 'ad-hoc') AS meeting_cadence, \
                (SELECT COUNT(DISTINCT r_fills.source_id) \
                 FROM relations r_tor \
                 JOIN relations r_fills ON r_tor.source_id = r_fills.target_id \
                 WHERE r_tor.target_id = e.id \
                   AND r_tor.relation_type_id = (\
                       SELECT id FROM entities \
                       WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
                   AND r_fills.relation_type_id = (\
                       SELECT id FROM entities \
                       WHERE entity_type = 'relation_type' AND name = 'fills_position') \
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

/// Find all positions in a ToR with their current holders.
/// Returns positions even when vacant (holder fields will be None).
pub fn find_members(conn: &Connection, tor_id: i64) -> rusqlite::Result<Vec<TorMember>> {
    let mut stmt = conn.prepare(
        "SELECT f.id AS position_id, f.name AS position_name, f.label AS position_label, \
                COALESCE(p_mt.value, 'optional') AS membership_type, \
                u.id AS holder_id, u.name AS holder_name, u.label AS holder_label \
         FROM entities f \
         JOIN relations r_tor ON f.id = r_tor.source_id \
         LEFT JOIN relations r_fills ON f.id = r_fills.target_id \
             AND r_fills.relation_type_id = ( \
                 SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position') \
         LEFT JOIN entities u ON r_fills.source_id = u.id AND u.entity_type = 'user' \
         LEFT JOIN entity_properties p_mt ON f.id = p_mt.entity_id AND p_mt.key = 'membership_type' \
         WHERE r_tor.target_id = ?1 \
           AND r_tor.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
           AND f.entity_type = 'tor_function' \
         ORDER BY CASE WHEN COALESCE(p_mt.value, 'optional') = 'mandatory' THEN 0 ELSE 1 END, f.label",
    )?;

    let members = stmt
        .query_map(params![tor_id], |row| {
            let holder_id: Option<i64> = row.get("holder_id")?;
            Ok(TorMember {
                position_id: row.get("position_id")?,
                position_name: row.get("position_name")?,
                position_label: row.get("position_label")?,
                membership_type: row.get("membership_type")?,
                holder_id,
                holder_name: if holder_id.is_some() { row.get("holder_name")? } else { None },
                holder_label: if holder_id.is_some() { row.get("holder_label")? } else { None },
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(members)
}

/// Assign a user to a position (creates fills_position relation).
pub fn assign_to_position(
    conn: &Connection,
    user_id: i64,
    position_id: i64,
    membership_type: &str,
) -> rusqlite::Result<()> {
    // Set the membership_type property on the position
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'membership_type', ?2) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value",
        params![position_id, membership_type],
    )?;

    // Create fills_position relation
    conn.execute(
        "INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id) \
         VALUES ( \
             (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position'), \
             ?1, ?2)",
        params![user_id, position_id],
    )?;

    Ok(())
}

/// Remove the current holder from a position.
pub fn vacate_position(conn: &Connection, position_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM relations WHERE target_id = ?1 \
         AND relation_type_id = ( \
             SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position')",
        params![position_id],
    )?;
    Ok(())
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
           AND r.relation_type_id = ( \
               SELECT id FROM entities \
               WHERE entity_type = 'relation_type' AND name = 'fills_position') \
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

/// Count positions with holders in a ToR (not vacant positions).
pub fn count_members(conn: &Connection, tor_id: i64) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(DISTINCT r_fills.source_id) \
         FROM entities f \
         JOIN relations r_tor ON f.id = r_tor.source_id \
         JOIN relations r_fills ON f.id = r_fills.target_id \
         WHERE r_tor.target_id = ?1 \
           AND r_tor.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
           AND r_fills.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position') \
           AND f.entity_type = 'tor_function'",
        params![tor_id],
        |row| row.get(0),
    )
}

/// Find users not currently filling any position in this ToR.
pub fn find_non_members(
    conn: &Connection,
    tor_id: i64,
) -> rusqlite::Result<Vec<(i64, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label \
         FROM entities e \
         WHERE e.entity_type = 'user' \
           AND e.is_active = 1 \
           AND e.id NOT IN ( \
               SELECT r_fills.source_id \
               FROM relations r_fills \
               JOIN relations r_tor ON r_fills.target_id = r_tor.source_id \
               WHERE r_tor.target_id = ?1 \
                 AND r_tor.relation_type_id = ( \
                     SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
                 AND r_fills.relation_type_id = ( \
                     SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position')) \
         ORDER BY e.label",
    )?;

    let users = stmt
        .query_map(params![tor_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(users)
}

/// Verify user fills a position in the given ToR. Returns AppError::PermissionDenied if not.
pub fn require_tor_membership(
    conn: &Connection,
    user_id: i64,
    tor_id: i64,
) -> Result<(), AppError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) \
         FROM relations r_fills \
         JOIN relations r_tor ON r_fills.target_id = r_tor.source_id \
         WHERE r_fills.source_id = ?1 \
           AND r_tor.target_id = ?2 \
           AND r_fills.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position') \
           AND r_tor.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor')",
        params![user_id, tor_id],
        |row| row.get(0),
    )?;

    if count == 0 {
        return Err(AppError::PermissionDenied("Not a member of this ToR".into()));
    }
    Ok(())
}

/// Get a ToR's display name (label) by ID.
pub fn get_tor_name(conn: &Connection, tor_id: i64) -> Result<String, AppError> {
    let name: String = conn.query_row(
        "SELECT label FROM entities WHERE id = ?1 AND entity_type = 'tor'",
        params![tor_id],
        |row| row.get(0),
    )?;
    Ok(name)
}
