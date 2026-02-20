# Synthesized Spec — ABAC Core Module

Combines: initial spec + codebase research + rusqlite patterns + interview decisions.

---

## What We're Building

A new Rust module `src/auth/abac.rs` that provides Attribute-Based Access Control for ToR resource operations. This is Split 1 of 3 in the ABAC implementation series. Splits 2 and 3 (handler migration and template wiring) depend on this module being complete and tested.

The problem: the current system uses flat global RBAC (`tor.edit` permission applies to all ToRs). Regular ToR members (e.g., Chair, Secretary) with explicitly encoded capabilities in EAV cannot perform lifecycle operations without the global `tor.edit` permission, which is granted only to admins. This module adds fine-grained capability checking through the existing EAV graph without any schema changes.

---

## Three Functions to Implement

### 1. `has_resource_capability`

```rust
pub fn has_resource_capability(
    conn: &Connection,
    user_id: i64,
    resource_id: i64,
    belongs_to_rel: &str,
    capability: &str,
) -> Result<bool, AppError>
```

- **Purpose:** Generic EAV graph query — checks if a user fills any function entity that (a) belongs to the resource via `belongs_to_rel` AND (b) has `capability = "true"` in `entity_properties`.
- **OR semantics:** If the user holds multiple positions in the ToR, returns `true` if ANY of them has the capability.
- **DB errors:** Propagate as `Err(AppError::Db(e))` — do not swallow errors.
- **Non-member:** Returns `Ok(false)` safely (COUNT(*) = 0).
- **Generic design confirmed:** `belongs_to_rel` is a string parameter enabling future reuse for `belongs_to_governance` or similar.

### 2. `load_tor_capabilities`

```rust
pub fn load_tor_capabilities(
    conn: &Connection,
    user_id: i64,
    tor_id: i64,
) -> Result<Permissions, AppError>
```

- **Purpose:** Bulk loader returning all capabilities the user has in a given ToR as a `Permissions` struct. One query, all `can_*` keys where value is `'true'`.
- **Non-member:** Returns `Ok(Permissions::default())` (empty).
- **ToR-specific:** Hard-codes `"belongs_to_tor"` and `"fills_position"` internally.
- **Used by:** The `detail` meeting handler to populate `tor_capabilities` in template context (Split 3).

### 3. `require_tor_capability`

```rust
pub fn require_tor_capability(
    conn: &Connection,
    session: &Session,
    tor_id: i64,
    capability: &str,
) -> Result<(), AppError>
```

- **Purpose:** Handler helper. Two-phase check: (1) `tor.edit` global bypass, (2) ABAC capability check.
- **Phase 1:** `require_permission(session, "tor.edit")` — if `Ok`, return `Ok(())` immediately.
- **Phase 2:** Call `has_resource_capability(conn, user_id, tor_id, "belongs_to_tor", capability)`.
- **Unauthenticated:** If `get_user_id` returns `None`, return `Err(AppError::Session("Not authenticated".to_string()))`.
- **Neither phase:** Return `Err(AppError::PermissionDenied(capability.to_string()))`.

---

## Files

| Action | File |
|--------|------|
| Create | `src/auth/abac.rs` |
| Modify | `src/auth/mod.rs` — add `pub mod abac;` |
| Create | `tests/abac_test.rs` |

---

## Key Constraints

### No Schema Changes
All queries traverse the existing EAV graph. Relation types `fills_position` and `belongs_to_tor` are already seeded by `seed_base_entities()` in `tests/common/mod.rs`.

### Permissions Struct
Defined in `src/auth/session.rs`:
```rust
#[derive(Debug, Clone, Default)]
pub struct Permissions(pub Vec<String>);
```
Constructor for `load_tor_capabilities`: `Ok(Permissions(keys))` where `keys: Vec<String>`.

### AppError Conversions
`rusqlite::Error` → `AppError::Db` conversion already exists in the codebase. Use `?` propagation freely.

### SQL Pattern — Relation Type ID Resolution
The canonical codebase pattern is an inline scalar subquery (NOT a JOIN):
```sql
AND r_fills.relation_type_id = (
    SELECT id FROM entities
    WHERE entity_type = 'relation_type' AND name = 'fills_position'
)
```
This pattern is used throughout `src/models/relation.rs`, `src/warnings/generators.rs`, and permission queries.

### rusqlite Parameter Syntax
Use `params![p1, p2, ...]` with `?1`, `?2` positional placeholders:
```rust
conn.query_row("... WHERE x = ?1 AND y = ?2", params![x, y], |row| row.get(0))?;
```

---

## SQL Queries

### `has_resource_capability` query
```sql
SELECT COUNT(*)
FROM relations r_fills
JOIN relations r_belongs ON r_belongs.source_id = r_fills.target_id
JOIN entity_properties ep ON ep.entity_id = r_fills.target_id
WHERE r_fills.source_id = ?1
  AND r_belongs.target_id = ?2
  AND r_fills.relation_type_id = (
      SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position')
  AND r_belongs.relation_type_id = (
      SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = ?3)
  AND ep.key = ?4
  AND ep.value = 'true'
```

Returns `count > 0` as bool.

### `load_tor_capabilities` query
```sql
SELECT DISTINCT ep.key
FROM relations r_fills
JOIN relations r_tor ON r_tor.source_id = r_fills.target_id
JOIN entity_properties ep ON ep.entity_id = r_fills.target_id
WHERE r_fills.source_id = ?1
  AND r_tor.target_id = ?2
  AND r_fills.relation_type_id = (
      SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position')
  AND r_tor.relation_type_id = (
      SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor')
  AND ep.key LIKE 'can_%'
  AND ep.value = 'true'
```

Collect results as `Vec<String>` → wrap in `Permissions(keys)`.

---

## Test Cases (7 Total)

| # | Name | Setup | Assert |
|---|------|-------|--------|
| 1 | `test_has_capability_true` | alice → chair_alpha (can_call_meetings=true) → tor_alpha | `true` |
| 2 | `test_has_capability_false_when_flag_is_false` | bob → member_beta (can_call_meetings=false) → tor_beta | `false` |
| 3 | `test_has_capability_false_when_not_member` | charlie + tor_gamma, no relations | `false` |
| 4 | `test_boundary_isolation_different_tor` | diana → chair_a (cap=true) → tor_a; check against tor_b | `false` |
| 5 | `test_missing_capability_key_returns_false` | eve → secretary (can_manage_agenda=true only) → tor_delta; check can_call_meetings | `false` |
| 6 | `test_load_tor_capabilities_returns_all_true_flags` | frank → chair_epsilon (2 true + 1 false) → tor_epsilon | 2 keys in Permissions |
| 7 | `test_load_tor_capabilities_empty_for_non_member` | grace + tor_zeta, no relations | empty Permissions |

---

## Provides to Split 2

Public exports from `crate::auth::abac`:
- `has_resource_capability(conn, user_id, resource_id, belongs_to_rel, capability) → Result<bool, AppError>`
- `load_tor_capabilities(conn, user_id, tor_id) → Result<Permissions, AppError>`
- `require_tor_capability(conn, session, tor_id, capability) → Result<(), AppError>`
