# Table Enhancements Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add query builder (nested AND/OR), server-side sorting, column picker (EAV-persisted), per-page selector, and CSV export to the users table — with shared infrastructure reusable by future tables.

**Architecture:** Shared `src/models/table_filter/` module provides generic `FilterTree` types, a parameterised SQL builder, and column preference resolver. Per-table modules (`user/filter.rs`) supply field maps and operator whitelists. Three reusable Askama partials (`table_filter.html`, `column_picker.html`, `table_controls.html`) are wired into the users list template. Filter state lives in URL query params (GET form, bookmarkable). Column preferences live in EAV (per-user `entity_property` + global `setting` entity).

**Tech Stack:** Rust / Actix-web 4 / Askama 0.14 / SQLite (rusqlite) / vanilla JS (no innerHTML, `createElement` only per security hook)

**Design doc:** `docs/plans/2026-02-20-table-enhancements-design.md`

**Key gotchas before starting:**
- Askama string equality in loops: `col.key.as_str() == "email"` (use `.as_str()` on both sides)
- No `&&` in `{% if %}` — use nested `{% if a %}{% if b %}` blocks
- Security hook rejects `innerHTML` — all DOM building via `createElement`/`textContent`/`appendChild`
- `relation::create()` takes relation type name string, not ID
- After template changes run `cargo clean` if you get "template not found" errors

---

## Task 1: `table_filter` — Core Types

**Files:**
- Create: `src/models/table_filter/mod.rs`
- Create: `src/models/table_filter/builder.rs` (stub only — filled in Task 2)
- Create: `src/models/table_filter/columns.rs` (stub only — filled in Task 3)

**Step 1: Create the module directory and core types**

```rust
// src/models/table_filter/mod.rs
use serde::{Deserialize, Serialize};

pub mod builder;
pub mod columns;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Logic { #[default] And, Or }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub field: String,
    pub op: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Group {
    #[serde(default)]
    pub logic: Logic,
    #[serde(default)]
    pub conditions: Vec<Condition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FilterTree {
    #[serde(default)]
    pub logic: Logic,
    #[serde(default)]
    pub conditions: Vec<Condition>,
    #[serde(default)]
    pub groups: Vec<Group>,
}

impl FilterTree {
    pub fn is_empty(&self) -> bool {
        self.conditions.is_empty() && self.groups.is_empty()
    }
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum SortDir { #[default] Asc, Desc }

#[derive(Debug, Clone, Default)]
pub struct SortSpec {
    pub column: String,  // e.g. "username" — empty = default sort
    pub dir: SortDir,
}

impl SortSpec {
    pub fn from_params(sort: Option<&str>, dir: Option<&str>) -> Self {
        SortSpec {
            column: sort.unwrap_or("").to_string(),
            dir: if dir == Some("desc") { SortDir::Desc } else { SortDir::Asc },
        }
    }
    pub fn dir_str(&self) -> &'static str {
        match self.dir { SortDir::Asc => "asc", SortDir::Desc => "desc" }
    }
    pub fn toggle_dir(&self) -> &'static str {
        match self.dir { SortDir::Asc => "desc", SortDir::Desc => "asc" }
    }
}

/// Ordered column definition passed to templates.
#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub key: String,
    pub label: String,
    pub visible: bool,
    pub always_visible: bool,
    pub sortable: bool,
    pub sort_key: String,  // sort param value, e.g. "username" (empty if not sortable)
}
```

Create empty stubs for `builder.rs` and `columns.rs`:
```rust
// src/models/table_filter/builder.rs
// (populated in Task 2)
```
```rust
// src/models/table_filter/columns.rs
// (populated in Task 3)
```

**Step 2: Write serde roundtrip test inline in mod.rs**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_tree_empty_roundtrip() {
        let tree = FilterTree::default();
        let json = tree.to_json();
        let back = FilterTree::from_json(&json).unwrap();
        assert!(back.is_empty());
    }

    #[test]
    fn filter_tree_full_roundtrip() {
        let tree = FilterTree {
            logic: Logic::And,
            conditions: vec![Condition {
                field: "username".into(),
                op: "contains".into(),
                value: "alice".into(),
            }],
            groups: vec![Group {
                logic: Logic::Or,
                conditions: vec![
                    Condition { field: "email".into(), op: "contains".into(), value: "@acme".into() },
                    Condition { field: "role".into(), op: "is".into(), value: "admin".into() },
                ],
            }],
        };
        let json = tree.to_json();
        let back = FilterTree::from_json(&json).unwrap();
        assert_eq!(back.conditions.len(), 1);
        assert_eq!(back.groups.len(), 1);
        assert_eq!(back.groups[0].conditions.len(), 2);
        assert_eq!(back.logic, Logic::And);
        assert_eq!(back.groups[0].logic, Logic::Or);
    }

    #[test]
    fn sort_spec_from_params() {
        let s = SortSpec::from_params(Some("username"), Some("desc"));
        assert_eq!(s.column, "username");
        assert_eq!(s.dir, SortDir::Desc);
        assert_eq!(s.toggle_dir(), "asc");
    }
}
```

**Step 3: Run tests**
```bash
cargo test table_filter
```
Expected: 3 tests pass.

**Step 4: Commit**
```bash
git add src/models/table_filter/
git commit -m "feat: add table_filter core types (FilterTree, SortSpec, ColumnDef)"
```

---

## Task 2: `table_filter` — SQL Builder

**Files:**
- Modify: `src/models/table_filter/builder.rs`

**Step 1: Write failing tests first**

```rust
// src/models/table_filter/builder.rs

use std::collections::HashMap;
use super::{FilterTree, Group, Condition, Logic};

#[derive(Debug)]
pub enum BuildError {
    UnknownField(String),
    UnknownOp(String),
}

/// Build a parameterized WHERE fragment from a FilterTree.
/// Returns (sql_fragment, params_vec).
/// param_offset: the ?N index to start from (so callers can combine with other params).
/// field_map: "field_key" -> "sql_col_expression" (must be pre-validated SQL, not user input)
/// op_whitelist: set of allowed operator strings for this table
pub fn build_where_clause(
    tree: &FilterTree,
    field_map: &HashMap<&str, &str>,
    op_whitelist: &[&str],
    param_offset: usize,
) -> Result<(String, Vec<String>), BuildError> {
    if tree.is_empty() {
        return Ok(("1=1".to_string(), vec![]));
    }
    let mut params: Vec<String> = vec![];
    let mut parts: Vec<String> = vec![];

    // Root-level conditions
    for cond in &tree.conditions {
        let (sql, mut p) = build_condition(cond, field_map, op_whitelist, param_offset + params.len())?;
        params.append(&mut p);
        parts.push(sql);
    }

    // Groups (one level deep)
    for group in &tree.groups {
        let (sql, mut p) = build_group(group, field_map, op_whitelist, param_offset + params.len())?;
        if !sql.is_empty() {
            params.append(&mut p);
            parts.push(format!("({sql})"));
        }
    }

    if parts.is_empty() {
        return Ok(("1=1".to_string(), vec![]));
    }

    let logic = match tree.logic { Logic::And => " AND ", Logic::Or => " OR " };
    Ok((parts.join(logic), params))
}

fn build_group(
    group: &Group,
    field_map: &HashMap<&str, &str>,
    op_whitelist: &[&str],
    param_offset: usize,
) -> Result<(String, Vec<String>), BuildError> {
    let mut params: Vec<String> = vec![];
    let mut parts: Vec<String> = vec![];
    for cond in &group.conditions {
        let (sql, mut p) = build_condition(cond, field_map, op_whitelist, param_offset + params.len())?;
        params.append(&mut p);
        parts.push(sql);
    }
    if parts.is_empty() { return Ok(("".to_string(), vec![])); }
    let logic = match group.logic { Logic::And => " AND ", Logic::Or => " OR " };
    Ok((parts.join(logic), params))
}

fn build_condition(
    cond: &Condition,
    field_map: &HashMap<&str, &str>,
    op_whitelist: &[&str],
    param_offset: usize,
) -> Result<(String, Vec<String>), BuildError> {
    let col = field_map.get(cond.field.as_str())
        .ok_or_else(|| BuildError::UnknownField(cond.field.clone()))?;
    if !op_whitelist.contains(&cond.op.as_str()) {
        return Err(BuildError::UnknownOp(cond.op.clone()));
    }
    let n = param_offset + 1;  // rusqlite uses 1-based ?N
    let (sql, value) = match cond.op.as_str() {
        "contains"     => (format!("{col} LIKE '%' || ?{n} || '%'"), cond.value.clone()),
        "not_contains" => (format!("{col} NOT LIKE '%' || ?{n} || '%'"), cond.value.clone()),
        "equals" | "is"       => (format!("{col} = ?{n}"), cond.value.clone()),
        "not_equals" | "is_not" => (format!("{col} != ?{n}"), cond.value.clone()),
        "starts_with"  => (format!("{col} LIKE ?{n} || '%'"), cond.value.clone()),
        "before"       => (format!("{col} < ?{n}"), cond.value.clone()),
        "after"        => (format!("{col} > ?{n}"), cond.value.clone()),
        "on"           => (format!("DATE({col}) = DATE(?{n})"), cond.value.clone()),
        _ => return Err(BuildError::UnknownOp(cond.op.clone())),
    };
    Ok((sql, vec![value]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::table_filter::{Condition, FilterTree, Group, Logic};

    fn user_field_map() -> HashMap<&'static str, &'static str> {
        let mut m = HashMap::new();
        m.insert("username", "e.name");
        m.insert("email", "COALESCE(p_email.value, '')");
        m.insert("role", "COALESCE(role_e.name, '')");
        m
    }

    const USER_OPS: &[&str] = &[
        "contains", "not_contains", "equals", "not_equals",
        "starts_with", "is", "is_not", "before", "after", "on",
    ];

    #[test]
    fn empty_tree_returns_passthrough() {
        let (sql, params) = build_where_clause(
            &FilterTree::default(), &user_field_map(), USER_OPS, 0
        ).unwrap();
        assert_eq!(sql, "1=1");
        assert!(params.is_empty());
    }

    #[test]
    fn single_contains_condition() {
        let tree = FilterTree {
            conditions: vec![Condition { field: "username".into(), op: "contains".into(), value: "alice".into() }],
            ..Default::default()
        };
        let (sql, params) = build_where_clause(&tree, &user_field_map(), USER_OPS, 0).unwrap();
        assert_eq!(sql, "e.name LIKE '%' || ?1 || '%'");
        assert_eq!(params, vec!["alice"]);
    }

    #[test]
    fn root_and_two_conditions() {
        let tree = FilterTree {
            logic: Logic::And,
            conditions: vec![
                Condition { field: "username".into(), op: "contains".into(), value: "alice".into() },
                Condition { field: "role".into(), op: "is".into(), value: "admin".into() },
            ],
            ..Default::default()
        };
        let (sql, params) = build_where_clause(&tree, &user_field_map(), USER_OPS, 0).unwrap();
        assert_eq!(sql, "e.name LIKE '%' || ?1 || '%' AND COALESCE(role_e.name, '') = ?2");
        assert_eq!(params, vec!["alice", "admin"]);
    }

    #[test]
    fn condition_with_nested_or_group() {
        let tree = FilterTree {
            logic: Logic::And,
            conditions: vec![
                Condition { field: "role".into(), op: "is".into(), value: "admin".into() },
            ],
            groups: vec![Group {
                logic: Logic::Or,
                conditions: vec![
                    Condition { field: "username".into(), op: "contains".into(), value: "alice".into() },
                    Condition { field: "email".into(), op: "contains".into(), value: "@acme".into() },
                ],
            }],
        };
        let (sql, params) = build_where_clause(&tree, &user_field_map(), USER_OPS, 0).unwrap();
        assert_eq!(
            sql,
            "COALESCE(role_e.name, '') = ?1 AND (e.name LIKE '%' || ?2 || '%' OR COALESCE(p_email.value, '') LIKE '%' || ?3 || '%')"
        );
        assert_eq!(params, vec!["admin", "alice", "@acme"]);
    }

    #[test]
    fn unknown_field_returns_error() {
        let tree = FilterTree {
            conditions: vec![Condition { field: "nonexistent".into(), op: "equals".into(), value: "x".into() }],
            ..Default::default()
        };
        assert!(build_where_clause(&tree, &user_field_map(), USER_OPS, 0).is_err());
    }
}
```

**Step 2: Run tests**
```bash
cargo test table_filter::builder
```
Expected: 5 tests pass.

**Step 3: Commit**
```bash
git add src/models/table_filter/builder.rs
git commit -m "feat: add table_filter SQL builder with nested AND/OR support"
```

---

## Task 3: `table_filter` — Column Resolver

**Files:**
- Modify: `src/models/table_filter/columns.rs`

**Step 1: Implement**

```rust
// src/models/table_filter/columns.rs
use rusqlite::{Connection, params};
use super::ColumnDef;

/// Read the user's per-table column preference from entity_properties.
/// key: "pref.{table}_table_columns"
fn read_user_pref(user_id: i64, table: &str, conn: &Connection) -> Option<String> {
    let key = format!("pref.{table}_table_columns");
    conn.query_row(
        "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = ?2",
        params![user_id, key],
        |row| row.get(0),
    ).ok()
}

/// Read the global default from a setting entity.
/// setting name: "{table}_table_columns"
fn read_global_default(table: &str, conn: &Connection) -> Option<String> {
    let name = format!("{table}_table_columns");
    conn.query_row(
        "SELECT COALESCE(p.value, '') FROM entities e \
         JOIN entity_properties p ON e.id = p.entity_id AND p.key = 'value' \
         WHERE e.entity_type = 'setting' AND e.name = ?1",
        params![name],
        |row| row.get(0),
    ).ok().filter(|s: &String| !s.is_empty())
}

/// Resolve the ordered column list for a table.
/// all_columns: the full ordered default list for the table.
/// Resolution: user pref > global default > all_columns order (visible = always_visible || default).
pub fn resolve_columns(
    table: &str,
    user_id: i64,
    conn: &Connection,
    all_columns: &[ColumnDef],
) -> Vec<ColumnDef> {
    let source = read_user_pref(user_id, table, conn)
        .or_else(|| read_global_default(table, conn));

    match source {
        Some(pref) => apply_pref(all_columns, &pref),
        None => all_columns.to_vec(),
    }
}

/// Apply a comma-separated ordered column string to the full column list.
/// Columns in the string appear first (in order), rest appended hidden at end.
fn apply_pref(all_columns: &[ColumnDef], pref: &str) -> Vec<ColumnDef> {
    let ordered_keys: Vec<&str> = pref.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
    let mut result: Vec<ColumnDef> = vec![];

    // First: columns in pref order, visible if present
    for key in &ordered_keys {
        if let Some(col) = all_columns.iter().find(|c| c.key.as_str() == *key) {
            let mut c = col.clone();
            c.visible = true;
            result.push(c);
        }
    }

    // Then: any always_visible columns not yet in result
    for col in all_columns {
        if col.always_visible && !result.iter().any(|c| c.key == col.key) {
            let mut c = col.clone();
            c.visible = true;
            result.push(c);
        }
    }

    // Then: remaining columns hidden
    for col in all_columns {
        if !result.iter().any(|c| c.key == col.key) {
            let mut c = col.clone();
            c.visible = false;
            result.push(c);
        }
    }

    result
}

/// Serialize a Vec<ColumnDef> to a pref string (only visible columns, in order).
pub fn columns_to_pref(columns: &[ColumnDef]) -> String {
    columns.iter()
        .filter(|c| c.visible || c.always_visible)
        .map(|c| c.key.as_str())
        .collect::<Vec<_>>()
        .join(",")
}

/// Save per-user column preference.
pub fn save_user_columns(user_id: i64, table: &str, pref: &str, conn: &Connection) -> rusqlite::Result<()> {
    let key = format!("pref.{table}_table_columns");
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value",
        params![user_id, key, pref],
    )?;
    Ok(())
}

/// Save global column default (updates existing setting entity value property).
pub fn save_global_columns(table: &str, pref: &str, conn: &Connection) -> rusqlite::Result<()> {
    let name = format!("{table}_table_columns");
    // Upsert: update if exists, insert if not
    let updated = conn.execute(
        "UPDATE entity_properties SET value = ?1 \
         WHERE entity_id = (SELECT id FROM entities WHERE entity_type = 'setting' AND name = ?2) \
         AND key = 'value'",
        params![pref, name],
    )?;
    if updated == 0 {
        // Create setting entity + property
        conn.execute(
            "INSERT OR IGNORE INTO entities (entity_type, name, label) VALUES ('setting', ?1, ?2)",
            params![name, format!("{table} table columns")],
        )?;
        let setting_id: i64 = conn.query_row(
            "SELECT id FROM entities WHERE entity_type = 'setting' AND name = ?1",
            params![name], |r| r.get(0),
        )?;
        conn.execute(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'value', ?2) \
             ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value",
            params![setting_id, pref],
        )?;
    }
    Ok(())
}
```

**Step 2: Run compile check** (no unit tests here — integration tests come with the handler):
```bash
cargo check
```
Expected: no errors.

**Step 3: Commit**
```bash
git add src/models/table_filter/columns.rs
git commit -m "feat: add column resolver with per-user and global EAV preferences"
```

---

## Task 4: Register `table_filter` Module

**Files:**
- Modify: `src/models/mod.rs`

**Step 1: Add the module declaration**

Add `pub mod table_filter;` to `src/models/mod.rs` (after the existing entries, alphabetical order):

```rust
pub mod table_filter;
```

**Step 2: Verify compilation**
```bash
cargo check
```
Expected: no errors.

**Step 3: Commit**
```bash
git add src/models/mod.rs
git commit -m "chore: register table_filter module"
```

---

## Task 5: Users Filter Definitions

**Files:**
- Create: `src/models/user/filter.rs`
- Modify: `src/models/user/mod.rs`

**Step 1: Create filter.rs with field map and operator whitelist**

```rust
// src/models/user/filter.rs
use std::collections::HashMap;

/// SQL column expressions for each filter field.
/// Values are hardcoded SQL expressions — never user input.
pub fn field_map() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("username",     "e.name");
    m.insert("display_name", "e.label");
    m.insert("email",        "COALESCE(p_email.value, '')");
    m.insert("role",         "COALESCE(role_e.name, '')");
    m.insert("created_at",   "e.created_at");
    m.insert("updated_at",   "e.updated_at");
    m
}

/// Allowed operators for the users table.
pub const OPS: &[&str] = &[
    "contains", "not_contains", "equals", "not_equals",
    "starts_with", "is", "is_not", "before", "after", "on",
];

/// Allowed sort column keys and their SQL expressions.
pub fn sort_col(key: &str) -> &'static str {
    match key {
        "username"     => "e.name",
        "display_name" => "e.label",
        "email"        => "COALESCE(p_email.value, '')",
        "role"         => "COALESCE(role_e.name, '')",
        "created_at"   => "e.created_at",
        "updated_at"   => "e.updated_at",
        _              => "e.id",
    }
}

/// Default column definitions for the users table (order = default display order).
pub fn default_columns() -> Vec<crate::models::table_filter::ColumnDef> {
    use crate::models::table_filter::ColumnDef;
    vec![
        ColumnDef { key: "user".into(),       label: "User".into(),    visible: true,  always_visible: true,  sortable: true,  sort_key: "username".into() },
        ColumnDef { key: "email".into(),      label: "Email".into(),   visible: true,  always_visible: false, sortable: true,  sort_key: "email".into() },
        ColumnDef { key: "status".into(),     label: "Status".into(),  visible: true,  always_visible: false, sortable: false, sort_key: "".into() },
        ColumnDef { key: "created_at".into(), label: "Created".into(), visible: false, always_visible: false, sortable: true,  sort_key: "created_at".into() },
        ColumnDef { key: "updated_at".into(), label: "Updated".into(), visible: false, always_visible: false, sortable: true,  sort_key: "updated_at".into() },
        ColumnDef { key: "actions".into(),    label: "Actions".into(), visible: true,  always_visible: true,  sortable: false, sort_key: "".into() },
    ]
}

/// Field definitions for the filter builder JS (as JSON).
/// Includes label, type, and allowed operators.
/// role_options: Vec<(name, label)> fetched from DB.
pub fn fields_json(role_options: &[(String, String)]) -> String {
    let roles_json: String = role_options.iter()
        .map(|(name, label)| format!(r#"{{"value":"{name}","label":"{label}"}}"#))
        .collect::<Vec<_>>()
        .join(",");

    format!(r#"[
  {{"key":"username","label":"Username","type":"text","ops":["contains","not_contains","equals","not_equals","starts_with"]}},
  {{"key":"display_name","label":"Display Name","type":"text","ops":["contains","not_contains","equals","not_equals","starts_with"]}},
  {{"key":"email","label":"Email","type":"text","ops":["contains","not_contains","equals","not_equals"]}},
  {{"key":"role","label":"Role","type":"select","ops":["is","is_not"],"options":[{roles_json}]}},
  {{"key":"created_at","label":"Created","type":"date","ops":["before","after","on"]}},
  {{"key":"updated_at","label":"Updated","type":"date","ops":["before","after","on"]}}
]"#)
}
```

**Step 2: Register in `user/mod.rs`**

Add `pub mod filter;` to `src/models/user/mod.rs`.

**Step 3: Compile check**
```bash
cargo check
```

**Step 4: Commit**
```bash
git add src/models/user/filter.rs src/models/user/mod.rs
git commit -m "feat: add users filter field map, sort whitelist, and column defaults"
```

---

## Task 6: Extend `find_paginated` with Filter and Sort

**Files:**
- Modify: `src/models/user/queries.rs`
- Modify: `tests/user_test.rs`

**Step 1: Update the query function signature**

Replace the current `find_paginated` signature:
```rust
// OLD: pub fn find_paginated(conn, page, per_page, search: Option<&str>)
// NEW:
pub fn find_paginated(
    conn: &Connection,
    page: i64,
    per_page: i64,
    filter: &crate::models::table_filter::FilterTree,
    sort: &crate::models::table_filter::SortSpec,
) -> rusqlite::Result<UserPage> {
    use crate::models::table_filter::{builder, SortDir};
    use crate::models::user::filter as uf;

    let page = page.max(1);
    let per_page = per_page.clamp(1, 100);
    let offset = (page - 1) * per_page;

    // Build WHERE clause
    let (where_clause, filter_params) = builder::build_where_clause(
        filter, &uf::field_map(), uf::OPS, 0,
    ).unwrap_or_else(|_| ("1=1".to_string(), vec![]));

    // Build ORDER BY
    let sort_col = uf::sort_col(&sort.column);
    let sort_dir = match sort.dir { SortDir::Asc => "ASC", SortDir::Desc => "DESC" };

    // Count
    let count_sql = format!(
        "SELECT COUNT(*) FROM entities e \
         LEFT JOIN entity_properties p_email ON e.id = p_email.entity_id AND p_email.key = 'email' \
         LEFT JOIN relations r_role ON r_role.source_id = e.id AND r_role.relation_type_id = \
             (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
         LEFT JOIN entities role_e ON r_role.target_id = role_e.id \
         WHERE e.entity_type = 'user' AND ({where_clause})"
    );

    let total_count: i64 = {
        let mut stmt = conn.prepare(&count_sql)?;
        stmt.query_row(rusqlite::params_from_iter(filter_params.iter()), |r| r.get(0))?
    };

    // Data
    let n = filter_params.len();
    let data_sql = format!(
        "{SELECT_USER_DISPLAY} AND ({where_clause}) \
         ORDER BY {sort_col} {sort_dir} \
         LIMIT ?{} OFFSET ?{}",
        n + 1, n + 2
    );

    let mut all_params: Vec<rusqlite::types::Value> = filter_params.iter()
        .map(|s| rusqlite::types::Value::Text(s.clone()))
        .collect();
    all_params.push(rusqlite::types::Value::Integer(per_page));
    all_params.push(rusqlite::types::Value::Integer(offset));

    let mut stmt = conn.prepare(&data_sql)?;
    let users = stmt.query_map(rusqlite::params_from_iter(all_params.iter()), row_to_user_display)?
        .collect::<Result<Vec<_>, _>>()?;

    let total_pages = ((total_count as f64) / (per_page as f64)).ceil() as i64;

    Ok(UserPage { users, page, per_page, total_count, total_pages })
}
```

**Step 2: Update existing tests in `tests/user_test.rs`**

The two calls to `find_paginated` need to be updated. Find them (lines ~171 and ~195) and change:

```rust
// OLD:
let page = find_paginated(&conn, 1, 10, None).expect(...);
let results = find_paginated(&conn, 1, 10, Some("search")).expect(...);

// NEW:
use ahlt::models::table_filter::{FilterTree, Condition, SortSpec};

let page = find_paginated(&conn, 1, 10, &FilterTree::default(), &SortSpec::default())
    .expect("Failed to list users");

// For the search test, use a filter condition:
let filter = FilterTree {
    conditions: vec![Condition {
        field: "username".into(),
        op: "contains".into(),
        value: "search".into(),
    }],
    ..Default::default()
};
let results = find_paginated(&conn, 1, 10, &filter, &SortSpec::default())
    .expect("Failed to search users");
```

**Step 3: Write new sort and filter integration tests in `tests/user_test.rs`**

```rust
#[test]
fn test_find_paginated_sort_by_username_asc() {
    use ahlt::models::user::{create, find_paginated, types::NewUser};
    use ahlt::models::table_filter::{FilterTree, SortSpec, SortDir};
    use crate::common::setup_test_db;

    let (_dir, conn) = setup_test_db();
    // Create two users with known usernames
    for (name, label) in [("beta_user", "Beta"), ("alpha_user", "Alpha")] {
        let _ = create(&conn, &NewUser {
            username: name.into(), password: "hash".into(),
            email: format!("{name}@test.com"), display_name: label.into(), role_id: 0,
        });
    }
    let sort = SortSpec { column: "username".into(), dir: SortDir::Asc };
    let page = find_paginated(&conn, 1, 10, &FilterTree::default(), &sort).unwrap();
    let names: Vec<&str> = page.users.iter().map(|u| u.username.as_str()).collect();
    let idx_alpha = names.iter().position(|&n| n == "alpha_user").unwrap();
    let idx_beta = names.iter().position(|&n| n == "beta_user").unwrap();
    assert!(idx_alpha < idx_beta, "alpha should come before beta when sorted ASC");
}

#[test]
fn test_find_paginated_filter_by_role() {
    use ahlt::models::user::{create, find_paginated, types::NewUser};
    use ahlt::models::table_filter::{FilterTree, Condition, SortSpec};
    use ahlt::models::role;
    use crate::common::{setup_test_db, seed_base_entities};

    let (_dir, conn) = setup_test_db();
    seed_base_entities(&conn);

    // Find admin role id
    let roles = role::queries::find_all_display(&conn).unwrap();
    let admin_role = roles.iter().find(|r| r.name == "admin").unwrap();

    let _ = create(&conn, &NewUser {
        username: "role_filtered_user".into(),
        password: "hash".into(),
        email: "rf@test.com".into(),
        display_name: "RF".into(),
        role_id: admin_role.id,
    });

    let filter = FilterTree {
        conditions: vec![Condition {
            field: "role".into(), op: "is".into(), value: "admin".into(),
        }],
        ..Default::default()
    };
    let page = find_paginated(&conn, 1, 10, &filter, &SortSpec::default()).unwrap();
    assert!(page.users.iter().any(|u| u.username == "role_filtered_user"));
    assert!(page.users.iter().all(|u| u.role_name == "admin"));
}
```

**Step 4: Run all tests**
```bash
cargo test
```
Expected: all tests pass (141+ passing).

**Step 5: Commit**
```bash
git add src/models/user/queries.rs tests/user_test.rs
git commit -m "feat: extend find_paginated with FilterTree and SortSpec support"
```

---

## Task 7: Add `find_all_filtered` for CSV Export

**Files:**
- Modify: `src/models/user/queries.rs`

**Step 1: Add the function below `find_paginated`**

```rust
/// Return all users matching the filter (no pagination) — used for CSV export.
pub fn find_all_filtered(
    conn: &Connection,
    filter: &crate::models::table_filter::FilterTree,
    sort: &crate::models::table_filter::SortSpec,
) -> rusqlite::Result<Vec<UserDisplay>> {
    use crate::models::table_filter::{builder, SortDir};
    use crate::models::user::filter as uf;

    let (where_clause, filter_params) = builder::build_where_clause(
        filter, &uf::field_map(), uf::OPS, 0,
    ).unwrap_or_else(|_| ("1=1".to_string(), vec![]));

    let sort_col = uf::sort_col(&sort.column);
    let sort_dir = match sort.dir { SortDir::Asc => "ASC", SortDir::Desc => "DESC" };

    let sql = format!(
        "{SELECT_USER_DISPLAY} AND ({where_clause}) ORDER BY {sort_col} {sort_dir}"
    );

    let mut stmt = conn.prepare(&sql)?;
    let users = stmt.query_map(
        rusqlite::params_from_iter(filter_params.iter()),
        row_to_user_display,
    )?.collect::<Result<Vec<_>, _>>()?;

    Ok(users)
}
```

**Step 2: Compile check**
```bash
cargo check
```

**Step 3: Commit**
```bash
git add src/models/user/queries.rs
git commit -m "feat: add find_all_filtered for CSV export (no pagination)"
```

---

## Task 8: Update Template Structs and List Handler Query Parsing

**Files:**
- Modify: `src/templates_structs.rs`
- Modify: `src/handlers/user_handlers/list.rs`

**Step 1: Update `UserListTemplate` in `templates_structs.rs`**

Find `UserListTemplate` and replace it:

```rust
pub struct UserListTemplate {
    pub ctx: PageContext,
    pub user_page: crate::models::user::types::UserPage,
    pub filter_json: String,               // active filter as JSON (for form hidden input)
    pub filter_active: bool,               // true if any conditions are set (affects UI)
    pub sort_column: String,               // e.g. "username"
    pub sort_dir: String,                  // "asc" or "desc"
    pub columns: Vec<crate::models::table_filter::ColumnDef>,
    pub available_roles: Vec<(String, String)>,  // (name, label) for filter builder dropdown
    pub fields_json: String,               // field definitions JSON for JS filter builder
}
```

**Step 2: Update `PaginationQuery` and list handler in `list.rs`**

```rust
use serde::Deserialize;
use actix_session::Session;
use actix_web::{web, HttpResponse};
use crate::db::DbPool;
use crate::models::{user, role};
use crate::models::table_filter::{FilterTree, SortSpec, columns as col_resolver};
use crate::models::user::filter as uf;
use crate::auth::session::{require_permission, get_user_id};
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, UserListTemplate};

#[derive(Deserialize)]
pub struct ListQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    filter: Option<String>,
    sort: Option<String>,
    dir: Option<String>,
}

pub async fn list(
    pool: web::Data<DbPool>,
    session: Session,
    query: web::Query<ListQuery>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.list")?;

    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/users")?;
    let user_id = get_user_id(&session).unwrap_or(0);

    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(25);

    // Parse filter
    let filter = query.filter.as_deref()
        .and_then(|s| FilterTree::from_json(s).ok())
        .unwrap_or_default();
    let filter_active = !filter.is_empty();
    let filter_json = filter.to_json();

    // Parse sort
    let sort = SortSpec::from_params(query.sort.as_deref(), query.dir.as_deref());

    // Resolve columns
    let all_cols = uf::default_columns();
    let columns = col_resolver::resolve_columns("users", user_id, &conn, &all_cols);

    // Fetch roles for filter builder dropdown
    let roles = role::queries::find_all_display(&conn)?;
    let available_roles: Vec<(String, String)> = roles.iter()
        .map(|r| (r.name.clone(), r.label.clone()))
        .collect();

    let fields_json = uf::fields_json(&available_roles);

    let user_page = user::find_paginated(&conn, page, per_page, &filter, &sort)?;

    let tmpl = UserListTemplate {
        ctx,
        user_page,
        filter_json,
        filter_active,
        sort_column: sort.column.clone(),
        sort_dir: sort.dir_str().to_string(),
        columns,
        available_roles,
        fields_json,
    };
    render(tmpl)
}
```

**Step 3: Compile check**
```bash
cargo check
```
Fix any type errors (the template struct change may require updating the Askama template — if it won't compile due to missing template fields, add placeholder values for now).

**Step 4: Commit**
```bash
git add src/templates_structs.rs src/handlers/user_handlers/list.rs
git commit -m "feat: update UserListTemplate and list handler for filter/sort/columns"
```

---

## Task 9: CSV Export Handler

**Files:**
- Modify: `src/handlers/user_handlers/crud.rs`

**Step 1: Add `export_csv` function at the bottom of crud.rs**

```rust
#[derive(serde::Deserialize)]
pub struct ExportQuery {
    filter: Option<String>,
    sort: Option<String>,
    dir: Option<String>,
}

pub async fn export_csv(
    pool: web::Data<DbPool>,
    session: Session,
    query: web::Query<ExportQuery>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.list")?;

    let conn = pool.get()?;

    let filter = query.filter.as_deref()
        .and_then(|s| crate::models::table_filter::FilterTree::from_json(s).ok())
        .unwrap_or_default();
    let sort = crate::models::table_filter::SortSpec::from_params(
        query.sort.as_deref(), query.dir.as_deref()
    );

    let users = crate::models::user::find_all_filtered(&conn, &filter, &sort)?;

    // Audit log
    let uid = crate::auth::session::get_user_id(&session).unwrap_or(0);
    let _ = crate::audit::log(&conn, uid, "users.export", "user", 0,
        serde_json::json!({ "count": users.len(), "format": "csv" }));

    // Get today's date from SQLite for filename
    let today: String = conn.query_row("SELECT DATE('now')", [], |r| r.get(0))
        .unwrap_or_else(|_| "unknown".to_string());

    fn escape_csv(s: &str) -> String {
        if s.contains(',') || s.contains('"') || s.contains('\n') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    }

    let mut csv = String::from("id,username,display_name,email,role,created_at,updated_at\n");
    for u in &users {
        csv.push_str(&format!("{},{},{},{},{},{},{}\n",
            u.id,
            escape_csv(&u.username),
            escape_csv(&u.display_name),
            escape_csv(&u.email),
            escape_csv(&u.role_label),
            u.created_at,
            u.updated_at,
        ));
    }

    Ok(HttpResponse::Ok()
        .content_type("text/csv; charset=utf-8")
        .insert_header(("Content-Disposition",
            format!("attachment; filename=\"users-{today}.csv\"")))
        .body(csv))
}
```

**Step 2: Compile check**
```bash
cargo check
```

**Step 3: Commit** (route registration in Task 11)
```bash
git add src/handlers/user_handlers/crud.rs
git commit -m "feat: add CSV export handler for users"
```

---

## Task 10: Column Save Handler

**Files:**
- Modify: `src/handlers/user_handlers/crud.rs`

**Step 1: Add `save_columns` function**

```rust
#[derive(serde::Deserialize)]
pub struct SaveColumnsForm {
    pub columns: String,              // "user,email,status,actions"
    pub set_global: Option<String>,   // "true" if admin wants to set global default
    pub csrf_token: String,
    pub redirect_to: Option<String>,  // URL to return to after save
}

pub async fn save_columns(
    pool: web::Data<DbPool>,
    session: Session,
    form: web::Form<SaveColumnsForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.list")?;
    crate::auth::csrf::validate_csrf(&session, &form.csrf_token)?;

    let conn = pool.get()?;
    let user_id = crate::auth::session::get_user_id(&session)?;

    // Validate: only known column keys allowed (no arbitrary string injection into EAV)
    const VALID_KEYS: &[&str] = &["user", "email", "status", "created_at", "updated_at", "actions"];
    let sanitized: String = form.columns.split(',')
        .map(str::trim)
        .filter(|k| VALID_KEYS.contains(k))
        .collect::<Vec<_>>()
        .join(",");

    // Always include always-visible columns
    let pref = if !sanitized.contains("user") {
        format!("user,{sanitized}")
    } else { sanitized.clone() };
    let pref = if !pref.contains("actions") {
        format!("{pref},actions")
    } else { pref };

    crate::models::table_filter::columns::save_user_columns(user_id, "users", &pref, &conn)?;

    // Optionally save global default
    if form.set_global.as_deref() == Some("true") {
        require_permission(&session, "settings.manage")?;
        crate::models::table_filter::columns::save_global_columns("users", &pref, &conn)?;
    }

    let redirect = form.redirect_to.as_deref().unwrap_or("/users");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", redirect.to_string()))
        .finish())
}
```

**Step 2: Compile check**
```bash
cargo check
```

**Step 3: Commit**
```bash
git add src/handlers/user_handlers/crud.rs
git commit -m "feat: add save_columns handler with per-user and global EAV persistence"
```

---

## Task 11: Register New Routes

**Files:**
- Modify: `src/main.rs`

**Step 1: Find the users route block in `main.rs` and add two routes**

Look for the block that registers `/users` routes. Add:

```rust
// In the users scope or alongside existing user routes:
.route("/users/export.csv", web::get().to(handlers::user_handlers::crud::export_csv))
.route("/users/columns", web::post().to(handlers::user_handlers::crud::save_columns))
```

**Important:** `GET /users/export.csv` must be registered **before** any `GET /users/{id}` route if that pattern exists, to prevent the path param swallowing "export.csv".

**Step 2: Verify**
```bash
cargo check
```

**Step 3: Run all tests**
```bash
cargo test
```
Expected: all tests pass.

**Step 4: Commit**
```bash
git add src/main.rs
git commit -m "feat: register /users/export.csv and /users/columns routes"
```

---

## Task 12: Create `table_filter.html` Partial

**Files:**
- Create: `templates/partials/table_filter.html`

This partial receives from the parent template: `filter_json` (current filter JSON), `fields_json` (field defs for JS), `csrf_token`.

```html
{# templates/partials/table_filter.html #}
<div class="filter-builder" id="filter-builder">
    <div class="filter-builder__header">
        <span class="filter-builder__title">Filters</span>
        <div class="filter-builder__root-logic">
            Match
            <select class="filter-logic-select" id="root-logic-select">
                <option value="and">ALL</option>
                <option value="or">ANY</option>
            </select>
            of the following
        </div>
        <div class="filter-builder__header-actions">
            <button type="button" class="btn btn-sm" id="add-root-condition">+ Condition</button>
            <button type="button" class="btn btn-sm" id="add-group">+ Group</button>
        </div>
    </div>

    <div class="filter-builder__conditions" id="root-conditions"></div>
    <div class="filter-builder__groups" id="root-groups"></div>

    <div class="filter-builder__footer">
        <a href="/users" class="btn btn-sm">Clear</a>
        <button type="button" class="btn btn-sm btn-primary" id="apply-filter">Apply ▶</button>
    </div>
</div>

{# Hidden form that GET-submits with serialized filter #}
<form method="get" action="/users" id="filter-form" style="display:none">
    <input type="hidden" name="filter" id="filter-json-input">
    <input type="hidden" name="sort" id="sort-hidden" value="{{ sort_column }}">
    <input type="hidden" name="dir" id="dir-hidden" value="{{ sort_dir }}">
    <input type="hidden" name="per_page" id="per-page-hidden" value="{{ user_page.per_page }}">
</form>

<script>
(function() {
    // Field definitions from server
    const FIELDS = JSON.parse(document.getElementById('filter-fields-json').textContent);
    // Current filter state from server
    let state;
    try { state = JSON.parse(document.getElementById('filter-state-json').textContent); }
    catch(e) { state = { logic: 'and', conditions: [], groups: [] }; }

    const OP_LABELS = {
        contains: 'contains', not_contains: 'does not contain', equals: 'equals',
        not_equals: 'not equals', starts_with: 'starts with',
        is: 'is', is_not: 'is not', before: 'before', after: 'after', on: 'on'
    };

    function el(tag, cls, text) {
        const e = document.createElement(tag);
        if (cls) e.className = cls;
        if (text !== undefined) e.textContent = text;
        return e;
    }

    function makeFieldSelect(selected) {
        const s = el('select', 'filter-field-select');
        FIELDS.forEach(f => {
            const o = el('option', '', f.label);
            o.value = f.key;
            if (f.key === selected) o.selected = true;
            s.appendChild(o);
        });
        return s;
    }

    function makeOpSelect(fieldKey, selectedOp) {
        const field = FIELDS.find(f => f.key === fieldKey) || FIELDS[0];
        const s = el('select', 'filter-op-select');
        field.ops.forEach(op => {
            const o = el('option', '', OP_LABELS[op] || op);
            o.value = op;
            if (op === selectedOp) o.selected = true;
            s.appendChild(o);
        });
        return s;
    }

    function makeValueInput(fieldKey, value) {
        const field = FIELDS.find(f => f.key === fieldKey) || FIELDS[0];
        if (field.type === 'select') {
            const s = el('select', 'filter-value-select');
            (field.options || []).forEach(opt => {
                const o = el('option', '', opt.label);
                o.value = opt.value;
                if (opt.value === value) o.selected = true;
                s.appendChild(o);
            });
            return s;
        } else if (field.type === 'date') {
            const inp = el('input', 'filter-value-input');
            inp.type = 'date';
            inp.value = value || '';
            return inp;
        } else {
            const inp = el('input', 'filter-value-input');
            inp.type = 'text';
            inp.placeholder = 'Value...';
            inp.value = value || '';
            return inp;
        }
    }

    function makeConditionRow(container, cond, addBtn) {
        const row = el('div', 'filter-condition-row');
        const fieldSel = makeFieldSelect(cond.field);
        const opSel = makeOpSelect(cond.field, cond.op);
        const valInp = makeValueInput(cond.field, cond.value);
        const removeBtn = el('button', 'filter-remove-btn', '✕');
        removeBtn.type = 'button';
        removeBtn.setAttribute('aria-label', 'Remove condition');

        // When field changes, rebuild op and value inputs
        fieldSel.addEventListener('change', () => {
            const newOp = makeOpSelect(fieldSel.value, '');
            const newVal = makeValueInput(fieldSel.value, '');
            row.replaceChild(newOp, opSel);
            row.replaceChild(newVal, valInp);
            // Update references — we need to rebuild using mutation, but
            // since we don't capture references cleanly, just re-render on apply
        });

        removeBtn.addEventListener('click', () => row.remove());

        row.appendChild(fieldSel);
        row.appendChild(opSel);
        row.appendChild(valInp);
        row.appendChild(removeBtn);
        if (addBtn) {
            container.insertBefore(row, addBtn);
        } else {
            container.appendChild(row);
        }
    }

    function makeGroup(groupState) {
        const wrapper = el('div', 'filter-group');

        const header = el('div', 'filter-group__header');
        const logicSel = el('select', 'filter-logic-select');
        [['and','ALL'],['or','ANY']].forEach(([v,l]) => {
            const o = el('option', '', l); o.value = v;
            if (v === (groupState.logic || 'and')) o.selected = true;
            logicSel.appendChild(o);
        });
        const logicLabel = el('span', 'filter-group__label', 'Match ');
        const logicSuffix = el('span', 'filter-group__label', ' of:');
        const addCondBtn = el('button', 'btn btn-sm', '+ Condition');
        addCondBtn.type = 'button';
        const removeGroupBtn = el('button', 'filter-remove-btn', '✕ Group');
        removeGroupBtn.type = 'button';

        header.appendChild(logicLabel);
        header.appendChild(logicSel);
        header.appendChild(logicSuffix);
        header.appendChild(addCondBtn);
        header.appendChild(removeGroupBtn);

        const condContainer = el('div', 'filter-group__conditions');
        (groupState.conditions || []).forEach(c => makeConditionRow(condContainer, c, null));

        addCondBtn.addEventListener('click', () => {
            makeConditionRow(condContainer, { field: FIELDS[0].key, op: FIELDS[0].ops[0], value: '' }, null);
        });
        removeGroupBtn.addEventListener('click', () => wrapper.remove());

        wrapper.appendChild(header);
        wrapper.appendChild(condContainer);
        return wrapper;
    }

    function readConditions(container) {
        return Array.from(container.querySelectorAll('.filter-condition-row')).map(row => {
            const selects = row.querySelectorAll('select');
            const input = row.querySelector('input, select:last-of-type');
            return {
                field: selects[0] ? selects[0].value : '',
                op: selects[1] ? selects[1].value : '',
                value: input ? input.value : '',
            };
        }).filter(c => c.field && c.value !== '');
    }

    function collectState() {
        const rootLogic = document.getElementById('root-logic-select').value;
        const rootConds = readConditions(document.getElementById('root-conditions'));
        const groups = Array.from(document.getElementById('root-groups').querySelectorAll('.filter-group')).map(g => {
            const logic = g.querySelector('.filter-logic-select').value;
            const conds = readConditions(g.querySelector('.filter-group__conditions'));
            return { logic, conditions: conds };
        }).filter(g => g.conditions.length > 0);

        return { logic: rootLogic, conditions: rootConds, groups };
    }

    // Render initial state
    const rootLogicSel = document.getElementById('root-logic-select');
    if (rootLogicSel) rootLogicSel.value = state.logic || 'and';
    const rootConds = document.getElementById('root-conditions');
    (state.conditions || []).forEach(c => makeConditionRow(rootConds, c, null));
    const rootGroups = document.getElementById('root-groups');
    (state.groups || []).forEach(g => rootGroups.appendChild(makeGroup(g)));

    // Add condition button
    document.getElementById('add-root-condition').addEventListener('click', () => {
        makeConditionRow(rootConds, { field: FIELDS[0].key, op: FIELDS[0].ops[0], value: '' }, null);
    });

    // Add group button
    document.getElementById('add-group').addEventListener('click', () => {
        rootGroups.appendChild(makeGroup({ logic: 'or', conditions: [] }));
        // Auto-add one empty condition to new group
        const newGroup = rootGroups.lastElementChild;
        const condContainer = newGroup.querySelector('.filter-group__conditions');
        makeConditionRow(condContainer, { field: FIELDS[0].key, op: FIELDS[0].ops[0], value: '' }, null);
    });

    // Apply button
    document.getElementById('apply-filter').addEventListener('click', () => {
        const tree = collectState();
        const isEmpty = tree.conditions.length === 0 && tree.groups.length === 0;
        const filterInput = document.getElementById('filter-json-input');
        filterInput.value = isEmpty ? '' : JSON.stringify(tree);
        document.getElementById('filter-form').submit();
    });
})();
</script>
```

Also add two `<script type="application/json">` elements in the parent template to pass data (done in Task 15). For now, stub the partial.

**Step 2: Compile check** (Askama compiles templates on build)
```bash
cargo check
```

**Step 3: Commit**
```bash
git add templates/partials/table_filter.html
git commit -m "feat: add table_filter.html partial — filter builder UI and JS"
```

---

## Task 13: Create `column_picker.html` Partial

**Files:**
- Create: `templates/partials/column_picker.html`

```html
{# templates/partials/column_picker.html #}
{# Receives: columns (Vec<ColumnDef>), ctx (PageContext), table ("users"), redirect_to (URL) #}

<div class="col-picker" id="col-picker" hidden>
    <div class="col-picker__title">Columns</div>
    <ul class="col-picker__list" id="col-picker-list">
    {% for col in columns %}
        <li class="col-picker__item" draggable="true" data-key="{{ col.key }}">
            <span class="col-picker__handle" aria-hidden="true">⠿</span>
            {% if col.always_visible %}
            <input type="checkbox" class="col-picker__check" checked disabled>
            {% else %}
            <input type="checkbox" class="col-picker__check" {% if col.visible %}checked{% endif %}>
            {% endif %}
            <span class="col-picker__label">{{ col.label }}</span>
            {% if col.always_visible %}
            <span class="col-picker__always-on">(always on)</span>
            {% endif %}
        </li>
    {% endfor %}
    </ul>
    <div class="col-picker__footer">
        {% if ctx.permissions.has("settings.manage") %}
        <button type="button" class="btn btn-sm" id="col-picker-set-global">Set as global default</button>
        {% endif %}
    </div>
</div>

{# Hidden form for saving columns #}
<form id="col-picker-form" method="post" action="/{{ table }}/columns" style="display:none">
    <input type="hidden" name="csrf_token" value="{{ ctx.csrf_token }}">
    <input type="hidden" name="columns" id="col-picker-columns-input">
    <input type="hidden" name="set_global" id="col-picker-set-global-input" value="false">
    <input type="hidden" name="redirect_to" id="col-picker-redirect-input" value="">
</form>

<script>
(function() {
    const picker = document.getElementById('col-picker');
    const btn = document.getElementById('col-picker-btn');
    const list = document.getElementById('col-picker-list');
    if (!picker || !btn || !list) return;

    // Toggle open/close
    btn.addEventListener('click', (e) => {
        e.stopPropagation();
        picker.hidden = !picker.hidden;
    });
    document.addEventListener('click', (e) => {
        if (!picker.contains(e.target) && !btn.contains(e.target)) {
            picker.hidden = true;
        }
    });

    function getColumnOrder() {
        return Array.from(list.querySelectorAll('.col-picker__item')).map(item => ({
            key: item.dataset.key,
            visible: item.querySelector('.col-picker__check').checked,
        }));
    }

    function saveColumns(setGlobal) {
        const cols = getColumnOrder();
        const visibleKeys = cols.filter(c => c.visible).map(c => c.key).join(',');
        document.getElementById('col-picker-columns-input').value = visibleKeys;
        document.getElementById('col-picker-set-global-input').value = setGlobal ? 'true' : 'false';
        document.getElementById('col-picker-redirect-input').value = window.location.href;
        document.getElementById('col-picker-form').submit();
    }

    // Save on checkbox change
    list.addEventListener('change', (e) => {
        if (e.target.classList.contains('col-picker__check')) {
            saveColumns(false);
        }
    });

    // Global default button
    const globalBtn = document.getElementById('col-picker-set-global');
    if (globalBtn) {
        globalBtn.addEventListener('click', () => saveColumns(true));
    }

    // Drag-and-drop reordering
    let dragSrc = null;
    list.addEventListener('dragstart', (e) => {
        dragSrc = e.target.closest('.col-picker__item');
        if (dragSrc) e.dataTransfer.effectAllowed = 'move';
    });
    list.addEventListener('dragover', (e) => {
        e.preventDefault();
        const target = e.target.closest('.col-picker__item');
        if (target && target !== dragSrc) {
            const rect = target.getBoundingClientRect();
            const after = e.clientY > rect.top + rect.height / 2;
            list.insertBefore(dragSrc, after ? target.nextSibling : target);
        }
    });
    list.addEventListener('dragend', () => {
        dragSrc = null;
        saveColumns(false);
    });
})();
</script>
```

**Step 2: Compile check**
```bash
cargo check
```

**Step 3: Commit**
```bash
git add templates/partials/column_picker.html
git commit -m "feat: add column_picker.html partial with drag-and-drop reordering"
```

---

## Task 14: Create `table_controls.html` Partial

**Files:**
- Create: `templates/partials/table_controls.html`

```html
{# templates/partials/table_controls.html #}
{# Receives: user_page, sort_column, sort_dir, filter_json, table ("users") #}

<div class="table-controls">
    <div class="table-controls__left">
        <div class="table-controls__per-page">
            <label for="per-page-select" class="sr-only">Rows per page</label>
            <select id="per-page-select" class="table-controls__select" onchange="changePerPage(this.value)">
                {% for n in [10_i64, 25_i64, 50_i64, 100_i64] %}
                <option value="{{ n }}" {% if n == user_page.per_page %}selected{% endif %}>{{ n }} rows</option>
                {% endfor %}
            </select>
        </div>
    </div>
    <div class="table-controls__right">
        <div class="table-controls__summary">
            Showing <strong>{{ user_page.users.len() }}</strong> of <strong>{{ user_page.total_count }}</strong>
        </div>
        <button type="button" class="btn btn-sm" id="col-picker-btn">⊞ Columns</button>
        <a href="/{{ table }}/export.csv?filter={{ filter_json }}&sort={{ sort_column }}&dir={{ sort_dir }}"
           class="btn btn-sm" target="_blank">↓ Export CSV</a>
    </div>
</div>

<script>
function changePerPage(n) {
    const url = new URL(window.location.href);
    url.searchParams.set('per_page', n);
    url.searchParams.set('page', '1');
    window.location.href = url.toString();
}
</script>
```

**Note:** The `{% for n in [...] %}` syntax may need adjustment for Askama — if array literals aren't supported in loops, hardcode the four `<option>` elements instead.

**Step 2: Compile check**
```bash
cargo check
```
If Askama doesn't support inline array literals in `{% for %}`, replace with four explicit `<option>` elements.

**Step 3: Commit**
```bash
git add templates/partials/table_controls.html
git commit -m "feat: add table_controls.html partial with per-page, export, and column picker button"
```

---

## Task 15: Rewrite `templates/users/list.html`

**Files:**
- Modify: `templates/users/list.html`

This is the largest template change. Key points:
- Remove old search form
- Add JSON data elements for JS (filter state, field definitions)
- Include the three partials
- Column headers become sort links
- `tbody` iterates `Vec<ColumnDef>` for column ordering

```html
{% extends "base.html" %}

{% block title %}Users — {{ ctx.app_name }}{% endblock %}
{% block nav %}{% include "partials/nav.html" %}{% endblock %}
{% block sidebar %}{% include "partials/sidebar.html" %}{% endblock %}

{% block content %}
{% if let Some(msg) = ctx.flash %}
<div class="alert alert-success">{{ msg }}</div>
{% endif %}

<div class="page-header">
    <h1>Users</h1>
    {% if ctx.permissions.has("users.create") %}
    <a href="/users/new" class="btn btn-primary">New User</a>
    {% endif %}
</div>

{# JSON data for JS (no innerHTML risk — these are read by JS, not rendered as HTML) #}
<script type="application/json" id="filter-state-json">{{ filter_json }}</script>
<script type="application/json" id="filter-fields-json">{{ fields_json }}</script>

{# Filter builder partial — pass sort_column, sort_dir, user_page for the hidden form #}
{% include "partials/table_filter.html" %}

{# Controls bar + column picker #}
{% include "partials/table_controls.html" %}
{% include "partials/column_picker.html" %}

{# Bulk Action Toolbar #}
<div id="users-bulk-toolbar" class="users-bulk-toolbar" hidden>
    <span class="toolbar-label">
        <span id="users-selected-count">0</span> user<span id="users-plural">s</span> selected
    </span>
    <div class="toolbar-actions">
        {% if ctx.permissions.has("users.delete") %}
        <button type="button" class="btn btn-danger btn-sm" onclick="confirmBulkDelete()">
            Delete Selected
        </button>
        {% endif %}
        <button type="button" class="btn btn-sm" onclick="clearSelection()">Cancel</button>
    </div>
</div>

{% if user_page.users.is_empty() %}
<div class="empty-state">
    <div class="empty-state-icon">👥</div>
    <div class="empty-state-title">No users found</div>
    <div class="empty-state-text">
        {% if filter_active %}
        No users match the active filters. <a href="/users">Clear filters</a>.
        {% else %}
        Create your first user to get started.
        {% endif %}
    </div>
</div>
{% else %}
<div class="table-wrapper">
    <table class="table users-table">
        <thead>
            <tr>
                <th class="users-table__checkbox">
                    <label class="checkbox-label">
                        <input type="checkbox" id="users-select-all" class="checkbox-input" onchange="toggleSelectAll()">
                        <span class="checkbox-mark"></span>
                    </label>
                </th>
                {% for col in columns %}
                {% if col.visible %}
                <th class="users-table__{{ col.key }}">
                    {% if col.sortable %}
                    <a href="/users?sort={{ col.sort_key }}&dir={% if sort_column == col.sort_key %}{{ sort_dir == "asc" | then("desc") | or("asc") }}{% else %}asc{% endif %}&filter={{ filter_json }}&per_page={{ user_page.per_page }}" class="sort-link">
                        {{ col.label }}
                        {% if sort_column == col.sort_key %}
                        {% if sort_dir.as_str() == "asc" %}▲{% else %}▼{% endif %}
                        {% endif %}
                    </a>
                    {% else %}
                    {{ col.label }}
                    {% endif %}
                </th>
                {% endif %}
                {% endfor %}
            </tr>
        </thead>
        <tbody>
        {% for user in user_page.users %}
            <tr class="users-table__row" data-user-id="{{ user.id }}">
                <td class="users-table__checkbox">
                    <label class="checkbox-label">
                        <input type="checkbox" class="checkbox-input users-row-checkbox" value="{{ user.id }}" onchange="updateBulkToolbar()">
                        <span class="checkbox-mark"></span>
                    </label>
                </td>
                {% for col in columns %}
                {% if col.visible %}
                <td class="users-table__{{ col.key }}">
                    {% if col.key.as_str() == "user" %}
                    <div class="user-cell">
                        <div class="user-name">👤 {{ user.display_name }}</div>
                        <div class="user-username">@{{ user.username }}</div>
                        <div class="user-role"><span class="badge badge-{{ user.role_name }}">{{ user.role_label }}</span></div>
                    </div>
                    {% else %}
                    {% if col.key.as_str() == "email" %}{{ user.email }}
                    {% else %}
                    {% if col.key.as_str() == "status" %}<span class="status-badge status-active" title="Active">✓ Active</span>
                    {% else %}
                    {% if col.key.as_str() == "created_at" %}{{ user.created_at }}
                    {% else %}
                    {% if col.key.as_str() == "updated_at" %}{{ user.updated_at }}
                    {% else %}
                    {% if col.key.as_str() == "actions" %}
                    <div class="action-buttons">
                        {% if ctx.permissions.has("users.edit") %}<a href="/users/{{ user.id }}/edit" class="btn btn-sm">Edit</a>{% endif %}
                        {% if ctx.permissions.has("users.delete") %}<button type="button" class="btn btn-sm btn-danger" onclick="deleteUser({{ user.id }})">Delete</button>{% endif %}
                    </div>
                    {% endif %}
                    {% endif %}
                    {% endif %}
                    {% endif %}
                    {% endif %}
                    {% endif %}
                </td>
                {% endif %}
                {% endfor %}
            </tr>
        {% endfor %}
        </tbody>
    </table>
</div>
{% endif %}

{% if user_page.total_pages > 1 %}
<div class="pagination">
    <div class="pagination-info">Page {{ user_page.page }} of {{ user_page.total_pages }}</div>
    <div class="pagination-controls">
        {% if user_page.page > 1 %}
        <a href="/users?page={{ user_page.page - 1 }}&per_page={{ user_page.per_page }}&sort={{ sort_column }}&dir={{ sort_dir }}&filter={{ filter_json }}" class="btn btn-sm">← Previous</a>
        {% else %}
        <span class="btn btn-sm" aria-disabled="true">← Previous</span>
        {% endif %}
        <span class="pagination-current">Page {{ user_page.page }} of {{ user_page.total_pages }}</span>
        {% if user_page.page < user_page.total_pages %}
        <a href="/users?page={{ user_page.page + 1 }}&per_page={{ user_page.per_page }}&sort={{ sort_column }}&dir={{ sort_dir }}&filter={{ filter_json }}" class="btn btn-sm">Next →</a>
        {% else %}
        <span class="btn btn-sm" aria-disabled="true">Next →</span>
        {% endif %}
    </div>
</div>
{% endif %}

<form id="users-bulk-delete-form" method="post" action="/users/bulk-delete" style="display:none">
    <input type="hidden" name="csrf_token" value="{{ ctx.csrf_token }}">
    <input type="hidden" id="users-bulk-delete-ids" name="user_ids" value="">
</form>

<script>
// (Keep existing bulk select JS unchanged)
function updateBulkToolbar() {
    const checkboxes = document.querySelectorAll('.users-row-checkbox');
    const selectedCount = Array.from(checkboxes).filter(cb => cb.checked).length;
    const toolbar = document.getElementById('users-bulk-toolbar');
    const countSpan = document.getElementById('users-selected-count');
    const pluralSpan = document.getElementById('users-plural');
    countSpan.textContent = selectedCount;
    pluralSpan.textContent = selectedCount === 1 ? '' : 's';
    toolbar.hidden = selectedCount === 0;
    const selectAll = document.getElementById('users-select-all');
    const total = checkboxes.length;
    selectAll.checked = selectedCount === total && total > 0;
    selectAll.indeterminate = selectedCount > 0 && selectedCount < total;
}

function toggleSelectAll() {
    const checked = document.getElementById('users-select-all').checked;
    document.querySelectorAll('.users-row-checkbox').forEach(cb => cb.checked = checked);
    updateBulkToolbar();
}

function clearSelection() {
    document.querySelectorAll('.users-row-checkbox, #users-select-all').forEach(cb => cb.checked = false);
    updateBulkToolbar();
}

function deleteUser(userId) {
    const row = document.querySelector(`[data-user-id="${userId}"]`);
    const nameEl = row ? row.querySelector('.user-name') : null;
    const displayName = nameEl ? nameEl.textContent.replace('👤 ', '') : userId;
    if (confirm(`Delete user "${displayName}"?\n\nThis action cannot be undone.`)) {
        const form = document.createElement('form');
        form.method = 'POST';
        form.action = `/users/${userId}/delete`;
        const csrf = document.createElement('input');
        csrf.type = 'hidden';
        csrf.name = 'csrf_token';
        csrf.value = '{{ ctx.csrf_token }}';
        form.appendChild(csrf);
        document.body.appendChild(form);
        form.submit();
    }
}

function confirmBulkDelete() {
    const checkboxes = document.querySelectorAll('.users-row-checkbox:checked');
    const count = checkboxes.length;
    if (count === 0) { alert('No users selected'); return; }
    if (confirm(`Delete ${count} user${count !== 1 ? 's' : ''}?\n\nThis action cannot be undone.`)) {
        const ids = Array.from(checkboxes).map(cb => cb.value);
        document.getElementById('users-bulk-delete-ids').value = JSON.stringify(ids);
        document.getElementById('users-bulk-delete-form').submit();
    }
}

updateBulkToolbar();
</script>
{% endblock %}
```

**Important Askama notes:**
- The nested `{% if %}` chain for column keys is verbose but required (no `&&` in conditions)
- The sort link toggle (`{% if sort_dir == "asc" %}desc{% else %}asc{% endif %}`) is the correct Askama pattern
- Use `.as_str()` for string comparisons: `col.key.as_str() == "email"`
- Run `cargo clean` if you get "template not found" errors after editing

**Step 2: Build and test**
```bash
cargo build
```
Fix any Askama compile errors (they appear as Rust errors).

**Step 3: Run all tests**
```bash
cargo test
```
Expected: all tests pass.

**Step 4: Commit**
```bash
git add templates/users/list.html
git commit -m "feat: rewrite users list template with filter builder, sort headers, and column picker"
```

---

## Task 16: Add CSS

**Files:**
- Modify: `static/css/style.css`

Add to the bottom of the Users List section (after the existing `.users-table` styles):

```css
/* ========================================================
   Filter Builder
   ======================================================== */

.filter-builder {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.75rem 1rem;
    margin-bottom: 0.75rem;
}

.filter-builder__header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
    flex-wrap: wrap;
}

.filter-builder__title {
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
    margin-right: 0.25rem;
}

.filter-builder__header-actions {
    display: flex;
    gap: 0.375rem;
    margin-left: auto;
}

.filter-builder__footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 0.5rem;
    padding-top: 0.5rem;
    border-top: 1px solid var(--border);
}

.filter-condition-row {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    margin-bottom: 0.375rem;
    flex-wrap: wrap;
}

.filter-field-select,
.filter-op-select {
    font-size: 0.8125rem;
    padding: 0.25rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg);
    color: var(--text);
}

.filter-field-select { min-width: 9rem; }
.filter-op-select { min-width: 9rem; }

.filter-value-input,
.filter-value-select {
    font-size: 0.8125rem;
    padding: 0.25rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg);
    color: var(--text);
    min-width: 10rem;
    flex: 1;
}

.filter-remove-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0.25rem 0.375rem;
    border-radius: var(--radius-sm);
    font-size: 0.875rem;
    line-height: 1;
}
.filter-remove-btn:hover { background: var(--danger-bg); color: var(--danger); }

.filter-logic-select {
    font-size: 0.8125rem;
    padding: 0.2rem 0.4rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg);
    color: var(--text);
}

.filter-group {
    border-left: 3px solid var(--accent);
    padding: 0.5rem 0.75rem;
    margin: 0.375rem 0;
    background: var(--surface-hover);
    border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
}

.filter-group__header {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    margin-bottom: 0.375rem;
}

.filter-group__label {
    font-size: 0.8125rem;
    color: var(--text-muted);
}

.filter-group__conditions { padding-left: 0; }

/* ========================================================
   Table Controls Bar
   ======================================================== */

.table-controls {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.5rem;
    gap: 0.75rem;
    flex-wrap: wrap;
}

.table-controls__left,
.table-controls__right {
    display: flex;
    align-items: center;
    gap: 0.5rem;
}

.table-controls__select {
    font-size: 0.8125rem;
    padding: 0.25rem 0.5rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg);
    color: var(--text);
}

.table-controls__summary {
    font-size: 0.8125rem;
    color: var(--text-muted);
}

/* ========================================================
   Column Picker Popover
   ======================================================== */

.col-picker-wrapper {
    position: relative;
    display: inline-block;
}

.col-picker {
    position: absolute;
    top: calc(100% + 0.25rem);
    right: 0;
    z-index: 200;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: 0 4px 12px rgba(0,0,0,0.12);
    min-width: 14rem;
    padding: 0.5rem 0;
}

.col-picker__title {
    font-size: 0.6875rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
    padding: 0.25rem 0.75rem 0.5rem;
}

.col-picker__list {
    list-style: none;
    margin: 0;
    padding: 0;
}

.col-picker__item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.3125rem 0.75rem;
    cursor: grab;
    user-select: none;
}
.col-picker__item:hover { background: var(--surface-hover); }
.col-picker__item.dragging { opacity: 0.5; }

.col-picker__handle {
    color: var(--text-muted);
    font-size: 0.875rem;
    cursor: grab;
}

.col-picker__check {
    cursor: pointer;
    flex-shrink: 0;
}
.col-picker__check:disabled { opacity: 0.5; cursor: not-allowed; }

.col-picker__label { font-size: 0.875rem; flex: 1; }

.col-picker__always-on {
    font-size: 0.6875rem;
    color: var(--text-muted);
    font-style: italic;
}

.col-picker__footer {
    border-top: 1px solid var(--border);
    padding: 0.5rem 0.75rem 0.25rem;
}

/* Sort link in column headers */
.sort-link {
    color: inherit;
    text-decoration: none;
    display: flex;
    align-items: center;
    gap: 0.25rem;
    white-space: nowrap;
}
.sort-link:hover { color: var(--accent); }
```

**Step 2: Build and verify**
```bash
cargo build
```

**Step 3: Commit**
```bash
git add static/css/style.css
git commit -m "feat: add CSS for filter builder, table controls bar, and column picker"
```

---

## Task 17: Manual Verification

**Step 1: Start the server**
```bash
APP_ENV=staging cargo run
```
Login at http://localhost:8080 with admin credentials.

**Step 2: Verify filter builder**
- Navigate to http://localhost:8080/users
- Add a condition: username contains "admin" → Apply → verify only admin users shown
- Add a group with OR: email contains "@" OR role is "admin" → Apply
- Clear filters → all users returned
- Check URL contains `filter=` param (bookmarkable)

**Step 3: Verify sorting**
- Click "User" column header → URL gets `sort=username&dir=asc`
- Click again → `dir=desc`, order reverses
- Click "Created" → sort changes column, resets to asc

**Step 4: Verify column picker**
- Click ⊞ Columns → popover appears
- Uncheck "Email" → page reloads, email column hidden
- Reload page → email still hidden (persisted in DB)
- Drag "Actions" above "Status" → columns reorder
- If admin: click "Set as global default" → another user sees same layout

**Step 5: Verify per-page selector**
- Change to 10 rows → page reloads with 10 results
- Change to 100 → all users shown (if < 100)

**Step 6: Verify CSV export**
- Click ↓ Export CSV → file downloads
- Open CSV: verify all columns present, data correct
- Apply filter first → export respects filter

**Step 7: Run tests one final time**
```bash
cargo test
```
Expected: all tests pass.

**Step 8: Final commit**
```bash
git add -A
git commit -m "feat: complete table enhancement pattern — users table

- Nested AND/OR query builder with 2-level depth
- Server-side sorting via column header links
- Column picker with drag-and-drop reorder, EAV persistence (per-user + global)
- Per-page selector (10/25/50/100)
- CSV export honoring active filter and sort
- Shared table_filter infrastructure ready for roles and future tables

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Rollout to Roles Table (Reference)

Once users table is complete, adding the same features to roles requires:

1. `src/models/role/filter.rs` — field map for role fields (name, label, description, user_count)
2. Extend `role::find_all_list_items()` to accept `FilterTree` + `SortSpec`
3. Add `find_all_filtered()` to role queries for CSV
4. Update `RoleListTemplate` with same new fields as `UserListTemplate`
5. Update role list handler (`src/handlers/role_handlers/list.rs`) to parse new params
6. Add routes `/roles/export.csv` and `/roles/columns` in `main.rs`
7. Wire the three partials in `templates/roles/list.html`

No changes to shared infrastructure needed.
