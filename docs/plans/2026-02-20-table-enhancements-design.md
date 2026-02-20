# Table Enhancement Pattern — Design Document

**Date:** 2026-02-20
**Status:** Approved
**First target:** Users table (`/users`)
**Next targets:** Roles table, and any future entity list tables

---

## Overview

This document describes a reusable data table enhancement pattern to be applied across all entity list pages. The pattern adds:

1. **Query builder** — nested AND/OR filter conditions (2 levels deep)
2. **Server-side sorting** — clickable column headers with direction toggle
3. **Column picker** — show/hide/reorder columns, persisted per-user and globally via EAV
4. **Per-page selector** — 10 / 25 / 50 / 100 rows
5. **CSV export** — all matching rows, honours filter/sort, ignores column visibility

All features are server-rendered. Filter and sort state lives in URL query params (bookmarkable, shareable). Column preferences live in the database.

---

## Layout

The existing search bar is **replaced** by the query builder. Below the builder sits a single controls bar.

```
┌─ Filters ───────────────────────────── Match [ALL ▼] ─ [+Cond] [+Group] ─┐
│  [username ▼] [contains ▼] [              ]  [✕]                         │
│  ┌─ Group ──────────────────────────── Match [ANY ▼] ─────────────────┐  │
│  │  [email  ▼] [contains ▼] [                    ]  [✕]               │  │
│  │  [role   ▼] [is       ▼] [Admin             ▼]  [✕]               │  │
│  │                                          [+ Add condition]          │  │
│  └────────────────────────────────────────────────────────────────────┘  │
│                                                      [Clear]  [Apply ▶]  │
└──────────────────────────────────────────────────────────────────────────┘

Show: [25 ▼]   [⊞ Columns]   [↓ Export CSV]   Showing 25 of 142 users

 ☐  User ▲          Email          Created ▼      Status    Actions
────────────────────────────────────────────────────────────────────
 ☐  👤 Alice...      alice@...      2024-01-15     ✓ Active  Edit  Del
```

**Bulk toolbar** continues to appear above the table when rows are selected (unchanged).

When no filters are active, the builder shows one empty condition row. "Apply" triggers a full page reload with the filter serialized into the URL. "Clear" resets to `/users` with no params.

---

## Feature 1: Query Builder (Nested AND/OR)

### Filter Serialization

Filter state is JSON-encoded in a single `filter` URL param. The URL remains human-readable in browser history:

```
/users?filter={"logic":"AND","conditions":[...],"groups":[...]}&sort=username&dir=asc&page=1&per_page=25
```

### Data Model

```rust
// src/models/table_filter/mod.rs

pub enum Logic { And, Or }

pub struct FilterTree {
    pub logic: Logic,
    pub conditions: Vec<Condition>,
    pub groups: Vec<Group>,     // max one level deep (depth = 2 total)
}

pub struct Group {
    pub logic: Logic,
    pub conditions: Vec<Condition>,
}

pub struct Condition {
    pub field: String,   // validated against per-table field whitelist
    pub op: String,      // validated against per-table operator whitelist
    pub value: String,   // user-supplied value — always parameterized, never interpolated
}
```

`FilterTree` serializes/deserializes via `serde_json`. Empty trees (no conditions, no groups) are treated as "no filter".

### SQL Builder

`src/models/table_filter/builder.rs` exposes:

```rust
pub fn build_where_clause(
    tree: &FilterTree,
    field_map: &HashMap<&str, &str>,   // "username" -> "e.name"
    op_map: &HashMap<&str, OpKind>,    // "contains" -> OpKind::Like
    param_offset: usize,               // for correct ?N numbering alongside other params
) -> Result<(String, Vec<String>), FilterError>
```

Returns `(sql_fragment, params)`. The SQL fragment is a parenthesised WHERE clause ready to append to the table's base query. Field names and operators come from enums/whitelists — **user values are never interpolated into SQL**.

Operator SQL mappings:

| Op | SQL |
|---|---|
| `contains` | `col LIKE '%' \|\| ?N \|\| '%'` |
| `not_contains` | `col NOT LIKE '%' \|\| ?N \|\| '%'` |
| `equals` | `col = ?N` |
| `not_equals` | `col != ?N` |
| `starts_with` | `col LIKE ?N \|\| '%'` |
| `is` / `is_not` | `col = ?N` / `col != ?N` |
| `before` | `col < ?N` |
| `after` | `col > ?N` |
| `on` | `DATE(col) = DATE(?N)` |

### Per-Table Field Maps

Each table defines its own field map in `src/models/{table}/filter.rs`:

**Users:**
| Field key | SQL column | Operators |
|---|---|---|
| `username` | `e.name` | contains, not_contains, equals, not_equals, starts_with |
| `display_name` | `e.label` | contains, not_contains, equals, not_equals, starts_with |
| `email` | `COALESCE(p_email.value, '')` | contains, not_contains, equals, not_equals |
| `role` | `COALESCE(role_e.name, '')` | is, is_not |
| `created_at` | `e.created_at` | before, after, on |
| `updated_at` | `e.updated_at` | before, after, on |

### Frontend Query Builder UI

The filter builder is rendered server-side from the parsed `FilterTree`. On page load, the template renders the current filter state back into the UI so the user sees their active conditions.

JS handles:
- Adding/removing condition rows and groups (DOM manipulation via `createElement`, no `innerHTML`)
- Updating operator options when field changes (some fields have different operator lists)
- Serializing the current builder state to JSON on "Apply" click, writing to a hidden `<input>` before form submit

The filter panel is always visible (not collapsible) — it is the primary filter mechanism, replacing the old search bar.

---

## Feature 2: Server-Side Sorting

### URL Params

`?sort=username&dir=asc` — sort state is preserved when filters or pagination change.

Clicking an active column header toggles `dir` (asc ↔ desc). Clicking a new column resets to `asc`.

Active column shows ▲ (asc) or ▼ (desc) indicator. Inactive sortable columns show no indicator.

### SQL — Whitelist Only

```rust
// src/models/table_filter/mod.rs

pub enum SortDir { Asc, Desc }

pub struct SortSpec {
    pub column: String,   // validated against per-table sort whitelist
    pub dir: SortDir,
}
```

Field names come from a per-table whitelist enum. Only the safe SQL column expression is interpolated:

```rust
let sql_col = match sort.column.as_str() {
    "username"     => "e.name",
    "display_name" => "e.label",
    "email"        => "COALESCE(p_email.value, '')",
    "role"         => "COALESCE(role_e.name, '')",
    "created_at"   => "e.created_at",
    "updated_at"   => "e.updated_at",
    _              => "e.id",    // unknown → default, safe
};
let dir_str = match sort.dir { SortDir::Asc => "ASC", SortDir::Desc => "DESC" };
format!("ORDER BY {sql_col} {dir_str}")
```

### Sortable Columns per Table

**Users:** username, display_name, email, role, created_at, updated_at
**Roles (planned):** name, label, user_count, permission_count, created_at

The checkbox, Status, and Actions columns are never sortable.

---

## Feature 3: Column Picker

### Columns

All columns except the checkbox are shown in the picker and can be reordered. User and Actions are always visible (no hide toggle). Email, Status, Created, Updated can be hidden.

| Column key | Always visible | Sortable | Toggleable |
|---|---|---|---|
| `user` | ✓ | username / display_name | no |
| `email` | no | ✓ | ✓ |
| `status` | no | no | ✓ |
| `created_at` | no | ✓ | ✓ |
| `updated_at` | no | ✓ | ✓ |
| `actions` | ✓ | no | no |

The **checkbox** column is fixed leftmost and not shown in the picker.

### Picker UI

Clicking `[⊞ Columns]` opens an absolute-positioned popover:

```
┌─ Columns ──────────────────────┐
│  ⠿ ☑ User          (always on) │
│  ⠿ ☑ Email                     │
│  ⠿ ☑ Status                    │
│  ⠿ ☐ Created                   │
│  ⠿ ☐ Updated                   │
│  ⠿ ☑ Actions       (always on) │
│  ───────────────────────────── │
│  [Set as global default]        │
└────────────────────────────────┘
```

Rows are drag-and-drop reorderable (HTML5 drag API). Checkboxes are toggled inline. Changes trigger `POST /users/columns` immediately. Panel closes on outside click.

### Storage — EAV Pattern

Parameterised by table name for reuse across tables.

| Layer | Storage | Key pattern | Value format |
|---|---|---|---|
| Global default | `setting` entity | `{table}_table_columns` | `"user,email,status,actions"` |
| Per-user override | `entity_property` on current user | `pref.{table}_table_columns` | `"user,email,created_at,actions"` |

Resolution order: per-user override → global default → hardcoded fallback.

Value format: **ordered comma-separated column keys**. Order = column order. Presence = visible. Always-on columns (`user`, `actions`) are always in the string.

### Handler: `POST /users/columns`

Params:
- `columns` — new ordered column string
- `set_global` — optional boolean; if `true` and user has `settings.manage`, also updates the `setting` entity

Requires `users.list` permission. Returns redirect to `/users` with current query params preserved.

### Template Integration

Column state is passed as `Vec<ColumnDef>` to the template:

```rust
pub struct ColumnDef {
    pub key: String,
    pub label: String,
    pub visible: bool,
}
```

Template iterates the ordered vec, renders column-specific HTML per key:

```
{% for col in columns %}
{% if col.visible %}
<td class="users-table__{{ col.key }}">
  {% if col.key == "email" %}{{ user.email }}
  {% else if col.key == "status" %}<span class="status-badge">✓ Active</span>
  {% else if col.key == "created_at" %}{{ user.created_at }}
  {% else if col.key == "updated_at" %}{{ user.updated_at }}
  {% endif %}
</td>
{% endif %}
{% endfor %}
```

---

## Feature 4: Per-Page Selector

A `<select>` in the controls bar with options 10 / 25 / 50 / 100. Auto-submits via JS `onchange`. Appends to existing filter/sort params. Clamps to 1–100 server-side (existing behaviour, unchanged). Resets to page 1 on change.

---

## Feature 5: CSV Export

**Endpoint:** `GET /users/export.csv`

- Accepts `filter`, `sort`, `dir` params (same as list view)
- Exports **all matching rows** — no pagination
- Column picker is **ignored** — CSV always includes all columns
- Requires `users.list` permission (no new permission)
- Audit logs as `users.export` with filter params and row count
- Opens in new tab so the user stays on the list page

**CSV columns (fixed):** id, username, display_name, email, role, created_at, updated_at

**Response headers:**
```
Content-Type: text/csv; charset=utf-8
Content-Disposition: attachment; filename="users-{YYYY-MM-DD}.csv"
```

Backend: `find_all_filtered()` in `queries.rs` — same SQL as `find_paginated()` without `LIMIT`/`OFFSET`.

---

## Shared Infrastructure

### Module: `src/models/table_filter/`

```
src/models/table_filter/
├── mod.rs      — FilterTree, Group, Condition, Logic, SortSpec, SortDir — pub use
├── builder.rs  — build_where_clause(), generic SQL clause builder
└── columns.rs  — ColumnDef, resolve_columns(table, user_id, conn)
```

`resolve_columns()` reads user's `entity_property` then global `setting`, returning the final ordered `Vec<ColumnDef>`. Takes `table: &str` to parameterise the keys.

### Template Partials

```
templates/partials/
├── table_filter.html    — filter builder UI; receives field definitions as JSON in template var
├── column_picker.html   — column picker popover; receives Vec<ColumnDef>
└── table_controls.html  — per-page selector + export button + column picker button + result count
```

Each partial is table-agnostic. Table-specific behaviour (field labels, operator lists for the filter builder) is injected via template variables from the handler.

### Rolling Out to New Tables

To add these features to a new table (e.g. Roles):

1. Add `src/models/role/filter.rs` — field map + operator whitelist
2. Extend the list handler to parse `filter`, `sort`, `dir`, `per_page` and resolve columns
3. Add `POST /roles/columns` and `GET /roles/export.csv` routes and handlers
4. Include the three partials in the list template with table-specific field definitions
5. No changes to shared infrastructure

---

## Files Changed (Users Baseline)

| Action | Path |
|---|---|
| **Create** | `src/models/table_filter/mod.rs` |
| **Create** | `src/models/table_filter/builder.rs` |
| **Create** | `src/models/table_filter/columns.rs` |
| **Create** | `src/models/user/filter.rs` |
| **Create** | `templates/partials/table_filter.html` |
| **Create** | `templates/partials/column_picker.html` |
| **Create** | `templates/partials/table_controls.html` |
| **Modify** | `src/models/user/queries.rs` — extend `find_paginated()`, add `find_all_filtered()` |
| **Modify** | `src/models/user/types.rs` — remove `UserPage`, replace with generic paged result |
| **Modify** | `src/models/user/mod.rs` — pub use filter |
| **Modify** | `src/lib.rs` — pub mod table_filter |
| **Modify** | `src/handlers/user_handlers/list.rs` — parse all new params, resolve columns |
| **Modify** | `src/handlers/user_handlers/crud.rs` — add `export_csv()`, `save_columns()` |
| **Modify** | `src/templates_structs.rs` — update `UserListTemplate` |
| **Modify** | `src/main.rs` — register `/users/export.csv` and `/users/columns` routes |
| **Modify** | `templates/users/list.html` — wire partials, sortable headers, Vec<ColumnDef> iteration |
| **Modify** | `static/css/style.css` — filter builder styles, column picker popover styles |

---

## Out of Scope

- Saved/named filter presets (future extension)
- Infinite group nesting (2 levels is the hard limit)
- Per-column CSV export (CSV always includes all columns)
- Client-side live filtering without page reload
