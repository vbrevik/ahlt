# Implementation Plan — ABAC Core Module

**Feature:** Attribute-Based Access Control for ToR resource operations
**Files to create/modify:** `src/auth/abac.rs`, `src/auth/mod.rs`, `tests/abac_test.rs`
**Depends on:** Nothing (this is the foundation)
**Provides to:** Split 2 (handler migration), Split 3 (template wiring)

---

## Background and Problem

The im-ctrl system currently uses flat, global role-based access control. The `tor.edit` permission, when granted, applies universally — a user either can edit any ToR or none. Regular members of a Terms of Reference (ToR) document — such as a Chairperson or Secretary — have their function-specific capabilities explicitly encoded in the database, but those capabilities are never enforced in handlers. Currently, if a Chairperson wants to confirm a meeting, they need the global `tor.edit` permission (admin-only), which gives them access to all ToRs rather than just their own.

The fix is Attribute-Based Access Control: add the ability to check a user's capabilities relative to a specific resource (a ToR), based on their membership in that resource's function graph.

This module is the authorization infrastructure. It provides three functions that other parts of the system will use to enforce fine-grained access. No UI changes and no schema changes are part of this split.

---

## Data Model (Existing)

The codebase uses an Entity-Attribute-Value (EAV) graph. All data is stored in three tables: `entities`, `entity_properties` (key-value pairs on entities), and `relations` (typed edges between entities).

The relevant graph structure is:

- A `user` entity can have a `fills_position` relation pointing to a `tor_function` entity
- A `tor_function` entity can have a `belongs_to_tor` relation pointing to a `tor` entity
- The `tor_function` entity has `entity_properties` entries with keys like `can_call_meetings`, `can_manage_agenda`, `can_record_decisions`, `can_review_suggestions`, `can_create_proposals`, `can_approve_proposals`, and values that are the string `'true'` or `'false'`

Relation types (`fills_position`, `belongs_to_tor`) are themselves stored as entities with `entity_type = 'relation_type'`. To reference a relation type in a SQL query, the canonical codebase pattern is an inline scalar subquery: `(SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position')`. This is used consistently throughout the codebase and should be used here.

---

## Authorization Logic

The key insight is a **two-phase check** that preserves backward compatibility:

1. **Phase 1 — Global bypass:** Check if the user has `tor.edit` via the session permissions. If yes, allow immediately. This ensures admins and other global editors are never blocked by the new system.

2. **Phase 2 — Resource-level capability:** Check if the user fills any function in the specific ToR that has the required capability set to `'true'`. If yes, allow. If no, deny.

This means the ABAC check is additive — it never removes existing access, only grants new access to members who previously needed a global permission.

### Relationship to `require_tor_membership`

The codebase has an existing function `tor::require_tor_membership()` (in `src/models/tor/queries.rs`) that checks whether a user fills any position in a ToR. It is called in 37+ handler call sites for suggestions, proposals, opinions, agenda, queues, and workflows.

These two functions serve distinct purposes and will coexist:
- `require_tor_membership` answers: "Is this user a member of the ToR at all?"
- `require_tor_capability` answers: "Does this member have a specific capability in this ToR?"

They are not redundant — a user could be a member but lack a specific capability, or have the capability via a different mechanism (global `tor.edit`). Do not replace `require_tor_membership` with `require_tor_capability`.

---

## Three Functions to Implement

### `has_resource_capability`

The low-level primitive. Given a user, a resource (e.g., a specific ToR), a relation type name for "belongs to resource", and a capability key, it traverses the EAV graph and returns whether the user has that capability in that resource.

The traversal: find all `tor_function` entities that (a) the user fills via `fills_position`, (b) that function belongs to the resource via the `belongs_to_rel` argument, and (c) that function has an `entity_property` with the given key set to `'true'`. Any match across all the user's positions in the ToR is sufficient (OR semantics — confirmed in interview).

**Genericity note:** The function signature accepts `belongs_to_rel: &str` as a parameter. The `fills_position` relation is intentionally hard-coded — the genericity is limited to the "belongs to resource" dimension. This is sufficient for all current use cases. If a future resource type uses a different membership relation (not `fills_position`), a second `fills_rel: &str` parameter can be added at that time.

**Fail-closed on typo:** If `belongs_to_rel` is misspelled, the subquery `SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = ?3` returns NULL, and the WHERE clause evaluates to false via SQL three-valued logic. The function returns `Ok(false)` — access denied. This is the correct security posture (fail closed) but can be hard to debug. Use relation type names as named constants, not inline string literals, to prevent this class of bug.

On database error, propagate the error as `AppError::Db` — do not swallow errors or fail open/closed silently. The return type `Result<bool, AppError>` is intentional.

### `load_tor_capabilities`

The bulk loader. Given a user and a ToR ID, return a `Permissions` struct containing all capability keys where the user has `value = 'true'` in any of their positions in that ToR. This is used at page-render time to populate the template context so that UI buttons can be conditionally shown without multiple database round-trips.

The function hard-codes `"fills_position"` and `"belongs_to_tor"` since it is ToR-specific. It filters for keys matching the pattern `can_%` to capture all capability properties.

**All 6 capability types are returned.** The `LIKE 'can_%'` filter returns all properties whose key starts with `can_`, currently: `can_call_meetings`, `can_manage_agenda`, `can_record_decisions`, `can_review_suggestions`, `can_create_proposals`, `can_approve_proposals`. Split 2 uses only the first three (meeting lifecycle handlers), but the full set flows into the `Permissions` struct. This is intentional — future splits for suggestion/proposal ABAC can use `tor_capabilities.has("can_review_suggestions")` without any changes to this function.

For a non-member, the query returns zero rows and the function returns an empty `Permissions` (via `Permissions::default()`).

The `Permissions` struct (from `src/auth/session.rs`) is a newtype wrapper: `pub struct Permissions(pub Vec<String>)`. To construct one from a `Vec<String>`, use `Permissions(keys)`.

### `require_tor_capability`

The handler-level helper. Given a database connection, the current session, a ToR ID, and a capability name, perform the two-phase check described above and return `Ok(())` if access is granted or `Err` if not.

If `get_user_id(&session)` returns `None` (unauthenticated), return `Err(AppError::Session(...))` — this is a session error, not a permission error, and the distinction is important for error handling upstream.

If Phase 1 (global bypass via `require_permission(session, "tor.edit")`) succeeds, return `Ok(())` immediately without touching the database.

If Phase 1 fails, proceed to Phase 2: call `has_resource_capability` with the session user's ID, the given `tor_id`, `"belongs_to_tor"` as the relation, and the given capability. If it returns `Ok(true)`, return `Ok(())`. If it returns `Ok(false)`, return `Err(AppError::PermissionDenied(capability.to_string()))`. If it returns an error, propagate it.

**Optional pure-function extraction for testability:** The three-branch logic in `require_tor_capability` cannot be easily unit-tested because `actix_session::Session` requires an Actix runtime. Consider extracting a pure helper `check_tor_access(conn, user_id: i64, has_global_edit: bool, tor_id: i64, capability: &str) -> Result<(), AppError>` that contains the branching logic, with `require_tor_capability` becoming a thin wrapper that unpacks the session and delegates. This allows direct unit testing of the three branches (unauthenticated handled separately at the wrapper level). This is optional — if the implementer prefers simplicity, accepting the coverage gap until Split 2 integration tests is also acceptable.

---

## Module Integration

Add `pub mod abac;` to `src/auth/mod.rs`. The existing modules are `csrf`, `middleware`, `password`, `rate_limit`, `session`, and `validate`. Adding `abac` keeps the flat sibling structure.

All three functions in `src/auth/abac.rs` should be `pub`. The crate re-exports the auth module, so callers in tests can use `ahlt::auth::abac::require_tor_capability` and handlers can use `crate::auth::abac::require_tor_capability`.

The module needs these imports: `rusqlite::Connection`, `actix_session::Session`, and the local crate's `errors::AppError` and `auth::session::{get_user_id, require_permission, Permissions}`.

---

## Test Strategy (TDD)

Write tests first (`tests/abac_test.rs`), confirm they fail to compile (module not yet declared), then implement until all pass.

The test file uses `mod common;` to pull in the shared test infrastructure, then `use ahlt::auth::abac;` to access the functions under test.

**Test helper functions** (to be defined in the test file, not in `common/mod.rs` since they're ABAC-specific):

- `create_function(conn, name, capability, value) → i64` — creates a `tor_function` entity with ONE `entity_property`. For tests needing multiple properties on the same function (see test 6), add additional `entity_properties` rows via direct SQL after calling this helper.
- `create_user(conn, name) → i64` — creates a `user` entity
- `create_tor(conn, name) → i64` — creates a `tor` entity
- `rel_type(conn, name) → i64` — looks up a relation type's entity ID by name (uses the pre-seeded types from `seed_base_entities`)
- `fills_position(conn, user_id, func_id)` — creates a `fills_position` relation
- `belongs_to_tor(conn, func_id, tor_id)` — creates a `belongs_to_tor` relation

The helpers call `.unwrap()` since test panics are acceptable and the setup must be deterministic.

**Seven test cases:**

1. **`test_has_capability_true`** — A user fills a position with a `can_call_meetings=true` property in a given ToR. `has_resource_capability` should return `true`.

2. **`test_has_capability_false_when_flag_is_false`** — The user fills a position but the capability value is `'false'`. Should return `false`.

3. **`test_has_capability_false_when_not_member`** — The user exists and the ToR exists but there are no relations between them. Should return `false`.

4. **`test_boundary_isolation_different_tor`** — The user has the capability in ToR A but the check is against ToR B. Should return `false`, confirming that capability checks are scoped to the specific resource.

5. **`test_missing_capability_key_returns_false`** — The user's function has a different capability (`can_manage_agenda=true`) but not the one being checked (`can_call_meetings`). Should return `false`.

6. **`test_load_tor_capabilities_returns_all_true_flags`** — A function entity has three capability properties: `can_call_meetings=true`, `can_manage_agenda=true`, `can_record_decisions=false`. `load_tor_capabilities` should return a `Permissions` struct containing the two true keys but not the false one. **Implementation note:** `create_function` sets one property. Add the second and third properties via direct `INSERT INTO entity_properties` statements after calling the helper.

7. **`test_load_tor_capabilities_empty_for_non_member`** — No `fills_position` or `belongs_to_tor` relations exist for the user in the ToR. `load_tor_capabilities` should return an empty `Permissions`.

Note: `require_tor_capability` is tested indirectly through integration tests in later splits. This split focuses on the two query functions. The branching logic in `require_tor_capability` has a coverage gap in this split unless the optional pure-function extraction is done (see above).

---

## Verification

After implementation, the following must pass:

- `cargo test --test abac_test` — all 7 tests green
- `cargo clippy` — no new warnings
- `cargo check` — clean compile

The existing test suite (`cargo test`) should continue to pass with no regressions, since this split only adds new files and one module declaration to `mod.rs`.
