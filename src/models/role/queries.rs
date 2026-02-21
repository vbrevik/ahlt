use sqlx::PgPool;
use super::types::{RoleDisplay, RoleListItem, RoleDetail, PermissionCheckbox, RoleMember};

/// Find all roles for display (dropdowns, lists).
pub async fn find_all_display(pool: &PgPool) -> Result<Vec<RoleDisplay>, sqlx::Error> {
    let roles = sqlx::query_as::<_, RoleDisplay>(
        "SELECT id, name, label FROM entities WHERE entity_type = 'role' ORDER BY sort_order, id"
    )
    .fetch_all(pool)
    .await?;
    Ok(roles)
}

/// Find all roles with user count and permission count for the list page.
pub async fn find_all_list_items(pool: &PgPool) -> Result<Vec<RoleListItem>, sqlx::Error> {
    let roles = sqlx::query_as::<_, RoleListItem>(
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
    )
    .fetch_all(pool)
    .await?;
    Ok(roles)
}

/// Find a role entity by id.
pub async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<RoleDisplay>, sqlx::Error> {
    let role = sqlx::query_as::<_, RoleDisplay>(
        "SELECT id, name, label FROM entities WHERE id = $1 AND entity_type = 'role'"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(role)
}

/// Find a role with its description for editing.
pub async fn find_detail_by_id(pool: &PgPool, id: i64) -> Result<Option<RoleDetail>, sqlx::Error> {
    let role = sqlx::query_as::<_, RoleDetail>(
        "SELECT e.id, e.name, e.label, COALESCE(p_desc.value, '') AS description \
         FROM entities e \
         LEFT JOIN entity_properties p_desc ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
         WHERE e.id = $1 AND e.entity_type = 'role'"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(role)
}

/// Helper struct for reading permission checkbox rows from the DB.
/// The `checked` column comes as an integer (0 or 1) from the CASE expression.
#[derive(sqlx::FromRow)]
struct PermissionCheckboxRow {
    id: i64,
    code: String,
    label: String,
    group_name: String,
    checked: i32,
}

/// Get all permissions as checkboxes, with `checked` set for those assigned to the given role.
pub async fn find_permission_checkboxes(pool: &PgPool, role_id: i64) -> Result<Vec<PermissionCheckbox>, sqlx::Error> {
    let rows = sqlx::query_as::<_, PermissionCheckboxRow>(
        "SELECT p.id, p.name AS code, p.label, \
                COALESCE(pg.value, '') AS group_name, \
                CASE WHEN r.id IS NOT NULL THEN 1 ELSE 0 END AS checked \
         FROM entities p \
         LEFT JOIN entity_properties pg ON p.id = pg.entity_id AND pg.key = 'group_name' \
         LEFT JOIN relations r ON r.source_id = $1 AND r.target_id = p.id \
             AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission') \
         WHERE p.entity_type = 'permission' \
         ORDER BY group_name, p.name"
    )
    .bind(role_id)
    .fetch_all(pool)
    .await?;

    let perms = rows.into_iter().map(|row| PermissionCheckbox {
        id: row.id,
        code: row.code,
        label: row.label,
        group_name: row.group_name,
        checked: row.checked == 1,
    }).collect();

    Ok(perms)
}

/// Create a new role entity with description and permission relations.
pub async fn create(pool: &PgPool, name: &str, label: &str, description: &str, permission_ids: &[i64]) -> Result<i64, sqlx::Error> {
    let (role_id,) = sqlx::query_as::<_, (i64,)>(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', $1, $2) RETURNING id"
    )
    .bind(name)
    .bind(label)
    .fetch_one(pool)
    .await?;

    if !description.is_empty() {
        sqlx::query(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'description', $2)"
        )
        .bind(role_id)
        .bind(description)
        .execute(pool)
        .await?;
    }

    let has_perm_id = get_has_permission_id(pool).await?;
    for perm_id in permission_ids {
        sqlx::query(
            "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES ($1, $2, $3)"
        )
        .bind(has_perm_id)
        .bind(role_id)
        .bind(perm_id)
        .execute(pool)
        .await?;
    }

    Ok(role_id)
}

/// Update a role's name, label, description, and permission relations.
pub async fn update(pool: &PgPool, id: i64, name: &str, label: &str, description: &str, permission_ids: &[i64]) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE entities SET name = $1, label = $2, updated_at = NOW() WHERE id = $3"
    )
    .bind(name)
    .bind(label)
    .bind(id)
    .execute(pool)
    .await?;

    // Upsert description
    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'description', $2) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value"
    )
    .bind(id)
    .bind(description)
    .execute(pool)
    .await?;

    // Replace permission relations: delete all, re-insert selected
    let has_perm_id = get_has_permission_id(pool).await?;
    sqlx::query(
        "DELETE FROM relations WHERE source_id = $1 AND relation_type_id = $2"
    )
    .bind(id)
    .bind(has_perm_id)
    .execute(pool)
    .await?;

    for perm_id in permission_ids {
        sqlx::query(
            "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES ($1, $2, $3)"
        )
        .bind(has_perm_id)
        .bind(id)
        .bind(perm_id)
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Delete a role entity (cascades to properties and relations via FK).
pub async fn delete(pool: &PgPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM entities WHERE id = $1 AND entity_type = 'role'")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Count users assigned to a role.
pub async fn count_users(pool: &PgPool, role_id: i64) -> Result<i64, sqlx::Error> {
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

/// Find all users assigned to a specific role.
pub async fn find_users_by_role(pool: &PgPool, role_id: i64) -> Result<Vec<RoleMember>, sqlx::Error> {
    let members = sqlx::query_as::<_, RoleMember>(
        "SELECT e.id AS user_id, e.name AS username, e.label AS display_name \
         FROM entities e \
         JOIN relations r ON r.source_id = e.id AND r.target_id = $1 \
           AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
         WHERE e.entity_type = 'user' \
         ORDER BY e.label, e.name"
    )
    .bind(role_id)
    .fetch_all(pool)
    .await?;
    Ok(members)
}

/// Find users NOT assigned to a specific role (for "Add User" dropdown).
pub async fn find_users_not_in_role(pool: &PgPool, role_id: i64) -> Result<Vec<RoleMember>, sqlx::Error> {
    let members = sqlx::query_as::<_, RoleMember>(
        "SELECT e.id AS user_id, e.name AS username, e.label AS display_name \
         FROM entities e \
         WHERE e.entity_type = 'user' \
           AND e.id NOT IN ( \
               SELECT r.source_id FROM relations r \
               WHERE r.target_id = $1 \
                 AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
           ) \
         ORDER BY e.label, e.name"
    )
    .bind(role_id)
    .fetch_all(pool)
    .await?;
    Ok(members)
}

/// Helper to get the has_permission relation type id.
async fn get_has_permission_id(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let (id,) = sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission'"
    )
    .fetch_one(pool)
    .await?;
    Ok(id)
}
