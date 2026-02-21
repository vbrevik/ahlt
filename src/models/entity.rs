#![allow(dead_code)]
use sqlx::PgPool;
use std::collections::HashMap;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Entity {
    pub id: i64,
    pub entity_type: String,
    pub name: String,
    pub label: String,
    pub sort_order: i64,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Find all entities of a given type.
pub async fn find_by_type(pool: &PgPool, entity_type: &str) -> Result<Vec<Entity>, sqlx::Error> {
    sqlx::query_as::<_, Entity>(
        "SELECT id, entity_type, name, label, sort_order::BIGINT as sort_order, is_active, \
         created_at::TEXT, updated_at::TEXT \
         FROM entities WHERE entity_type = $1 ORDER BY sort_order, id",
    )
    .bind(entity_type)
    .fetch_all(pool)
    .await
}

/// Find a single entity by type and name.
pub async fn find_by_type_and_name(pool: &PgPool, entity_type: &str, name: &str) -> Result<Option<Entity>, sqlx::Error> {
    sqlx::query_as::<_, Entity>(
        "SELECT id, entity_type, name, label, sort_order::BIGINT as sort_order, is_active, \
         created_at::TEXT, updated_at::TEXT \
         FROM entities WHERE entity_type = $1 AND name = $2",
    )
    .bind(entity_type)
    .bind(name)
    .fetch_optional(pool)
    .await
}

/// Find a single entity by id.
pub async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<Entity>, sqlx::Error> {
    sqlx::query_as::<_, Entity>(
        "SELECT id, entity_type, name, label, sort_order::BIGINT as sort_order, is_active, \
         created_at::TEXT, updated_at::TEXT \
         FROM entities WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Create a new entity, returning its id.
pub async fn create(pool: &PgPool, entity_type: &str, name: &str, label: &str) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO entities (entity_type, name, label) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(entity_type)
    .bind(name)
    .bind(label)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Create a new entity with sort_order, returning its id.
pub async fn create_with_sort(pool: &PgPool, entity_type: &str, name: &str, label: &str, sort_order: i64) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO entities (entity_type, name, label, sort_order) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(entity_type)
    .bind(name)
    .bind(label)
    .bind(sort_order as i32)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Update an entity's name and label.
pub async fn update(pool: &PgPool, id: i64, name: &str, label: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE entities SET name = $1, label = $2, updated_at = NOW() WHERE id = $3",
    )
    .bind(name)
    .bind(label)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Delete an entity (cascades to properties and relations).
pub async fn delete(pool: &PgPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM entities WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Count entities of a given type.
pub async fn count_by_type(pool: &PgPool, entity_type: &str) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM entities WHERE entity_type = $1",
    )
    .bind(entity_type)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

// --- Property helpers ---

/// Get a single property value for an entity.
pub async fn get_property(pool: &PgPool, entity_id: i64, key: &str) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM entity_properties WHERE entity_id = $1 AND key = $2",
    )
    .bind(entity_id)
    .bind(key)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}

/// Get all properties for an entity as a HashMap.
pub async fn get_properties(pool: &PgPool, entity_id: i64) -> Result<HashMap<String, String>, sqlx::Error> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT key, value FROM entity_properties WHERE entity_id = $1",
    )
    .bind(entity_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
}

/// Set a property (upsert).
pub async fn set_property(pool: &PgPool, entity_id: i64, key: &str, value: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, $2, $3) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(entity_id)
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

/// Delete a property.
pub async fn delete_property(pool: &PgPool, entity_id: i64, key: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM entity_properties WHERE entity_id = $1 AND key = $2",
    )
    .bind(entity_id)
    .bind(key)
    .execute(pool)
    .await?;
    Ok(())
}

/// Set multiple properties at once.
pub async fn set_properties(pool: &PgPool, entity_id: i64, props: &[(&str, &str)]) -> Result<(), sqlx::Error> {
    for (key, value) in props {
        set_property(pool, entity_id, key, value).await?;
    }
    Ok(())
}
