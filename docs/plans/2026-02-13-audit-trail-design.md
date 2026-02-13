# Audit Trail System - Design Document

**Date:** 2026-02-13
**Status:** Approved
**Approach:** Hybrid EAV + Filesystem

---

## Overview

Two-tier audit system providing comprehensive logging with queryable high-value events and complete filesystem audit trail for external analysis.

**Goals:**
- Track all mutations (user, role, setting create/update/delete)
- Queryable UI for high-value security events
- Complete filesystem logs for compliance/forensics
- Configurable retention and export
- Security best practices

---

## Architecture

### Two-Tier Logging

**1. Database (EAV)** - High-value events only
- Query-optimized for UI display
- Search, filter, pagination
- Automatic retention cleanup
- Events: user.created, user.deleted, role.permissions_changed, critical settings

**2. Filesystem** - Complete audit trail
- JSON Lines format (`.jsonl`)
- ALL mutations logged with full details
- Daily rotation: `audit-YYYY-MM-DD.jsonl`
- External tool analysis (jq, grep, log parsers)
- No automatic cleanup (external management)

### Logging Flow

```
Handler mutation → audit::log() → {
    if is_important(action) → write to database (EAV)
    always → append to filesystem
}
```

---

## Database Model (EAV)

### Entity Type

`audit_entry`

### Properties

- `user_id` - ID of user who performed the action
- `action` - Action code (e.g., "user.created", "role.permissions_changed")
- `target_type` - Entity type being audited ("user", "role", "setting")
- `target_id` - ID of the target entity
- `summary` - Human-readable summary (e.g., "Created user 'john'")

### Rust Structs

```rust
pub struct AuditEntry {
    pub id: i64,
    pub user_id: i64,
    pub username: String,  // Joined from entities
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
```

### Query Pattern

Similar to users/roles: LEFT JOINs for properties, pagination support, search/filter on action, target_type, username.

### High-Value Events

Events written to database:
- `user.created` - New user account
- `user.deleted` - User account deleted
- `role.created` - New role
- `role.deleted` - Role removed
- `role.permissions_changed` - Role permissions modified
- `setting.critical_changed` - Critical settings (audit.*, session keys, etc.)

---

## Filesystem Logging

### File Format

**JSON Lines** (one JSON object per line):

```jsonl
{"timestamp":"2026-02-13T17:30:45Z","user_id":13,"username":"admin","action":"user.created","target_type":"user","target_id":15,"target_name":"john","details":{"email":"john@example.com","role":"viewer"},"ip":"127.0.0.1"}
{"timestamp":"2026-02-13T17:31:12Z","user_id":13,"username":"admin","action":"role.permissions_changed","target_type":"role","target_id":5,"target_name":"editor","details":{"added":["users.delete"],"removed":[]}}
```

### File Structure

- **Daily rotation:** `audit-YYYY-MM-DD.jsonl`
- **Location:** Configurable via `audit.log_path` setting (default: `data/audit/`)
- **Permissions:** 0600 (owner read/write only), directory 0700
- **No automatic cleanup:** External tools manage retention

### Logging Module

```rust
// src/audit/mod.rs
pub fn log(
    conn: &Connection,
    user_id: i64,
    action: &str,
    target_type: &str,
    target_id: i64,
    details: serde_json::Value
) -> Result<(), AuditError> {
    // 1. Always write to filesystem
    write_to_file(user_id, action, target_type, target_id, &details)?;

    // 2. If high-value event, also write to database
    if is_important(action) {
        write_to_database(conn, user_id, action, target_type, target_id, &details)?;
    }

    Ok(())
}

fn is_important(action: &str) -> bool {
    matches!(action,
        "user.created" | "user.deleted" |
        "role.created" | "role.deleted" | "role.permissions_changed" |
        "setting.critical_changed"
    )
}
```

---

## Settings Integration

### New Settings

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `audit.enabled` | boolean | true | Master toggle for audit logging |
| `audit.log_path` | text | `data/audit/` | Directory for audit log files |
| `audit.retention_days` | number | 90 | Days to keep database audit entries (0 = forever) |

### Behavior

- **audit.enabled = false**: No logging (database or filesystem)
- **audit.log_path**: Can be absolute or relative path, must be writable
- **audit.retention_days = 0**: Keep database entries forever
- **File logs**: Never auto-delete (external management only)

### Retention Cleanup

- **When:** Server startup
- **SQL:** `DELETE FROM entities WHERE entity_type='audit_entry' AND created_at < date('now', '-N days')`
- **CASCADE:** Automatically deletes properties via FK
- **Skip:** If retention_days = 0

---

## User Interface

### Route

`GET /audit` - Audit log viewer

### Components

**1. Filter Bar**
- Search input (username or summary, LIKE query)
- Action dropdown (all, user.*, role.*, setting.*)
- Target type dropdown (all, user, role, setting)
- Date range picker (optional: from/to dates)

**2. Audit Table** (paginated)

| Timestamp | User | Action | Target | Summary |
|-----------|------|--------|--------|---------|
| 2026-02-13 17:30 | admin | user.created | user:15 | Created user 'john' |
| 2026-02-13 17:31 | admin | role.permissions_changed | role:5 | Added users.delete to editor |

**3. Pagination**
- Existing pattern: `?page=N&per_page=M`
- Preserve filters across pages

### Navigation

- **Nav item:** "Audit Log" under Admin module
- **URL:** `/audit`
- **Permission:** `audit.view` (new permission)
- **Badge:** Optional count of entries from last 7 days

### Template

`templates/audit/list.html` following existing patterns

---

## Permissions

### New Permission

- **Code:** `audit.view`
- **Description:** View audit log entries
- **Group:** Admin operations

### Existing Permission

- **Code:** `settings.manage`
- **Usage:** Required to change audit settings (enabled, log_path, retention_days)

---

## Security Considerations

### Best Practices

1. **Filesystem permissions:** 0600 for log files, 0700 for audit directory
2. **No PII in logs:** Avoid logging passwords, tokens, or sensitive personal data
3. **Structured data:** Use serde_json::Value for details to avoid injection
4. **Audit the auditor:** Log changes to audit settings themselves
5. **Non-repudiation:** user_id from session, not user input
6. **Tamper evidence:** Append-only files, immutable database records
7. **Access control:** audit.view permission strictly enforced

### Error Handling

- **File write fails:** Log to stderr, do NOT block request
- **Database write fails:** Log to stderr, do NOT block request
- **Rationale:** Audit logging should never break application functionality

---

## Implementation Notes

### Handler Integration

Each CRUD handler adds audit logging after successful mutation:

```rust
pub async fn create_user(...) -> impl Responder {
    // ... existing validation and creation ...

    match user::create(&conn, &new) {
        Ok(user_id) => {
            // Audit log
            let details = serde_json::json!({
                "email": new.email,
                "role_id": new.role_id
            });
            let _ = audit::log(&conn, current_user_id, "user.created",
                              "user", user_id, details);

            // ... existing redirect ...
        }
        // ... error handling ...
    }
}
```

### Startup Cleanup

```rust
// In main.rs, after pool initialization
audit::cleanup_old_entries(&pool);
```

### Files to Modify

- `src/audit/mod.rs` (new) - Core audit logging module
- `src/db.rs` - Seed audit settings, create audit directory
- `src/models/audit.rs` (new) - AuditEntry model with EAV queries
- `src/handlers/audit_handlers.rs` (new) - Audit log viewer
- `src/handlers/user_handlers.rs` - Add audit calls to create/update/delete
- `src/handlers/role_handlers.rs` - Add audit calls to create/update/delete
- `src/handlers/settings_handlers.rs` - Add audit calls for critical settings
- `src/handlers/account_handlers.rs` - Add audit call for password changes
- `templates/audit/list.html` (new) - Audit log UI
- `Cargo.toml` - Add `serde_json` dependency (if not already present)

### Dependencies

- `serde_json` - JSON serialization for log files and details
- `chrono` (optional) - Better timestamp handling for date range filters

---

## Testing Strategy

### Manual Testing

1. Enable audit logging via settings
2. Create/update/delete users, roles
3. Verify database entries appear in `/audit` UI
4. Verify filesystem logs in `data/audit/audit-YYYY-MM-DD.jsonl`
5. Test search/filter functionality
6. Test retention cleanup (set retention_days=1, restart after 2 days)
7. Disable audit logging, verify no new entries created

### External Analysis

```bash
# Count events by action
jq -r '.action' data/audit/audit-*.jsonl | sort | uniq -c

# Find all actions by specific user
grep '"user_id":13' data/audit/audit-*.jsonl | jq .

# Search for specific target
jq 'select(.target_type=="user" and .target_id==15)' data/audit/audit-*.jsonl
```

---

## Future Enhancements

(Out of scope for initial implementation)

- Export to CSV/Excel from UI
- Real-time audit log streaming via WebSocket
- IP address logging (requires middleware integration)
- User agent logging
- Diff view for before/after changes
- Audit log signing for tamper detection
- External SIEM integration (syslog, etc.)

---

## Decision Rationale

**Why hybrid EAV + filesystem?**
- Database: Fast queries for UI, EAV consistency
- Filesystem: Complete record for compliance, external tool compatibility
- Best of both worlds for different use cases

**Why JSON Lines over CSV?**
- Nested data (details field) without escaping complexity
- Streaming compatible (one record per line)
- Industry standard for log analysis tools

**Why daily rotation over size-based?**
- Predictable file names for external tooling
- Simpler implementation (no file size monitoring)
- Sufficient for low-traffic admin system

**Why append-only files?**
- Tamper evidence (can't modify past entries without detection)
- Simpler concurrency (no locks needed)
- Standard practice for audit logs

---

## Success Criteria

✅ All mutations logged to filesystem
✅ High-value events queryable in UI
✅ Search/filter functionality working
✅ Settings control (enable/disable, path, retention)
✅ Retention cleanup runs on startup
✅ File permissions secure (0600/0700)
✅ No application breakage if logging fails
✅ External tools can parse logs with jq/grep
