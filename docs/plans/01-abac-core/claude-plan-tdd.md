# TDD Plan — ABAC Core Module

**Companion to:** `claude-plan.md`
**Test file:** `tests/abac_test.rs`
**Run with:** `cargo test --test abac_test`
**Framework:** Rust built-in `#[test]` + rusqlite integration via `setup_test_db()` from `tests/common/mod.rs`

TDD order: write all tests first → confirm compile failure (no `abac` module) → implement → all tests pass.

---

## Background and Problem

_No tests for this section — it describes motivation, not behavior._

---

## Data Model (Existing)

_No tests for this section — it describes existing infrastructure, not new code._

---

## Authorization Logic

_No dedicated tests for the narrative — the test cases below cover all the described behaviors._

### Relationship to `require_tor_membership`

_No tests — this is an architectural note, not code._

---

## Three Functions to Implement

### `has_resource_capability`

Tests to write first (before implementing):

- **Test: member with capability=true returns true**
  User fills a position in the ToR; that position's `entity_properties` has the capability key set to `'true'`. Expect `Ok(true)`.

- **Test: member with capability=false returns false**
  User fills a position in the ToR; the property value is `'false'`. Expect `Ok(false)`.

- **Test: non-member returns false**
  User and ToR entities exist, no `fills_position` or `belongs_to_tor` relations. Expect `Ok(false)`.

- **Test: boundary isolation between different ToRs**
  User has capability in ToR A. Call with ToR B. Expect `Ok(false)`.

- **Test: missing capability key returns false**
  User's position has a different `can_*` key (not the one being checked). Expect `Ok(false)`.

### `load_tor_capabilities`

Tests to write first:

- **Test: returns all true flags (not false ones)**
  Position has 3 properties: 2 true, 1 false. Expect `Permissions` containing exactly 2 keys.

- **Test: returns empty Permissions for non-member**
  No membership relations. Expect `Permissions::default()` (empty).

### `require_tor_capability`

_No direct tests in this split._ The function wraps session extraction around `has_resource_capability`. Direct testing requires an Actix runtime mock. Coverage comes from Split 2 handler integration tests.

Optional: if the pure-function extraction is implemented (`check_tor_access(conn, user_id, has_global_edit, tor_id, capability)`), write these stubs before implementing:
- Test: `has_global_edit=true` returns `Ok(())` without DB call
- Test: `has_global_edit=false`, user has capability → `Ok(())`
- Test: `has_global_edit=false`, user lacks capability → `Err(AppError::PermissionDenied)`

---

## Module Integration

Tests to write first:

- **Test: module is accessible via `ahlt::auth::abac`**
  Verified implicitly — if the test file compiles and runs, module declaration in `src/auth/mod.rs` is correct. No separate test needed.

---

## Test Strategy (TDD)

### Test File Structure

Write `tests/abac_test.rs` with this structure before writing any implementation:

```
mod common;          // pulls in setup_test_db()
use ahlt::auth::abac;
use rusqlite::params;

// --- Helpers ---
fn create_function(conn, name, capability, value) -> i64 { ... }
fn create_user(conn, name) -> i64 { ... }
fn create_tor(conn, name) -> i64 { ... }
fn rel_type(conn, name) -> i64 { ... }
fn fills_position(conn, user_id, func_id) { ... }
fn belongs_to_tor(conn, func_id, tor_id) { ... }

// --- Test 1 ---
#[test]
fn test_has_capability_true() { ... }

// --- Test 2 ---
#[test]
fn test_has_capability_false_when_flag_is_false() { ... }

// ... tests 3-7 ...
```

**Confirm red before green:** Run `cargo test --test abac_test` immediately after writing the test file. It must fail to compile with `unresolved import ahlt::auth::abac`. If it compiles, something is wrong.

**Then implement:** Create `src/auth/abac.rs`, add `pub mod abac;` to `src/auth/mod.rs`. Run tests again — all 7 should pass.

### Test 6 Special Note

`create_function` sets one property. Test 6 needs three properties on the same function. After calling `create_function` to create the entity and set the first property, add the remaining two via:
```
// pseudo-code — actual INSERT INTO entity_properties with params![func_id, key, value]
```

### `has_resource_capability` Test Stubs (numbered)

| # | Stub Name | Key Assertion |
|---|-----------|---------------|
| 1 | `test_has_capability_true` | `assert_eq!(result.unwrap(), true)` |
| 2 | `test_has_capability_false_when_flag_is_false` | `assert_eq!(result.unwrap(), false)` |
| 3 | `test_has_capability_false_when_not_member` | `assert_eq!(result.unwrap(), false)` |
| 4 | `test_boundary_isolation_different_tor` | `assert_eq!(result.unwrap(), false)` |
| 5 | `test_missing_capability_key_returns_false` | `assert_eq!(result.unwrap(), false)` |

### `load_tor_capabilities` Test Stubs (numbered)

| # | Stub Name | Key Assertion |
|---|-----------|---------------|
| 6 | `test_load_tor_capabilities_returns_all_true_flags` | `assert!(caps.has("can_call_meetings"))` + `assert!(caps.has("can_manage_agenda"))` + `assert!(!caps.has("can_record_decisions"))` |
| 7 | `test_load_tor_capabilities_empty_for_non_member` | `assert!(!caps.has("can_call_meetings"))` (empty) |

---

## Verification

Before claiming implementation complete:

1. `cargo test --test abac_test` — all 7 tests must output `ok`
2. `cargo test` — full test suite must show no regressions
3. `cargo clippy` — zero new warnings
4. `cargo check` — clean compile
