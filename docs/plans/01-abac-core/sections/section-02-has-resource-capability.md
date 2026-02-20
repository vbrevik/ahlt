Now I have all the context needed to write the section. Here is the complete markdown content for `section-02-has-resource-capability.md`:

# Section 02 — Implement `has_resource_capability`

**Depends on:** `section-01-failing-tests` (test file already written, compile currently fails with `unresolved import ahlt::auth::abac`)

**Goal:** Create `src/auth/abac.rs` containing the `has_resource_capability` function, and add `pub mod abac;` to `src/auth/mod.rs`. After this section, tests 1-5 pass and tests 6-7 still fail (functions not yet implemented).

---

## Background

The im-ctrl system uses an Entity-Attribute-Value (EAV) graph. All data lives in three tables:

- `entities (id, entity_type, name, label, created_at, sort_order)`
- `entity_properties (entity_id, key, value)` — key-value properties on entities
- `relations (id, relation_type_id, source_id, target_id)` — typed directed edges

The relevant graph for this function:

```
user  --fills_position-->  tor_function  --belongs_to_tor-->  tor
                           tor_function has entity_properties:
                             can_call_meetings    = 'true' | 'false'
                             can_manage_agenda    = 'true' | 'false'
                             can_record_decisions = 'true' | 'false'
                             can_review_suggestions = 'true' | 'false'
                             can_create_proposals   = 'true' | 'false'
                             can_approve_proposals  = 'true' | 'false'
```

Relation type names (`fills_position`, `belongs_to_tor`) are stored as entities with `entity_type = 'relation_type'`. The canonical pattern to reference them in SQL is an inline scalar subquery:

```sql
(SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position')
```

This pattern is used throughout the codebase (e.g. `src/models/tor/queries.rs`) and must be followed here.

---

## What Was Done in Section 01

Section 01 created `tests/abac_test.rs` with:

- Six helper functions: `create_function`, `create_user`, `create_tor`, `rel_type`, `fills_position`, `belongs_to_tor`
- Seven `#[test]` functions (tests 1-7)
- `mod common;` and `use ahlt::auth::abac;` imports

The file currently fails to compile: `unresolved import ahlt::auth::abac`. This section fixes that.

---

## Files to Create / Modify

| Action | File |
|--------|------|
| Create | `/Users/vidarbrevik/projects/im-ctrl/src/auth/abac.rs` |
| Modify | `/Users/vidarbrevik/projects/im-ctrl/src/auth/mod.rs` |

---

## Step 1 — Register the Module

Add `pub mod abac;` to `/Users/vidarbrevik/projects/im-ctrl/src/auth/mod.rs`.

Current content of that file:

```
pub mod csrf;
pub mod middleware;
pub mod password;
pub mod rate_limit;
pub mod session;
pub mod validate;
```

After edit:

```
pub mod abac;
pub mod csrf;
pub mod middleware;
pub mod password;
pub mod rate_limit;
pub mod session;
pub mod validate;
```

---

## Step 2 — Create `src/auth/abac.rs`

Create `/Users/vidarbrevik/projects/im-ctrl/src/auth/abac.rs` with only the `has_resource_capability` function. The other two functions (`load_tor_capabilities`, `require_tor_capability`) are implemented in section 03 — add stubs or leave them absent for now.

### Imports needed

```rust
use rusqlite::{Connection, params};
use crate::errors::AppError;
use crate::auth::session::Permissions;
```

`require_tor_capability` (section 03) will also need `actix_session::Session` and `crate::auth::session::{get_user_id, require_permission}`, but those can be added in section 03.

### Function signature

```rust
/// Check whether a user holds a specific capability in a given resource,
/// by traversing the EAV graph:
///   user --(fills_position)--> tor_function --(belongs_to_rel)--> resource
///
/// Returns Ok(true) if ANY of the user's positions in the resource
/// has the capability property set to 'true'.
/// Returns Ok(false) for non-members, wrong-resource, or missing/false flag.
/// Returns Err on database error.
///
/// Fail-closed: a misspelled `belongs_to_rel` returns Ok(false) via SQL
/// three-valued logic (the subquery returns NULL, WHERE evaluates false).
pub fn has_resource_capability(
    conn: &Connection,
    user_id: i64,
    resource_id: i64,
    belongs_to_rel: &str,
    capability: &str,
) -> Result<bool, AppError>
```

### SQL Query Design

The query traverses the two-hop path in a single statement. It looks for at least one `tor_function` entity that:

1. The given user `fills_position` (i.e., there is a relation from `user_id` to the function entity)
2. That function `belongs_to_rel` to the given `resource_id` (i.e., there is a relation from the function to `resource_id` using the named relation type)
3. That function has an `entity_properties` row with `key = capability` and `value = 'true'`

Use `EXISTS` or `COUNT` returning a boolean. Named parameters via `params!` are fine. Relation type IDs are looked up via inline scalar subqueries as described above.

The `belongs_to_rel` parameter drives one of the subqueries — the `fills_position` relation name is hard-coded.

### Return Value

- Use `query_row` with a closure that extracts a count or boolean.
- If the count is 0, return `Ok(false)`. If >= 1, return `Ok(true)`.
- Propagate `rusqlite::Error` via the `From<rusqlite::Error> for AppError` impl (use `?`).

---

## Tests That Must Pass After This Section

Tests 1-5 in `tests/abac_test.rs` cover `has_resource_capability`. Their structure (as written in section 01):

| Test | Stub Name | Key Assertion |
|------|-----------|---------------|
| 1 | `test_has_capability_true` | `assert_eq!(result.unwrap(), true)` |
| 2 | `test_has_capability_false_when_flag_is_false` | `assert_eq!(result.unwrap(), false)` |
| 3 | `test_has_capability_false_when_not_member` | `assert_eq!(result.unwrap(), false)` |
| 4 | `test_boundary_isolation_different_tor` | `assert_eq!(result.unwrap(), false)` |
| 5 | `test_missing_capability_key_returns_false` | `assert_eq!(result.unwrap(), false)` |

What each test sets up:

**Test 1** — Creates a user, a tor_function with `can_call_meetings=true`, and a tor. Calls `fills_position(user, func)` and `belongs_to_tor(func, tor)`. Calls `has_resource_capability(conn, user_id, tor_id, "belongs_to_tor", "can_call_meetings")`. Expects `Ok(true)`.

**Test 2** — Same structure as test 1 but `create_function` is called with value `'false'`. Expects `Ok(false)`.

**Test 3** — User and tor exist with no relations between them. Expects `Ok(false)`.

**Test 4** — User has capability in `tor_a` (full graph set up). Query is called with `tor_b` (different entity, no relations). Expects `Ok(false)`.

**Test 5** — User's function has `can_manage_agenda=true` but the query checks `can_call_meetings`. No `can_call_meetings` property exists. Expects `Ok(false)`.

Tests 6-7 (`load_tor_capabilities`) will fail with a compile error until section 03 adds that function. If you add a placeholder stub for `load_tor_capabilities` with `unimplemented!()` body and correct signature, tests 6-7 will panic rather than fail to compile — both outcomes are acceptable at this stage.

---

## Key Implementation Notes

**Relation type lookup pattern** — always use an inline scalar subquery, not a stored ID:

```sql
AND r_fills.relation_type_id = (
    SELECT id FROM entities
    WHERE entity_type = 'relation_type' AND name = 'fills_position'
)
```

**Fail-closed on misspelled `belongs_to_rel`** — because the name is passed in as a parameter, a typo causes the subquery to return NULL. The WHERE clause with `= NULL` evaluates to UNKNOWN in SQL three-valued logic, which is treated as false. The function returns `Ok(false)` — access denied. This is correct security behavior. To prevent this class of bug, callers should use named constants for relation type strings rather than inline string literals.

**OR semantics across positions** — a user may fill multiple positions in the same ToR. Any single position with the capability set to `'true'` is sufficient. A `COUNT > 0` or `EXISTS` query naturally handles this.

**Error propagation** — do not return `Ok(false)` on a database error. Use `?` to propagate `rusqlite::Error` as `AppError::Db`. The `From<rusqlite::Error> for AppError` conversion is already defined in `src/errors.rs`.

**The `Permissions` struct** — imported from `crate::auth::session`. It is a newtype `pub struct Permissions(pub Vec<String>)` with a `has(&str) -> bool` method. You will need this import in scope for section 03's `load_tor_capabilities` return type. You can add it now or defer to section 03.

---

## Verification After This Section

```bash
# Tests 1-5 pass; tests 6-7 may fail or panic depending on stub approach
cargo test --test abac_test

# No regressions in the rest of the suite
cargo test

# No new warnings
cargo clippy

# Clean compile
cargo check
```

Expected output for `cargo test --test abac_test` at the end of this section:

```
test test_has_capability_true                           ... ok
test test_has_capability_false_when_flag_is_false       ... ok
test test_has_capability_false_when_not_member          ... ok
test test_boundary_isolation_different_tor              ... ok
test test_missing_capability_key_returns_false          ... ok
test test_load_tor_capabilities_returns_all_true_flags  ... ok
test test_load_tor_capabilities_empty_for_non_member    ... ok
```

## Implementation Notes

**Deviation from plan:** All three functions were implemented in section-02 rather than splitting across sections 02 and 03. Section-03 becomes verification-only. This is not a design change — all tests pass and the interface is identical to the spec.

**Files created/modified:**
- `src/auth/abac.rs` (new) — `has_resource_capability`, `load_tor_capabilities`, `require_tor_capability`
- `src/auth/mod.rs` — added `pub mod abac;`

**Code review fixes applied:**
- `require_tor_capability` restructured to call `get_user_id` before Phase 1 for accurate error semantics
- Module doc updated to list all 6 capability keys (LIKE filter is intentionally broad for forward-compat)
- Added comment explaining hardcoded `"belongs_to_tor"` in `require_tor_capability`