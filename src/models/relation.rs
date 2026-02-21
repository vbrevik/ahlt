#![allow(dead_code)]
use sqlx::PgPool;
use super::entity::Entity;

/// Find all target entities related to source via a named relation type.
/// e.g. find_targets(pool, user_id, "has_role") → [role entity]
pub async fn find_targets(pool: &PgPool, source_id: i64, relation_type_name: &str) -> Result<Vec<Entity>, sqlx::Error> {
    sqlx::query_as::<_, Entity>(
        "SELECT t.id, t.entity_type, t.name, t.label, t.sort_order::BIGINT as sort_order, t.is_active, \
         t.created_at::TEXT, t.updated_at::TEXT \
         FROM relations r \
         JOIN entities t ON r.target_id = t.id \
         WHERE r.source_id = $1 \
           AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = $2) \
         ORDER BY t.sort_order, t.id",
    )
    .bind(source_id)
    .bind(relation_type_name)
    .fetch_all(pool)
    .await
}

/// Find all source entities related to target via a named relation type.
/// e.g. find_sources(pool, role_id, "has_role") → [user entities with that role]
pub async fn find_sources(pool: &PgPool, target_id: i64, relation_type_name: &str) -> Result<Vec<Entity>, sqlx::Error> {
    sqlx::query_as::<_, Entity>(
        "SELECT s.id, s.entity_type, s.name, s.label, s.sort_order::BIGINT as sort_order, s.is_active, \
         s.created_at::TEXT, s.updated_at::TEXT \
         FROM relations r \
         JOIN entities s ON r.source_id = s.id \
         WHERE r.target_id = $1 \
           AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = $2) \
         ORDER BY s.sort_order, s.id",
    )
    .bind(target_id)
    .bind(relation_type_name)
    .fetch_all(pool)
    .await
}

/// Create a relation between two entities.
pub async fn create(pool: &PgPool, relation_type_name: &str, source_id: i64, target_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = $1), $2, $3) \
         ON CONFLICT DO NOTHING",
    )
    .bind(relation_type_name)
    .bind(source_id)
    .bind(target_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Delete a specific relation.
pub async fn delete(pool: &PgPool, relation_type_name: &str, source_id: i64, target_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM relations WHERE relation_type_id = \
         (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = $1) \
         AND source_id = $2 AND target_id = $3",
    )
    .bind(relation_type_name)
    .bind(source_id)
    .bind(target_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Delete all relations of a given type from a source entity.
/// e.g. delete_all_from_source(pool, user_id, "has_role") removes all role assignments.
pub async fn delete_all_from_source(pool: &PgPool, source_id: i64, relation_type_name: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM relations WHERE source_id = $1 AND relation_type_id = \
         (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = $2)",
    )
    .bind(source_id)
    .bind(relation_type_name)
    .execute(pool)
    .await?;
    Ok(())
}
