pub mod generators;
pub mod queries;
pub mod scheduler;

use rusqlite::{Connection, params};
use chrono::Utc;

use crate::models::{entity, relation};

/// Create a warning entity with properties. Returns the warning entity ID.
pub fn create_warning(
    conn: &Connection,
    severity: &str,
    category: &str,
    source_action: &str,
    message: &str,
    details: &str,
    scope: &str,
) -> rusqlite::Result<i64> {
    let timestamp = Utc::now().timestamp();
    let name = format!("{}.{}.{}", source_action, category, timestamp);
    let warning_id = entity::create(conn, "warning", &name, message)?;

    entity::set_properties(conn, warning_id, &[
        ("severity", severity),
        ("category", category),
        ("message", message),
        ("source_action", source_action),
        ("details", details),
        ("status", "active"),
        ("scope", scope),
    ])?;

    Ok(warning_id)
}

/// Create receipt entities for each target user. Returns receipt IDs.
pub fn create_receipts(
    conn: &Connection,
    warning_id: i64,
    target_user_ids: &[i64],
) -> rusqlite::Result<Vec<i64>> {
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let mut receipt_ids = Vec::new();

    for &user_id in target_user_ids {
        let receipt_name = format!("wr.{}.{}", warning_id, user_id);
        let receipt_id = entity::create(conn, "warning_receipt", &receipt_name, "Warning Receipt")?;

        entity::set_properties(conn, receipt_id, &[
            ("status", "unread"),
            ("status_at", &now),
        ])?;

        // Link receipt to warning and user
        relation::create(conn, "for_warning", receipt_id, warning_id)?;
        relation::create(conn, "for_user", receipt_id, user_id)?;

        // Link warning to target user
        relation::create(conn, "targets_user", warning_id, user_id)?;

        // Create "created" event
        create_event(conn, receipt_id, "created", user_id, None)?;

        receipt_ids.push(receipt_id);
    }

    Ok(receipt_ids)
}

/// Create a warning_event entity on a receipt.
pub fn create_event(
    conn: &Connection,
    receipt_id: i64,
    action: &str,
    actor_user_id: i64,
    note: Option<&str>,
) -> rusqlite::Result<i64> {
    let timestamp = Utc::now().timestamp();
    let event_name = format!("we.{}.{}.{}", receipt_id, action, timestamp);
    let event_id = entity::create(conn, "warning_event", &event_name, action)?;

    entity::set_properties(conn, event_id, &[
        ("action", action),
        ("actor_user_id", &actor_user_id.to_string()),
    ])?;

    if let Some(n) = note {
        entity::set_property(conn, event_id, "note", n)?;
    }

    relation::create(conn, "on_receipt", event_id, receipt_id)?;

    Ok(event_id)
}

/// Check if an active warning already exists (deduplication).
pub fn warning_exists(conn: &Connection, source_action: &str, dedup_key: &str) -> bool {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entities e
         JOIN entity_properties st ON st.entity_id = e.id AND st.key = 'status'
         JOIN entity_properties sa ON sa.entity_id = e.id AND sa.key = 'source_action'
         JOIN entity_properties det ON det.entity_id = e.id AND det.key = 'details'
         WHERE e.entity_type = 'warning'
           AND st.value = 'active'
           AND sa.value = ?1
           AND det.value LIKE ?2",
        params![source_action, format!("%{}%", dedup_key)],
        |row| row.get(0),
    ).unwrap_or(0);
    count > 0
}

/// Get all user IDs that have a specific permission code.
pub fn get_users_with_permission(conn: &Connection, permission_code: &str) -> rusqlite::Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT u.id
         FROM entities u
         JOIN relations ur ON ur.source_id = u.id
         JOIN entities rt_role ON rt_role.id = ur.relation_type_id AND rt_role.name = 'has_role'
         JOIN relations rp ON rp.source_id = ur.target_id
         JOIN entities rt_perm ON rt_perm.id = rp.relation_type_id AND rt_perm.name = 'has_permission'
         JOIN entities perm ON perm.id = rp.target_id AND perm.name = ?1
         WHERE u.entity_type = 'user' AND u.is_active = 1"
    )?;
    let ids = stmt.query_map(params![permission_code], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}

/// Update a receipt's status and create corresponding event.
pub fn update_receipt_status(
    conn: &Connection,
    receipt_id: i64,
    new_status: &str,
    actor_user_id: i64,
) -> rusqlite::Result<()> {
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    entity::set_properties(conn, receipt_id, &[
        ("status", new_status),
        ("status_at", &now),
    ])?;
    create_event(conn, receipt_id, new_status, actor_user_id, None)?;
    Ok(())
}

/// Resolve a warning: set status to resolved, update all receipts.
pub fn resolve_warning(conn: &Connection, warning_id: i64, actor_user_id: i64) -> rusqlite::Result<()> {
    entity::set_property(conn, warning_id, "status", "resolved")?;

    let receipt_ids = get_receipt_ids_for_warning(conn, warning_id)?;
    for receipt_id in receipt_ids {
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        entity::set_properties(conn, receipt_id, &[
            ("status", "resolved"),
            ("status_at", &now),
        ])?;
        create_event(conn, receipt_id, "resolved", actor_user_id, None)?;
    }
    Ok(())
}

/// Get all receipt entity IDs for a warning.
fn get_receipt_ids_for_warning(conn: &Connection, warning_id: i64) -> rusqlite::Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT r.source_id FROM relations r
         JOIN entities rt ON rt.id = r.relation_type_id AND rt.name = 'for_warning'
         WHERE r.target_id = ?1"
    )?;
    let ids = stmt.query_map(params![warning_id], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}
