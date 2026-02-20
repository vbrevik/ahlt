# Spec: Template Context + UI Guards

**Feature:** Wire `tor_capabilities` into meeting detail template context; update button visibility
**Design doc:** `docs/plans/2026-02-20-abac-design.md` — "Template Context" section
**Existing impl plan:** `docs/plans/2026-02-20-abac-implementation-plan.md` (Tasks 7–9)
**Part of series:** Split 3 of 3 (depends on 01-abac-core and 02-handler-migration)

---

## Goal

Add a `tor_capabilities: Permissions` field to `MeetingDetailTemplate` so the meeting detail page can show/hide action buttons for ToR members with the appropriate `can_*` capability, not just global `tor.edit` holders.

## What to Build

### 1. Extend `MeetingDetailTemplate` (`src/templates_structs.rs`)

Add field:
```rust
pub tor_capabilities: crate::auth::session::Permissions,
```

### 2. Populate in `detail` handler (`src/handlers/meeting_handlers/crud.rs`)

Before building `MeetingDetailTemplate`, call:
```rust
let user_id = get_user_id(&session).unwrap_or(0);
let tor_capabilities = abac::load_tor_capabilities(&conn, user_id, tor_id)
    .unwrap_or_default();
```

Add `tor_capabilities` to the struct literal.

### 3. Update `templates/meetings/detail.html`

Find all `{% if ctx.permissions.has("tor.edit") %}` guards that protect action buttons (NOT read-only sections). Replace each with nested `{% if %}` blocks to allow either global bypass OR ToR capability.

**Askama constraint — no `||` in `{% if %}`:**
```html
{% if ctx.permissions.has("tor.edit") %}
  <button>Confirm</button>
{% else %}{% if tor_capabilities.has("can_call_meetings") %}
  <button>Confirm</button>
{% endif %}{% endif %}
```

**Capability mapping for template guards:**
- Confirm/transition buttons → `can_call_meetings`
- Agenda management buttons → `can_manage_agenda`
- Roll call section (lines ~295, ~305) → `can_record_decisions`

### 4. Run full test suite

```bash
cargo test 2>&1 | tail -20
cargo clippy 2>&1 | grep -E "^error|warning\["
```

Expected: ≥159 tests pass (152 existing + 7 new ABAC tests).

## Key Constraints

- **`unwrap_or_default()`** on `load_tor_capabilities` — `Permissions` must implement `Default` (returns empty). Confirm this in `src/auth/session.rs` before assuming.
- **Template field name** — use `tor_capabilities` (consistent with the struct field). The design doc draft uses `user_tor_capabilities` — prefer the shorter name.
- **Duplicate button HTML** — the nested `{% if %}{% else %}{% if %}` pattern means each button block appears twice. This is unavoidable without Askama macros. Keep button HTML identical in both branches.
- **Read-only sections** — do not change `{% if ctx.permissions.has("tor.edit") %}` guards on structural controls (edit metadata, manage functions, manage members).

## Files

- Modify: `src/templates_structs.rs`
- Modify: `src/handlers/meeting_handlers/crud.rs` (`detail` handler only)
- Modify: `templates/meetings/detail.html`

## Prerequisite

`01-abac-core` (for `load_tor_capabilities`), `02-handler-migration` (for conceptual completeness — server enforces, UI reflects).
