<!-- SPLIT_MANIFEST
01-abac-core
02-handler-migration
03-template-ui
END_MANIFEST -->

# ABAC Implementation — Project Manifest

**Feature:** Attribute-Based Access Control (ABAC) for ToR and meeting lifecycle operations.
**Design doc:** `docs/plans/2026-02-20-abac-design.md`
**Existing implementation plan:** `docs/plans/2026-02-20-abac-implementation-plan.md`

---

## Overview

The ABAC implementation is decomposed into three sequential splits. Each split builds on the previous:

```
01-abac-core  →  02-handler-migration  →  03-template-ui
```

The core module must compile and have passing tests before handlers can import it. The template context field must exist before the template can reference it. Splits cannot be parallelised.

---

## Split 1: `01-abac-core`

**Scope:** Create `src/auth/abac.rs` and its tests using TDD.

**Tasks (from existing impl plan):**
- Task 1: Write failing tests (`tests/abac_test.rs`) — 7 test cases covering `has_resource_capability` and `load_tor_capabilities`
- Task 2: Implement `src/auth/abac.rs` — three functions: `has_resource_capability`, `load_tor_capabilities`, `require_tor_capability`; add `pub mod abac;` to `src/auth/mod.rs`

**Key constraints:**
- No schema changes — queries traverse existing EAV graph via `fills_position` and `belongs_to_tor` relation types (both seeded)
- `Permissions` struct reused as return type for `load_tor_capabilities`
- Tests must fail on write, pass after implementation

**Files changed:**
- Create: `src/auth/abac.rs`
- Modify: `src/auth/mod.rs` (add `pub mod abac;`)
- Create: `tests/abac_test.rs`

---

## Split 2: `02-handler-migration`

**Scope:** Replace `require_permission("tor.edit")` in 9 handlers with ABAC checks.

**Depends on:** `01-abac-core` (must import `crate::auth::abac`)

**Tasks (from existing impl plan):**
- Task 3: Migrate `confirm` and `transition` handlers — `can_call_meetings`
- Task 4: Migrate `confirm_calendar` handler — special case: JSON response, uses `has_resource_capability` directly
- Task 5: Migrate `assign_agenda`, `remove_agenda`, `save_roll_call`, `generate_minutes` — `can_manage_agenda` and `can_record_decisions`
- Task 6: Migrate `save_attendance`, `save_action_items` in minutes handlers — must resolve `tor_id` via `minutes → meeting → tor_id`

**Key constraints:**
- `confirm_calendar` cannot use `?` operator — returns JSON, not `AppError` — must call `has_resource_capability` inline
- `save_attendance` and `save_action_items` only receive `minutes_id`, not `tor_id` — must join through `minutes::find_by_id` → `meeting::find_by_id`
- In all handlers: reorder so `conn` and `tor_id`/`path.into_inner()` come before the ABAC check (ABAC query needs `conn`)

**Files changed:**
- Modify: `src/handlers/meeting_handlers/crud.rs` (6 handlers)
- Modify: `src/handlers/minutes_handlers/crud.rs` (2 handlers + new imports)

---

## Split 3: `03-template-ui`

**Scope:** Wire `tor_capabilities` into the meeting detail template context and update button visibility guards.

**Depends on:** `02-handler-migration` (conceptually; technically only needs `01-abac-core` for the struct)

**Tasks (from existing impl plan):**
- Task 7: Add `tor_capabilities: crate::auth::session::Permissions` field to `MeetingDetailTemplate` in `src/templates_structs.rs`; populate via `abac::load_tor_capabilities` in the `detail` handler
- Task 8: Update `templates/meetings/detail.html` — change `{% if ctx.permissions.has("tor.edit") %}` guards to nested `{% if %}` blocks for ABAC capability OR global bypass
- Task 9: Run full test suite (`cargo test`), clippy, and verify all ≥159 tests pass

**Key constraints:**
- Askama 0.14 does not support `||` in `{% if %}` — must use nested `{% if %}{% else %}{% if %}{% endif %}{% endif %}` pattern
- `load_tor_capabilities` must be called with `unwrap_or_default()` to degrade gracefully for non-members
- Template field name: `tor_capabilities` (not `user_tor_capabilities` — keep consistent with struct field)

**Files changed:**
- Modify: `src/templates_structs.rs` (`MeetingDetailTemplate`)
- Modify: `src/handlers/meeting_handlers/crud.rs` (`detail` handler)
- Modify: `templates/meetings/detail.html`

---

## Execution Order

```bash
# Split 1
/deep-plan @docs/plans/01-abac-core/spec.md

# Split 2 (after 01 passes cargo test)
/deep-plan @docs/plans/02-handler-migration/spec.md

# Split 3 (after 02 passes cargo check)
/deep-plan @docs/plans/03-template-ui/spec.md
```

---

## Cross-Cutting Concerns

- **No new dependencies** — pure rusqlite + actix-session
- **Global bypass preserved** — `tor.edit` always bypasses ABAC; admins unaffected
- **Structural handlers untouched** — all `tor_handlers/` remain behind `require_permission("tor.edit")`
- **Minutes structural handlers untouched** — `view_minutes`, `update_minutes_status`, `update_section`, `save_distribution` remain behind `minutes.edit` / `minutes.approve`
