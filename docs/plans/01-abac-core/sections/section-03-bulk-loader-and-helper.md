Now I have all the context I need. Let me generate the section content.

# Section 03 — Bulk Loader and Helper

## Overview

This section completes the ABAC core module by adding `load_tor_capabilities` and `require_tor_capability` to `/Users/vidarbrevik/projects/im-ctrl/src/auth/abac.rs`.

**Depends on:** section-01 (test file written) and section-02 (`has_resource_capability` implemented and tests 1-5 green).

**Delivers:** Tests 6 and 7 passing. Full suite green. Module ready for Split 2 handler migration.

---

## Current State (After Section 02)

At the start of this section, the following are already in place:

- `/Users/vidarbrevik/projects/im-ctrl/tests/abac_test.rs` exists with all 7 tests and helpers
- `/Users/vidarbrevik/projects/im-ctrl/src/auth/abac.rs` exists with `has_resource_capability`
- `/Users/vidarbrevik/projects/im-ctrl/src/auth/mod.rs` has `pub mod abac;`
- Tests 1-5 pass; tests 6 and 7 fail (functions not implemented)

---

## Tests to Satisfy (Extract from `tests/abac_test.rs`)

These are the two tests written in section-01 that must go green in this section.

### Test 6 — `test_load_tor_capabilities_returns_all_true_flags`

Setup:
- Create a user, a tor, and a tor_function entity
- Link: `fills_position(user, func)` and `belongs_to_tor(func, tor)`
- The function entity has three `entity_properties`:
  - `can_call_meetings = 'true'`
  - `can_manage_agenda = 'true'`
  - `can_record_decisions = 'false'`

Note: `create_function(conn, name, capability, value)` sets only one property. The second and third properties are added directly via `INSERT INTO entity_properties` after calling the helper.

Key assertions:
```
assert!(caps.has("can_call_meetings"));
assert!(caps.has("can_manage_agenda"));
assert!(!caps.has("can_record_decisions"));
```

### Test 7 — `test_load_tor_capabilities_empty_for_non_member`

Setup:
- Create a user and a tor, but create no `fills_position` or `belongs_to_tor` relations

Key assertion:
```
assert!(!caps.has("can_call_meetings"));
```

The returned `Permissions` struct should contain zero keys.

---

## Data Model Recap

The EAV graph (from the existing database schema):

- `entities(id, entity_type, name, label, created_at)`
- `entity_properties(entity_id, key, value)` — key-value pairs on an entity
- `relations(id, relation_type_id, source_id, target_id)` — typed edges

The relevant graph path for a capability check:

```
user --[fills_position]--> tor_function --[belongs_to_tor]--> tor
                               |
                    entity_properties(key=can_*, value='true'/'false')
```

Relation types are stored as entities with `entity_type = 'relation_type'`. To look up a relation type ID inside SQL, use an inline scalar subquery:
```sql
(SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position')
```

The `Permissions` struct (from `src/auth/session.rs`) is:
```rust
pub struct Permissions(pub Vec<String>);
```

Construct one from a `Vec<String>` with `Permissions(keys)`. Its `has(&str)` method is used in test assertions and templates. `Permissions::default()` returns an empty instance.

---

## Implementation

### File to modify: `/Users/vidarbrevik/projects/im-ctrl/src/auth/abac.rs`

The file already has `has_resource_capability`. Add the two functions below it.

---

### `load_tor_capabilities`

**Purpose:** At page-render time, load all capability keys that the user holds in a specific ToR — in a single database round-trip — and return them as a `Permissions` struct for use in template contexts.

**Signature:**
```rust
pub fn load_tor_capabilities(
    conn: &Connection,
    user_id: i64,
    tor_id: i64,
) -> Result<Permissions, AppError>
```

**Behavior:**
- Hard-codes `"fills_position"` and `"belongs_to_tor"` (this function is ToR-specific, not generic)
- Filters `entity_properties` keys with `LIKE 'can_%'` to capture all capability properties
- Filters for `value = 'true'` — only keys whose value is `'true'` are included
- Returns a `Permissions` constructed from the collected `Vec<String>` of matching keys
- For a non-member (no relations), the query returns zero rows and `Permissions::default()` is returned (an empty `Permissions(vec![])`)
- Propagates database errors as `AppError::Db` via `?`

**SQL shape** — traverse the two-hop path and collect true capability keys:
```sql
SELECT DISTINCT ep.key
FROM entity_properties ep
JOIN entities func  ON ep.entity_id = func.id AND func.entity_type = 'tor_function'
JOIN relations r1   ON r1.target_id = func.id
                    AND r1.relation_type_id = (
                        SELECT id FROM entities
                        WHERE entity_type = 'relation_type' AND name = 'fills_position'
                    )
                    AND r1.source_id = ?1
JOIN relations r2   ON r2.source_id = func.id
                    AND r2.relation_type_id = (
                        SELECT id FROM entities
                        WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor'
                    )
                    AND r2.target_id = ?2
WHERE ep.key LIKE 'can_%'
  AND ep.value = 'true'
```

Parameters: `[user_id, tor_id]`

Collect all returned keys into a `Vec<String>`, then wrap in `Permissions(keys)`.

---

### `require_tor_capability`

**Purpose:** Handler-level guard. Performs the two-phase check and returns `Ok(())` or an appropriate `Err`.

**Signature:**
```rust
pub fn require_tor_capability(
    conn: &Connection,
    session: &Session,
    tor_id: i64,
    capability: &str,
) -> Result<(), AppError>
```

**Two-phase check logic:**

Phase 1 — Global bypass:
- Call `require_permission(session, "tor.edit")`
- If `Ok(())`, return `Ok(())` immediately (admin/global editor — skip DB check entirely)

Phase 2 — Resource-level capability:
- Call `get_user_id(session)`. If `None`, return `Err(AppError::Session("Not authenticated".to_string()))`
- Call `has_resource_capability(conn, user_id, tor_id, "belongs_to_tor", capability)?`
- If `true`, return `Ok(())`
- If `false`, return `Err(AppError::PermissionDenied(capability.to_string()))`

**Error semantics:**
- Unauthenticated session → `AppError::Session` (not PermissionDenied — the distinction matters for HTTP response codes upstream)
- Missing permission after DB check → `AppError::PermissionDenied(capability.to_string())`
- DB error → propagated as `AppError::Db` via `?`

**Phase 1 failure does not mean denial** — if `require_permission` returns `Err`, that means the user lacks `tor.edit`. Proceed to Phase 2. Do not return the Phase 1 error.

**Optional pure-function extraction:** To make the branching logic unit-testable without an Actix runtime, extract it into:
```rust
fn check_tor_access(
    conn: &Connection,
    user_id: i64,
    has_global_edit: bool,
    tor_id: i64,
    capability: &str,
) -> Result<(), AppError>
```

Then `require_tor_capability` becomes a thin wrapper: unpack `session` to get `user_id` and `has_global_edit` (from `get_permissions`), then delegate to `check_tor_access`. This is optional — if simpler to inline, accept the coverage gap until Split 2 integration tests.

---

### Required Imports for `abac.rs`

The full imports block the file needs (some already present from section-02):

```rust
use rusqlite::Connection;
use actix_session::Session;
use crate::errors::AppError;
use crate::auth::session::{get_user_id, require_permission, Permissions};
```

---

## Capability Key Reference

All six capability keys used by the system (the `LIKE 'can_%'` filter captures all of them):

| Key | Meaning |
|-----|---------|
| `can_call_meetings` | May confirm/cancel meetings |
| `can_manage_agenda` | May manage agenda points |
| `can_record_decisions` | May record meeting decisions/minutes |
| `can_review_suggestions` | May review member suggestions |
| `can_create_proposals` | May create governance proposals |
| `can_approve_proposals` | May approve/reject proposals |

Split 2 uses only the first three. The remaining three are returned by `load_tor_capabilities` and available for future splits without any change to this function.

---

## Verification Steps

Run these in order after completing implementation:

1. Confirm tests 6 and 7 now pass:
   ```
   cargo test --test abac_test
   ```
   Expected: all 7 tests output `ok`.

2. Confirm no regressions in the full test suite:
   ```
   cargo test
   ```
   Expected: all pre-existing tests continue to pass.

3. Confirm no new linter warnings:
   ```
   cargo clippy
   ```
   Expected: zero new warnings.

4. Confirm clean compile:
   ```
   cargo check
   ```
   Expected: `Finished` with no errors.