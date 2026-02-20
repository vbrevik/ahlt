# Spec: ABAC Core Module (`src/auth/abac.rs`)

**Feature:** Attribute-Based Access Control — core auth module
**Design doc:** `docs/plans/2026-02-20-abac-design.md`
**Existing impl plan:** `docs/plans/2026-02-20-abac-implementation-plan.md` (Tasks 1–2)
**Part of series:** Split 1 of 3 (02-handler-migration depends on this)

---

## Goal

Create `src/auth/abac.rs` using TDD. Write failing tests first, then implement until they pass.

## What to Build

Three functions in a new `pub mod abac` in `src/auth/`:

1. **`has_resource_capability(conn, user_id, resource_id, belongs_to_rel, capability) → Result<bool, AppError>`**
   — Generic EAV graph query: user fills a function entity that belongs to the resource AND has `capability = "true"` in `entity_properties`.

2. **`load_tor_capabilities(conn, user_id, tor_id) → Result<Permissions, AppError>`**
   — Bulk loader: one query returning all `can_*` keys with value `"true"` for the user in this ToR. Returns a `Permissions` struct.

3. **`require_tor_capability(conn, session, tor_id, capability) → Result<(), AppError>`**
   — Handler helper: global `tor.edit` bypass first, then calls `has_resource_capability`. Returns `Err(AppError::PermissionDenied)` if neither passes.

## Key Constraints

- **No schema changes** — queries traverse existing EAV graph via `fills_position` and `belongs_to_tor` relation types (both seeded by `seed_base_entities()` in `tests/common/mod.rs`)
- **`Permissions` struct** — defined in `src/auth/session.rs`, has a `has(&str) → bool` method and a `Default` impl (returns empty). Use it as the return type for `load_tor_capabilities`.
- **`Permissions` constructor** — it is a newtype `Permissions(Vec<String>)` or similar. Check `src/auth/session.rs` to confirm exact shape before implementing.
- **SQL pattern** — the design doc has the exact query. The subquery for relation type IDs is the correct approach for rusqlite (no joins on named relation types directly).

## Test File

`tests/abac_test.rs` — 7 test cases:

1. `test_has_capability_true` — member with flag `true` → `true`
2. `test_has_capability_false_when_flag_is_false` — member with flag `false` → `false`
3. `test_has_capability_false_when_not_member` — no membership → `false`
4. `test_boundary_isolation_different_tor` — member of ToR A, checking ToR B → `false`
5. `test_missing_capability_key_returns_false` — has a different `can_*` key, not the checked one → `false`
6. `test_load_tor_capabilities_returns_all_true_flags` — function with 2 true + 1 false → only 2 in result
7. `test_load_tor_capabilities_empty_for_non_member` — no membership → empty `Permissions`

The existing implementation plan (`Tasks 1–2`) contains complete, ready-to-use test helper functions and test bodies. Use them as the starting point.

## Files

- Create: `src/auth/abac.rs`
- Modify: `src/auth/mod.rs` — add `pub mod abac;`
- Create: `tests/abac_test.rs`

## Provides to Split 2

`crate::auth::abac::{has_resource_capability, load_tor_capabilities, require_tor_capability}`
