# ABAC Design: Attribute-Based Access Control for ToR and Meetings

**Date:** 2026-02-20
**Status:** Approved, pending implementation plan
**Scope:** ToR and meeting lifecycle operations (framework generalisable to other entity types)

---

## Problem

The current system uses flat, global RBAC. Permission codes like `tor.edit` are loaded into the session at login and apply uniformly across all resources. This means:

- A user with `tor.edit` can confirm meetings, manage agenda, and record decisions for *any* ToR
- A regular member of a ToR (filling a Chair or Secretary function) cannot perform lifecycle actions even though their function explicitly grants those capabilities
- The `TorFunctionDetail.can_*` fields already encode fine-grained capability intent in EAV entity_properties, but are never checked in handler enforcement

The goal is to wire up those existing capability flags so that ToR members can act within their function's authority, without requiring global `tor.edit`.

---

## Design Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Global bypass | `tor.edit` bypasses ABAC | Admins retain full access; ABAC is additive for members |
| Gated operations | All meeting lifecycle mutations | Confirm, transition, agenda, minutes, roll call, attendance, action items |
| EAV pattern | Generalised function-entity model | Consistent with existing `tor_function` entities; centralised capability management |
| Enforcement point | On-demand DB query | Single source of truth; no staleness risk from session caching |
| Template context | Reuse `Permissions` struct | Consistent with existing `has()` pattern in templates |
| Rollout scope | ToR + meetings now; framework ready for other entity types | Incremental adoption |

---

## Architecture

ABAC is a two-phase check, always in this order:

```
1. require_permission(session, "tor.edit")  → Ok  =>  allow (global bypass)
2. has_resource_capability(conn, user_id, resource_id,
       "belongs_to_tor", "can_call_meetings")
                                            → true =>  allow
                                            → false => PermissionDenied
```

Structural ToR operations (edit metadata, manage members, manage function definitions) remain exclusively behind `tor.edit`. There is no ABAC path for structural changes.

---

## Data Model

No schema changes required. The enforcement query traverses the existing EAV graph:

```
user ──fills_position──> tor_function_entity ──belongs_to_tor──> tor_entity
                               │
                     entity_properties:
                       can_call_meetings    = 'true' | 'false'
                       can_manage_agenda    = 'true' | 'false'
                       can_record_decisions = 'true' | 'false'
                       can_review_suggestions  = 'true' | 'false'
                       can_create_proposals    = 'true' | 'false'
                       can_approve_proposals   = 'true' | 'false'
```

The SQL for `has_resource_capability(user_id, tor_id, "belongs_to_tor", "can_call_meetings")`:

```sql
SELECT COUNT(*)
FROM relations r_fills
JOIN relations r_belongs ON r_belongs.source_id = r_fills.target_id
JOIN entity_properties ep ON ep.entity_id = r_fills.target_id
WHERE r_fills.source_id = ?1                          -- user_id
  AND r_belongs.target_id = ?2                        -- resource_id (tor_id)
  AND r_fills.relation_type_id = (
      SELECT id FROM entities
      WHERE entity_type = 'relation_type' AND name = 'fills_position')
  AND r_belongs.relation_type_id = (
      SELECT id FROM entities
      WHERE entity_type = 'relation_type' AND name = ?3)  -- belongs_to_rel
  AND ep.key = ?4                                     -- capability
  AND ep.value = 'true'
```

### Capability → Operation Mapping

| `can_*` flag | Operations gated |
|---|---|
| `can_call_meetings` | Confirm meeting, calendar confirm, status transitions |
| `can_manage_agenda` | Assign agenda point, remove agenda point |
| `can_record_decisions` | Generate minutes, save roll call, save attendance, save action items |
| `can_review_suggestions` | *(future)* Review suggestions within the ToR |
| `can_create_proposals` | *(future)* Create proposals under the ToR |
| `can_approve_proposals` | *(future)* Approve proposals under the ToR |

---

## Rust API

New module: `src/auth/abac.rs`, declared as `pub mod abac` in `src/auth/mod.rs`.

### Core function (generic, future-ready)

```rust
/// Returns true if user fills a function in the resource that has the given capability.
/// `belongs_to_rel` is the relation type name connecting the function entity to the resource
/// (e.g. "belongs_to_tor" for ToR resources).
pub fn has_resource_capability(
    conn: &Connection,
    user_id: i64,
    resource_id: i64,
    belongs_to_rel: &str,
    capability: &str,
) -> Result<bool, AppError>
```

### High-level handler helper (ToR-specific)

```rust
/// Check tor.edit global bypass first, then ABAC membership capability.
/// Use in meeting and tor lifecycle handlers (not structural handlers).
pub fn require_tor_capability(
    conn: &Connection,
    session: &Session,
    tor_id: i64,
    capability: &str,
) -> Result<(), AppError> {
    if require_permission(session, "tor.edit").is_ok() {
        return Ok(());
    }
    let user_id = get_user_id(session)
        .map_err(|e| AppError::Session(e))?;
    let has_cap = has_resource_capability(conn, user_id, tor_id, "belongs_to_tor", capability)?;
    if has_cap {
        Ok(())
    } else {
        Err(AppError::PermissionDenied(
            format!("Requires {} capability in this ToR", capability)
        ))
    }
}
```

### Import path in handlers

```rust
use ahlt::auth::abac::require_tor_capability;
```

---

## Handler Migration

### `src/handlers/meeting_handlers/crud.rs`

| Handler | Current check | New check |
|---|---|---|
| `confirm` | `require_permission("tor.edit")` | `require_tor_capability(..., "can_call_meetings")` |
| `confirm_calendar` | `require_permission("tor.edit")` | `require_tor_capability(..., "can_call_meetings")` |
| `transition` | `require_permission("tor.edit")` | `require_tor_capability(..., "can_call_meetings")` |
| `assign_agenda` | `require_permission("tor.edit")` | `require_tor_capability(..., "can_manage_agenda")` |
| `remove_agenda` | `require_permission("tor.edit")` | `require_tor_capability(..., "can_manage_agenda")` |
| `generate_minutes` | `require_permission("tor.edit")` | `require_tor_capability(..., "can_record_decisions")` |
| `save_roll_call` | `require_permission("tor.edit")` | `require_tor_capability(..., "can_record_decisions")` |
| `detail` (GET) | `require_permission("meetings.view")` | **unchanged** |

### `src/handlers/minutes_handlers/crud.rs`

| Handler | New check |
|---|---|
| `save_attendance` | `require_tor_capability(..., "can_record_decisions")` |
| `save_action_items` | `require_tor_capability(..., "can_record_decisions")` |

### `src/handlers/tor_handlers/` — unchanged

All ToR structural handlers remain behind `require_permission("tor.edit")`. No ABAC path for editing ToR metadata, managing members, or managing function definitions.

---

## Template Context

The meeting detail page needs to show/hide action buttons based on ABAC capabilities, not just global permissions. Add a `user_tor_capabilities: Permissions` field to `MeetingDetailTemplate`.

Populated at page load by calling `has_resource_capability` for each capability flag relevant to the page, combined into a `Permissions` struct (reusing the existing type).

Template usage (consistent with existing pattern):

```html
{% if ctx.permissions.has("tor.edit") || user_tor_capabilities.has("can_call_meetings") %}
  <!-- confirm / transition buttons -->
{% endif %}

{% if ctx.permissions.has("tor.edit") || user_tor_capabilities.has("can_manage_agenda") %}
  <!-- agenda management controls -->
{% endif %}
```

Helper function to build the capabilities struct efficiently (single query returning all capability flags for a user in a ToR, rather than N separate queries):

```rust
/// Returns a Permissions struct with all capabilities the user has in the given ToR.
pub fn load_tor_capabilities(
    conn: &Connection,
    user_id: i64,
    tor_id: i64,
) -> Result<Permissions, AppError>
```

This runs one query returning all `can_*` property keys where the value is `'true'`, building the `Permissions` CSV in one pass.

---

## Generalisation Pattern

For future entity types (governance boards, proposal committees, etc.):

1. Create function entities of the new type with `can_*` properties in EAV
2. Define a `belongs_to_{type}` relation type in the ontology seed
3. Write a `require_{type}_capability(conn, session, resource_id, capability)` wrapper that calls `has_resource_capability` with the appropriate `belongs_to_rel` string
4. Apply to handler checks and template context using the same pattern

No new EAV concepts are needed. The query is parameterised by relation type name.

---

## Testing

### New test file: `tests/abac_test.rs`

**`has_resource_capability` unit tests:**
- User with matching function + `can_*` flag `true` → returns `true`
- User with matching function + `can_*` flag `false` or missing → returns `false`
- User with no function in this ToR → returns `false`
- User with the capability in *a different* ToR → returns `false` (boundary isolation)
- Invalid `belongs_to_rel` name → returns `false` (no panic)

**`require_tor_capability` integration tests:**
- User with global `tor.edit` + no ToR membership → passes (bypass)
- Member with `can_call_meetings = true` → passes for `can_call_meetings`, fails for `can_record_decisions`
- Member with `can_call_meetings = false` → fails even if a member
- Non-member → fails regardless of capability

**`load_tor_capabilities` tests:**
- Returns only `true` capabilities for the user in this ToR
- Returns empty `Permissions` for non-members

### Extend `tests/meeting_test.rs`
- Confirm meeting as ToR member with `can_call_meetings` → succeeds
- Confirm meeting as ToR member without `can_call_meetings` → 403
- Confirm meeting as user with `tor.edit` but not a ToR member → succeeds

All tests use existing `setup_test_db()` + `seed_base_entities()` with custom `tor_function` entities and membership relations created per test case.
