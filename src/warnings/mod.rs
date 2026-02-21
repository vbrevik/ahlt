pub mod generators;
pub mod queries;
pub mod scheduler;

use sqlx::PgPool;
use chrono::Utc;

use crate::models::{entity, relation};

/// Create a warning entity with properties. Returns the warning entity ID.
pub async fn create_warning(
    pool: &PgPool,
    severity: &str,
    category: &str,
    source_action: &str,
    message: &str,
    details: &str,
    scope: &str,
) -> Result<i64, sqlx::Error> {
    let timestamp = Utc::now().timestamp();
    let name = format!("{}.{}.{}", source_action, category, timestamp);
    let warning_id = entity::create(pool, "warning", &name, message).await?;

    entity::set_properties(pool, warning_id, &[
        ("severity", severity),
        ("category", category),
        ("message", message),
        ("source_action", source_action),
        ("details", details),
        ("status", "active"),
        ("scope", scope),
    ]).await?;

    Ok(warning_id)
}

/// Create receipt entities for each target user. Returns receipt IDs.
pub async fn create_receipts(
    pool: &PgPool,
    warning_id: i64,
    target_user_ids: &[i64],
) -> Result<Vec<i64>, sqlx::Error> {
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let mut receipt_ids = Vec::new();

    for &user_id in target_user_ids {
        let receipt_name = format!("wr.{}.{}", warning_id, user_id);
        let receipt_id = entity::create(pool, "warning_receipt", &receipt_name, "Warning Receipt").await?;

        entity::set_properties(pool, receipt_id, &[
            ("status", "unread"),
            ("status_at", &now),
        ]).await?;

        // Link receipt to warning and user
        relation::create(pool, "for_warning", receipt_id, warning_id).await?;
        relation::create(pool, "for_user", receipt_id, user_id).await?;

        // Link warning to target user
        relation::create(pool, "targets_user", warning_id, user_id).await?;

        // Create "created" event
        create_event(pool, receipt_id, "created", user_id, None).await?;

        receipt_ids.push(receipt_id);
    }

    Ok(receipt_ids)
}

/// Create a warning_event entity on a receipt.
pub async fn create_event(
    pool: &PgPool,
    receipt_id: i64,
    action: &str,
    actor_user_id: i64,
    note: Option<&str>,
) -> Result<i64, sqlx::Error> {
    let timestamp = Utc::now().timestamp();
    let event_name = format!("we.{}.{}.{}", receipt_id, action, timestamp);
    let event_id = entity::create(pool, "warning_event", &event_name, action).await?;

    entity::set_properties(pool, event_id, &[
        ("action", action),
        ("actor_user_id", &actor_user_id.to_string()),
    ]).await?;

    if let Some(n) = note {
        entity::set_property(pool, event_id, "note", n).await?;
    }

    relation::create(pool, "on_receipt", event_id, receipt_id).await?;

    Ok(event_id)
}

/// Check if an active warning already exists (deduplication).
pub async fn warning_exists(pool: &PgPool, source_action: &str, dedup_key: &str) -> bool {
    let result: Result<(i64,), _> = sqlx::query_as(
        "SELECT COUNT(*) FROM entities e
         JOIN entity_properties st ON st.entity_id = e.id AND st.key = 'status'
         JOIN entity_properties sa ON sa.entity_id = e.id AND sa.key = 'source_action'
         JOIN entity_properties det ON det.entity_id = e.id AND det.key = 'details'
         WHERE e.entity_type = 'warning'
           AND st.value = 'active'
           AND sa.value = $1
           AND det.value LIKE $2",
    )
    .bind(source_action)
    .bind(format!("%{}%", dedup_key))
    .fetch_one(pool)
    .await;
    result.map(|r| r.0 > 0).unwrap_or(false)
}

/// Get all user IDs that have a specific permission code.
pub async fn get_users_with_permission(pool: &PgPool, permission_code: &str) -> Result<Vec<i64>, sqlx::Error> {
    let rows: Vec<(i64,)> = sqlx::query_as(
        "SELECT DISTINCT u.id
         FROM entities u
         JOIN relations ur ON ur.source_id = u.id
         JOIN entities rt_role ON rt_role.id = ur.relation_type_id AND rt_role.name = 'has_role'
         JOIN relations rp ON rp.source_id = ur.target_id
         JOIN entities rt_perm ON rt_perm.id = rp.relation_type_id AND rt_perm.name = 'has_permission'
         JOIN entities perm ON perm.id = rp.target_id AND perm.name = $1
         WHERE u.entity_type = 'user' AND u.is_active = true",
    )
    .bind(permission_code)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

/// Update a receipt's status and create corresponding event.
pub async fn update_receipt_status(
    pool: &PgPool,
    receipt_id: i64,
    new_status: &str,
    actor_user_id: i64,
) -> Result<(), sqlx::Error> {
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    entity::set_properties(pool, receipt_id, &[
        ("status", new_status),
        ("status_at", &now),
    ]).await?;
    create_event(pool, receipt_id, new_status, actor_user_id, None).await?;
    Ok(())
}

/// Resolve a warning: set status to resolved, update all receipts.
pub async fn resolve_warning(pool: &PgPool, warning_id: i64, actor_user_id: i64) -> Result<(), sqlx::Error> {
    entity::set_property(pool, warning_id, "status", "resolved").await?;

    let receipt_ids = get_receipt_ids_for_warning(pool, warning_id).await?;
    for receipt_id in receipt_ids {
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        entity::set_properties(pool, receipt_id, &[
            ("status", "resolved"),
            ("status_at", &now),
        ]).await?;
        create_event(pool, receipt_id, "resolved", actor_user_id, None).await?;
    }
    Ok(())
}

/// Get all receipt entity IDs for a warning.
async fn get_receipt_ids_for_warning(pool: &PgPool, warning_id: i64) -> Result<Vec<i64>, sqlx::Error> {
    let rows: Vec<(i64,)> = sqlx::query_as(
        "SELECT r.source_id FROM relations r
         JOIN entities rt ON rt.id = r.relation_type_id AND rt.name = 'for_warning'
         WHERE r.target_id = $1",
    )
    .bind(warning_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}
