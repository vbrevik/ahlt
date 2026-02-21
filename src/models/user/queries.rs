use sqlx::PgPool;
use super::types::{User, UserDisplay, UserPage, NewUser, UserWithRoles};

/// SQL for user display: entity + email property + roles via has_role relation.
/// Uses STRING_AGG to collect multiple roles into comma-separated strings.
const SELECT_USER_DISPLAY: &str = "\
    SELECT e.id, e.name AS username, e.label AS display_name, \
           COALESCE(p_email.value, '') AS email, \
           COALESCE(STRING_AGG(DISTINCT role_e.id::TEXT, ','), '') AS role_ids, \
           COALESCE(STRING_AGG(DISTINCT role_e.name, ','), '') AS role_names, \
           COALESCE(STRING_AGG(DISTINCT role_e.label, ','), '') AS role_labels, \
           e.created_at::TEXT AS created_at, e.updated_at::TEXT AS updated_at \
    FROM entities e \
    LEFT JOIN entity_properties p_email \
        ON e.id = p_email.entity_id AND p_email.key = 'email' \
    LEFT JOIN relations r_role \
        ON r_role.source_id = e.id \
        AND r_role.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
    LEFT JOIN entities role_e ON r_role.target_id = role_e.id \
    WHERE e.entity_type = 'user'";

/// Find users with pagination, filter, and sort support.
pub async fn find_paginated(
    pool: &PgPool,
    page: i64,
    per_page: i64,
    filter: &crate::models::table_filter::FilterTree,
    sort: &crate::models::table_filter::SortSpec,
) -> Result<UserPage, sqlx::Error> {
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

    // Count query needs JOINs for filter fields that reference joined tables.
    // Use COUNT(DISTINCT e.id) to avoid inflated counts from multi-role JOINs.
    let count_sql = format!(
        "SELECT COUNT(DISTINCT e.id) FROM entities e \
         LEFT JOIN entity_properties p_email ON e.id = p_email.entity_id AND p_email.key = 'email' \
         LEFT JOIN relations r_role ON r_role.source_id = e.id AND r_role.relation_type_id = \
             (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
         LEFT JOIN entities role_e ON r_role.target_id = role_e.id \
         WHERE e.entity_type = 'user' AND ({where_clause})"
    );

    let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
    for p in &filter_params {
        count_query = count_query.bind(p);
    }
    let total_count: i64 = count_query.fetch_one(pool).await?;

    // Data query
    let n = filter_params.len();
    let data_sql = format!(
        "{SELECT_USER_DISPLAY} AND ({where_clause}) \
         GROUP BY e.id \
         ORDER BY {sort_col} {sort_dir} \
         LIMIT ${} OFFSET ${}",
        n + 1, n + 2
    );

    let mut data_query = sqlx::query_as::<_, UserDisplay>(&data_sql);
    for p in &filter_params {
        data_query = data_query.bind(p);
    }
    data_query = data_query.bind(per_page).bind(offset);
    let users: Vec<UserDisplay> = data_query.fetch_all(pool).await?;

    let total_pages = ((total_count as f64) / (per_page as f64)).ceil() as i64;

    Ok(UserPage { users, page, per_page, total_count, total_pages })
}

/// Return all users matching the filter (no pagination) — used for CSV export.
pub async fn find_all_filtered(
    pool: &PgPool,
    filter: &crate::models::table_filter::FilterTree,
    sort: &crate::models::table_filter::SortSpec,
) -> Result<Vec<UserDisplay>, sqlx::Error> {
    use crate::models::table_filter::{builder, SortDir};
    use crate::models::user::filter as uf;

    let (where_clause, filter_params) = builder::build_where_clause(
        filter, &uf::field_map(), uf::OPS, 0,
    ).unwrap_or_else(|_| ("1=1".to_string(), vec![]));

    let sort_col = uf::sort_col(&sort.column);
    let sort_dir = match sort.dir { SortDir::Asc => "ASC", SortDir::Desc => "DESC" };

    let sql = format!(
        "{SELECT_USER_DISPLAY} AND ({where_clause}) GROUP BY e.id ORDER BY {sort_col} {sort_dir}"
    );

    let mut query = sqlx::query_as::<_, UserDisplay>(&sql);
    for p in &filter_params {
        query = query.bind(p);
    }
    let users: Vec<UserDisplay> = query.fetch_all(pool).await?;

    Ok(users)
}

pub async fn find_display_by_id(pool: &PgPool, id: i64) -> Result<Option<UserDisplay>, sqlx::Error> {
    let sql = format!("{SELECT_USER_DISPLAY} AND e.id = $1 GROUP BY e.id");
    let user = sqlx::query_as::<_, UserDisplay>(&sql)
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(user)
}

/// Find user by username for authentication. Returns internal User with password hash.
pub async fn find_by_username(pool: &PgPool, username: &str) -> Result<Option<User>, sqlx::Error> {
    let user = sqlx::query_as::<_, User>(
        "SELECT e.id, e.name AS username, e.label AS display_name, \
                COALESCE(p_pw.value, '') AS password, \
                COALESCE(p_email.value, '') AS email, \
                COALESCE(role_e.id, 0) AS role_id, \
                e.created_at::TEXT AS created_at, e.updated_at::TEXT AS updated_at \
         FROM entities e \
         LEFT JOIN entity_properties p_pw ON e.id = p_pw.entity_id AND p_pw.key = 'password' \
         LEFT JOIN entity_properties p_email ON e.id = p_email.entity_id AND p_email.key = 'email' \
         LEFT JOIN relations r_role \
             ON r_role.source_id = e.id \
             AND r_role.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
         LEFT JOIN entities role_e ON r_role.target_id = role_e.id \
         WHERE e.entity_type = 'user' AND e.name = $1"
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;
    Ok(user)
}

/// Count user entities.
pub async fn count(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let (count,) = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM entities WHERE entity_type = 'user'"
    )
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Create a new user entity with properties (no role assignment).
pub async fn create(pool: &PgPool, new: &NewUser) -> Result<i64, sqlx::Error> {
    // Insert user entity with RETURNING id
    let (user_id,) = sqlx::query_as::<_, (i64,)>(
        "INSERT INTO entities (entity_type, name, label) VALUES ('user', $1, $2) RETURNING id"
    )
    .bind(&new.username)
    .bind(&new.display_name)
    .fetch_one(pool)
    .await?;

    // Set properties
    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'password', $2)"
    )
    .bind(user_id)
    .bind(&new.password)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'email', $2)"
    )
    .bind(user_id)
    .bind(&new.email)
    .execute(pool)
    .await?;

    Ok(user_id)
}

/// Update a user entity: name, label (display_name), and properties. Does not touch roles.
pub async fn update(
    pool: &PgPool,
    id: i64,
    username: &str,
    password: Option<&str>,
    email: &str,
    display_name: &str,
) -> Result<(), sqlx::Error> {
    // Update entity name and label
    sqlx::query(
        "UPDATE entities SET name = $1, label = $2, updated_at = NOW() WHERE id = $3"
    )
    .bind(username)
    .bind(display_name)
    .bind(id)
    .execute(pool)
    .await?;

    // Update password if provided
    if let Some(pw) = password {
        sqlx::query(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'password', $2) \
             ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value"
        )
        .bind(id)
        .bind(pw)
        .execute(pool)
        .await?;
    }

    // Update email
    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'email', $2) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value"
    )
    .bind(id)
    .bind(email)
    .execute(pool)
    .await?;

    Ok(())
}

/// Assign the default "viewer" role to a user. No-op if viewer role doesn't exist.
pub async fn assign_default_role(pool: &PgPool, user_id: i64) -> Result<(), sqlx::Error> {
    let viewer_id: Option<(i64,)> = sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM entities WHERE entity_type = 'role' AND name = 'viewer'"
    )
    .fetch_optional(pool)
    .await?;

    if let Some((role_id,)) = viewer_id {
        sqlx::query(
            "INSERT INTO relations (relation_type_id, source_id, target_id) \
             VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role'), $1, $2) \
             ON CONFLICT DO NOTHING"
        )
        .bind(user_id)
        .bind(role_id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Delete a user entity (cascades to properties and relations via FK).
pub async fn delete(pool: &PgPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM entities WHERE id = $1 AND entity_type = 'user'")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Count users that have a specific role via has_role relation.
pub async fn count_by_role_id(pool: &PgPool, role_id: i64) -> Result<i64, sqlx::Error> {
    let (count,) = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM relations \
         WHERE relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
         AND target_id = $1"
    )
    .bind(role_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

/// Get password hash for a user by id.
pub async fn find_password_hash_by_id(pool: &PgPool, id: i64) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as::<_, (String,)>(
        "SELECT value FROM entity_properties WHERE entity_id = $1 AND key = 'password'"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(v,)| v))
}

/// Find all users with their assigned roles (for assignment page "By User" tab).
pub async fn find_all_with_roles(pool: &PgPool) -> Result<Vec<UserWithRoles>, sqlx::Error> {
    // First get all users
    let users: Vec<(i64, String, String)> = sqlx::query_as::<_, (i64, String, String)>(
        "SELECT id, name AS username, label AS display_name \
         FROM entities WHERE entity_type = 'user' \
         ORDER BY label, name"
    )
    .fetch_all(pool)
    .await?;

    // Then get all user-role assignments
    let assignments: Vec<(i64, i64, String, String)> = sqlx::query_as::<_, (i64, i64, String, String)>(
        "SELECT r.source_id AS user_id, role_e.id AS role_id, role_e.name, role_e.label \
         FROM relations r \
         JOIN entities role_e ON r.target_id = role_e.id AND role_e.entity_type = 'role' \
         WHERE r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
         ORDER BY role_e.label"
    )
    .fetch_all(pool)
    .await?;

    // Group assignments by user
    let result: Vec<UserWithRoles> = users.into_iter().map(|(id, username, display_name)| {
        let roles: Vec<(i64, String, String)> = assignments.iter()
            .filter(|(uid, _, _, _)| *uid == id)
            .map(|(_, rid, name, label)| (*rid, name.clone(), label.clone()))
            .collect();
        UserWithRoles { id, username, display_name, roles }
    }).collect();

    Ok(result)
}

/// Update only the password property for a user.
pub async fn update_password(pool: &PgPool, id: i64, password_hash: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'password', $2) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value"
    )
    .bind(id)
    .bind(password_hash)
    .execute(pool)
    .await?;

    sqlx::query(
        "UPDATE entities SET updated_at = NOW() WHERE id = $1"
    )
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}
