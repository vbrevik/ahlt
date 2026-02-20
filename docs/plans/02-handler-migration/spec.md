# Spec: Handler Migration

**Feature:** Replace `require_permission("tor.edit")` in 9 handlers with ABAC capability checks
**Design doc:** `docs/plans/2026-02-20-abac-design.md` — "Handler Migration" section
**Existing impl plan:** `docs/plans/2026-02-20-abac-implementation-plan.md` (Tasks 3–6)
**Part of series:** Split 2 of 3 (depends on 01-abac-core; 03-template-ui depends on this)

---

## Goal

Wire `require_tor_capability` and `has_resource_capability` into all meeting and minutes lifecycle mutation handlers. After this split, only global `tor.edit` holders AND ToR members with the correct `can_*` flag can perform meeting lifecycle operations.

## Handlers to Migrate

### `src/handlers/meeting_handlers/crud.rs` — 6 handlers

| Handler | Old check | New check | Capability |
|---|---|---|---|
| `confirm` | `require_permission("tor.edit")` | `abac::require_tor_capability` | `can_call_meetings` |
| `confirm_calendar` | `require_permission("tor.edit")` | inline `has_resource_capability` | `can_call_meetings` |
| `transition` | `require_permission("tor.edit")` | `abac::require_tor_capability` | `can_call_meetings` |
| `assign_agenda` | `require_permission("tor.edit")` | `abac::require_tor_capability` | `can_manage_agenda` |
| `remove_agenda` | `require_permission("tor.edit")` | `abac::require_tor_capability` | `can_manage_agenda` |
| `save_roll_call` | `require_permission("tor.edit")` | `abac::require_tor_capability` | `can_record_decisions` |
| `generate_minutes` | `require_permission("minutes.generate")` | `abac::require_tor_capability` | `can_record_decisions` |

### `src/handlers/minutes_handlers/crud.rs` — 2 handlers

| Handler | Old check | New check | Capability |
|---|---|---|---|
| `save_attendance` | `require_permission("minutes.edit")` | `abac::require_tor_capability` | `can_record_decisions` |
| `save_action_items` | `require_permission("minutes.edit")` | `abac::require_tor_capability` | `can_record_decisions` |

## Key Constraints

**Reordering pattern:** `require_tor_capability` needs `conn` (DB query). In most handlers, `require_permission` came before `pool.get()`. The fix is always: extract `tor_id` and `conn` first, then ABAC check, then CSRF.

**`confirm_calendar` special case:** This handler returns JSON responses on failure, not `AppError`. Cannot use `?` operator with `require_tor_capability`. Instead:
```rust
let has_access = require_permission(&session, "tor.edit").is_ok()
    || abac::has_resource_capability(&conn, current_user_id, tor_id, "belongs_to_tor", "can_call_meetings")
       .unwrap_or(false);
if !has_access { return Ok(HttpResponse::Forbidden().json(...)) }
```

**`save_attendance` / `save_action_items` special case:** These handlers receive `minutes_id` in the path, not `tor_id`. Must resolve:
```rust
let mins = minutes::find_by_id(&conn, minutes_id)?.ok_or(AppError::NotFound)?;
let meeting = meeting::find_by_id(&conn, mins.meeting_id)?.ok_or(AppError::NotFound)?;
abac::require_tor_capability(&conn, &session, meeting.tor_id, "can_record_decisions")?;
```
Requires adding `use crate::models::meeting;` import to `minutes_handlers/crud.rs`.

## Handlers NOT Changed

All structural ToR handlers (`tor_handlers/`) and read-only handlers (`detail` GET, meeting list, calendar view) remain behind their existing checks unchanged.

## Files

- Modify: `src/handlers/meeting_handlers/crud.rs`
- Modify: `src/handlers/minutes_handlers/crud.rs`

## Prerequisite

`01-abac-core` must be complete and passing `cargo test --test abac_test` before starting this split.

## Provides to Split 3

Handler-level ABAC enforcement is complete. Only the template UI gating remains.
