use rusqlite::{Connection, params};

use crate::db::DbPool;
use crate::handlers::warning_handlers::ws::ConnectionMap;

/// Check for users without a role assignment.
pub fn check_users_without_role(conn: &Connection, conn_map: &ConnectionMap, pool: &DbPool) {
    let mut stmt = match conn.prepare(
        "SELECT e.id FROM entities e
         WHERE e.entity_type = 'user'
           AND NOT EXISTS (
               SELECT 1 FROM relations r
               JOIN entities rt ON rt.id = r.relation_type_id AND rt.name = 'has_role'
               WHERE r.source_id = e.id
           )"
    ) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Generator check_users_without_role query failed: {}", e);
            return;
        }
    };

    let user_ids: Vec<i64> = match stmt.query_map([], |row| row.get(0)) {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(_) => return,
    };

    if user_ids.is_empty() {
        return;
    }

    let source_action = "scheduled.users_without_role";
    if super::warning_exists(conn, source_action, "users_without_role") {
        return;
    }

    let message = format!("{} user(s) have no role assigned", user_ids.len());
    let details = serde_json::json!({ "dedup": "users_without_role", "user_ids": user_ids }).to_string();

    let warning_id = match super::create_warning(
        conn, "medium", "data_integrity", source_action,
        &message, &details, "system",
    ) {
        Ok(id) => id,
        Err(e) => {
            log::error!("Failed to create users_without_role warning: {}", e);
            return;
        }
    };

    // Target all admin users
    let admin_ids = super::get_users_with_permission(conn, "admin.settings")
        .unwrap_or_default();
    if admin_ids.is_empty() {
        return;
    }

    if let Ok(receipt_ids) = super::create_receipts(conn, warning_id, &admin_ids) {
        let _ = receipt_ids; // receipts created
        crate::handlers::warning_handlers::ws::notify_users(
            conn_map, pool, &admin_ids, warning_id, "medium", &message,
        );
    }
}

/// Check database file size against threshold.
pub fn check_database_size(conn: &Connection, conn_map: &ConnectionMap, pool: &DbPool, data_dir: &str) {
    let db_path = format!("{}/app.db", data_dir);
    let size_bytes = match std::fs::metadata(&db_path) {
        Ok(m) => m.len(),
        Err(_) => return,
    };

    let threshold_mb: u64 = 500; // 500 MB
    let size_mb = size_bytes / (1024 * 1024);

    if size_mb < threshold_mb {
        return;
    }

    let source_action = "scheduled.database_size";
    if super::warning_exists(conn, source_action, "database_size") {
        return;
    }

    let message = format!("Database size is {} MB (threshold: {} MB)", size_mb, threshold_mb);
    let severity = if size_mb > threshold_mb * 2 { "high" } else { "medium" };

    let details = serde_json::json!({ "dedup": "database_size", "size_mb": size_mb }).to_string();
    let warning_id = match super::create_warning(
        conn, severity, "system", source_action,
        &message, &details, "system",
    ) {
        Ok(id) => id,
        Err(e) => {
            log::error!("Failed to create database_size warning: {}", e);
            return;
        }
    };

    let admin_ids = super::get_users_with_permission(conn, "admin.settings")
        .unwrap_or_default();
    if admin_ids.is_empty() {
        return;
    }

    if super::create_receipts(conn, warning_id, &admin_ids).is_ok() {
        crate::handlers::warning_handlers::ws::notify_users(
            conn_map, pool, &admin_ids, warning_id, severity, &message,
        );
    }
}

/// Clean up old warnings based on retention settings.
pub fn cleanup_old_warnings(conn: &Connection) -> rusqlite::Result<()> {
    let resolved_days = get_setting_days(conn, "warnings.retention_resolved_days", 30);
    let deleted_days = get_setting_days(conn, "warnings.retention_deleted_days", 7);

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
    let count = conn.execute(
        "DELETE FROM entities WHERE id IN (
            SELECT ep_ent.entity_id FROM entity_properties ep_ent
            JOIN entity_properties ep_at ON ep_at.entity_id = ep_ent.entity_id AND ep_at.key = 'status_at'
            WHERE ep_ent.key = 'status' AND ep_ent.value = 'resolved'
              AND ep_at.value < ?1
              AND ep_ent.entity_id IN (SELECT id FROM entities WHERE entity_type = 'warning_receipt')
        )",
        params![resolved_cutoff],
    )?;
    if count > 0 {
        log::info!("Cleaned up {} resolved warning receipts", count);
    }

    // Delete dismissed receipts past retention
    let count = conn.execute(
        "DELETE FROM entities WHERE id IN (
            SELECT ep_ent.entity_id FROM entity_properties ep_ent
            JOIN entity_properties ep_at ON ep_at.entity_id = ep_ent.entity_id AND ep_at.key = 'status_at'
            WHERE ep_ent.key = 'status' AND ep_ent.value = 'deleted'
              AND ep_at.value < ?1
              AND ep_ent.entity_id IN (SELECT id FROM entities WHERE entity_type = 'warning_receipt')
        )",
        params![deleted_cutoff],
    )?;
    if count > 0 {
        log::info!("Cleaned up {} deleted warning receipts", count);
    }

    // Delete orphaned warnings (no remaining receipts)
    let count = conn.execute(
        "DELETE FROM entities WHERE entity_type = 'warning'
         AND id NOT IN (
             SELECT r.target_id FROM relations r
             JOIN entities rt ON rt.id = r.relation_type_id AND rt.name = 'for_warning'
         )",
        [],
    )?;
    if count > 0 {
        log::info!("Cleaned up {} orphaned warnings", count);
    }

    Ok(())
}

fn get_setting_days(conn: &Connection, setting_name: &str, default: i64) -> i64 {
    conn.query_row(
        "SELECT ep.value FROM entities e
         JOIN entity_properties ep ON ep.entity_id = e.id AND ep.key = 'value'
         WHERE e.entity_type = 'setting' AND e.name = ?1",
        params![setting_name],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .and_then(|v| v.parse().ok())
    .unwrap_or(default)
}
