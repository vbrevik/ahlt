use sqlx::PgPool;

use crate::handlers::warning_handlers::ws::ConnectionMap;

/// Check for users without a role assignment.
pub async fn check_users_without_role(pool: &PgPool, conn_map: &ConnectionMap) {
    let user_ids: Vec<i64> = match sqlx::query_as::<_, (i64,)>(
        "SELECT e.id FROM entities e
         WHERE e.entity_type = 'user'
           AND NOT EXISTS (
               SELECT 1 FROM relations r
               JOIN entities rt ON rt.id = r.relation_type_id AND rt.name = 'has_role'
               WHERE r.source_id = e.id
           )"
    )
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows.into_iter().map(|r| r.0).collect(),
        Err(e) => {
            log::error!("Generator check_users_without_role query failed: {}", e);
            return;
        }
    };

    if user_ids.is_empty() {
        return;
    }

    let source_action = "scheduled.users_without_role";
    if super::warning_exists(pool, source_action, "users_without_role").await {
        return;
    }

    let message = format!("{} user(s) have no role assigned", user_ids.len());
    let details = serde_json::json!({ "dedup": "users_without_role", "user_ids": user_ids }).to_string();

    let warning_id = match super::create_warning(
        pool, "medium", "data_integrity", source_action,
        &message, &details, "system",
    ).await {
        Ok(id) => id,
        Err(e) => {
            log::error!("Failed to create users_without_role warning: {}", e);
            return;
        }
    };

    // Target all admin users
    let admin_ids = super::get_users_with_permission(pool, "admin.settings")
        .await
        .unwrap_or_default();
    if admin_ids.is_empty() {
        return;
    }

    if let Ok(receipt_ids) = super::create_receipts(pool, warning_id, &admin_ids).await {
        let _ = receipt_ids; // receipts created
        crate::handlers::warning_handlers::ws::notify_users(
            conn_map, pool, &admin_ids, warning_id, "medium", &message,
        );
    }
}

/// Check database size against threshold using pg_database_size().
pub async fn check_database_size(pool: &PgPool, conn_map: &ConnectionMap) {
    let size_bytes: i64 = match sqlx::query_as::<_, (i64,)>(
        "SELECT pg_database_size(current_database())",
    )
    .fetch_one(pool)
    .await
    {
        Ok(row) => row.0,
        Err(e) => {
            log::error!("Failed to check database size: {}", e);
            return;
        }
    };

    let threshold_mb: i64 = 500; // 500 MB
    let size_mb = size_bytes / (1024 * 1024);

    if size_mb < threshold_mb {
        return;
    }

    let source_action = "scheduled.database_size";
    if super::warning_exists(pool, source_action, "database_size").await {
        return;
    }

    let message = format!("Database size is {} MB (threshold: {} MB)", size_mb, threshold_mb);
    let severity = if size_mb > threshold_mb * 2 { "high" } else { "medium" };

    let details = serde_json::json!({ "dedup": "database_size", "size_mb": size_mb }).to_string();
    let warning_id = match super::create_warning(
        pool, severity, "system", source_action,
        &message, &details, "system",
    ).await {
        Ok(id) => id,
        Err(e) => {
            log::error!("Failed to create database_size warning: {}", e);
            return;
        }
    };

    let admin_ids = super::get_users_with_permission(pool, "admin.settings")
        .await
        .unwrap_or_default();
    if admin_ids.is_empty() {
        return;
    }

    if super::create_receipts(pool, warning_id, &admin_ids).await.is_ok() {
        crate::handlers::warning_handlers::ws::notify_users(
            conn_map, pool, &admin_ids, warning_id, severity, &message,
        );
    }
}

/// Check for vacant mandatory positions in active ToRs.
/// Creates one warning per ToR with unfilled mandatory positions.
/// Auto-resolves warnings when vacancies are filled.
pub async fn check_tor_vacancies(pool: &PgPool, conn_map: &ConnectionMap) {
    // Find all active ToRs with vacant mandatory positions
    let rows: Vec<(i64, String, i64, String)> = match sqlx::query_as::<_, (i64, String, i64, String)>(
        "SELECT t.id AS tor_id, t.label AS tor_label,
                f.id AS position_id, f.label AS position_label
         FROM entities t
         JOIN entity_properties tp ON tp.entity_id = t.id AND tp.key = 'status' AND tp.value = 'active'
         JOIN relations r_bt ON r_bt.target_id = t.id
             AND r_bt.relation_type_id = (
                 SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor')
         JOIN entities f ON f.id = r_bt.source_id AND f.entity_type = 'tor_function'
         JOIN entity_properties fp ON fp.entity_id = f.id AND fp.key = 'membership_type' AND fp.value = 'mandatory'
         WHERE t.entity_type = 'tor'
           AND NOT EXISTS (
               SELECT 1 FROM relations r_fill
               WHERE r_fill.target_id = f.id
                 AND r_fill.relation_type_id = (
                     SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position')
           )
         ORDER BY t.label, f.label"
    )
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            log::error!("Generator check_tor_vacancies query failed: {}", e);
            return;
        }
    };

    // Build per-ToR vacancy map
    let mut tor_vacancies: std::collections::HashMap<i64, (String, Vec<(i64, String)>)> =
        std::collections::HashMap::new();
    for (tor_id, tor_label, pos_id, pos_label) in &rows {
        tor_vacancies
            .entry(*tor_id)
            .or_insert_with(|| (tor_label.clone(), Vec::new()))
            .1
            .push((*pos_id, pos_label.clone()));
    }

    let source_action = "scheduled.tor_vacancy";

    // Create warnings for ToRs with vacancies
    let target_ids = super::get_users_with_permission(pool, "tor.manage_members")
        .await
        .unwrap_or_default();

    for (tor_id, (tor_label, positions)) in &tor_vacancies {
        let dedup_key = format!("tor_vacancy_{}", tor_id);
        if super::warning_exists(pool, source_action, &dedup_key).await {
            continue;
        }

        let pos_names: Vec<&str> = positions.iter().map(|(_, l)| l.as_str()).collect();
        let message = format!(
            "{} has {} unfilled mandatory position(s): {}",
            tor_label,
            positions.len(),
            pos_names.join(", ")
        );
        let details = serde_json::json!({
            "dedup": dedup_key,
            "tor_id": tor_id,
            "tor_label": tor_label,
            "vacant_positions": positions.iter().map(|(id, label)| {
                serde_json::json!({ "id": id, "label": label })
            }).collect::<Vec<_>>(),
        })
        .to_string();

        let warning_id = match super::create_warning(
            pool, "medium", "governance", source_action,
            &message, &details, "system",
        ).await {
            Ok(id) => id,
            Err(e) => {
                log::error!("Failed to create tor_vacancy warning for ToR {}: {}", tor_id, e);
                continue;
            }
        };

        if target_ids.is_empty() {
            continue;
        }

        if super::create_receipts(pool, warning_id, &target_ids).await.is_ok() {
            crate::handlers::warning_handlers::ws::notify_users(
                conn_map, pool, &target_ids, warning_id, "medium", &message,
            );
        }
    }

    // Auto-resolve: find active tor_vacancy warnings for ToRs that no longer have vacancies
    auto_resolve_tor_vacancies(pool, &tor_vacancies).await;
}

/// Resolve vacancy warnings for ToRs that no longer have unfilled mandatory positions.
async fn auto_resolve_tor_vacancies(
    pool: &PgPool,
    current_vacancies: &std::collections::HashMap<i64, (String, Vec<(i64, String)>)>,
) {
    let source_action = "scheduled.tor_vacancy";

    // Find all active tor_vacancy warnings
    let warnings: Vec<(i64, String)> = match sqlx::query_as::<_, (i64, String)>(
        "SELECT e.id, det.value AS details
         FROM entities e
         JOIN entity_properties st ON st.entity_id = e.id AND st.key = 'status' AND st.value = 'active'
         JOIN entity_properties sa ON sa.entity_id = e.id AND sa.key = 'source_action' AND sa.value = $1
         JOIN entity_properties det ON det.entity_id = e.id AND det.key = 'details'
         WHERE e.entity_type = 'warning'"
    )
    .bind(source_action)
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(_) => return,
    };

    for (warning_id, details_str) in warnings {
        // Extract tor_id from the warning details JSON
        let tor_id = match serde_json::from_str::<serde_json::Value>(&details_str) {
            Ok(v) => v.get("tor_id").and_then(|t| t.as_i64()),
            Err(_) => continue,
        };

        if let Some(tid) = tor_id {
            // If this ToR no longer has vacancies, resolve the warning
            if !current_vacancies.contains_key(&tid) {
                if let Err(e) = super::resolve_warning(pool, warning_id, 0).await {
                    log::error!("Failed to auto-resolve vacancy warning {}: {}", warning_id, e);
                }
                log::info!("Auto-resolved vacancy warning {} for ToR {}", warning_id, tid);
            }
        }
    }
}

/// Clean up old warnings based on retention settings.
pub async fn cleanup_old_warnings(pool: &PgPool) -> Result<(), sqlx::Error> {
    let resolved_days = get_setting_days(pool, "warnings.retention_resolved_days", 30).await;
    let deleted_days = get_setting_days(pool, "warnings.retention_deleted_days", 7).await;

    // Delete receipts that have been resolved for longer than retention
    let resolved_cutoff = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(resolved_days))
        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string())
        .unwrap_or_default();

    let deleted_cutoff = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(deleted_days))
        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string())
        .unwrap_or_default();

    // Delete resolved receipts past retention
    let result = sqlx::query(
        "DELETE FROM entities WHERE id IN (
            SELECT ep_ent.entity_id FROM entity_properties ep_ent
            JOIN entity_properties ep_at ON ep_at.entity_id = ep_ent.entity_id AND ep_at.key = 'status_at'
            WHERE ep_ent.key = 'status' AND ep_ent.value = 'resolved'
              AND ep_at.value < $1
              AND ep_ent.entity_id IN (SELECT id FROM entities WHERE entity_type = 'warning_receipt')
        )",
    )
    .bind(&resolved_cutoff)
    .execute(pool)
    .await?;
    let count = result.rows_affected();
    if count > 0 {
        log::info!("Cleaned up {} resolved warning receipts", count);
    }

    // Delete dismissed receipts past retention
    let result = sqlx::query(
        "DELETE FROM entities WHERE id IN (
            SELECT ep_ent.entity_id FROM entity_properties ep_ent
            JOIN entity_properties ep_at ON ep_at.entity_id = ep_ent.entity_id AND ep_at.key = 'status_at'
            WHERE ep_ent.key = 'status' AND ep_ent.value = 'deleted'
              AND ep_at.value < $1
              AND ep_ent.entity_id IN (SELECT id FROM entities WHERE entity_type = 'warning_receipt')
        )",
    )
    .bind(&deleted_cutoff)
    .execute(pool)
    .await?;
    let count = result.rows_affected();
    if count > 0 {
        log::info!("Cleaned up {} deleted warning receipts", count);
    }

    // Delete orphaned warnings (no remaining receipts)
    let result = sqlx::query(
        "DELETE FROM entities WHERE entity_type = 'warning'
         AND id NOT IN (
             SELECT r.target_id FROM relations r
             JOIN entities rt ON rt.id = r.relation_type_id AND rt.name = 'for_warning'
         )",
    )
    .execute(pool)
    .await?;
    let count = result.rows_affected();
    if count > 0 {
        log::info!("Cleaned up {} orphaned warnings", count);
    }

    Ok(())
}

async fn get_setting_days(pool: &PgPool, setting_name: &str, default: i64) -> i64 {
    let result: Option<(String,)> = sqlx::query_as(
        "SELECT ep.value FROM entities e
         JOIN entity_properties ep ON ep.entity_id = e.id AND ep.key = 'value'
         WHERE e.entity_type = 'setting' AND e.name = $1",
    )
    .bind(setting_name)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    result
        .and_then(|r| r.0.parse().ok())
        .unwrap_or(default)
}
