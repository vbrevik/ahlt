// src/models/table_filter/columns.rs
use sqlx::PgPool;
use super::ColumnDef;

/// Read the user's per-table column preference from entity_properties.
/// key: "pref.{table}_table_columns"
async fn read_user_pref(user_id: i64, table: &str, pool: &PgPool) -> Option<String> {
    let key = format!("pref.{table}_table_columns");
    let result: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM entity_properties WHERE entity_id = $1 AND key = $2",
    )
    .bind(user_id)
    .bind(&key)
    .fetch_optional(pool)
    .await
    .ok()?;
    result.map(|r| r.0)
}

/// Read the global default from a setting entity.
/// setting name: "{table}_table_columns"
async fn read_global_default(table: &str, pool: &PgPool) -> Option<String> {
    let name = format!("{table}_table_columns");
    let result: Option<(String,)> = sqlx::query_as(
        "SELECT COALESCE(p.value, '') FROM entities e \
         JOIN entity_properties p ON e.id = p.entity_id AND p.key = 'value' \
         WHERE e.entity_type = 'setting' AND e.name = $1",
    )
    .bind(&name)
    .fetch_optional(pool)
    .await
    .ok()?;
    result.map(|r| r.0).filter(|s| !s.is_empty())
}

/// Resolve the ordered column list for a table.
/// all_columns: the full ordered default list for the table.
/// Resolution: user pref > global default > all_columns order (visible = always_visible || default).
pub async fn resolve_columns(
    table: &str,
    user_id: i64,
    pool: &PgPool,
    all_columns: &[ColumnDef],
) -> Vec<ColumnDef> {
    let source = match read_user_pref(user_id, table, pool).await {
        Some(pref) => Some(pref),
        None => read_global_default(table, pool).await,
    };

    match source {
        Some(pref) => apply_pref(all_columns, &pref),
        None => all_columns.to_vec(),
    }
}

/// Apply a comma-separated ordered column string to the full column list.
/// Columns in the string appear first (in order), rest appended hidden at end.
fn apply_pref(all_columns: &[ColumnDef], pref: &str) -> Vec<ColumnDef> {
    let ordered_keys: Vec<&str> = pref.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
    let mut result: Vec<ColumnDef> = vec![];

    // First: columns in pref order, visible if present
    for key in &ordered_keys {
        if let Some(col) = all_columns.iter().find(|c| c.key.as_str() == *key) {
            let mut c = col.clone();
            c.visible = true;
            result.push(c);
        }
    }

    // Then: any always_visible columns not yet in result
    for col in all_columns {
        if col.always_visible && !result.iter().any(|c| c.key == col.key) {
            let mut c = col.clone();
            c.visible = true;
            result.push(c);
        }
    }

    // Then: remaining columns hidden
    for col in all_columns {
        if !result.iter().any(|c| c.key == col.key) {
            let mut c = col.clone();
            c.visible = false;
            result.push(c);
        }
    }

    result
}

/// Serialize a Vec<ColumnDef> to a pref string (only visible columns, in order).
pub fn columns_to_pref(columns: &[ColumnDef]) -> String {
    columns.iter()
        .filter(|c| c.visible || c.always_visible)
        .map(|c| c.key.as_str())
        .collect::<Vec<_>>()
        .join(",")
}

/// Save per-user column preference.
pub async fn save_user_columns(user_id: i64, table: &str, pref: &str, pool: &PgPool) -> Result<(), sqlx::Error> {
    let key = format!("pref.{table}_table_columns");
    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, $2, $3) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value",
    )
    .bind(user_id)
    .bind(&key)
    .bind(pref)
    .execute(pool)
    .await?;
    Ok(())
}

/// Save global column default (updates existing setting entity value property).
pub async fn save_global_columns(table: &str, pref: &str, pool: &PgPool) -> Result<(), sqlx::Error> {
    let name = format!("{table}_table_columns");
    // Upsert: update if exists, insert if not
    let result = sqlx::query(
        "UPDATE entity_properties SET value = $1 \
         WHERE entity_id = (SELECT id FROM entities WHERE entity_type = 'setting' AND name = $2) \
         AND key = 'value'",
    )
    .bind(pref)
    .bind(&name)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        // Create setting entity + property
        sqlx::query(
            "INSERT INTO entities (entity_type, name, label) VALUES ('setting', $1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(&name)
        .bind(&format!("{table} table columns"))
        .execute(pool)
        .await?;
        let setting_row: (i64,) = sqlx::query_as(
            "SELECT id FROM entities WHERE entity_type = 'setting' AND name = $1",
        )
        .bind(&name)
        .fetch_one(pool)
        .await?;
        sqlx::query(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'value', $2) \
             ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value",
        )
        .bind(setting_row.0)
        .bind(pref)
        .execute(pool)
        .await?;
    }
    Ok(())
}
