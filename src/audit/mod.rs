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
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("Time went backwards");
    let secs = now.as_secs();

    // Simple date calculation (good enough for daily rotation)
    let days = secs / 86400;
    let epoch_days = days + 719468; // Days from 0000-01-01 to 1970-01-01

    let year = (epoch_days / 365) as i32; // Approximate
    let month = ((epoch_days % 365) / 30) as u32 + 1; // Approximate
    let day = ((epoch_days % 365) % 30) as u32 + 1; // Approximate

    format!("{:04}-{:02}-{:02}", year, month.min(12), day.min(31))
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
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = file.metadata()?.permissions();
        perms.set_mode(0o600); // Owner read/write only
        file.set_permissions(perms)?;
    }

    writeln!(file, "{}", serde_json::to_string(&entry)?)?;

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

    // If high-value event, also write to database (will implement in next task)
    if is_important(action) {
        // TODO: write to database
    }

    Ok(())
}

pub fn cleanup_old_entries(_conn: &Connection) {
    // Will implement later
}
