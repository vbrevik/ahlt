use std::collections::HashSet;
use sqlx::PgPool;

/// Permission info for the matrix display.
#[derive(sqlx::FromRow)]
pub struct PermissionInfo {
    pub id: i64,
    pub code: String,
    pub label: String,
    pub group_name: String,
    pub description: String,
}

/// Get all permissions with their group_name property, ordered by group then name.
pub async fn find_all_with_groups(pool: &PgPool) -> Result<Vec<PermissionInfo>, sqlx::Error> {
    let rows = sqlx::query_as::<_, PermissionInfo>(
        "SELECT e.id, e.name AS code, e.label, \
                COALESCE(ep.value, 'Other') AS group_name, \
                COALESCE(ed.value, '') AS description \
         FROM entities e \
         LEFT JOIN entity_properties ep ON e.id = ep.entity_id AND ep.key = 'group_name' \
         LEFT JOIN entity_properties ed ON e.id = ed.entity_id AND ed.key = 'description' \
         WHERE e.entity_type = 'permission' AND e.is_active = true \
         ORDER BY group_name, e.name"
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Get all (role_id, permission_id) pairs that have has_permission relations.
pub async fn find_all_role_grants(pool: &PgPool) -> Result<HashSet<(i64, i64)>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (i64, i64)>(
        "SELECT r.source_id, r.target_id \
         FROM relations r \
         WHERE r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission')"
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
}

/// Add a has_permission relation between a role and permission.
pub async fn grant_permission(pool: &PgPool, role_id: i64, permission_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission'), $1, $2) \
         ON CONFLICT DO NOTHING"
    )
    .bind(role_id)
    .bind(permission_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Remove a has_permission relation between a role and permission.
pub async fn revoke_permission(pool: &PgPool, role_id: i64, permission_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM relations WHERE relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission') \
         AND source_id = $1 AND target_id = $2"
    )
    .bind(role_id)
    .bind(permission_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Get all permission codes for a user across ALL assigned roles (multi-role union).
/// Traverses: user --[has_role]--> role --[has_permission]--> permission entities.
/// Returns sorted, deduplicated permission codes.
pub async fn find_codes_by_user_id(pool: &PgPool, user_id: i64) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT DISTINCT perm.name AS code \
         FROM relations r_role \
         JOIN relations r_perm ON r_perm.source_id = r_role.target_id \
         JOIN entities perm ON r_perm.target_id = perm.id \
         WHERE r_role.source_id = $1 \
           AND r_role.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
           AND r_perm.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission') \
           AND perm.entity_type = 'permission' \
         ORDER BY perm.name"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

/// Get all permission codes for a given role entity id.
/// Traverses: role --[has_permission]--> permission entities, returns their names (codes).
pub async fn find_codes_by_role_id(pool: &PgPool, role_id: i64) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT perm.name AS code \
         FROM relations r \
         JOIN entities perm ON r.target_id = perm.id \
         WHERE r.source_id = $1 \
           AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission') \
         ORDER BY perm.name"
    )
    .bind(role_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}
