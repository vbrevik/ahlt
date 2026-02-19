use rusqlite::{Connection, params};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    pub id: i64,
    pub user_id: i64,
    pub username: String,
    pub action: String,
    pub target_type: String,
    pub target_id: i64,
    pub summary: String,
    pub created_at: String,
}

pub struct AuditEntryPage {
    pub entries: Vec<AuditEntry>,
    pub page: i64,
    pub per_page: i64,
    pub total_count: i64,
    pub total_pages: i64,
}

/// SQL for audit entry display: entity + properties + username via JOIN
const SELECT_AUDIT_DISPLAY: &str = "\
    SELECT e.id, \
           COALESCE(p_user_id.value, '0') AS user_id, \
           COALESCE(u.name, 'unknown') AS username, \
           COALESCE(p_action.value, '') AS action, \
           COALESCE(p_target_type.value, '') AS target_type, \
           COALESCE(p_target_id.value, '0') AS target_id, \
           COALESCE(p_summary.value, '') AS summary, \
           e.created_at \
    FROM entities e \
    LEFT JOIN entity_properties p_user_id ON e.id = p_user_id.entity_id AND p_user_id.key = 'user_id' \
    LEFT JOIN entity_properties p_action ON e.id = p_action.entity_id AND p_action.key = 'action' \
    LEFT JOIN entity_properties p_target_type ON e.id = p_target_type.entity_id AND p_target_type.key = 'target_type' \
    LEFT JOIN entity_properties p_target_id ON e.id = p_target_id.entity_id AND p_target_id.key = 'target_id' \
    LEFT JOIN entity_properties p_summary ON e.id = p_summary.entity_id AND p_summary.key = 'summary' \
    LEFT JOIN entities u ON CAST(p_user_id.value AS INTEGER) = u.id AND u.entity_type = 'user' \
    WHERE e.entity_type = 'audit_entry'";

fn row_to_audit_entry(row: &rusqlite::Row) -> rusqlite::Result<AuditEntry> {
    Ok(AuditEntry {
        id: row.get("id")?,
        user_id: row.get::<_, String>("user_id")?.parse().unwrap_or(0),
        username: row.get("username")?,
        action: row.get("action")?,
        target_type: row.get("target_type")?,
        target_id: row.get::<_, String>("target_id")?.parse().unwrap_or(0),
        summary: row.get("summary")?,
        created_at: row.get("created_at")?,
    })
}

/// Find audit entries with pagination and optional filters
pub fn find_paginated(
    conn: &Connection,
    page: i64,
    per_page: i64,
    search: Option<&str>,
    action_filter: Option<&str>,
    target_type_filter: Option<&str>,
) -> rusqlite::Result<AuditEntryPage> {
    let page = page.max(1);
    let per_page = per_page.clamp(1, 100);
    let offset = (page - 1) * per_page;

    // Build filter clauses
    let mut filters = Vec::new();
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(q) = search.filter(|s| !s.trim().is_empty()) {
        let pattern = format!("%{}%", q.trim());
        filters.push(format!("(u.name LIKE ?{} OR p_summary.value LIKE ?{})", params_vec.len() + 1, params_vec.len() + 2));
        params_vec.push(Box::new(pattern.clone()));
        params_vec.push(Box::new(pattern));
    }

    if let Some(action) = action_filter.filter(|a| a != &"all") {
        filters.push(format!("p_action.value LIKE ?{}", params_vec.len() + 1));
        params_vec.push(Box::new(format!("{}%", action)));
    }

    if let Some(target) = target_type_filter.filter(|t| t != &"all") {
        filters.push(format!("p_target_type.value = ?{}", params_vec.len() + 1));
        params_vec.push(Box::new(target.to_string()));
    }

    let filter_clause = if filters.is_empty() {
        String::new()
    } else {
        format!(" AND {}", filters.join(" AND "))
    };

    // Get total count
    let count_sql = format!(
        "SELECT COUNT(*) FROM entities e \
         LEFT JOIN entity_properties p_user_id ON e.id = p_user_id.entity_id AND p_user_id.key = 'user_id' \
         LEFT JOIN entity_properties p_action ON e.id = p_action.entity_id AND p_action.key = 'action' \
         LEFT JOIN entity_properties p_target_type ON e.id = p_target_type.entity_id AND p_target_type.key = 'target_type' \
         LEFT JOIN entity_properties p_summary ON e.id = p_summary.entity_id AND p_summary.key = 'summary' \
         LEFT JOIN entities u ON CAST(p_user_id.value AS INTEGER) = u.id AND u.entity_type = 'user' \
         WHERE e.entity_type = 'audit_entry'{}",
        filter_clause
    );

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();
    let total_count: i64 = conn.query_row(&count_sql, param_refs.as_slice(), |row| row.get(0))?;
    let total_pages = (total_count as f64 / per_page as f64).ceil() as i64;

    // Get paginated results
    let sql = format!(
        "{}{} ORDER BY e.created_at DESC LIMIT ?{} OFFSET ?{}",
        SELECT_AUDIT_DISPLAY,
        filter_clause,
        params_vec.len() + 1,
        params_vec.len() + 2
    );

    params_vec.push(Box::new(per_page));
    params_vec.push(Box::new(offset));
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();

    let mut stmt = conn.prepare(&sql)?;
    let entries = stmt.query_map(param_refs.as_slice(), row_to_audit_entry)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(AuditEntryPage {
        entries,
        page,
        per_page,
        total_count,
        total_pages,
    })
}

/// Fetch the N most recent audit entries (for dashboard activity feed).
pub fn find_recent(conn: &Connection, limit: i64) -> rusqlite::Result<Vec<AuditEntry>> {
    let sql = format!(
        "{} ORDER BY e.created_at DESC LIMIT ?1",
        SELECT_AUDIT_DISPLAY
    );
    let mut stmt = conn.prepare(&sql)?;
    let entries = stmt.query_map(params![limit], row_to_audit_entry)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(entries)
}

/// Create an audit entry in the database
pub fn create(
    conn: &Connection,
    user_id: i64,
    action: &str,
    target_type: &str,
    target_id: i64,
    summary: &str,
) -> rusqlite::Result<i64> {
    // Insert audit_entry entity
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('audit_entry', ?1, ?2)",
        params![action, summary],
    )?;
    let entry_id = conn.last_insert_rowid();

    // Set properties
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'user_id', ?2)",
        params![entry_id, user_id.to_string()],
    )?;
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'action', ?2)",
        params![entry_id, action],
    )?;
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'target_type', ?2)",
        params![entry_id, target_type],
    )?;
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'target_id', ?2)",
        params![entry_id, target_id.to_string()],
    )?;
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'summary', ?2)",
        params![entry_id, summary],
    )?;

    Ok(entry_id)
}
