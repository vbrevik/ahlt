# ABAC Core Module — Usage Guide

## What Was Built

`src/auth/abac.rs` — three public functions for resource-scoped capability checks.

## Functions

### `has_resource_capability`

Low-level graph traversal. Checks if a user holds a specific capability in a resource.

```rust
use ahlt::auth::abac;

let can_confirm = abac::has_resource_capability(
    &conn,
    user_id,
    tor_id,
    "belongs_to_tor",
    "can_call_meetings",
)?;
```

Returns `Ok(true)` if ANY of the user's positions in the resource has that capability set to `'true'`.

### `load_tor_capabilities`

Bulk loader for template contexts. Returns all true capability flags for a user in a ToR.

```rust
let tor_caps = abac::load_tor_capabilities(&conn, user_id, tor_id)?;
// In template context:
// ctx.tor_capabilities = tor_caps
// Template: {% if ctx.tor_capabilities.has("can_call_meetings") %}
```

### `require_tor_capability`

Handler guard. Two-phase: global `tor.edit` bypass, then ABAC check.

```rust
pub async fn confirm_meeting(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    let (tor_id, meeting_id) = path.into_inner();
    let conn = pool.get()?;
    abac::require_tor_capability(&conn, &session, tor_id, "can_call_meetings")?;
    // ... rest of handler
}
```

## Capability Keys

| Key | Meaning |
|-----|---------|
| `can_call_meetings` | May confirm/cancel meetings |
| `can_manage_agenda` | May manage agenda points |
| `can_record_decisions` | May record decisions/minutes |
| `can_review_suggestions` | May review member suggestions |
| `can_create_proposals` | May create governance proposals |
| `can_approve_proposals` | May approve/reject proposals |

## Next Steps (Split 2)

Migrate ToR meeting handlers from `require_permission(&session, "tor.edit")` to `require_tor_capability(&conn, &session, tor_id, "can_call_meetings")` etc.

See `docs/plans/02-handler-migration/` for the handler migration plan.
