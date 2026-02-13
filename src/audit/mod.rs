use rusqlite::Connection;
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;

#[derive(Debug)]
pub enum AuditError {
    FileError(std::io::Error),
    DbError(rusqlite::Error),
    JsonError(serde_json::Error),
}

impl From<std::io::Error> for AuditError {
    fn from(err: std::io::Error) -> Self {
        AuditError::FileError(err)
    }
}

impl From<rusqlite::Error> for AuditError {
    fn from(err: rusqlite::Error) -> Self {
        AuditError::DbError(err)
    }
}

impl From<serde_json::Error> for AuditError {
    fn from(err: serde_json::Error) -> Self {
        AuditError::JsonError(err)
    }
}

impl std::fmt::Display for AuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditError::FileError(e) => write!(f, "File error: {}", e),
            AuditError::DbError(e) => write!(f, "Database error: {}", e),
            AuditError::JsonError(e) => write!(f, "JSON error: {}", e),
        }
    }
}

// Helper: Check if action is high-value (requires database logging)
fn is_important(action: &str) -> bool {
    matches!(action,
        "user.created" | "user.deleted" |
        "role.created" | "role.deleted" | "role.permissions_changed" |
        "setting.critical_changed"
    )
}

// Helper: Get current date in YYYY-MM-DD format
fn get_current_date() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
}

// Helper: Get log file path for given date
fn get_log_path(conn: &Connection, date: &str) -> Result<String, AuditError> {
    // Get audit.log_path setting
    let log_path: String = conn.query_row(
        "SELECT value FROM entity_properties
         WHERE entity_id = (SELECT id FROM entities WHERE entity_type='setting' AND name='audit.log_path')
           AND key='value'",
        [],
        |row| row.get(0),
    ).unwrap_or_else(|_| "data/audit/".to_string());

    // Ensure directory exists
    fs::create_dir_all(&log_path)?;

    let filename = format!("audit-{}.jsonl", date);
    let full_path = std::path::Path::new(&log_path).join(filename);

    Ok(full_path.to_string_lossy().to_string())
}

// Helper: Get username from user_id
fn get_username(conn: &Connection, user_id: i64) -> String {
    conn.query_row(
        "SELECT name FROM entities WHERE id = ? AND entity_type = 'user'",
        [user_id],
        |row| row.get::<_, String>(0),
    ).unwrap_or_else(|_| "unknown".to_string())
}

// Write audit entry to filesystem
fn write_to_file(
    conn: &Connection,
    user_id: i64,
    action: &str,
    target_type: &str,
    target_id: i64,
    details: &Value,
) -> Result<(), AuditError> {
    // Check if audit is enabled
    let enabled: String = conn.query_row(
        "SELECT value FROM entity_properties
         WHERE entity_id = (SELECT id FROM entities WHERE entity_type='setting' AND name='audit.enabled')
           AND key='value'",
        [],
        |row| row.get(0),
    ).unwrap_or_else(|_| "false".to_string());

    if enabled != "true" {
        return Ok(());
    }

    let date = get_current_date();
    let log_path = get_log_path(conn, &date)?;
    let username = get_username(conn, user_id);

    // Get current timestamp in ISO 8601 format
    let timestamp = chrono::Utc::now().to_rfc3339();

    // Build log entry
    let entry = serde_json::json!({
        "timestamp": timestamp,
        "user_id": user_id,
        "username": username,
        "action": action,
        "target_type": target_type,
        "target_id": target_id,
        "details": details,
    });

    // Append to file
    // Append to file with secure permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)  // Set permissions atomically on creation
            .open(&log_path)?;

        writeln!(file, "{}", serde_json::to_string(&entry)?)?;
    }

    #[cfg(not(unix))]
    {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        writeln!(file, "{}", serde_json::to_string(&entry)?)?;
    }

    Ok(())
}

// Main audit logging function
pub fn log(
    conn: &Connection,
    user_id: i64,
    action: &str,
    target_type: &str,
    target_id: i64,
    details: Value,
) -> Result<(), AuditError> {
    // Always write to filesystem (errors logged but not propagated)
    if let Err(e) = write_to_file(conn, user_id, action, target_type, target_id, &details) {
        eprintln!("Audit filesystem write failed: {:?}", e);
    }

    // If high-value event, also write to database
    if is_important(action) {
        let summary = format!("{} {}", action, details.get("summary").and_then(|v| v.as_str()).unwrap_or(""));
        if let Err(e) = crate::models::audit::create(conn, user_id, action, target_type, target_id, &summary) {
            eprintln!("Audit database write failed: {:?}", e);
        }
    }

    Ok(())
}

pub fn cleanup_old_entries(_conn: &Connection) {
    // Will implement later
}
