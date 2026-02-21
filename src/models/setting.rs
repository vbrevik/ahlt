use sqlx::PgPool;

/// A setting for display and editing.
#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct SettingDisplay {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub value: String,
    pub description: String,
    pub setting_type: String, // "text", "number", "boolean"
}

/// Find all active settings, ordered by sort_order.
pub async fn find_all(pool: &PgPool) -> Result<Vec<SettingDisplay>, sqlx::Error> {
    let settings = sqlx::query_as::<_, SettingDisplay>(
        "SELECT e.id, e.name, e.label, \
                COALESCE(p_val.value, '') AS value, \
                COALESCE(p_desc.value, '') AS description, \
                COALESCE(p_type.value, 'text') AS setting_type \
         FROM entities e \
         LEFT JOIN entity_properties p_val ON e.id = p_val.entity_id AND p_val.key = 'value' \
         LEFT JOIN entity_properties p_desc ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
         LEFT JOIN entity_properties p_type ON e.id = p_type.entity_id AND p_type.key = 'setting_type' \
         WHERE e.entity_type = 'setting' AND e.is_active = true \
         ORDER BY e.sort_order, e.id"
    )
    .fetch_all(pool)
    .await?;
    Ok(settings)
}

/// Get a single setting's value by name, returning a default if not found.
pub async fn get_value(pool: &PgPool, name: &str, default: &str) -> String {
    let result = sqlx::query_as::<_, (String,)>(
        "SELECT COALESCE(p.value, $2) \
         FROM entities e \
         LEFT JOIN entity_properties p ON e.id = p.entity_id AND p.key = 'value' \
         WHERE e.entity_type = 'setting' AND e.name = $1"
    )
    .bind(name)
    .bind(default)
    .fetch_one(pool)
    .await;
    result.map(|r| r.0).unwrap_or_else(|_| default.to_string())
}

/// Update a single setting's value by entity id (upsert on entity_properties).
pub async fn update_value(pool: &PgPool, id: i64, value: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'value', $2) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value"
    )
    .bind(id)
    .bind(value)
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
