I now have all the context needed to write the section content. Let me generate the complete, self-contained section.

# Section 01 — Write Failing Tests (Red State)

## Overview

This section covers the TDD "red" phase: write the complete test file `tests/abac_test.rs` so that `cargo test --test abac_test` fails to compile with `unresolved import ahlt::auth::abac`. No `src/` files are modified in this section.

**Sections this depends on:** None — this is the foundation.

**Sections that depend on this:** section-02 (implement `has_resource_capability`), section-03 (implement `load_tor_capabilities` and `require_tor_capability`).

---

## Background

The im-ctrl system needs Attribute-Based Access Control (ABAC) to allow ToR members with specific roles (e.g. Chairperson) to perform function-scoped operations (confirm meetings, manage agendas) without giving them global `tor.edit` rights.

The authorization model stores capabilities as `entity_properties` on `tor_function` entities in the EAV graph:

- A `user` entity connects to a `tor_function` entity via a `fills_position` relation
- A `tor_function` entity connects to a `tor` entity via a `belongs_to_tor` relation
- The `tor_function` entity has `entity_properties` entries like `can_call_meetings = 'true'`

The three new functions (to be implemented in sections 02 and 03) live in `src/auth/abac.rs`:

- `has_resource_capability(conn, user_id, resource_id, belongs_to_rel, capability) -> Result<bool, AppError>` — low-level graph traversal
- `load_tor_capabilities(conn, user_id, tor_id) -> Result<Permissions, AppError>` — bulk loader returning all true capability flags
- `require_tor_capability(conn, session, tor_id, capability) -> Result<(), AppError>` — handler-level helper with two-phase check

This section only writes the tests — the module does not exist yet, so they will not compile.

---

## File to Create

**`/Users/vidarbrevik/projects/im-ctrl/tests/abac_test.rs`**

---

## Test Infrastructure

The test file uses the shared `tests/common/mod.rs` infrastructure:

- `setup_test_db()` returns `(TempDir, Connection)` with schema applied and base entities seeded
- Base entities seeded by `setup_test_db()` include the relation types `fills_position` and `belongs_to_tor` — these are available without any additional setup

The `Permissions` struct (from `src/auth/session.rs`) is a newtype `pub struct Permissions(pub Vec<String>)` with a `has(&str) -> bool` method.

---

## Helper Functions (ABAC-specific, defined in the test file)

Six helper functions are needed for test setup. These are defined in `tests/abac_test.rs` itself — they are specific to ABAC and do not belong in `tests/common/mod.rs`. All helpers call `.unwrap()` directly since test setup panics are acceptable and expected to be deterministic.

```rust
/// Create a tor_function entity with a single entity_property.
/// Returns the new entity's ID.
fn create_function(conn: &Connection, name: &str, capability: &str, value: &str) -> i64 { ... }

/// Create a user entity. Returns the new entity's ID.
fn create_user(conn: &Connection, name: &str) -> i64 { ... }

/// Create a tor entity. Returns the new entity's ID.
fn create_tor(conn: &Connection, name: &str) -> i64 { ... }

/// Look up a relation type entity ID by name.
/// Relies on the relation types seeded by setup_test_db().
fn rel_type(conn: &Connection, name: &str) -> i64 { ... }

/// Create a fills_position relation between a user and a tor_function.
fn fills_position(conn: &Connection, user_id: i64, func_id: i64) { ... }

/// Create a belongs_to_tor relation between a tor_function and a tor.
fn belongs_to_tor(conn: &Connection, func_id: i64, tor_id: i64) { ... }
```

Helper implementation notes:

- `create_function`, `create_user`, `create_tor`: INSERT into `entities (entity_type, name, label)`, then for `create_function` also INSERT into `entity_properties (entity_id, key, value)` using `conn.last_insert_rowid()` after the entities insert.
- `rel_type`: `SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = ?1` — returns the row's ID.
- `fills_position` and `belongs_to_tor`: INSERT into `relations (relation_type_id, source_id, target_id)` using `rel_type()` to resolve the relation type ID.

---

## Test 6 Special Note

`create_function` inserts one `entity_properties` row. Test 6 requires three properties on the same function. After calling `create_function` to create the entity and its first property, add the remaining two via direct `conn.execute("INSERT INTO entity_properties ...")` calls using `rusqlite::params![func_id, key, value]`.

---

## Seven Test Cases

### Test 1 — `test_has_capability_true`

A user fills a position in a ToR. The `tor_function` has `can_call_meetings = 'true'`. Calling `has_resource_capability` should return `Ok(true)`.

Key assertion: `assert_eq!(result.unwrap(), true)`

Setup:
1. `setup_test_db()`
2. Create user, tor, and function (with `can_call_meetings`/`'true'`)
3. Wire `fills_position(user, func)` and `belongs_to_tor(func, tor)`
4. Call `abac::has_resource_capability(&conn, user_id, tor_id, "belongs_to_tor", "can_call_meetings")`

---

### Test 2 — `test_has_capability_false_when_flag_is_false`

Same graph structure as test 1, but the capability value is `'false'` instead of `'true'`. Should return `Ok(false)`.

Key assertion: `assert_eq!(result.unwrap(), false)`

---

### Test 3 — `test_has_capability_false_when_not_member`

The user and ToR both exist, but there are no `fills_position` or `belongs_to_tor` relations. Should return `Ok(false)`.

Key assertion: `assert_eq!(result.unwrap(), false)`

Setup: create user and tor, do NOT wire any relations.

---

### Test 4 — `test_boundary_isolation_different_tor`

The user has `can_call_meetings = 'true'` in ToR A, but the check is called with ToR B's ID. Should return `Ok(false)`.

Key assertion: `assert_eq!(result.unwrap(), false)`

This test confirms that capability checks are scoped to the specific resource. Create two tors, wire the user to tor_a only, but call `has_resource_capability` with `tor_b_id`.

---

### Test 5 — `test_missing_capability_key_returns_false`

The user's function has `can_manage_agenda = 'true'` but the check is for `can_call_meetings`. Should return `Ok(false)`.

Key assertion: `assert_eq!(result.unwrap(), false)`

Create the function with `can_manage_agenda`/`'true'`, wire the graph correctly, then call with capability `"can_call_meetings"`.

---

### Test 6 — `test_load_tor_capabilities_returns_all_true_flags`

A function entity has three capability properties:
- `can_call_meetings = 'true'`
- `can_manage_agenda = 'true'`
- `can_record_decisions = 'false'`

`load_tor_capabilities` should return a `Permissions` struct containing the two `true` keys but not the `false` one.

Key assertions:
```rust
assert!(caps.has("can_call_meetings"));
assert!(caps.has("can_manage_agenda"));
assert!(!caps.has("can_record_decisions"));
```

Setup note: call `create_function` with the first property (`can_call_meetings`/`'true'`), then add the second and third via direct SQL:
```rust
conn.execute(
    "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
    params![func_id, "can_manage_agenda", "true"],
).unwrap();
conn.execute(
    "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
    params![func_id, "can_record_decisions", "false"],
).unwrap();
```

---

### Test 7 — `test_load_tor_capabilities_empty_for_non_member`

No `fills_position` or `belongs_to_tor` relations exist for the user in the ToR. `load_tor_capabilities` should return an empty `Permissions` (no capabilities).

Key assertion: `assert!(!caps.has("can_call_meetings"))`

Setup: create user and tor, do NOT wire any relations.

---

## Complete Test File Structure

```rust
//! ABAC (Attribute-Based Access Control) tests.
//!
//! Tests the abac module's query functions:
//! - has_resource_capability: checks if a user has a capability in a resource
//! - load_tor_capabilities: loads all true capability flags for a user in a ToR
//!
//! TDD: these tests are written BEFORE the implementation exists.
//! They will fail to compile until src/auth/abac.rs is created and
//! pub mod abac; is added to src/auth/mod.rs.

mod common;

use ahlt::auth::abac;
use ahlt::auth::session::Permissions;
use common::setup_test_db;
use rusqlite::{params, Connection};

// --- Helpers ---

fn create_function(conn: &Connection, name: &str, capability: &str, value: &str) -> i64 {
    // INSERT INTO entities with entity_type='tor_function'
    // INSERT INTO entity_properties for the capability
    // return last inserted entity ID
    todo!()
}

fn create_user(conn: &Connection, name: &str) -> i64 {
    // INSERT INTO entities with entity_type='user'
    todo!()
}

fn create_tor(conn: &Connection, name: &str) -> i64 {
    // INSERT INTO entities with entity_type='tor'
    todo!()
}

fn rel_type(conn: &Connection, name: &str) -> i64 {
    // SELECT id FROM entities WHERE entity_type='relation_type' AND name=?
    todo!()
}

fn fills_position(conn: &Connection, user_id: i64, func_id: i64) {
    // INSERT INTO relations (relation_type_id, source_id, target_id)
    todo!()
}

fn belongs_to_tor(conn: &Connection, func_id: i64, tor_id: i64) {
    // INSERT INTO relations (relation_type_id, source_id, target_id)
    todo!()
}

// --- Test 1 ---
#[test]
fn test_has_capability_true() {
    let (_dir, conn) = setup_test_db();
    // setup: user fills position with can_call_meetings=true in tor
    let result = abac::has_resource_capability(&conn, user_id, tor_id, "belongs_to_tor", "can_call_meetings");
    assert_eq!(result.unwrap(), true);
}

// --- Test 2 ---
#[test]
fn test_has_capability_false_when_flag_is_false() {
    let (_dir, conn) = setup_test_db();
    // setup: user fills position with can_call_meetings=false in tor
    let result = abac::has_resource_capability(&conn, user_id, tor_id, "belongs_to_tor", "can_call_meetings");
    assert_eq!(result.unwrap(), false);
}

// --- Test 3 ---
#[test]
fn test_has_capability_false_when_not_member() {
    let (_dir, conn) = setup_test_db();
    // setup: user and tor exist but no relations
    let result = abac::has_resource_capability(&conn, user_id, tor_id, "belongs_to_tor", "can_call_meetings");
    assert_eq!(result.unwrap(), false);
}

// --- Test 4 ---
#[test]
fn test_boundary_isolation_different_tor() {
    let (_dir, conn) = setup_test_db();
    // setup: user has capability in tor_a, check is against tor_b
    let result = abac::has_resource_capability(&conn, user_id, tor_b_id, "belongs_to_tor", "can_call_meetings");
    assert_eq!(result.unwrap(), false);
}

// --- Test 5 ---
#[test]
fn test_missing_capability_key_returns_false() {
    let (_dir, conn) = setup_test_db();
    // setup: function has can_manage_agenda=true but checking can_call_meetings
    let result = abac::has_resource_capability(&conn, user_id, tor_id, "belongs_to_tor", "can_call_meetings");
    assert_eq!(result.unwrap(), false);
}

// --- Test 6 ---
#[test]
fn test_load_tor_capabilities_returns_all_true_flags() {
    let (_dir, conn) = setup_test_db();
    // setup: function with 3 props (2 true, 1 false)
    // add extra props via direct INSERT INTO entity_properties
    let caps: Permissions = abac::load_tor_capabilities(&conn, user_id, tor_id).unwrap();
    assert!(caps.has("can_call_meetings"));
    assert!(caps.has("can_manage_agenda"));
    assert!(!caps.has("can_record_decisions"));
}

// --- Test 7 ---
#[test]
fn test_load_tor_capabilities_empty_for_non_member() {
    let (_dir, conn) = setup_test_db();
    // setup: user and tor exist, no relations
    let caps: Permissions = abac::load_tor_capabilities(&conn, user_id, tor_id).unwrap();
    assert!(!caps.has("can_call_meetings"));
}
```

The stubs above use `todo!()` for helper bodies and omit local variable declarations for clarity. The actual implementation must fill in the SQL and variable setup as described in each helper and test description above.

---

## Confirm the Red State

After writing the file, run:

```bash
cargo test --test abac_test
```

Expected output: a compile error containing `unresolved import ahlt::auth::abac`. If it compiles, something is wrong — the module must not yet exist in `src/auth/mod.rs`.

The red state is a deliberate checkpoint before moving to section-02.

---

## What NOT to Do in This Section

- Do NOT create `src/auth/abac.rs`
- Do NOT add `pub mod abac;` to `src/auth/mod.rs`
- Do NOT modify any `src/` file at all

All `src/` changes happen in section-02 and section-03.

---

## Verification Checklist

- [x] `tests/abac_test.rs` exists
- [x] File imports `ahlt::auth::abac` (the not-yet-existing module)
- [x] Six helper functions are defined
- [x] Seven `#[test]` functions are present matching the names in the TDD plan
- [x] `cargo test --test abac_test` fails to compile with `unresolved import ahlt::auth::abac`
- [x] No `src/` files were modified

## Implementation Notes

**Deviations from spec skeleton:**
- SQL uses `source_id`/`target_id` (actual schema) — spec skeleton incorrectly showed `from_entity_id`/`to_entity_id`
- Boolean assertions use `assert!(result.unwrap())` / `assert!(!result.unwrap())` instead of `assert_eq!(result.unwrap(), true/false)` — Clippy prefers this form

**Files created:** `tests/abac_test.rs`