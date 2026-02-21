use sqlx::PgPool;
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::Write;

#[derive(Debug)]
pub enum AuditError {
    File(std::io::Error),
    Db(sqlx::Error),
    Json(serde_json::Error),
}

impl From<std::io::Error> for AuditError {
    fn from(err: std::io::Error) -> Self {
        AuditError::File(err)
    }
}

impl From<sqlx::Error> for AuditError {
    fn from(err: sqlx::Error) -> Self {
        AuditError::Db(err)
    }
}

impl From<serde_json::Error> for AuditError {
    fn from(err: serde_json::Error) -> Self {
        AuditError::Json(err)
    }
}

impl std::fmt::Display for AuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditError::File(e) => write!(f, "File error: {}", e),
            AuditError::Db(e) => write!(f, "Database error: {}", e),
            AuditError::Json(e) => write!(f, "JSON error: {}", e),
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
async fn get_log_path(pool: &PgPool, date: &str) -> Result<String, AuditError> {
    // Get audit.log_path setting
    let log_path: String = sqlx::query_as::<_, (String,)>(
        "SELECT value FROM entity_properties
         WHERE entity_id = (SELECT id FROM entities WHERE entity_type='setting' AND name='audit.log_path')
           AND key='value'",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .map(|r| r.0)
    .unwrap_or_else(|| "data/audit/".to_string());

    // Ensure directory exists
    fs::create_dir_all(&log_path)?;

    let filename = format!("audit-{}.jsonl", date);
    let full_path = std::path::Path::new(&log_path).join(filename);

    Ok(full_path.to_string_lossy().to_string())
}

// Helper: Get username from user_id
async fn get_username(pool: &PgPool, user_id: i64) -> String {
    sqlx::query_as::<_, (String,)>(
        "SELECT name FROM entities WHERE id = $1 AND entity_type = 'user'",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .map(|r| r.0)
    .unwrap_or_else(|| "unknown".to_string())
}

// Write audit entry to filesystem
async fn write_to_file(
    pool: &PgPool,
    user_id: i64,
    action: &str,
    target_type: &str,
    target_id: i64,
    details: &Value,
) -> Result<(), AuditError> {
    // Check if audit is enabled
    let enabled: String = sqlx::query_as::<_, (String,)>(
        "SELECT value FROM entity_properties
         WHERE entity_id = (SELECT id FROM entities WHERE entity_type='setting' AND name='audit.enabled')
           AND key='value'",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .map(|r| r.0)
    .unwrap_or_else(|| "false".to_string());

    if enabled != "true" {
        return Ok(());
    }

    let date = get_current_date();
    let log_path = get_log_path(pool, &date).await?;
    let username = get_username(pool, user_id).await;

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
pub async fn log(
    pool: &PgPool,
    user_id: i64,
    action: &str,
    target_type: &str,
    target_id: i64,
    details: Value,
) -> Result<(), AuditError> {
    // Always write to filesystem (errors logged but not propagated)
    if let Err(e) = write_to_file(pool, user_id, action, target_type, target_id, &details).await {
        eprintln!("Audit filesystem write failed: {:?}", e);
    }

    // If high-value event, also write to database
    if is_important(action) {
        let summary = format!("{} {}", action, details.get("summary").and_then(|v| v.as_str()).unwrap_or(""));
        if let Err(e) = crate::models::audit::create(pool, user_id, action, target_type, target_id, &summary).await {
            eprintln!("Audit database write failed: {:?}", e);
        }
    }

    Ok(())
}

pub async fn cleanup_old_entries(pool: &PgPool) {
    // Get retention_days setting
    let retention_days: i64 = sqlx::query_as::<_, (String,)>(
        "SELECT value FROM entity_properties
         WHERE entity_id = (SELECT id FROM entities WHERE entity_type='setting' AND name='audit.retention_days')
           AND key='value'",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .map(|r| r.0.parse().unwrap_or(90))
    .unwrap_or(90);

    // Skip if retention is 0 (keep forever)
    if retention_days == 0 {
        eprintln!("Audit retention: keeping all entries (retention_days=0)");
        return;
    }

    // Delete old audit entries (CASCADE deletes properties automatically)
    let result = sqlx::query(
        "DELETE FROM entities
         WHERE entity_type = 'audit_entry'
           AND created_at < NOW() - ($1 || ' days')::INTERVAL",
    )
    .bind(retention_days.to_string())
    .execute(pool)
    .await;

    match result {
        Ok(r) => {
            let deleted = r.rows_affected();
            if deleted > 0 {
                eprintln!("Audit cleanup: deleted {} entries older than {} days", deleted, retention_days);
            }
        }
        Err(e) => {
            eprintln!("Audit cleanup failed: {:?}", e);
        }
    }
}
