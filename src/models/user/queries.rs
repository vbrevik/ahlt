use rusqlite::{Connection, params};
use super::types::{User, UserDisplay, UserPage, NewUser};

/// SQL for user display: entity + email property + role via has_role relation.
const SELECT_USER_DISPLAY: &str = "\
    SELECT e.id, e.name AS username, e.label AS display_name, \
           COALESCE(p_email.value, '') AS email, \
           COALESCE(role_e.id, 0) AS role_id, \
           COALESCE(role_e.name, '') AS role_name, \
           COALESCE(role_e.label, '') AS role_label, \
           e.created_at, e.updated_at \
    FROM entities e \
    LEFT JOIN entity_properties p_email \
        ON e.id = p_email.entity_id AND p_email.key = 'email' \
    LEFT JOIN relations r_role \
        ON r_role.source_id = e.id \
        AND r_role.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
    LEFT JOIN entities role_e ON r_role.target_id = role_e.id \
    WHERE e.entity_type = 'user'";

fn row_to_user_display(row: &rusqlite::Row) -> rusqlite::Result<UserDisplay> {
    Ok(UserDisplay {
        id: row.get("id")?,
        username: row.get("username")?,
        email: row.get("email")?,
        display_name: row.get("display_name")?,
        role_id: row.get("role_id")?,
        role_name: row.get("role_name")?,
        role_label: row.get("role_label")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

/// Find users with pagination, filter, and sort support.
pub fn find_paginated(
    conn: &Connection,
    page: i64,
    per_page: i64,
    filter: &crate::models::table_filter::FilterTree,
    sort: &crate::models::table_filter::SortSpec,
) -> rusqlite::Result<UserPage> {
    use crate::models::table_filter::{builder, SortDir};
    use crate::models::user::filter as uf;

    let page = page.max(1);
    let per_page = per_page.clamp(1, 100);
    let offset = (page - 1) * per_page;

    // Build WHERE clause
    let (where_clause, filter_params) = builder::build_where_clause(
        filter, &uf::field_map(), uf::OPS, 0,
    ).unwrap_or_else(|_| ("1=1".to_string(), vec![]));

    // Build ORDER BY
    let sort_col = uf::sort_col(&sort.column);
    let sort_dir = match sort.dir { SortDir::Asc => "ASC", SortDir::Desc => "DESC" };

    // Count query needs JOINs for filter fields that reference joined tables
    let count_sql = format!(
        "SELECT COUNT(*) FROM entities e \
         LEFT JOIN entity_properties p_email ON e.id = p_email.entity_id AND p_email.key = 'email' \
         LEFT JOIN relations r_role ON r_role.source_id = e.id AND r_role.relation_type_id = \
             (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
         LEFT JOIN entities role_e ON r_role.target_id = role_e.id \
         WHERE e.entity_type = 'user' AND ({where_clause})"
    );

    let total_count: i64 = {
        let mut stmt = conn.prepare(&count_sql)?;
        stmt.query_row(rusqlite::params_from_iter(filter_params.iter()), |r| r.get(0))?
    };

    // Data query
    let n = filter_params.len();
    let data_sql = format!(
        "{SELECT_USER_DISPLAY} AND ({where_clause}) \
         ORDER BY {sort_col} {sort_dir} \
         LIMIT ?{} OFFSET ?{}",
        n + 1, n + 2
    );

    let mut all_params: Vec<rusqlite::types::Value> = filter_params.iter()
        .map(|s| rusqlite::types::Value::Text(s.clone()))
        .collect();
    all_params.push(rusqlite::types::Value::Integer(per_page));
    all_params.push(rusqlite::types::Value::Integer(offset));

    let mut stmt = conn.prepare(&data_sql)?;
    let users = stmt.query_map(rusqlite::params_from_iter(all_params.iter()), row_to_user_display)?
        .collect::<Result<Vec<_>, _>>()?;

    let total_pages = ((total_count as f64) / (per_page as f64)).ceil() as i64;

    Ok(UserPage { users, page, per_page, total_count, total_pages })
}

/// Return all users matching the filter (no pagination) — used for CSV export.
pub fn find_all_filtered(
    conn: &Connection,
    filter: &crate::models::table_filter::FilterTree,
    sort: &crate::models::table_filter::SortSpec,
) -> rusqlite::Result<Vec<UserDisplay>> {
    use crate::models::table_filter::{builder, SortDir};
    use crate::models::user::filter as uf;

    let (where_clause, filter_params) = builder::build_where_clause(
        filter, &uf::field_map(), uf::OPS, 0,
    ).unwrap_or_else(|_| ("1=1".to_string(), vec![]));

    let sort_col = uf::sort_col(&sort.column);
    let sort_dir = match sort.dir { SortDir::Asc => "ASC", SortDir::Desc => "DESC" };

    let sql = format!(
        "{SELECT_USER_DISPLAY} AND ({where_clause}) ORDER BY {sort_col} {sort_dir}"
    );

    let mut stmt = conn.prepare(&sql)?;
    let users = stmt.query_map(
        rusqlite::params_from_iter(filter_params.iter()),
        row_to_user_display,
    )?.collect::<Result<Vec<_>, _>>()?;

    Ok(users)
}

pub fn find_display_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<UserDisplay>> {
    let sql = format!("{SELECT_USER_DISPLAY} AND e.id = ?1");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map(params![id], row_to_user_display)?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Find user by username for authentication. Returns internal User with password hash.
pub fn find_by_username(conn: &Connection, username: &str) -> rusqlite::Result<Option<User>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name AS username, e.label AS display_name, \
                COALESCE(p_pw.value, '') AS password, \
                COALESCE(p_email.value, '') AS email, \
                COALESCE(role_e.id, 0) AS role_id, \
                e.created_at, e.updated_at \
         FROM entities e \
         LEFT JOIN entity_properties p_pw ON e.id = p_pw.entity_id AND p_pw.key = 'password' \
         LEFT JOIN entity_properties p_email ON e.id = p_email.entity_id AND p_email.key = 'email' \
         LEFT JOIN relations r_role \
             ON r_role.source_id = e.id \
             AND r_role.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
         LEFT JOIN entities role_e ON r_role.target_id = role_e.id \
         WHERE e.entity_type = 'user' AND e.name = ?1"
    )?;
    let mut rows = stmt.query_map(params![username], |row| {
        Ok(User {
            id: row.get("id")?,
            username: row.get("username")?,
            password: row.get("password")?,
            email: row.get("email")?,
            display_name: row.get("display_name")?,
            role_id: row.get("role_id")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Count user entities.
pub fn count(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM entities WHERE entity_type = 'user'",
        [],
        |row| row.get(0),
    )
}

/// Create a new user entity with properties and role relation.
pub fn create(conn: &Connection, new: &NewUser) -> rusqlite::Result<i64> {
    // Insert user entity
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('user', ?1, ?2)",
        params![new.username, new.display_name],
    )?;
    let user_id = conn.last_insert_rowid();

    // Set properties
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'password', ?2)",
        params![user_id, new.password],
    )?;
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'email', ?2)",
        params![user_id, new.email],
    )?;

    // Create has_role relation
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role'), ?1, ?2)",
        params![user_id, new.role_id],
    )?;

    Ok(user_id)
}

/// Update a user entity: name, label (display_name), properties, and role relation.
pub fn update(
    conn: &Connection,
    id: i64,
    username: &str,
    password: Option<&str>,
    email: &str,
    display_name: &str,
    role_id: i64,
) -> rusqlite::Result<()> {
    // Update entity name and label
    conn.execute(
        "UPDATE entities SET name = ?1, label = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%S','now') WHERE id = ?3",
        params![username, display_name, id],
    )?;

    // Update password if provided
    if let Some(pw) = password {
        conn.execute(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'password', ?2) \
             ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value",
            params![id, pw],
        )?;
    }

    // Update email
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'email', ?2) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value",
        params![id, email],
    )?;

    // Update role: delete old has_role relation, insert new one
    conn.execute(
        "DELETE FROM relations WHERE source_id = ?1 AND relation_type_id = \
         (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role')",
        params![id],
    )?;
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role'), ?1, ?2)",
        params![id, role_id],
    )?;

    Ok(())
}

/// Delete a user entity (cascades to properties and relations via FK).
pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM entities WHERE id = ?1 AND entity_type = 'user'", params![id])?;
    Ok(())
}

/// Count users that have a specific role via has_role relation.
pub fn count_by_role_id(conn: &Connection, role_id: i64) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM relations \
         WHERE relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
         AND target_id = ?1",
        params![role_id],
        |row| row.get(0),
    )
}

/// Get password hash for a user by id.
pub fn find_password_hash_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = 'password'"
    )?;
    let mut rows = stmt.query_map(params![id], |row| row.get::<_, String>(0))?;
    match rows.next() {
        Some(val) => Ok(Some(val?)),
        None => Ok(None),
    }
}

/// Update only the password property for a user.
pub fn update_password(conn: &Connection, id: i64, password_hash: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'password', ?2) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value",
        params![id, password_hash],
    )?;
    conn.execute(
        "UPDATE entities SET updated_at = strftime('%Y-%m-%dT%H:%M:%S','now') WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}
