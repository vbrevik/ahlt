# Warnings System Design

**Date:** 2026-02-16
**Status:** Approved

## Overview

A real-time warnings system for surfacing actionable alerts to users based on their roles. Warnings are first-class EAV entities with per-user receipt tracking, full event audit trails, and WebSocket push notifications.

## Requirements

- **Audience:** Role-based. Admins see system-level warnings; regular users see warnings relevant to their work.
- **Categories:** Governance workflow, security & access, data integrity, system health.
- **Severity model:** Both combined — some events have fixed severity, others escalate based on thresholds.
- **Storage:** Warnings as EAV entities with relations for read/unread/forwarded/deleted tracking.
- **Real-time:** WebSocket for immediate push to connected clients.
- **Triggers:** Both event-driven (inline in handlers) and scheduled (background scans every 5 min).

## Section 1: Data Model

### New Entity Types

| Entity Type | Purpose |
|---|---|
| `warning` | The alert itself |
| `warning_receipt` | Per-user status for a warning |
| `warning_event` | Audit trail of status changes on a receipt |

### New Relation Types (seeded in db.rs)

| Relation Type | From | To | Purpose |
|---|---|---|---|
| `targets_user` | warning | user | Who should see the warning |
| `for_warning` | receipt | warning | Links receipt to its warning |
| `for_user` | receipt | user | Links receipt to its recipient |
| `on_receipt` | event | receipt | Links event to its receipt |
| `forwarded_to_user` | receipt | user | Forward target (when forwarded) |
| `has_read` | user | warning | Quick read tracking |

### Warning Entity

**Naming convention:** `{category}.{subcategory}.{dedup_key}.{timestamp}`

**Properties:**

| Key | Value | Example |
|---|---|---|
| `severity` | `info` / `warning` / `critical` | `"warning"` |
| `category` | `security` / `governance` / `data_integrity` / `system` | `"security"` |
| `message` | Human-readable description | `"5 failed login attempts for 'admin'"` |
| `source_action` | What triggered it | `"login.failed"` |
| `details` | JSON blob with context | `{"count": 5, "window": "1h"}` |
| `status` | `active` / `resolved` | `"active"` |
| `scope` | `user` / `role` / `system` | How targets were determined |

### Warning Receipt Entity

**Naming convention:** `wr.{warning_id}.{user_id}`

One receipt per (warning, user) pair. Tracks that user's interaction with the warning.

**Properties:**

| Key | Value |
|---|---|
| `status` | `unread` / `read` / `forwarded` / `deleted` |
| `status_at` | ISO timestamp of last status change |
| `forwarded_to` | User entity ID (only when forwarded) |
| `forwarded_at` | ISO timestamp (only when forwarded) |

**Relations:**

- `receipt --for_warning--> warning`
- `receipt --for_user--> user`
- `receipt --forwarded_to_user--> user` (if forwarded)

### Warning Event Entity

**Naming convention:** `we.{receipt_id}.{action}.{timestamp}`

Full audit trail. One event per status transition on a receipt.

**Properties:**

| Key | Value |
|---|---|
| `action` | `created` / `read` / `forwarded` / `deleted` / `resolved` |
| `actor_user_id` | Who performed the action |
| `forwarded_to` | Target user ID (only for `forwarded` action) |
| `note` | Optional context |

**Relation:** `warning_event --on_receipt--> warning_receipt`

### Key Queries

**Unread badge count (replaces hardcoded 0 in PageContext):**

```sql
SELECT COUNT(DISTINCT r_warn.target_id)
FROM entities receipt
JOIN entity_properties st ON st.entity_id = receipt.id AND st.key = 'status'
JOIN relations r_user ON r_user.source_id = receipt.id
JOIN entities rt_user ON rt_user.id = r_user.relation_type_id AND rt_user.name = 'for_user'
JOIN relations r_warn ON r_warn.source_id = receipt.id
JOIN entities rt_warn ON rt_warn.id = r_warn.relation_type_id AND rt_warn.name = 'for_warning'
WHERE receipt.entity_type = 'warning_receipt'
  AND st.value = 'unread'
  AND r_user.target_id = ?current_user_id
```

**Who hasn't read warning #X:**

```sql
SELECT u.id, u.name, u.label, st.value as status, sa.value as status_at
FROM entities receipt
JOIN entity_properties st ON st.entity_id = receipt.id AND st.key = 'status'
JOIN entity_properties sa ON sa.entity_id = receipt.id AND sa.key = 'status_at'
JOIN relations r_user ON r_user.source_id = receipt.id
JOIN entities rt_user ON rt_user.id = r_user.relation_type_id AND rt_user.name = 'for_user'
JOIN entities u ON u.id = r_user.target_id
JOIN relations r_warn ON r_warn.source_id = receipt.id
JOIN entities rt_warn ON rt_warn.id = r_warn.relation_type_id AND rt_warn.name = 'for_warning'
WHERE r_warn.target_id = ?warning_id
  AND st.value != 'read'
```

**Receipt timeline for a specific warning+user:**

```sql
SELECT ep_action.value as action, ep_actor.value as actor, evt.created_at
FROM entities evt
JOIN relations r ON r.source_id = evt.id
JOIN entities rt ON rt.id = r.relation_type_id AND rt.name = 'on_receipt'
JOIN entity_properties ep_action ON ep_action.entity_id = evt.id AND ep_action.key = 'action'
JOIN entity_properties ep_actor ON ep_actor.entity_id = evt.id AND ep_actor.key = 'actor_user_id'
WHERE evt.entity_type = 'warning_event'
  AND r.target_id = ?receipt_id
ORDER BY evt.created_at ASC
```

## Section 2: WebSocket Architecture

### Dependency

New crate: `actix-ws` (lightweight closure-based WebSocket for Actix-web).

### Connection Flow

```
Browser                          Server
  |                                |
  |---- GET /ws/notifications ---->|  (session cookie sent automatically)
  |                                |-- validate session, extract user_id
  |<--- 101 Switching Protocols ---|-- register (user_id, sender) in ConnectionMap
  |                                |
  |     ... page is open ...       |
  |                                |-- warning created, targets user_42
  |<--- {"type":"warning", ...} ---|-- lookup user_42 in ConnectionMap, push
  |                                |
  |---- {"type":"mark_read", id} ->|-- update receipt status, broadcast updated count
  |<--- {"type":"count", n: 2} ----|
  |                                |
  |---- close / navigate away ---->|-- remove sender from ConnectionMap
```

### Shared State

```rust
pub type ConnectionMap = Arc<RwLock<HashMap<i64, Vec<mpsc::UnboundedSender<WsMessage>>>>>;
```

Registered as `web::Data` in `main.rs`. One user can have multiple tabs (Vec of senders). Dead senders cleaned up lazily on failed send.

### Messages

**Server -> Client:**

```json
{"type": "new_warning", "warning_id": 481, "severity": "warning", "title": "Failed login attempts", "unread_count": 5}
{"type": "count_update", "unread_count": 3}
{"type": "warning_resolved", "warning_id": 481, "unread_count": 2}
```

**Client -> Server:**

```json
{"type": "mark_read", "warning_id": 481}
{"type": "mark_deleted", "warning_id": 481}
```

### Client-Side JS

Added to `templates/partials/nav.html`:

```javascript
const ws = new WebSocket(`ws://${location.host}/ws/notifications`);
ws.onmessage = (e) => {
    const msg = JSON.parse(e.data);
    if (msg.type === 'new_warning' || msg.type === 'count_update') {
        updateBadge(msg.unread_count);
    }
    if (msg.type === 'new_warning' && msg.severity !== 'info') {
        showToast(msg.title);
    }
};
```

### Auth

The `/ws/notifications` endpoint reads the session cookie via the same `actix-session` middleware. Invalid sessions rejected with 401.

## Section 3: Warning Generators

### Event-Driven Generators

Inline in existing handlers. Fire immediately when something happens.

| Generator | Trigger Point | Severity | Targets | Threshold |
|---|---|---|---|---|
| `security.failed_logins` | `auth_handlers::login_post` | `warning` at 5+, `critical` at 10+ | All admins | Count per user per hour |
| `security.permission_changed` | `role_handlers::crud::update` | `info` | All admins | Always |
| `security.user_created` | `user_handlers::crud::create` | `info` | All admins | Always |
| `security.user_deleted` | `user_handlers::crud::delete` | `warning` | All admins | Always |
| `governance.status_changed` | Workflow transition handler | `info` | Users with governance permissions | Always |
| `governance.needs_review` | Proposal submitted | `warning` | Users with `governance.approve` permission | Always |

### Scheduled Generators

Background task runs every 5 minutes. Idempotent — deduplication prevents duplicate warnings.

| Generator | Check | Severity | Targets | Dedup Key |
|---|---|---|---|---|
| `governance.overdue_review` | Proposals in `submitted`/`under_review` for >7 days | `warning` | Users with `governance.approve` | `proposal_{id}` |
| `governance.stalled_suggestion` | Suggestions in `open` for >14 days | `info` | Suggestion creator + admins | `suggestion_{id}` |
| `data.orphaned_entities` | Entities with no relations (excluding standalone types) | `warning` | All admins | `orphan_{entity_id}` |
| `data.user_without_role` | User entities with no `has_role` relation | `warning` | All admins | `norole_{user_id}` |
| `data.missing_properties` | Entities missing expected properties for their type | `info` | All admins | `missingprop_{entity_id}_{key}` |
| `system.database_size` | `data/{env}/app.db` exceeds threshold (100MB) | `warning` | All admins | `dbsize` |
| `system.audit_retention` | Audit entries older than retention setting still exist | `info` | All admins | `audit_retention` |

### Deduplication

Before creating a warning, check if an `active` warning with the same `source_action` and dedup key exists:

```rust
pub fn warning_exists(conn: &Connection, source_action: &str, dedup_key: &str) -> bool {
    let name_pattern = format!("{}.{}.%", source_action, dedup_key);
    conn.query_row(
        "SELECT COUNT(*) FROM entities e
         JOIN entity_properties st ON st.entity_id = e.id AND st.key = 'status'
         WHERE e.entity_type = 'warning' AND e.name LIKE ?1 AND st.value = 'active'",
        [&name_pattern], |row| row.get::<_, i64>(0)
    ).unwrap_or(0) > 0
}
```

### Auto-Resolution

Scheduled checks also resolve warnings when conditions clear (e.g., user-without-role warning resolved when user gets a role). Sets `status=resolved` and creates `resolved` event on all receipts.

## Section 4: UI & Routes

### Routes

| Method | Path | Handler | Purpose |
|---|---|---|---|
| GET | `/warnings` | `warnings_list` | Main list page (filterable, paginated) |
| GET | `/warnings/{id}` | `warning_detail` | Single warning with receipt timeline |
| POST | `/warnings/{id}/read` | `mark_read` | Mark as read (CSRF) |
| POST | `/warnings/{id}/delete` | `mark_deleted` | Soft-delete / dismiss (CSRF) |
| POST | `/warnings/{id}/forward` | `forward_warning` | Forward to another user (CSRF) |
| GET | `/ws/notifications` | `ws_connect` | WebSocket upgrade |

### Warnings List Page (`/warnings`)

Filterable table with pagination (same pattern as audit log):

- Filter by category (dropdown), severity (dropdown)
- Checkbox filters: show read, show deleted
- Unread warnings: bold with filled severity dot (critical=red, warning=amber, info=blue)
- Read warnings: muted text, hollow dot
- Row actions: Read, Forward, Delete (POST forms with CSRF)
- Clicking title opens detail page

### Warning Detail Page (`/warnings/{id}`)

- Warning metadata: severity badge, category, source action, created timestamp
- Full message text and details JSON
- Recipients table: each user's current status + timestamp
- Event timeline for current user's receipt (created -> read -> forwarded -> etc.)
- Actions: Mark Read, Forward (select dropdown of users), Delete

### Toast Notifications

Brief notification in bottom-right when WebSocket pushes a new warning (severity != info):

```javascript
function showToast(title, severity) {
    const toast = document.createElement('div');
    toast.className = `toast toast-${severity}`;
    toast.textContent = title;
    document.body.appendChild(toast);
    setTimeout(() => toast.remove(), 5000);
}
```

### Template Files

| Template | Purpose |
|---|---|
| `templates/warnings/list.html` | Main list page |
| `templates/warnings/detail.html` | Single warning + timeline |

Both extend `base.html` and include `partials/nav.html` + `partials/sidebar.html`.

## Section 5: Retention & Cleanup

### Retention Rules

| Entity Type | Default Retention | Condition | Action |
|---|---|---|---|
| `warning` (resolved) | 30 days after resolution | `status=resolved` AND `created_at < now - 30d` | Delete entity (CASCADE) |
| `warning` (deleted by all) | 7 days | All receipts `status=deleted` | Delete entity + receipts + events |
| `warning` (active, info) | 90 days | `severity=info` AND `created_at < now - 90d` | Auto-resolve, then 30-day resolved rule |
| `warning` (active, warning/critical) | Never auto-deleted | Manual resolution only | -- |
| `warning_receipt` | Follows parent warning | CASCADE delete | -- |
| `warning_event` | Follows parent receipt | CASCADE delete | -- |

### Configurable Settings

| Setting | Default | Purpose |
|---|---|---|
| `warnings.retention_resolved_days` | `30` | Days to keep resolved warnings |
| `warnings.retention_info_days` | `90` | Days before auto-resolving info warnings |
| `warnings.retention_deleted_days` | `7` | Days to keep fully-dismissed warnings |

### Cleanup Integration

Runs inside the same scheduled background task as warning generators:

```rust
pub fn cleanup_old_warnings(conn: &Connection) -> Result<(), rusqlite::Error> {
    let resolved_days = setting::get_value(conn, "warnings.retention_resolved_days", "30")
        .parse::<i64>().unwrap_or(30);
    conn.execute(
        "DELETE FROM entities WHERE entity_type = 'warning' AND id IN (
            SELECT e.id FROM entities e
            JOIN entity_properties st ON st.entity_id = e.id AND st.key = 'status'
            WHERE e.entity_type = 'warning' AND st.value = 'resolved'
            AND e.created_at < datetime('now', ?1)
        )",
        [format!("-{} days", resolved_days)],
    )?;
    Ok(())
}
```

CASCADE deletes on the `entities` foreign key clean up properties, relations, receipts, and events automatically. Each cleanup run logged to the audit system.

## Module Structure

```
src/warnings/
  mod.rs           -- create_warning, create_receipts, notify, warning_exists
  generators.rs    -- All generator functions (event-driven + scheduled)
  scheduler.rs     -- Background task loop, runs generators + cleanup
  queries.rs       -- count_unread, get_warnings_for_user, get_receipt_timeline

src/handlers/
  warning_handlers/
    mod.rs         -- Route registration
    list.rs        -- warnings_list handler
    detail.rs      -- warning_detail handler
    actions.rs     -- mark_read, mark_deleted, forward_warning handlers
    ws.rs          -- WebSocket upgrade handler

templates/warnings/
  list.html        -- Warnings list page
  detail.html      -- Warning detail + timeline
```

## New Dependencies

- `actix-ws` -- WebSocket support for Actix-web
