# Warnings System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement a real-time warnings system with EAV entities, per-user receipt tracking, event audit trails, WebSocket push, and a warnings UI page.

**Architecture:** Warnings are EAV entities (entity_type `warning`) with `warning_receipt` junction entities per user and `warning_event` entities for audit trails. WebSocket via `actix-ws` pushes real-time updates. Event-driven generators fire inline in handlers; scheduled generators run in a background task every 5 minutes.

**Tech Stack:** Rust, Actix-web 4, actix-ws, rusqlite, Askama templates, vanilla JS WebSocket

**Design doc:** `docs/plans/2026-02-16-warnings-system-design.md`

---

## Task 1: Add `actix-ws` dependency and seed new relation types

**Files:**
- Modify: `Cargo.toml` (add actix-ws dependency)
- Modify: `src/db.rs` (seed new relation types in `seed_ontology` and new settings)

**Step 1: Add actix-ws to Cargo.toml**

Add under `[dependencies]`:
```toml
actix-ws = "0.3"
tokio = { version = "1", features = ["time"] }
```

**Step 2: Seed new relation types in `src/db.rs`**

In `seed_ontology()`, after the existing relation type seeds (after line ~93), add:

```rust
// Warning system relation types
let _targets_user_id = insert_entity(&conn, "relation_type", "targets_user", "Targets User", 0);
let _for_warning_id = insert_entity(&conn, "relation_type", "for_warning", "For Warning", 0);
let _for_user_id = insert_entity(&conn, "relation_type", "for_user", "For User", 0);
let _on_receipt_id = insert_entity(&conn, "relation_type", "on_receipt", "On Receipt", 0);
let _forwarded_to_user_id = insert_entity(&conn, "relation_type", "forwarded_to_user", "Forwarded To User", 0);
```

**Step 3: Seed warning settings in `src/db.rs`**

In `seed_ontology()`, after the audit settings block (after line ~274), add:

```rust
// Warning retention settings
let warn_ret_resolved = insert_entity(&conn, "setting", "warnings.retention_resolved_days", "Warning Retention (Resolved)", 6);
insert_prop(&conn, warn_ret_resolved, "value", "30");
insert_prop(&conn, warn_ret_resolved, "description", "Days to keep resolved warnings before deletion");
insert_prop(&conn, warn_ret_resolved, "setting_type", "number");

let warn_ret_info = insert_entity(&conn, "setting", "warnings.retention_info_days", "Warning Auto-Resolve (Info)", 7);
insert_prop(&conn, warn_ret_info, "value", "90");
insert_prop(&conn, warn_ret_info, "description", "Days before info-severity warnings are auto-resolved");
insert_prop(&conn, warn_ret_info, "setting_type", "number");

let warn_ret_deleted = insert_entity(&conn, "setting", "warnings.retention_deleted_days", "Warning Retention (Deleted)", 8);
insert_prop(&conn, warn_ret_deleted, "value", "7");
insert_prop(&conn, warn_ret_deleted, "description", "Days to keep fully-dismissed warnings before deletion");
insert_prop(&conn, warn_ret_deleted, "setting_type", "number");
```

**Step 4: Build and verify**

Run: `cargo check`
Expected: Compiles with no errors. You may need to delete `data/dev/app.db` to re-seed.

**Step 5: Commit**

```bash
rm data/dev/app.db  # Force re-seed with new relation types
git add Cargo.toml src/db.rs
git commit -m "feat: add actix-ws dependency and seed warning relation types + settings"
```

---

## Task 2: Create `src/warnings/mod.rs` — core warning creation functions

**Files:**
- Create: `src/warnings/mod.rs`
- Modify: `src/main.rs` (add `mod warnings`)

**Step 1: Create the warnings module**

Create `src/warnings/mod.rs`:

```rust
pub mod queries;

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
    let name_pattern = format!("{}.%.%", source_action);
    // Check for active warning matching the source_action prefix
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

    // Find all receipts for this warning
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
```

**Step 2: Register the module in `src/main.rs`**

Add `mod warnings;` after the existing mod declarations (after `mod templates_structs;`).

**Step 3: Build and verify**

Run: `cargo check`
Expected: Compiles with no errors.

**Step 4: Commit**

```bash
git add src/warnings/mod.rs src/main.rs
git commit -m "feat: warnings core module with create/receipt/event functions"
```

---

## Task 3: Create `src/warnings/queries.rs` — query functions for warnings

**Files:**
- Create: `src/warnings/queries.rs`

**Step 1: Create the queries module**

Create `src/warnings/queries.rs`:

```rust
use rusqlite::{Connection, params};
use serde::Serialize;

/// Count of unread warnings for a specific user.
pub fn count_unread(conn: &Connection, user_id: i64) -> i64 {
    conn.query_row(
        "SELECT COUNT(*)
         FROM entities receipt
         JOIN entity_properties st ON st.entity_id = receipt.id AND st.key = 'status'
         JOIN relations r_user ON r_user.source_id = receipt.id
         JOIN entities rt_user ON rt_user.id = r_user.relation_type_id AND rt_user.name = 'for_user'
         WHERE receipt.entity_type = 'warning_receipt'
           AND st.value = 'unread'
           AND r_user.target_id = ?1",
        params![user_id],
        |row| row.get(0),
    ).unwrap_or(0)
}

/// A warning for display in list view.
#[derive(Debug, Clone, Serialize)]
pub struct WarningListItem {
    pub warning_id: i64,
    pub receipt_id: i64,
    pub severity: String,
    pub category: String,
    pub message: String,
    pub status: String,
    pub status_at: String,
    pub created_at: String,
}

/// Page of warnings for a user.
pub struct WarningPage {
    pub items: Vec<WarningListItem>,
    pub page: i64,
    pub per_page: i64,
    pub total_count: i64,
    pub total_pages: i64,
}

/// Find paginated warnings for a user with optional filters.
pub fn find_for_user(
    conn: &Connection,
    user_id: i64,
    page: i64,
    per_page: i64,
    category_filter: Option<&str>,
    severity_filter: Option<&str>,
    show_read: bool,
    show_deleted: bool,
) -> rusqlite::Result<WarningPage> {
    let page = page.max(1);
    let per_page = per_page.clamp(1, 100);
    let offset = (page - 1) * per_page;

    let base_from = "\
        FROM entities receipt \
        JOIN entity_properties rst ON rst.entity_id = receipt.id AND rst.key = 'status' \
        JOIN entity_properties rsa ON rsa.entity_id = receipt.id AND rsa.key = 'status_at' \
        JOIN relations r_user ON r_user.source_id = receipt.id \
        JOIN entities rt_user ON rt_user.id = r_user.relation_type_id AND rt_user.name = 'for_user' \
        JOIN relations r_warn ON r_warn.source_id = receipt.id \
        JOIN entities rt_warn ON rt_warn.id = r_warn.relation_type_id AND rt_warn.name = 'for_warning' \
        JOIN entities w ON w.id = r_warn.target_id \
        JOIN entity_properties wsev ON wsev.entity_id = w.id AND wsev.key = 'severity' \
        JOIN entity_properties wcat ON wcat.entity_id = w.id AND wcat.key = 'category' \
        JOIN entity_properties wmsg ON wmsg.entity_id = w.id AND wmsg.key = 'message' \
        WHERE receipt.entity_type = 'warning_receipt' \
          AND r_user.target_id = ?1";

    let mut filters = Vec::new();
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    params_vec.push(Box::new(user_id));

    // Status filters
    let mut excluded_statuses = Vec::new();
    if !show_read {
        excluded_statuses.push("read");
    }
    if !show_deleted {
        excluded_statuses.push("deleted");
    }
    for status in &excluded_statuses {
        filters.push(format!("rst.value != ?{}", params_vec.len() + 1));
        params_vec.push(Box::new(status.to_string()));
    }

    if let Some(cat) = category_filter.filter(|c| c != &"all") {
        filters.push(format!("wcat.value = ?{}", params_vec.len() + 1));
        params_vec.push(Box::new(cat.to_string()));
    }

    if let Some(sev) = severity_filter.filter(|s| s != &"all") {
        filters.push(format!("wsev.value = ?{}", params_vec.len() + 1));
        params_vec.push(Box::new(sev.to_string()));
    }

    let filter_clause = if filters.is_empty() {
        String::new()
    } else {
        format!(" AND {}", filters.join(" AND "))
    };

    // Count
    let count_sql = format!("SELECT COUNT(*) {}{}", base_from, filter_clause);
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();
    let total_count: i64 = conn.query_row(&count_sql, param_refs.as_slice(), |row| row.get(0))?;
    let total_pages = ((total_count as f64) / (per_page as f64)).ceil() as i64;

    // Results
    let select_sql = format!(
        "SELECT w.id as warning_id, receipt.id as receipt_id, \
                wsev.value as severity, wcat.value as category, \
                wmsg.value as message, rst.value as status, \
                rsa.value as status_at, w.created_at \
         {} {} \
         ORDER BY CASE rst.value WHEN 'unread' THEN 0 ELSE 1 END, w.created_at DESC \
         LIMIT ?{} OFFSET ?{}",
        base_from, filter_clause,
        params_vec.len() + 1, params_vec.len() + 2,
    );

    params_vec.push(Box::new(per_page));
    params_vec.push(Box::new(offset));
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|b| b.as_ref()).collect();

    let mut stmt = conn.prepare(&select_sql)?;
    let items = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(WarningListItem {
            warning_id: row.get("warning_id")?,
            receipt_id: row.get("receipt_id")?,
            severity: row.get("severity")?,
            category: row.get("category")?,
            message: row.get("message")?,
            status: row.get("status")?,
            status_at: row.get("status_at")?,
            created_at: row.get("created_at")?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;

    Ok(WarningPage { items, page, per_page, total_count, total_pages })
}

/// Warning detail for the detail page.
#[derive(Debug, Clone)]
pub struct WarningDetail {
    pub id: i64,
    pub severity: String,
    pub category: String,
    pub message: String,
    pub source_action: String,
    pub details: String,
    pub status: String,
    pub scope: String,
    pub created_at: String,
}

/// Recipient with status info.
#[derive(Debug, Clone)]
pub struct WarningRecipient {
    pub user_id: i64,
    pub username: String,
    pub user_label: String,
    pub receipt_id: i64,
    pub status: String,
    pub status_at: String,
}

/// Event in the timeline.
#[derive(Debug, Clone)]
pub struct WarningTimelineEvent {
    pub action: String,
    pub actor_user_id: i64,
    pub actor_username: String,
    pub created_at: String,
    pub note: String,
}

/// Get full warning detail by warning entity ID.
pub fn get_warning_detail(conn: &Connection, warning_id: i64) -> rusqlite::Result<Option<WarningDetail>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.created_at,
                COALESCE(psev.value, '') as severity,
                COALESCE(pcat.value, '') as category,
                COALESCE(pmsg.value, '') as message,
                COALESCE(psa.value, '') as source_action,
                COALESCE(pdet.value, '') as details,
                COALESCE(pst.value, '') as status,
                COALESCE(psc.value, '') as scope
         FROM entities e
         LEFT JOIN entity_properties psev ON e.id = psev.entity_id AND psev.key = 'severity'
         LEFT JOIN entity_properties pcat ON e.id = pcat.entity_id AND pcat.key = 'category'
         LEFT JOIN entity_properties pmsg ON e.id = pmsg.entity_id AND pmsg.key = 'message'
         LEFT JOIN entity_properties psa ON e.id = psa.entity_id AND psa.key = 'source_action'
         LEFT JOIN entity_properties pdet ON e.id = pdet.entity_id AND pdet.key = 'details'
         LEFT JOIN entity_properties pst ON e.id = pst.entity_id AND pst.key = 'status'
         LEFT JOIN entity_properties psc ON e.id = psc.entity_id AND psc.key = 'scope'
         WHERE e.id = ?1 AND e.entity_type = 'warning'"
    )?;
    let mut rows = stmt.query_map(params![warning_id], |row| {
        Ok(WarningDetail {
            id: row.get("id")?,
            severity: row.get("severity")?,
            category: row.get("category")?,
            message: row.get("message")?,
            source_action: row.get("source_action")?,
            details: row.get("details")?,
            status: row.get("status")?,
            scope: row.get("scope")?,
            created_at: row.get("created_at")?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Get all recipients for a warning with their receipt status.
pub fn get_recipients(conn: &Connection, warning_id: i64) -> rusqlite::Result<Vec<WarningRecipient>> {
    let mut stmt = conn.prepare(
        "SELECT u.id as user_id, u.name as username, u.label as user_label,
                receipt.id as receipt_id,
                COALESCE(rst.value, 'unread') as status,
                COALESCE(rsa.value, '') as status_at
         FROM entities receipt
         JOIN relations r_warn ON r_warn.source_id = receipt.id
         JOIN entities rt_warn ON rt_warn.id = r_warn.relation_type_id AND rt_warn.name = 'for_warning'
         JOIN relations r_user ON r_user.source_id = receipt.id
         JOIN entities rt_user ON rt_user.id = r_user.relation_type_id AND rt_user.name = 'for_user'
         JOIN entities u ON u.id = r_user.target_id
         LEFT JOIN entity_properties rst ON rst.entity_id = receipt.id AND rst.key = 'status'
         LEFT JOIN entity_properties rsa ON rsa.entity_id = receipt.id AND rsa.key = 'status_at'
         WHERE r_warn.target_id = ?1 AND receipt.entity_type = 'warning_receipt'
         ORDER BY u.name"
    )?;
    let rows = stmt.query_map(params![warning_id], |row| {
        Ok(WarningRecipient {
            user_id: row.get("user_id")?,
            username: row.get("username")?,
            user_label: row.get("user_label")?,
            receipt_id: row.get("receipt_id")?,
            status: row.get("status")?,
            status_at: row.get("status_at")?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Get event timeline for a receipt.
pub fn get_receipt_timeline(conn: &Connection, receipt_id: i64) -> rusqlite::Result<Vec<WarningTimelineEvent>> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(pa.value, '') as action,
                COALESCE(pau.value, '0') as actor_user_id,
                COALESCE(u.name, 'system') as actor_username,
                evt.created_at,
                COALESCE(pn.value, '') as note
         FROM entities evt
         JOIN relations r ON r.source_id = evt.id
         JOIN entities rt ON rt.id = r.relation_type_id AND rt.name = 'on_receipt'
         LEFT JOIN entity_properties pa ON pa.entity_id = evt.id AND pa.key = 'action'
         LEFT JOIN entity_properties pau ON pau.entity_id = evt.id AND pau.key = 'actor_user_id'
         LEFT JOIN entities u ON u.id = CAST(pau.value AS INTEGER) AND u.entity_type = 'user'
         LEFT JOIN entity_properties pn ON pn.entity_id = evt.id AND pn.key = 'note'
         WHERE evt.entity_type = 'warning_event' AND r.target_id = ?1
         ORDER BY evt.created_at ASC"
    )?;
    let rows = stmt.query_map(params![receipt_id], |row| {
        Ok(WarningTimelineEvent {
            action: row.get("action")?,
            actor_user_id: row.get::<_, String>("actor_user_id")?.parse().unwrap_or(0),
            actor_username: row.get("actor_username")?,
            created_at: row.get("created_at")?,
            note: row.get("note")?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Find a receipt for a specific user and warning.
pub fn find_receipt_for_user(conn: &Connection, warning_id: i64, user_id: i64) -> rusqlite::Result<Option<i64>> {
    let receipt_name = format!("wr.{}.{}", warning_id, user_id);
    let mut stmt = conn.prepare(
        "SELECT id FROM entities WHERE entity_type = 'warning_receipt' AND name = ?1"
    )?;
    let mut rows = stmt.query_map(params![receipt_name], |row| row.get::<_, i64>(0))?;
    match rows.next() {
        Some(id) => Ok(Some(id?)),
        None => Ok(None),
    }
}
```

**Step 2: Build and verify**

Run: `cargo check`
Expected: Compiles with no errors.

**Step 3: Commit**

```bash
git add src/warnings/queries.rs
git commit -m "feat: warning query functions for list, detail, recipients, timeline"
```

---

## Task 4: Wire `warning_count` into PageContext

**Files:**
- Modify: `src/templates_structs.rs:47` (replace hardcoded 0)

**Step 1: Update PageContext::build**

In `src/templates_structs.rs`, add import at top:
```rust
use crate::warnings::queries as warning_queries;
```

Replace line 47 (`let warning_count = 0;`) with:
```rust
let user_id = crate::auth::session::get_user_id(session).unwrap_or(0);
let warning_count = warning_queries::count_unread(&conn, user_id);
```

**Step 2: Build and verify**

Run: `cargo check`
Expected: Compiles. The badge in the nav now reflects real data.

**Step 3: Commit**

```bash
git add src/templates_structs.rs
git commit -m "feat: wire real unread warning count into PageContext badge"
```

---

## Task 5: Create WebSocket infrastructure

**Files:**
- Create: `src/handlers/warning_handlers/mod.rs`
- Create: `src/handlers/warning_handlers/ws.rs`
- Modify: `src/handlers/mod.rs` (add module)
- Modify: `src/main.rs` (add ConnectionMap + WS route)

**Step 1: Create `src/handlers/warning_handlers/mod.rs`**

```rust
pub mod ws;
pub mod list;
pub mod detail;
pub mod actions;
```

Note: `list.rs`, `detail.rs`, and `actions.rs` will be created in later tasks. For now, create them as empty placeholder files so the module compiles. Each placeholder can contain just a comment: `// TODO: implement in Task 7/8`.

**Step 2: Create `src/handlers/warning_handlers/ws.rs`**

```rust
use actix_session::Session;
use actix_web::{web, HttpRequest, HttpResponse};
use actix_ws::Message;
use std::collections::HashMap;
use std::sync::RwLock;
use tokio::sync::mpsc;

use crate::auth::session::get_user_id;
use crate::db::DbPool;
use crate::warnings::queries;

pub type ConnectionMap = std::sync::Arc<RwLock<HashMap<i64, Vec<mpsc::UnboundedSender<String>>>>>;

pub fn new_connection_map() -> ConnectionMap {
    std::sync::Arc::new(RwLock::new(HashMap::new()))
}

/// Notify connected users about a new warning.
pub fn notify_users(
    conn_map: &ConnectionMap,
    pool: &DbPool,
    target_user_ids: &[i64],
    warning_id: i64,
    severity: &str,
    title: &str,
) {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return,
    };
    let map = match conn_map.read() {
        Ok(m) => m,
        Err(_) => return,
    };
    for &user_id in target_user_ids {
        if let Some(senders) = map.get(&user_id) {
            let unread = queries::count_unread(&conn, user_id);
            let msg = serde_json::json!({
                "type": "new_warning",
                "warning_id": warning_id,
                "severity": severity,
                "title": title,
                "unread_count": unread,
            });
            let msg_str = msg.to_string();
            for sender in senders {
                let _ = sender.send(msg_str.clone());
            }
        }
    }
}

/// Send count update to a specific user.
pub fn send_count_update(conn_map: &ConnectionMap, pool: &DbPool, user_id: i64) {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return,
    };
    let unread = queries::count_unread(&conn, user_id);
    let msg = serde_json::json!({
        "type": "count_update",
        "unread_count": unread,
    });
    let msg_str = msg.to_string();
    let map = match conn_map.read() {
        Ok(m) => m,
        Err(_) => return,
    };
    if let Some(senders) = map.get(&user_id) {
        for sender in senders {
            let _ = sender.send(msg_str.clone());
        }
    }
}

/// WebSocket upgrade handler.
pub async fn ws_connect(
    req: HttpRequest,
    body: web::Payload,
    session: Session,
    conn_map: web::Data<ConnectionMap>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = match get_user_id(&session) {
        Some(id) => id,
        None => return Ok(HttpResponse::Unauthorized().finish()),
    };

    let (response, mut ws_session, mut msg_stream) = actix_ws::handle(&req, body)?;

    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    // Register this connection
    {
        let mut map = conn_map.write().unwrap();
        map.entry(user_id).or_default().push(tx);
    }

    let conn_map_clone = conn_map.into_inner().clone();

    actix_web::rt::spawn(async move {
        loop {
            tokio::select! {
                // Forward server messages to client
                Some(msg) = rx.recv() => {
                    if ws_session.text(msg).await.is_err() {
                        break;
                    }
                }
                // Handle client messages
                Some(Ok(msg)) = msg_stream.recv() => {
                    match msg {
                        Message::Ping(bytes) => {
                            if ws_session.pong(&bytes).await.is_err() {
                                break;
                            }
                        }
                        Message::Close(_) => break,
                        Message::Text(_text) => {
                            // Client messages (mark_read, etc.) handled via HTTP POST
                            // WS is primarily server->client push
                        }
                        _ => {}
                    }
                }
                else => break,
            }
        }

        // Clean up on disconnect
        if let Ok(mut map) = conn_map_clone.write() {
            if let Some(senders) = map.get_mut(&user_id) {
                senders.retain(|s| !s.is_closed());
                if senders.is_empty() {
                    map.remove(&user_id);
                }
            }
        }
    });

    Ok(response)
}
```

**Step 3: Register in `src/handlers/mod.rs`**

Add: `pub mod warning_handlers;`

**Step 4: Update `src/main.rs`**

Add the ConnectionMap as app data and the WS route. Near the top of `main()`, after pool creation:

```rust
let conn_map = handlers::warning_handlers::ws::new_connection_map();
```

In the `App::new()` block, add `.app_data(web::Data::new(conn_map.clone()))` after the pool data line.

Add the WS route inside the protected scope (before the default 404):
```rust
.route("/ws/notifications", web::get().to(handlers::warning_handlers::ws::ws_connect))
```

**Step 5: Create placeholder files**

Create empty `src/handlers/warning_handlers/list.rs`, `src/handlers/warning_handlers/detail.rs`, `src/handlers/warning_handlers/actions.rs` with just a comment.

**Step 6: Build and verify**

Run: `cargo check`
Expected: Compiles with no errors (placeholders may produce dead_code warnings, which is fine).

**Step 7: Commit**

```bash
git add src/handlers/warning_handlers/ src/handlers/mod.rs src/main.rs
git commit -m "feat: WebSocket infrastructure with ConnectionMap and ws_connect handler"
```

---

## Task 6: Add client-side WebSocket JS and toast notifications

**Files:**
- Modify: `templates/partials/nav.html` (add WS JS)
- Create: `static/css/toast.css` (toast styles)

**Step 1: Add WebSocket JS to nav.html**

At the bottom of `templates/partials/nav.html`, add a `<script>` block:

```html
<script>
(function() {
    var protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
    var ws = new WebSocket(protocol + '//' + location.host + '/ws/notifications');
    ws.onmessage = function(e) {
        var msg = JSON.parse(e.data);
        if (msg.unread_count !== undefined) {
            updateBadge(msg.unread_count);
        }
        if (msg.type === 'new_warning' && msg.severity !== 'info') {
            showToast(msg.title, msg.severity);
        }
    };
    ws.onclose = function() {
        // Reconnect after 5 seconds
        setTimeout(function() { location.reload(); }, 5000);
    };

    function updateBadge(count) {
        var badges = document.querySelectorAll('.avatar-badge, .badge-count');
        badges.forEach(function(el) {
            if (count > 0) {
                el.textContent = count;
                el.style.display = '';
            } else {
                el.style.display = 'none';
            }
        });
    }

    function showToast(title, severity) {
        var toast = document.createElement('div');
        toast.className = 'toast toast-' + severity;
        toast.textContent = title;
        document.body.appendChild(toast);
        setTimeout(function() {
            toast.classList.add('toast-fade');
            setTimeout(function() { toast.remove(); }, 300);
        }, 4700);
    }
})();
</script>
```

**Step 2: Add toast CSS**

Create `static/css/toast.css`:

```css
.toast {
    position: fixed;
    bottom: 1rem;
    right: 1rem;
    padding: 0.75rem 1.25rem;
    border-radius: 0.5rem;
    color: #fff;
    font-size: 0.875rem;
    z-index: 9999;
    animation: toast-in 0.3s ease-out;
    max-width: 24rem;
}
.toast-warning { background: #d97706; }
.toast-critical { background: #dc2626; }
.toast-info { background: #2563eb; }
.toast-fade { opacity: 0; transition: opacity 0.3s; }
@keyframes toast-in {
    from { transform: translateY(1rem); opacity: 0; }
    to { transform: translateY(0); opacity: 1; }
}
```

**Step 3: Include toast.css in base.html**

Add a `<link>` tag in `templates/base.html` head section for `toast.css`.

**Step 4: Build and verify**

Run: `cargo check`
Expected: Compiles. Run the app and check browser console — WS should connect (no warnings to push yet).

**Step 5: Commit**

```bash
git add templates/partials/nav.html static/css/toast.css templates/base.html
git commit -m "feat: client-side WebSocket with badge updates and toast notifications"
```

---

## Task 7: Create warning list and detail handlers + templates

**Files:**
- Modify: `src/handlers/warning_handlers/list.rs`
- Modify: `src/handlers/warning_handlers/detail.rs`
- Modify: `src/templates_structs.rs` (add template structs)
- Create: `templates/warnings/list.html`
- Create: `templates/warnings/detail.html`
- Modify: `src/main.rs` (add routes)

**Step 1: Add template structs to `src/templates_structs.rs`**

Add import at top: `use crate::warnings::queries::{WarningPage, WarningDetail, WarningRecipient, WarningTimelineEvent};`

Add at the bottom:

```rust
// --- Warning templates ---

#[derive(Template)]
#[template(path = "warnings/list.html")]
pub struct WarningListTemplate {
    pub ctx: PageContext,
    pub warning_page: WarningPage,
    pub category_filter: Option<String>,
    pub severity_filter: Option<String>,
    pub show_read: bool,
    pub show_deleted: bool,
}

#[derive(Template)]
#[template(path = "warnings/detail.html")]
pub struct WarningDetailTemplate {
    pub ctx: PageContext,
    pub warning: WarningDetail,
    pub recipients: Vec<WarningRecipient>,
    pub timeline: Vec<WarningTimelineEvent>,
    pub user_receipt_id: i64,
    pub users: Vec<UserOption>,
}
```

**Step 2: Implement `src/handlers/warning_handlers/list.rs`**

```rust
use actix_session::Session;
use actix_web::{web, HttpResponse};
use serde::Deserialize;

use crate::db::DbPool;
use crate::auth::session::get_user_id;
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, WarningListTemplate};
use crate::warnings::queries;

#[derive(Deserialize)]
pub struct WarningQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    category: Option<String>,
    severity: Option<String>,
    show_read: Option<String>,
    show_deleted: Option<String>,
}

pub async fn list(
    pool: web::Data<DbPool>,
    session: Session,
    query: web::Query<WarningQuery>,
) -> Result<HttpResponse, AppError> {
    let user_id = get_user_id(&session).ok_or_else(|| AppError::Session("No user".into()))?;
    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/warnings")?;

    let show_read = query.show_read.as_deref() == Some("true");
    let show_deleted = query.show_deleted.as_deref() == Some("true");

    let warning_page = queries::find_for_user(
        &conn,
        user_id,
        query.page.unwrap_or(1),
        query.per_page.unwrap_or(25),
        query.category.as_deref(),
        query.severity.as_deref(),
        show_read,
        show_deleted,
    )?;

    let tmpl = WarningListTemplate {
        ctx,
        warning_page,
        category_filter: query.category.clone(),
        severity_filter: query.severity.clone(),
        show_read,
        show_deleted,
    };

    render(tmpl)
}
```

**Step 3: Implement `src/handlers/warning_handlers/detail.rs`**

```rust
use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::auth::session::get_user_id;
use crate::errors::{AppError, render};
use crate::models::user;
use crate::templates_structs::{PageContext, WarningDetailTemplate, UserOption};
use crate::warnings::queries;

pub async fn detail(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let warning_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or_else(|| AppError::Session("No user".into()))?;
    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/warnings")?;

    let warning = queries::get_warning_detail(&conn, warning_id)?
        .ok_or(AppError::NotFound)?;

    let recipients = queries::get_recipients(&conn, warning_id)?;

    // Get timeline for current user's receipt
    let receipt_id = queries::find_receipt_for_user(&conn, warning_id, user_id)?
        .unwrap_or(0);
    let timeline = if receipt_id > 0 {
        queries::get_receipt_timeline(&conn, receipt_id)?
    } else {
        Vec::new()
    };

    // Get users for forward dropdown
    let all_users = user::find_all(&conn)?;
    let users: Vec<UserOption> = all_users.into_iter()
        .filter(|u| u.id != user_id)
        .map(|u| UserOption { id: u.id, name: u.username.clone(), label: u.label.clone() })
        .collect();

    let tmpl = WarningDetailTemplate {
        ctx,
        warning,
        recipients,
        timeline,
        user_receipt_id: receipt_id,
        users,
    };

    render(tmpl)
}
```

**Step 4: Create `templates/warnings/list.html`**

Follow the audit list template pattern with filter dropdowns, severity badges, and pagination. Use `warning_page.items` for rows. Each row shows severity dot, message (linked to `/warnings/{id}`), category badge, status, time, and action buttons (Read/Forward/Delete as POST forms).

**Step 5: Create `templates/warnings/detail.html`**

Follow the proposal detail template pattern. Show warning metadata, message, details JSON, recipients table with status, event timeline list, and action buttons.

**Step 6: Add routes in `src/main.rs`**

Inside the protected scope, add (before parameterized routes to avoid conflicts):

```rust
// Warning routes — /warnings before /warnings/{id}
.route("/warnings", web::get().to(handlers::warning_handlers::list::list))
.route("/warnings/{id}", web::get().to(handlers::warning_handlers::detail::detail))
```

**Step 7: Build and verify**

Run: `cargo check`
Expected: Compiles. Templates may need iteration to get Askama syntax right.

**Step 8: Commit**

```bash
git add src/handlers/warning_handlers/ src/templates_structs.rs templates/warnings/ src/main.rs
git commit -m "feat: warning list and detail pages with filtering and pagination"
```

---

## Task 8: Create action handlers (mark read, delete, forward)

**Files:**
- Modify: `src/handlers/warning_handlers/actions.rs`
- Modify: `src/main.rs` (add POST routes)

**Step 1: Implement `src/handlers/warning_handlers/actions.rs`**

```rust
use actix_session::Session;
use actix_web::{web, HttpResponse};
use serde::Deserialize;

use crate::db::DbPool;
use crate::auth::{csrf, session::get_user_id};
use crate::errors::AppError;
use crate::warnings::{self, queries};
use crate::handlers::warning_handlers::ws::{ConnectionMap, send_count_update};

#[derive(Deserialize)]
pub struct CsrfForm {
    pub csrf_token: String,
}

#[derive(Deserialize)]
pub struct ForwardForm {
    pub csrf_token: String,
    pub target_user_id: i64,
}

pub async fn mark_read(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<CsrfForm>,
    conn_map: web::Data<ConnectionMap>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;
    let warning_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or_else(|| AppError::Session("No user".into()))?;
    let conn = pool.get()?;

    if let Some(receipt_id) = queries::find_receipt_for_user(&conn, warning_id, user_id)? {
        warnings::update_receipt_status(&conn, receipt_id, "read", user_id)?;
    }

    send_count_update(&conn_map, &pool, user_id);

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/warnings"))
        .finish())
}

pub async fn mark_deleted(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<CsrfForm>,
    conn_map: web::Data<ConnectionMap>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;
    let warning_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or_else(|| AppError::Session("No user".into()))?;
    let conn = pool.get()?;

    if let Some(receipt_id) = queries::find_receipt_for_user(&conn, warning_id, user_id)? {
        warnings::update_receipt_status(&conn, receipt_id, "deleted", user_id)?;
    }

    send_count_update(&conn_map, &pool, user_id);

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/warnings"))
        .finish())
}

pub async fn forward(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<ForwardForm>,
    conn_map: web::Data<ConnectionMap>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;
    let warning_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or_else(|| AppError::Session("No user".into()))?;
    let conn = pool.get()?;

    // Update sender's receipt to forwarded
    if let Some(receipt_id) = queries::find_receipt_for_user(&conn, warning_id, user_id)? {
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        crate::models::entity::set_properties(&conn, receipt_id, &[
            ("status", "forwarded"),
            ("status_at", &now),
            ("forwarded_to", &form.target_user_id.to_string()),
            ("forwarded_at", &now),
        ])?;
        crate::models::relation::create(&conn, "forwarded_to_user", receipt_id, form.target_user_id)?;
        warnings::create_event(&conn, receipt_id, "forwarded", user_id, None)?;
    }

    // Create receipt for target user
    warnings::create_receipts(&conn, warning_id, &[form.target_user_id])?;

    // Notify target user via WS
    let detail = queries::get_warning_detail(&conn, warning_id)?;
    if let Some(w) = detail {
        crate::handlers::warning_handlers::ws::notify_users(
            &conn_map, &pool, &[form.target_user_id],
            warning_id, &w.severity, &w.message,
        );
    }

    send_count_update(&conn_map, &pool, user_id);

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/warnings/{}", warning_id)))
        .finish())
}
```

**Step 2: Add POST routes in `src/main.rs`**

```rust
.route("/warnings/{id}/read", web::post().to(handlers::warning_handlers::actions::mark_read))
.route("/warnings/{id}/delete", web::post().to(handlers::warning_handlers::actions::mark_deleted))
.route("/warnings/{id}/forward", web::post().to(handlers::warning_handlers::actions::forward))
```

**Step 3: Build and verify**

Run: `cargo check`
Expected: Compiles with no errors.

**Step 4: Commit**

```bash
git add src/handlers/warning_handlers/actions.rs src/main.rs
git commit -m "feat: warning action handlers — mark read, delete, forward with WS updates"
```

---

## Task 9: Create background scheduler with warning generators

**Files:**
- Create: `src/warnings/scheduler.rs`
- Create: `src/warnings/generators.rs`
- Modify: `src/warnings/mod.rs` (add submodules)
- Modify: `src/main.rs` (spawn background task)

**Step 1: Create `src/warnings/generators.rs`**

Implement scheduled generators: `data.user_without_role`, `system.database_size`, and the cleanup function. Each generator checks a condition, deduplicates, and creates warnings if needed.

Key functions:
- `check_users_without_role(conn, conn_map, pool)` — finds user entities with no `has_role` relation
- `check_database_size(conn, conn_map, pool, data_dir)` — checks file size threshold
- `cleanup_old_warnings(conn)` — retention cleanup per design

**Step 2: Create `src/warnings/scheduler.rs`**

```rust
use std::sync::Arc;
use std::time::Duration;
use crate::db::DbPool;
use crate::handlers::warning_handlers::ws::ConnectionMap;

pub fn spawn_scheduler(pool: DbPool, conn_map: ConnectionMap, data_dir: String) {
    actix_web::rt::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
        loop {
            interval.tick().await;
            log::info!("Running warning scheduler");
            let conn = match pool.get() {
                Ok(c) => c,
                Err(e) => {
                    log::error!("Scheduler: failed to get DB connection: {}", e);
                    continue;
                }
            };
            // Run generators
            super::generators::check_users_without_role(&conn, &conn_map, &pool);
            super::generators::check_database_size(&conn, &conn_map, &pool, &data_dir);
            // Run cleanup
            if let Err(e) = super::generators::cleanup_old_warnings(&conn) {
                log::error!("Warning cleanup failed: {}", e);
            }
        }
    });
}
```

**Step 3: Update `src/warnings/mod.rs`**

Add: `pub mod generators;` and `pub mod scheduler;`

**Step 4: Spawn scheduler in `src/main.rs`**

After audit cleanup and before `HttpServer::new`, add:

```rust
warnings::scheduler::spawn_scheduler(pool.clone(), conn_map.clone(), data_dir.clone());
```

**Step 5: Build and verify**

Run: `cargo check`
Expected: Compiles with no errors.

**Step 6: Commit**

```bash
git add src/warnings/generators.rs src/warnings/scheduler.rs src/warnings/mod.rs src/main.rs
git commit -m "feat: background scheduler with data integrity and system health generators"
```

---

## Task 10: Add event-driven warning generators to existing handlers

**Files:**
- Modify: `src/handlers/auth_handlers.rs` (failed login tracking)
- Modify: `src/handlers/user_handlers/crud.rs` (user created/deleted)
- Modify: `src/handlers/role_handlers/crud.rs` (permission changed)

**Step 1: Add failed login warning to `auth_handlers.rs`**

In `login_submit`, after the failed password check (the `_ => render_error(...)` branch), add logic to count recent failures and create a warning if threshold is met. This requires the `ConnectionMap` to be added as a handler parameter.

**Step 2: Add user created/deleted warnings**

In `user_handlers::crud::create` (after successful creation) and `user_handlers::crud::delete` (after successful deletion), add warning generation calls.

**Step 3: Add permission changed warning**

In `role_handlers::crud::update` (after successful role permission update), add an info warning.

**Step 4: Build and verify**

Run: `cargo check`
Expected: Compiles. Test by creating/deleting a user and checking `/warnings`.

**Step 5: Commit**

```bash
git add src/handlers/auth_handlers.rs src/handlers/user_handlers/ src/handlers/role_handlers/
git commit -m "feat: event-driven warning generators in auth, user, and role handlers"
```

---

## Task 11: Add warnings nav item to seed data

**Files:**
- Modify: `src/db.rs` (add nav item + permission for warnings)

**Step 1: Seed warnings nav item and permission**

In `seed_ontology()`, add a `warnings.view` permission and a nav item for warnings under the admin module. This makes the warnings page appear in the sidebar.

**Step 2: Delete database and re-seed**

```bash
rm data/dev/app.db
cargo run  # Will re-seed
```

**Step 3: Commit**

```bash
git add src/db.rs
git commit -m "feat: seed warnings nav item and permission"
```

---

## Task 12: Integration testing

**Files:**
- Create: `tests/warnings_test.rs`

**Step 1: Write tests**

Test the core functions:
- `test_create_warning_and_receipts` — create a warning, verify entities exist
- `test_count_unread` — create warnings, verify count for different users
- `test_mark_read_updates_receipt` — create warning, mark read, verify status
- `test_warning_deduplication` — same source_action shouldn't create duplicate
- `test_forward_creates_new_receipt` — forward warning, verify new receipt
- `test_cleanup_resolved_warnings` — create resolved warning, run cleanup, verify deletion

Each test uses the same pattern as existing tests: `setup_test_db()` with TempDir, `include_str!("../src/schema.sql")`.

**Step 2: Run tests**

Run: `cargo test`
Expected: All tests pass.

**Step 3: Commit**

```bash
git add tests/warnings_test.rs
git commit -m "test: warnings system integration tests"
```

---

## Task 13: Final verification and cleanup

**Step 1: Build with no warnings**

Run: `cargo check 2>&1`
Expected: No errors, minimal warnings.

**Step 2: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

**Step 3: Visual verification**

Start the app, log in, navigate to `/warnings`. Verify:
- Page renders correctly
- Badge shows 0 (no warnings yet)
- Create a test warning via scheduled generators or manual seed

**Step 4: Commit any cleanup**

```bash
git add -A
git commit -m "chore: warnings system cleanup and polish"
```

---

## Summary

| Task | What | Key Files |
|------|------|-----------|
| 1 | Dependencies + seed | `Cargo.toml`, `src/db.rs` |
| 2 | Core module | `src/warnings/mod.rs` |
| 3 | Query functions | `src/warnings/queries.rs` |
| 4 | Wire badge count | `src/templates_structs.rs` |
| 5 | WebSocket infra | `src/handlers/warning_handlers/ws.rs` |
| 6 | Client JS + toast | `templates/partials/nav.html`, `static/css/toast.css` |
| 7 | List + detail pages | `src/handlers/warning_handlers/list.rs`, `detail.rs`, templates |
| 8 | Action handlers | `src/handlers/warning_handlers/actions.rs` |
| 9 | Background scheduler | `src/warnings/scheduler.rs`, `generators.rs` |
| 10 | Event-driven generators | `auth_handlers.rs`, `user_handlers`, `role_handlers` |
| 11 | Nav item + permission | `src/db.rs` |
| 12 | Integration tests | `tests/warnings_test.rs` |
| 13 | Final verification | Cleanup pass |
