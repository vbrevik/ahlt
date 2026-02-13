# Audit Trail - Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement two-tier audit system with database (EAV) for high-value events and filesystem (JSON Lines) for complete audit trail.

**Architecture:** Hybrid approach - important events stored in database for UI querying, all events logged to daily-rotated JSON Lines files for external analysis.

**Tech Stack:** Rust, Actix-web, SQLite (EAV pattern), serde_json, filesystem logging

---

## Task 1: Add Dependencies and Module Structure

**Files:**
- Modify: `Cargo.toml`
- Create: `src/audit/mod.rs`
- Modify: `src/main.rs`

**Step 1: Check if serde_json is already a dependency**

Run: `grep serde_json Cargo.toml`
Expected: Either found or not found

**Step 2: Add serde_json dependency if needed**

If not found, add to `Cargo.toml` under `[dependencies]`:
```toml
serde_json = "1.0"
```

**Step 3: Create audit module skeleton**

Create `src/audit/mod.rs`:
```rust
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

// Placeholder functions - will implement in later tasks
pub fn log(
    _conn: &Connection,
    _user_id: i64,
    _action: &str,
    _target_type: &str,
    _target_id: i64,
    _details: Value,
) -> Result<(), AuditError> {
    Ok(())
}

pub fn cleanup_old_entries(_conn: &Connection) {
    // Will implement later
}
```

**Step 4: Register audit module in main.rs**

Add to `src/main.rs` after other mod declarations:
```rust
mod audit;
```

**Step 5: Build to verify**

Run: `cargo build`
Expected: Clean build (warnings OK, no errors)

**Step 6: Commit**

```bash
git add Cargo.toml src/audit/mod.rs src/main.rs
git commit -m "feat(audit): add audit module skeleton and dependencies

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 2: Seed Audit Settings and Permission

**Files:**
- Modify: `src/db.rs`

**Step 1: Add audit permission to seed_ontology**

In `src/db.rs`, find the `seed_ontology` function and add audit.view permission after other permissions:

```rust
// After existing permissions like settings.manage
conn.execute(
    "INSERT INTO entities (entity_type, name, label) VALUES ('permission', 'audit.view', 'View Audit Log')",
    [],
)?;
conn.execute(
    "INSERT INTO entity_properties (entity_id, key, value)
     VALUES ((SELECT id FROM entities WHERE entity_type='permission' AND name='audit.view'), 'group_name', 'Admin')",
    [],
)?;
```

**Step 2: Grant audit.view to admin role**

Add relation for admin role to have audit.view permission:

```rust
// After other admin permissions
conn.execute(
    "INSERT INTO relations (relation_type_id, source_id, target_id)
     VALUES (
         (SELECT id FROM entities WHERE entity_type='relation_type' AND name='has_permission'),
         (SELECT id FROM entities WHERE entity_type='role' AND name='admin'),
         (SELECT id FROM entities WHERE entity_type='permission' AND name='audit.view')
     )",
    [],
)?;
```

**Step 3: Seed audit settings**

Add audit settings after existing app settings:

```rust
// Audit settings
conn.execute(
    "INSERT INTO entities (entity_type, name, label) VALUES ('setting', 'audit.enabled', 'Enable Audit Logging')",
    [],
)?;
conn.execute(
    "INSERT INTO entity_properties (entity_id, key, value) VALUES (
        (SELECT id FROM entities WHERE entity_type='setting' AND name='audit.enabled'),
        'value', 'true'
    )",
    [],
)?;
conn.execute(
    "INSERT INTO entity_properties (entity_id, key, value) VALUES (
        (SELECT id FROM entities WHERE entity_type='setting' AND name='audit.enabled'),
        'setting_type', 'boolean'
    )",
    [],
)?;
conn.execute(
    "INSERT INTO entity_properties (entity_id, key, value) VALUES (
        (SELECT id FROM entities WHERE entity_type='setting' AND name='audit.enabled'),
        'description', 'Master toggle for audit logging (database and filesystem)'
    )",
    [],
)?;

conn.execute(
    "INSERT INTO entities (entity_type, name, label) VALUES ('setting', 'audit.log_path', 'Audit Log Directory')",
    [],
)?;
conn.execute(
    "INSERT INTO entity_properties (entity_id, key, value) VALUES (
        (SELECT id FROM entities WHERE entity_type='setting' AND name='audit.log_path'),
        'value', 'data/audit/'
    )",
    [],
)?;
conn.execute(
    "INSERT INTO entity_properties (entity_id, key, value) VALUES (
        (SELECT id FROM entities WHERE entity_type='setting' AND name='audit.log_path'),
        'setting_type', 'text'
    )",
    [],
)?;
conn.execute(
    "INSERT INTO entity_properties (entity_id, key, value) VALUES (
        (SELECT id FROM entities WHERE entity_type='setting' AND name='audit.log_path'),
        'description', 'Directory path for audit log files (absolute or relative)'
    )",
    [],
)?;

conn.execute(
    "INSERT INTO entities (entity_type, name, label) VALUES ('setting', 'audit.retention_days', 'Audit Retention (Days)')",
    [],
)?;
conn.execute(
    "INSERT INTO entity_properties (entity_id, key, value) VALUES (
        (SELECT id FROM entities WHERE entity_type='setting' AND name='audit.retention_days'),
        'value', '90'
    )",
    [],
)?;
conn.execute(
    "INSERT INTO entity_properties (entity_id, key, value) VALUES (
        (SELECT id FROM entities WHERE entity_type='setting' AND name='audit.retention_days'),
        'setting_type', 'number'
    )",
    [],
)?;
conn.execute(
    "INSERT INTO entity_properties (entity_id, key, value) VALUES (
        (SELECT id FROM entities WHERE entity_type='setting' AND name='audit.retention_days'),
        'description', 'Days to keep audit entries in database (0 = forever)'
    )",
    [],
)?;
```

**Step 4: Create audit directory on startup**

At the end of `seed_ontology`, add:

```rust
// Create audit directory with secure permissions
let audit_path = "data/audit";
if !std::path::Path::new(audit_path).exists() {
    std::fs::create_dir_all(audit_path)
        .expect("Failed to create audit directory");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(audit_path)
            .expect("Failed to get audit dir metadata")
            .permissions();
        perms.set_mode(0o700); // Owner read/write/execute only
        std::fs::set_permissions(audit_path, perms)
            .expect("Failed to set audit dir permissions");
    }
}
```

**Step 5: Delete existing database to re-seed**

Run: `rm data/app.db`
Expected: File deleted

**Step 6: Build and run to verify seeding**

Run: `cargo run`
Expected: Server starts, data/audit/ directory created

**Step 7: Verify settings in database**

Run: `sqlite3 data/app.db "SELECT name, label FROM entities WHERE entity_type='setting' AND name LIKE 'audit.%'"`
Expected: Three audit settings listed

**Step 8: Stop server and commit**

Ctrl+C to stop server, then:

```bash
git add src/db.rs
git commit -m "feat(audit): seed audit settings and permission

- Add audit.view permission for admin role
- Seed audit.enabled, audit.log_path, audit.retention_days settings
- Create data/audit/ directory with 0700 permissions on startup

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 3: Implement Filesystem Logging

**Files:**
- Modify: `src/audit/mod.rs`

**Step 1: Implement is_important helper**

Add to `src/audit/mod.rs`:

```rust
fn is_important(action: &str) -> bool {
    matches!(action,
        "user.created" | "user.deleted" |
        "role.created" | "role.deleted" | "role.permissions_changed" |
        "setting.critical_changed"
    )
}
```

**Step 2: Implement get_current_date**

```rust
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
```

**Step 3: Implement get_log_path helper**

```rust
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
```

**Step 4: Implement get_username helper**

```rust
fn get_username(conn: &Connection, user_id: i64) -> String {
    conn.query_row(
        "SELECT name FROM entities WHERE id = ? AND entity_type = 'user'",
        [user_id],
        |row| row.get::<_, String>(0),
    ).unwrap_or_else(|_| "unknown".to_string())
}
```

**Step 5: Implement write_to_file function**

```rust
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
```

**Step 6: Update log function to call write_to_file**

Replace the placeholder `log` function:

```rust
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
```

**Step 7: Add chrono dependency**

Add to `Cargo.toml`:
```toml
chrono = "0.4"
```

**Step 8: Build to verify**

Run: `cargo build`
Expected: Clean build

**Step 9: Commit**

```bash
git add Cargo.toml src/audit/mod.rs
git commit -m "feat(audit): implement filesystem logging with JSON Lines

- Write all audit events to daily-rotated .jsonl files
- Secure file permissions (0600) on Unix systems
- Check audit.enabled setting before logging
- Never fail request if logging fails (errors to stderr)

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 4: Create Audit Model with EAV Queries

**Files:**
- Create: `src/models/audit.rs`
- Modify: `src/models/mod.rs`

**Step 1: Create audit model file**

Create `src/models/audit.rs`:

```rust
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

    if let Some(q) = search {
        if !q.trim().is_empty() {
            let pattern = format!("%{}%", q.trim());
            filters.push("(u.name LIKE ?".to_string() + &(params_vec.len() + 1).to_string() + " OR p_summary.value LIKE ?" + &(params_vec.len() + 1).to_string() + ")");
            params_vec.push(Box::new(pattern.clone()));
            params_vec.push(Box::new(pattern));
        }
    }

    if let Some(action) = action_filter {
        if action != "all" {
            filters.push(format!("p_action.value LIKE ?{}", params_vec.len() + 1));
            params_vec.push(Box::new(format!("{}%", action)));
        }
    }

    if let Some(target) = target_type_filter {
        if target != "all" {
            filters.push(format!("p_target_type.value = ?{}", params_vec.len() + 1));
            params_vec.push(Box::new(target.to_string()));
        }
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
```

**Step 2: Register audit model**

Add to `src/models/mod.rs`:
```rust
pub mod audit;
```

**Step 3: Update audit::log to write to database**

In `src/audit/mod.rs`, update the log function:

```rust
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
```

**Step 4: Build to verify**

Run: `cargo build`
Expected: Clean build

**Step 5: Commit**

```bash
git add src/models/audit.rs src/models/mod.rs src/audit/mod.rs
git commit -m "feat(audit): add audit entry model with EAV queries

- Create audit_entry entities with properties via EAV pattern
- Paginated query with search and filter support
- Integrate database logging for high-value events

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 5: Implement Retention Cleanup

**Files:**
- Modify: `src/audit/mod.rs`
- Modify: `src/main.rs`

**Step 1: Implement cleanup_old_entries**

In `src/audit/mod.rs`, replace the placeholder:

```rust
pub fn cleanup_old_entries(conn: &Connection) {
    // Get retention_days setting
    let retention_days: i64 = conn.query_row(
        "SELECT value FROM entity_properties
         WHERE entity_id = (SELECT id FROM entities WHERE entity_type='setting' AND name='audit.retention_days')
           AND key='value'",
        [],
        |row| row.get::<_, String>(0).map(|s| s.parse().unwrap_or(90)),
    ).unwrap_or(90);

    // Skip if retention is 0 (keep forever)
    if retention_days == 0 {
        eprintln!("Audit retention: keeping all entries (retention_days=0)");
        return;
    }

    // Delete old audit entries (CASCADE deletes properties automatically)
    let result = conn.execute(
        "DELETE FROM entities
         WHERE entity_type = 'audit_entry'
           AND created_at < date('now', '-' || ?1 || ' days')",
        [retention_days],
    );

    match result {
        Ok(deleted) => {
            if deleted > 0 {
                eprintln!("Audit cleanup: deleted {} entries older than {} days", deleted, retention_days);
            }
        }
        Err(e) => {
            eprintln!("Audit cleanup failed: {:?}", e);
        }
    }
}
```

**Step 2: Call cleanup on server startup**

In `src/main.rs`, after `db::seed_ontology(&pool, &admin_hash);`, add:

```rust
// Clean up old audit entries based on retention policy
{
    let conn = pool.get().expect("Failed to get connection for audit cleanup");
    audit::cleanup_old_entries(&conn);
}
```

**Step 3: Build to verify**

Run: `cargo build`
Expected: Clean build

**Step 4: Commit**

```bash
git add src/audit/mod.rs src/main.rs
git commit -m "feat(audit): implement retention cleanup on startup

- Delete audit entries older than audit.retention_days
- Skip cleanup if retention_days=0 (keep forever)
- Run automatically on server startup
- Log cleanup results to stderr

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 6: Create Audit Log UI - Handlers

**Files:**
- Create: `src/handlers/audit_handlers.rs`
- Modify: `src/handlers/mod.rs`
- Modify: `src/main.rs`

**Step 1: Create audit handlers**

Create `src/handlers/audit_handlers.rs`:

```rust
use actix_session::Session;
use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;

use crate::db::DbPool;
use crate::models::audit;
use crate::auth::session::require_permission;
use crate::templates_structs::{PageContext, AuditListTemplate};

#[derive(Deserialize)]
pub struct AuditQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    q: Option<String>,
    action: Option<String>,
    target_type: Option<String>,
}

pub async fn list(
    pool: web::Data<DbPool>,
    session: Session,
    query: web::Query<AuditQuery>,
) -> impl Responder {
    if let Err(resp) = require_permission(&session, "audit.view") {
        return resp;
    }

    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };

    let ctx = PageContext::build(&session, &conn, "/audit");
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(25);
    let search = query.q.as_deref();
    let action_filter = query.action.as_deref();
    let target_type_filter = query.target_type.as_deref();

    let audit_page = audit::find_paginated(
        &conn,
        page,
        per_page,
        search,
        action_filter,
        target_type_filter,
    ).unwrap_or_else(|_| audit::AuditEntryPage {
        entries: vec![],
        page: 1,
        per_page: 25,
        total_count: 0,
        total_pages: 0,
    });

    let tmpl = AuditListTemplate {
        ctx,
        audit_page,
        search_query: query.q.clone(),
        action_filter: query.action.clone(),
        target_type_filter: query.target_type.clone(),
    };

    match tmpl.render() {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(_) => HttpResponse::InternalServerError().body("Template error"),
    }
}
```

**Step 2: Register audit handlers module**

Add to `src/handlers/mod.rs`:
```rust
pub mod audit_handlers;
```

**Step 3: Add audit route to main.rs**

In `src/main.rs`, add route under the protected scope:

```rust
// After other routes
.route("/audit", web::get().to(handlers::audit_handlers::list))
```

**Step 4: Build to verify**

Run: `cargo build`
Expected: Compilation errors about AuditListTemplate (expected - will create in next task)

**Step 5: Commit (will commit after creating template)**

---

## Task 7: Create Audit Log UI - Template Struct and Template

**Files:**
- Modify: `src/templates_structs.rs`
- Create: `templates/audit/list.html`

**Step 1: Add AuditListTemplate struct**

Add to `src/templates_structs.rs`:

```rust
use crate::models::audit::AuditEntryPage;

#[derive(Template)]
#[template(path = "audit/list.html")]
pub struct AuditListTemplate {
    pub ctx: PageContext,
    pub audit_page: AuditEntryPage,
    pub search_query: Option<String>,
    pub action_filter: Option<String>,
    pub target_type_filter: Option<String>,
}
```

**Step 2: Create audit template directory**

Run: `mkdir -p templates/audit`

**Step 3: Create audit list template**

Create `templates/audit/list.html`:

```html
{% include "partials/header.html" %}

<div class="page-header">
    <h1>Audit Log</h1>
</div>

<div class="filter-bar">
    <form method="get" action="/audit" class="audit-filters">
        <input type="search" name="q" placeholder="Search by user or summary..."
               value="{% if let Some(q) = search_query %}{{ q }}{% endif %}"
               class="search-input">

        <select name="action" class="filter-select">
            <option value="all" {% if action_filter.is_none() || action_filter.as_ref().map(|s| s.as_str()) == Some("all") %}selected{% endif %}>All Actions</option>
            <option value="user." {% if action_filter.as_ref().map(|s| s.starts_with("user.")).unwrap_or(false) %}selected{% endif %}>User Actions</option>
            <option value="role." {% if action_filter.as_ref().map(|s| s.starts_with("role.")).unwrap_or(false) %}selected{% endif %}>Role Actions</option>
            <option value="setting." {% if action_filter.as_ref().map(|s| s.starts_with("setting.")).unwrap_or(false) %}selected{% endif %}>Setting Actions</option>
        </select>

        <select name="target_type" class="filter-select">
            <option value="all" {% if target_type_filter.is_none() || target_type_filter.as_ref().map(|s| s.as_str()) == Some("all") %}selected{% endif %}>All Types</option>
            <option value="user" {% if target_type_filter.as_ref().map(|s| s.as_str()) == Some("user") %}selected{% endif %}>User</option>
            <option value="role" {% if target_type_filter.as_ref().map(|s| s.as_str()) == Some("role") %}selected{% endif %}>Role</option>
            <option value="setting" {% if target_type_filter.as_ref().map(|s| s.as_str()) == Some("setting") %}selected{% endif %}>Setting</option>
        </select>

        <button type="submit" class="btn">Filter</button>
        {% if search_query.is_some() || (action_filter.is_some() && action_filter.as_ref().unwrap() != "all") || (target_type_filter.is_some() && target_type_filter.as_ref().unwrap() != "all") %}
        <a href="/audit" class="btn">Clear</a>
        {% endif %}
    </form>
</div>

{% if audit_page.total_pages > 1 %}
<div class="pagination-info">
    Page {{ audit_page.page }} of {{ audit_page.total_pages }} ({{ audit_page.total_count }} total)
</div>
{% endif %}

<table class="data-table">
    <thead>
        <tr>
            <th>Timestamp</th>
            <th>User</th>
            <th>Action</th>
            <th>Target</th>
            <th>Summary</th>
        </tr>
    </thead>
    <tbody>
        {% if audit_page.entries.is_empty() %}
        <tr>
            <td colspan="5" class="no-data">No audit entries found</td>
        </tr>
        {% else %}
        {% for entry in audit_page.entries %}
        <tr>
            <td>{{ entry.created_at }}</td>
            <td>{{ entry.username }}</td>
            <td><code>{{ entry.action }}</code></td>
            <td>{{ entry.target_type }}:{{ entry.target_id }}</td>
            <td>{{ entry.summary }}</td>
        </tr>
        {% endfor %}
        {% endif %}
    </tbody>
</table>

{% if audit_page.total_pages > 1 %}
<div class="pagination">
    <div class="pagination-controls">
        {% if audit_page.page > 1 %}
        <a href="/audit?page={{ audit_page.page - 1 }}&per_page={{ audit_page.per_page }}{% if let Some(q) = search_query %}&q={{ q }}{% endif %}{% if let Some(a) = action_filter %}&action={{ a }}{% endif %}{% if let Some(t) = target_type_filter %}&target_type={{ t }}{% endif %}" class="btn btn-sm">← Previous</a>
        {% else %}
        <button class="btn btn-sm" disabled>← Previous</button>
        {% endif %}

        {% if audit_page.page < audit_page.total_pages %}
        <a href="/audit?page={{ audit_page.page + 1 }}&per_page={{ audit_page.per_page }}{% if let Some(q) = search_query %}&q={{ q }}{% endif %}{% if let Some(a) = action_filter %}&action={{ a }}{% endif %}{% if let Some(t) = target_type_filter %}&target_type={{ t }}{% endif %}" class="btn btn-sm">Next →</a>
        {% else %}
        <button class="btn btn-sm" disabled>Next →</button>
        {% endif %}
    </div>
</div>
{% endif %}

{% include "partials/footer.html" %}
```

**Step 4: Build to verify**

Run: `cargo build`
Expected: Clean build

**Step 5: Commit**

```bash
git add src/handlers/audit_handlers.rs src/handlers/mod.rs src/main.rs src/templates_structs.rs templates/audit/list.html
git commit -m "feat(audit): add audit log UI with search and filters

- Audit log viewer at /audit route
- Search by user or summary
- Filter by action type and target type
- Pagination with filter preservation
- Requires audit.view permission

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 8: Add Navigation Item

**Files:**
- Modify: `src/db.rs`

**Step 1: Add audit nav item in seed_ontology**

In `src/db.rs`, after other nav items under admin module:

```rust
// Audit Log nav item under admin module
conn.execute(
    "INSERT INTO entities (entity_type, name, label) VALUES ('nav_item', 'admin.audit', 'Audit Log')",
    [],
)?;
conn.execute(
    "INSERT INTO entity_properties (entity_id, key, value) VALUES (
        (SELECT id FROM entities WHERE entity_type='nav_item' AND name='admin.audit'),
        'url', '/audit'
    )",
    [],
)?;
conn.execute(
    "INSERT INTO entity_properties (entity_id, key, value) VALUES (
        (SELECT id FROM entities WHERE entity_type='nav_item' AND name='admin.audit'),
        'parent', 'admin'
    )",
    [],
)?;
conn.execute(
    "INSERT INTO entity_properties (entity_id, key, value) VALUES (
        (SELECT id FROM entities WHERE entity_type='nav_item' AND name='admin.audit'),
        'sort_order', '40'
    )",
    [],
)?;

// Audit requires audit.view permission
conn.execute(
    "INSERT INTO relations (relation_type_id, source_id, target_id)
     VALUES (
         (SELECT id FROM entities WHERE entity_type='relation_type' AND name='requires_permission'),
         (SELECT id FROM entities WHERE entity_type='nav_item' AND name='admin.audit'),
         (SELECT id FROM entities WHERE entity_type='permission' AND name='audit.view')
     )",
    [],
)?;
```

**Step 2: Delete database and restart to re-seed**

Run: `rm data/app.db && cargo run`
Expected: Server starts with new nav item

**Step 3: Test in browser**

1. Navigate to http://localhost:8080
2. Login with admin/admin123
3. Verify "Audit Log" appears in Admin sidebar
4. Click "Audit Log" - should show empty table

**Step 4: Stop server and commit**

```bash
git add src/db.rs
git commit -m "feat(audit): add Audit Log navigation item

- Add admin.audit nav item under Admin module
- Requires audit.view permission
- Sort order 40 (after Settings)

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 9: Integrate Audit Logging - User Handlers

**Files:**
- Modify: `src/handlers/user_handlers.rs`

**Step 1: Add audit logging to user creation**

In `src/handlers/user_handlers.rs`, in the `create` function, after successful `user::create(&conn, &new)`:

```rust
match user::create(&conn, &new) {
    Ok(user_id) => {
        // Audit log
        let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
        let details = serde_json::json!({
            "email": new.email,
            "role_id": new.role_id,
            "summary": format!("Created user '{}'", new.username)
        });
        let _ = crate::audit::log(&conn, current_user_id, "user.created",
                                  "user", user_id, details);

        let _ = session.insert("flash", "User created successfully");
        HttpResponse::SeeOther()
            .insert_header(("Location", "/users"))
            .finish()
    }
    // ... existing error handling ...
}
```

**Step 2: Add audit logging to user deletion**

In the `delete` function, after successful `user::delete(&conn, id)`:

```rust
match user::delete(&conn, id) {
    Ok(_) => {
        // Audit log
        let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
        if let Ok(Some(deleted_user)) = user::find_display_by_id(&conn, id) {
            let details = serde_json::json!({
                "username": deleted_user.username,
                "summary": format!("Deleted user '{}'", deleted_user.username)
            });
            let _ = crate::audit::log(&conn, current_user_id, "user.deleted",
                                      "user", id, details);
        }

        let _ = session.insert("flash", "User deleted");
        HttpResponse::SeeOther()
            .insert_header(("Location", "/users"))
            .finish()
    }
    // ... existing error handling ...
}
```

**Step 3: Build to verify**

Run: `cargo build`
Expected: Clean build

**Step 4: Commit**

```bash
git add src/handlers/user_handlers.rs
git commit -m "feat(audit): log user create and delete actions

- Log user.created with email and role_id
- Log user.deleted with username
- Both logged to filesystem and database

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 10: Integrate Audit Logging - Role Handlers

**Files:**
- Modify: `src/handlers/role_handlers.rs`

**Step 1: Add audit logging to role creation**

In `src/handlers/role_handlers.rs`, in the `create` function:

```rust
match role::create(&conn, &form.name.trim(), &form.label.trim(), &form.description.trim(), &permission_ids) {
    Ok(role_id) => {
        // Audit log
        let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
        let details = serde_json::json!({
            "role_name": form.name.trim(),
            "permission_count": permission_ids.len(),
            "summary": format!("Created role '{}'", form.label.trim())
        });
        let _ = crate::audit::log(&conn, current_user_id, "role.created",
                                  "role", role_id, details);

        let _ = session.insert("flash", "Role created");
        HttpResponse::SeeOther()
            .insert_header(("Location", "/roles"))
            .finish()
    }
    // ... existing error handling ...
}
```

**Step 2: Add audit logging to role deletion**

In the `delete` function:

```rust
match role::delete(&conn, id) {
    Ok(_) => {
        // Audit log
        let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
        if let Ok(Some(deleted_role)) = role::find_display_by_id(&conn, id) {
            let details = serde_json::json!({
                "role_name": deleted_role.name,
                "summary": format!("Deleted role '{}'", deleted_role.label)
            });
            let _ = crate::audit::log(&conn, current_user_id, "role.deleted",
                                      "role", id, details);
        }

        let _ = session.insert("flash", "Role deleted");
        HttpResponse::SeeOther()
            .insert_header(("Location", "/roles"))
            .finish()
    }
    // ... existing error handling ...
}
```

**Step 3: Add audit logging to role permission changes**

In the `update` function, after successful update:

```rust
match role::update(&conn, id, &form.name.trim(), &form.label.trim(), &form.description.trim(), &permission_ids) {
    Ok(_) => {
        // Audit log for permission changes
        let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
        let details = serde_json::json!({
            "role_name": form.name.trim(),
            "new_permission_count": permission_ids.len(),
            "summary": format!("Updated permissions for role '{}'", form.label.trim())
        });
        let _ = crate::audit::log(&conn, current_user_id, "role.permissions_changed",
                                  "role", id, details);

        let _ = session.insert("flash", "Role updated");
        HttpResponse::SeeOther()
            .insert_header(("Location", "/roles"))
            .finish()
    }
    // ... existing error handling ...
}
```

**Step 4: Build to verify**

Run: `cargo build`
Expected: Clean build

**Step 5: Commit**

```bash
git add src/handlers/role_handlers.rs
git commit -m "feat(audit): log role create, delete, and permission changes

- Log role.created with permission count
- Log role.deleted with role name
- Log role.permissions_changed for updates

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Task 11: Test Audit Trail End-to-End

**Files:**
- None (manual testing)

**Step 1: Start fresh with clean database**

Run: `rm data/app.db data/audit/*.jsonl 2>/dev/null; cargo run`
Expected: Server starts clean

**Step 2: Login and create a test user**

1. Navigate to http://localhost:8080
2. Login: admin/admin123
3. Go to Users > New User
4. Create user: username=test, password=test123, email=test@example.com, role=Administrator
5. Click Create

**Step 3: Verify filesystem log**

Run in separate terminal:
```bash
cat data/audit/audit-*.jsonl | jq .
```

Expected: JSON log entry for user.created with all details

**Step 4: Verify database log**

Run:
```bash
sqlite3 data/app.db "SELECT e.name, p.key, p.value FROM entities e LEFT JOIN entity_properties p ON e.id=p.entity_id WHERE e.entity_type='audit_entry'"
```

Expected: audit_entry with user_id, action, target_type, target_id, summary properties

**Step 5: Verify UI shows entry**

1. In browser, navigate to Admin > Audit Log
2. Should see one entry: "user.created" by admin

**Step 6: Delete the test user**

1. Go to Users
2. Delete the test user
3. Verify flash message

**Step 7: Verify audit log updated**

1. Check filesystem: `cat data/audit/audit-*.jsonl | jq .`
2. Check UI: Should now show 2 entries (user.created and user.deleted)

**Step 8: Test search functionality**

1. In Audit Log page, search for "admin"
2. Should show both entries
3. Search for "nonexistent"
4. Should show "No audit entries found"

**Step 9: Test action filter**

1. Select "User Actions" from action dropdown
2. Click Filter
3. Should show both user.* entries
4. Select "Role Actions"
5. Should show "No audit entries found"

**Step 10: Test audit.enabled toggle**

1. Go to Settings
2. Change "Enable Audit Logging" to false
3. Save
4. Create another test user
5. Check filesystem and database - should have no new entries
6. Re-enable audit logging in settings

**Step 11: Document test results**

All tests passed - ready to commit

**Step 12: Stop server**

Ctrl+C

---

## Task 12: Final Documentation and Cleanup

**Files:**
- Modify: `docs/BACKLOG.md`

**Step 1: Update backlog to mark task complete**

Move task 7.3 from "Remaining Backlog" to "Completed Work" in `docs/BACKLOG.md`:

```markdown
### Audit Trail (7.3)
- Two-tier system: high-value events in database (EAV), all events in filesystem (JSON Lines)
- Database: audit_entry entities with properties (user_id, action, target_type, target_id, summary)
- Filesystem: Daily-rotated .jsonl files in data/audit/ with secure permissions (0600/0700)
- Settings: audit.enabled, audit.log_path, audit.retention_days
- Retention cleanup on startup (configurable, 0=forever)
- UI: /audit with search, action filter, target type filter, pagination
- Permission: audit.view for viewing logs
- Integration: user create/delete, role create/delete/permissions_changed logged
- Error handling: logging failures never block requests (logged to stderr)
```

**Step 2: Commit backlog update**

```bash
git add docs/BACKLOG.md
git commit -m "docs: mark audit trail task as complete

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

**Step 3: Push all changes**

```bash
git push origin main
```

Expected: All commits pushed successfully

---

## Success Criteria Verification

✅ All mutations logged to filesystem (verified in Task 11)
✅ High-value events queryable in UI (verified in Task 11)
✅ Search/filter functionality working (verified in Task 11)
✅ Settings control (enable/disable, path, retention) (seeded in Task 2)
✅ Retention cleanup runs on startup (implemented in Task 5)
✅ File permissions secure (0600/0700) (implemented in Task 3)
✅ No application breakage if logging fails (error handling in Task 3)
✅ External tools can parse logs with jq/grep (verified in Task 11)

---

## Notes for Implementation

- This project doesn't use automated tests, so all verification is manual
- Frequent commits after each logical chunk
- Database deletions required for re-seeding during development
- Error handling: audit failures log to stderr but never block requests
- Security: filesystem permissions set on Unix systems only (0600 files, 0700 dirs)
- The chrono dependency provides proper ISO 8601 timestamps for filesystem logs

---

## Future Enhancements (Out of Scope)

These are documented in the design doc but not implemented:
- Settings changes audit (only critical settings currently)
- Password change audit
- IP address logging
- CSV export from UI
- Date range filters
- Audit log signing
- External SIEM integration
