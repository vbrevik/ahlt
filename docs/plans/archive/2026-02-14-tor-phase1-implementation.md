# ToR Phase 1: Foundation — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create and manage Terms of Reference with membership, functions, and authority properties.

**Architecture:** New `tor` and `tor_function` entity types in the EAV system, with `member_of`, `has_tor_role`, and `belongs_to_tor` relations. Follows existing role/user CRUD patterns: model module (types + queries), handlers (list + crud + members), Askama templates, route registration.

**Tech Stack:** Actix-web 4, Askama 0.14, rusqlite, r2d2_sqlite — same as existing codebase.

**Design doc:** `docs/plans/2026-02-14-tor-governance-design.md`

---

### Task 1: Extend seed data with new relation types, permissions, and nav items

**Files:**
- Modify: `src/db.rs` (seed_ontology function)

**Step 1: Add new relation types after existing ones**

In `seed_ontology()`, after the `_requires_perm_id` line, add:

```rust
// --- ToR relation types ---
let _member_of_id = insert_entity(&conn, "relation_type", "member_of", "Member Of", 0);
let _has_tor_role_id = insert_entity(&conn, "relation_type", "has_tor_role", "Has ToR Role", 0);
let _belongs_to_tor_id = insert_entity(&conn, "relation_type", "belongs_to_tor", "Belongs to ToR", 0);
```

**Step 2: Add ToR permissions after existing permissions array**

Add these to the `perms` array:

```rust
("tor.list", "List Terms of Reference", "Governance"),
("tor.create", "Create Terms of Reference", "Governance"),
("tor.edit", "Edit Terms of Reference", "Governance"),
("tor.manage_members", "Manage ToR Members", "Governance"),
```

**Step 3: Grant ToR permissions to admin role**

Already handled — admin gets ALL permissions via the loop.

**Step 4: Add Governance nav module and sidebar items**

After the existing nav items block:

```rust
// Governance: module header
let _nav_governance_id = insert_entity(&conn, "nav_item", "governance", "Governance", 3);
insert_prop(&conn, _nav_governance_id, "url", "/tor");

// Governance -> Terms of Reference: sidebar child
let nav_gov_tor_id = insert_entity(&conn, "nav_item", "governance.tor", "Terms of Reference", 1);
insert_prop(&conn, nav_gov_tor_id, "url", "/tor");
insert_prop(&conn, nav_gov_tor_id, "parent", "governance");
```

**Step 5: Add nav->permission relations for Governance items**

Look up the `tor.list` permission ID and create the relation:

```rust
let tor_list_perm_id: i64 = conn.query_row(
    "SELECT id FROM entities WHERE entity_type='permission' AND name='tor.list'",
    [], |row| row.get(0),
).unwrap();

insert_relation(&conn, requires_permission_rel_type_id, nav_gov_tor_id, tor_list_perm_id);
```

**Step 6: Verify**

```bash
rm data/app.db && cargo run
```

Check that http://localhost:8080 shows "Governance" in the nav bar for admin. Stop the server.

**Step 7: Commit**

```bash
git add src/db.rs
git commit -m "feat(tor): seed relation types, permissions, and nav items for governance"
```

---

### Task 2: Create ToR model types

**Files:**
- Create: `src/models/tor/types.rs`

**Step 1: Write type definitions**

```rust
/// For the ToR list page.
#[derive(Debug, Clone)]
pub struct TorListItem {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub description: String,
    pub status: String,
    pub meeting_cadence: String,
    pub member_count: i64,
    pub function_count: i64,
}

/// For ToR detail/edit.
#[derive(Debug, Clone)]
pub struct TorDetail {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub description: String,
    pub status: String,
    pub meeting_cadence: String,
    pub cadence_day: String,
    pub cadence_time: String,
    pub cadence_duration_minutes: String,
    pub default_location: String,
    pub remote_url: String,
    pub background_repo_url: String,
}

/// A member of a ToR with their function(s).
#[derive(Debug, Clone)]
pub struct TorMember {
    pub user_id: i64,
    pub user_name: String,
    pub user_label: String,
    pub functions: Vec<TorFunctionRef>,
}

/// A function assigned to a member (lightweight reference).
#[derive(Debug, Clone)]
pub struct TorFunctionRef {
    pub id: i64,
    pub name: String,
    pub label: String,
}

/// A tor_function entity with its authority properties.
#[derive(Debug, Clone)]
pub struct TorFunctionDetail {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub description: String,
    pub category: String,
    pub can_review_suggestions: bool,
    pub can_create_proposals: bool,
    pub can_approve_proposals: bool,
    pub can_manage_agenda: bool,
    pub can_record_decisions: bool,
    pub can_call_meetings: bool,
}

/// For the function list on the ToR detail page.
#[derive(Debug, Clone)]
pub struct TorFunctionListItem {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub category: String,
    pub assigned_to: Vec<String>,  // user labels
}
```

**Step 2: Verify**

```bash
cargo check
```

**Step 3: Commit**

```bash
git add src/models/tor/types.rs
git commit -m "feat(tor): add ToR model type definitions"
```

---

### Task 3: Create ToR model queries

**Files:**
- Create: `src/models/tor/queries.rs`

**Step 1: Write query functions**

```rust
use rusqlite::{Connection, params};
use super::types::*;

/// Find all ToRs for the list page.
pub fn find_all_list_items(conn: &Connection) -> rusqlite::Result<Vec<TorListItem>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label, \
                COALESCE(p_desc.value, '') AS description, \
                COALESCE(p_status.value, 'active') AS status, \
                COALESCE(p_cadence.value, 'ad-hoc') AS meeting_cadence, \
                (SELECT COUNT(*) FROM relations r_member \
                 WHERE r_member.target_id = e.id \
                   AND r_member.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'member_of') \
                ) AS member_count, \
                (SELECT COUNT(*) FROM relations r_func \
                 WHERE r_func.target_id = e.id \
                   AND r_func.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
                ) AS function_count \
         FROM entities e \
         LEFT JOIN entity_properties p_desc ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
         LEFT JOIN entity_properties p_status ON e.id = p_status.entity_id AND p_status.key = 'status' \
         LEFT JOIN entity_properties p_cadence ON e.id = p_cadence.entity_id AND p_cadence.key = 'meeting_cadence' \
         WHERE e.entity_type = 'tor' \
         ORDER BY e.sort_order, e.id"
    )?;
    let items = stmt.query_map([], |row| {
        Ok(TorListItem {
            id: row.get("id")?,
            name: row.get("name")?,
            label: row.get("label")?,
            description: row.get("description")?,
            status: row.get("status")?,
            meeting_cadence: row.get("meeting_cadence")?,
            member_count: row.get("member_count")?,
            function_count: row.get("function_count")?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

/// Find a ToR by id for editing.
pub fn find_detail_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<TorDetail>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label, \
                COALESCE(p_desc.value, '') AS description, \
                COALESCE(p_status.value, 'active') AS status, \
                COALESCE(p_cadence.value, 'ad-hoc') AS meeting_cadence, \
                COALESCE(p_day.value, '') AS cadence_day, \
                COALESCE(p_time.value, '') AS cadence_time, \
                COALESCE(p_dur.value, '60') AS cadence_duration_minutes, \
                COALESCE(p_loc.value, '') AS default_location, \
                COALESCE(p_remote.value, '') AS remote_url, \
                COALESCE(p_repo.value, '') AS background_repo_url \
         FROM entities e \
         LEFT JOIN entity_properties p_desc ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
         LEFT JOIN entity_properties p_status ON e.id = p_status.entity_id AND p_status.key = 'status' \
         LEFT JOIN entity_properties p_cadence ON e.id = p_cadence.entity_id AND p_cadence.key = 'meeting_cadence' \
         LEFT JOIN entity_properties p_day ON e.id = p_day.entity_id AND p_day.key = 'cadence_day' \
         LEFT JOIN entity_properties p_time ON e.id = p_time.entity_id AND p_time.key = 'cadence_time' \
         LEFT JOIN entity_properties p_dur ON e.id = p_dur.entity_id AND p_dur.key = 'cadence_duration_minutes' \
         LEFT JOIN entity_properties p_loc ON e.id = p_loc.entity_id AND p_loc.key = 'default_location' \
         LEFT JOIN entity_properties p_remote ON e.id = p_remote.entity_id AND p_remote.key = 'remote_url' \
         LEFT JOIN entity_properties p_repo ON e.id = p_repo.entity_id AND p_repo.key = 'background_repo_url' \
         WHERE e.id = ?1 AND e.entity_type = 'tor'"
    )?;
    let mut rows = stmt.query_map(params![id], |row| {
        Ok(TorDetail {
            id: row.get("id")?,
            name: row.get("name")?,
            label: row.get("label")?,
            description: row.get("description")?,
            status: row.get("status")?,
            meeting_cadence: row.get("meeting_cadence")?,
            cadence_day: row.get("cadence_day")?,
            cadence_time: row.get("cadence_time")?,
            cadence_duration_minutes: row.get("cadence_duration_minutes")?,
            default_location: row.get("default_location")?,
            remote_url: row.get("remote_url")?,
            background_repo_url: row.get("background_repo_url")?,
        })
    })?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Create a new ToR entity with properties.
pub fn create(
    conn: &Connection,
    name: &str,
    label: &str,
    description: &str,
    status: &str,
    meeting_cadence: &str,
    cadence_day: &str,
    cadence_time: &str,
    cadence_duration_minutes: &str,
    default_location: &str,
    remote_url: &str,
    background_repo_url: &str,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('tor', ?1, ?2)",
        params![name, label],
    )?;
    let tor_id = conn.last_insert_rowid();

    let props: Vec<(&str, &str)> = vec![
        ("description", description),
        ("status", status),
        ("meeting_cadence", meeting_cadence),
        ("cadence_day", cadence_day),
        ("cadence_time", cadence_time),
        ("cadence_duration_minutes", cadence_duration_minutes),
        ("default_location", default_location),
        ("remote_url", remote_url),
        ("background_repo_url", background_repo_url),
    ];
    for (key, value) in props {
        if !value.is_empty() {
            conn.execute(
                "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
                params![tor_id, key, value],
            )?;
        }
    }

    Ok(tor_id)
}

/// Update a ToR's base fields and properties.
pub fn update(
    conn: &Connection,
    id: i64,
    name: &str,
    label: &str,
    description: &str,
    status: &str,
    meeting_cadence: &str,
    cadence_day: &str,
    cadence_time: &str,
    cadence_duration_minutes: &str,
    default_location: &str,
    remote_url: &str,
    background_repo_url: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE entities SET name = ?1, label = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%S','now') WHERE id = ?3",
        params![name, label, id],
    )?;

    // Upsert all properties
    let props: Vec<(&str, &str)> = vec![
        ("description", description),
        ("status", status),
        ("meeting_cadence", meeting_cadence),
        ("cadence_day", cadence_day),
        ("cadence_time", cadence_time),
        ("cadence_duration_minutes", cadence_duration_minutes),
        ("default_location", default_location),
        ("remote_url", remote_url),
        ("background_repo_url", background_repo_url),
    ];
    for (key, value) in props {
        conn.execute(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3) \
             ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value",
            params![id, key, value],
        )?;
    }

    Ok(())
}

/// Delete a ToR entity (cascades to properties and relations).
pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM entities WHERE id = ?1 AND entity_type = 'tor'", params![id])?;
    Ok(())
}

/// Find all members of a ToR with their functions.
pub fn find_members(conn: &Connection, tor_id: i64) -> rusqlite::Result<Vec<TorMember>> {
    // Step 1: Get all users who are member_of this ToR
    let mut stmt = conn.prepare(
        "SELECT u.id, u.name, u.label \
         FROM relations r \
         JOIN entities u ON r.source_id = u.id \
         WHERE r.target_id = ?1 \
           AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'member_of') \
         ORDER BY u.label"
    )?;
    let users: Vec<(i64, String, String)> = stmt.query_map(params![tor_id], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })?.collect::<Result<Vec<_>, _>>()?;

    // Step 2: For each user, find their tor_functions that belong_to this ToR
    let mut func_stmt = conn.prepare(
        "SELECT f.id, f.name, f.label \
         FROM relations r_role \
         JOIN entities f ON r_role.target_id = f.id \
         JOIN relations r_tor ON f.id = r_tor.source_id \
         WHERE r_role.source_id = ?1 \
           AND r_role.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_tor_role') \
           AND r_tor.target_id = ?2 \
           AND r_tor.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
         ORDER BY f.label"
    )?;

    let mut members = Vec::new();
    for (user_id, user_name, user_label) in users {
        let functions: Vec<TorFunctionRef> = func_stmt.query_map(params![user_id, tor_id], |row| {
            Ok(TorFunctionRef {
                id: row.get(0)?,
                name: row.get(1)?,
                label: row.get(2)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        members.push(TorMember {
            user_id,
            user_name,
            user_label,
            functions,
        });
    }

    Ok(members)
}

/// Find all tor_functions that belong to a ToR.
pub fn find_functions(conn: &Connection, tor_id: i64) -> rusqlite::Result<Vec<TorFunctionListItem>> {
    let mut stmt = conn.prepare(
        "SELECT f.id, f.name, f.label, \
                COALESCE(p_cat.value, '') AS category \
         FROM relations r \
         JOIN entities f ON r.source_id = f.id \
         LEFT JOIN entity_properties p_cat ON f.id = p_cat.entity_id AND p_cat.key = 'category' \
         WHERE r.target_id = ?1 \
           AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
           AND f.entity_type = 'tor_function' \
         ORDER BY f.sort_order, f.id"
    )?;
    let functions: Vec<(i64, String, String, String)> = stmt.query_map(params![tor_id], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    })?.collect::<Result<Vec<_>, _>>()?;

    // For each function, find who holds it
    let mut user_stmt = conn.prepare(
        "SELECT u.label \
         FROM relations r \
         JOIN entities u ON r.source_id = u.id \
         WHERE r.target_id = ?1 \
           AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_tor_role') \
         ORDER BY u.label"
    )?;

    let mut result = Vec::new();
    for (id, name, label, category) in functions {
        let assigned_to: Vec<String> = user_stmt.query_map(params![id], |row| {
            row.get(0)
        })?.collect::<Result<Vec<_>, _>>()?;

        result.push(TorFunctionListItem {
            id,
            name,
            label,
            category,
            assigned_to,
        });
    }

    Ok(result)
}

/// Add a user as member of a ToR.
pub fn add_member(conn: &Connection, user_id: i64, tor_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'member_of'), ?1, ?2)",
        params![user_id, tor_id],
    )?;
    Ok(())
}

/// Remove a user from a ToR (also removes their function assignments for this ToR).
pub fn remove_member(conn: &Connection, user_id: i64, tor_id: i64) -> rusqlite::Result<()> {
    // Remove member_of relation
    conn.execute(
        "DELETE FROM relations WHERE source_id = ?1 AND target_id = ?2 \
         AND relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'member_of')",
        params![user_id, tor_id],
    )?;

    // Remove has_tor_role relations for functions belonging to this ToR
    conn.execute(
        "DELETE FROM relations WHERE source_id = ?1 \
         AND relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_tor_role') \
         AND target_id IN ( \
             SELECT r.source_id FROM relations r \
             WHERE r.target_id = ?2 \
               AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
         )",
        params![user_id, tor_id],
    )?;

    Ok(())
}

/// Create a new tor_function and link it to a ToR.
pub fn create_function(
    conn: &Connection,
    tor_id: i64,
    name: &str,
    label: &str,
    description: &str,
    category: &str,
    authority_props: &[(&str, bool)],
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('tor_function', ?1, ?2)",
        params![name, label],
    )?;
    let func_id = conn.last_insert_rowid();

    // Set properties
    if !description.is_empty() {
        conn.execute(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'description', ?2)",
            params![func_id, description],
        )?;
    }
    if !category.is_empty() {
        conn.execute(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'category', ?2)",
            params![func_id, category],
        )?;
    }

    // Set authority properties
    for (key, value) in authority_props {
        conn.execute(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
            params![func_id, key, if *value { "true" } else { "false" }],
        )?;
    }

    // Link function to ToR via belongs_to_tor
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor'), ?1, ?2)",
        params![func_id, tor_id],
    )?;

    Ok(func_id)
}

/// Assign a tor_function to a user (has_tor_role relation).
pub fn assign_function(conn: &Connection, user_id: i64, function_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_tor_role'), ?1, ?2)",
        params![user_id, function_id],
    )?;
    Ok(())
}

/// Unassign a tor_function from a user.
pub fn unassign_function(conn: &Connection, user_id: i64, function_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM relations WHERE source_id = ?1 AND target_id = ?2 \
         AND relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_tor_role')",
        params![user_id, function_id],
    )?;
    Ok(())
}

/// Count members of a ToR.
pub fn count_members(conn: &Connection, tor_id: i64) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM relations \
         WHERE relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'member_of') \
         AND target_id = ?1",
        params![tor_id],
        |row| row.get(0),
    )
}

/// Find all users NOT already members of this ToR (for the "add member" dropdown).
pub fn find_non_members(conn: &Connection, tor_id: i64) -> rusqlite::Result<Vec<(i64, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label FROM entities e \
         WHERE e.entity_type = 'user' AND e.is_active = 1 \
         AND e.id NOT IN ( \
             SELECT r.source_id FROM relations r \
             WHERE r.target_id = ?1 \
               AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'member_of') \
         ) \
         ORDER BY e.label"
    )?;
    let users = stmt.query_map(params![tor_id], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(users)
}
```

**Step 2: Verify**

```bash
cargo check
```

**Step 3: Commit**

```bash
git add src/models/tor/queries.rs
git commit -m "feat(tor): add ToR model query functions"
```

---

### Task 4: Register ToR model module

**Files:**
- Create: `src/models/tor/mod.rs`
- Modify: `src/models/mod.rs`

**Step 1: Create module file**

```rust
pub mod types;
pub mod queries;

pub use types::*;
pub use queries::*;
```

**Step 2: Add to models/mod.rs**

Add after existing modules:

```rust
pub mod tor;
```

**Step 3: Verify**

```bash
cargo check
```

**Step 4: Commit**

```bash
git add src/models/tor/mod.rs src/models/mod.rs
git commit -m "feat(tor): register tor model module"
```

---

### Task 5: Add template structs

**Files:**
- Modify: `src/templates_structs.rs`

**Step 1: Add imports**

Add to the use block at the top:

```rust
use crate::models::tor::{TorListItem, TorDetail, TorMember, TorFunctionListItem};
```

**Step 2: Add template structs**

After the existing template structs (before the `MenuBuilderTemplate`):

```rust
// --- ToR (Terms of Reference) templates ---

#[derive(Template)]
#[template(path = "tor/list.html")]
pub struct TorListTemplate {
    pub ctx: PageContext,
    pub tors: Vec<TorListItem>,
}

#[derive(Template)]
#[template(path = "tor/form.html")]
pub struct TorFormTemplate {
    pub ctx: PageContext,
    pub form_action: String,
    pub form_title: String,
    pub tor: Option<TorDetail>,
    pub errors: Vec<String>,
}

/// UserOption for the "add member" dropdown.
pub struct UserOption {
    pub id: i64,
    pub name: String,
    pub label: String,
}

#[derive(Template)]
#[template(path = "tor/detail.html")]
pub struct TorDetailTemplate {
    pub ctx: PageContext,
    pub tor: TorDetail,
    pub members: Vec<TorMember>,
    pub functions: Vec<TorFunctionListItem>,
    pub available_users: Vec<UserOption>,
}
```

**Step 3: Verify**

Templates won't exist yet, so `cargo check` will fail on template compilation. That's expected — we create them in Tasks 6-8.

**Step 4: Commit**

```bash
git add src/templates_structs.rs
git commit -m "feat(tor): add template structs for ToR pages"
```

---

### Task 6: Create ToR list template

**Files:**
- Create: `templates/tor/list.html`

**Step 1: Create the template directory and file**

Follow the pattern from `templates/roles/list.html`:

```html
{% extends "base.html" %}

{% block title %}Terms of Reference — {{ ctx.app_name }}{% endblock %}

{% block nav %}
{% include "partials/nav.html" %}
{% endblock %}

{% block sidebar %}
{% include "partials/sidebar.html" %}
{% endblock %}

{% block content %}
{% if let Some(msg) = ctx.flash %}
<div class="alert alert-success">{{ msg }}</div>
{% endif %}

<div class="page-header">
    <h1>Terms of Reference</h1>
    {% if ctx.permissions.has("tor.create") %}
    <a href="/tor/new" class="btn btn-primary">New ToR</a>
    {% endif %}
</div>

{% if tors.is_empty() %}
<div class="empty-state">
    <p>No terms of reference have been created yet.</p>
    {% if ctx.permissions.has("tor.create") %}
    <a href="/tor/new" class="btn btn-primary">Create your first ToR</a>
    {% endif %}
</div>
{% else %}
<table class="table">
    <thead>
        <tr>
            <th class="col-id">ID</th>
            <th>Name</th>
            <th>Label</th>
            <th>Status</th>
            <th>Cadence</th>
            <th>Members</th>
            <th>Functions</th>
            <th>Actions</th>
        </tr>
    </thead>
    <tbody>
    {% for tor in tors %}
        <tr>
            <td class="col-id">{{ tor.id }}</td>
            <td><code class="mono-type">{{ tor.name }}</code></td>
            <td><strong>{{ tor.label }}</strong></td>
            <td>
                {% if tor.status.as_str() == "active" %}
                <span class="badge badge-success">Active</span>
                {% else %}
                <span class="badge badge-muted">{{ tor.status }}</span>
                {% endif %}
            </td>
            <td>{{ tor.meeting_cadence }}</td>
            <td><span class="badge badge-user">{{ tor.member_count }}</span></td>
            <td><span class="badge badge-user">{{ tor.function_count }}</span></td>
            <td class="actions">
                <a href="/tor/{{ tor.id }}" class="btn btn-sm">View</a>
                {% if ctx.permissions.has("tor.edit") %}
                <a href="/tor/{{ tor.id }}/edit" class="btn btn-sm">Edit</a>
                {% endif %}
            </td>
        </tr>
    {% endfor %}
    </tbody>
</table>
{% endif %}
{% endblock %}
```

**Step 2: Commit**

```bash
git add templates/tor/list.html
git commit -m "feat(tor): add list template"
```

---

### Task 7: Create ToR form template

**Files:**
- Create: `templates/tor/form.html`

**Step 1: Write the form template**

```html
{% extends "base.html" %}

{% block title %}{{ form_title }} — {{ ctx.app_name }}{% endblock %}

{% block nav %}
{% include "partials/nav.html" %}
{% endblock %}

{% block sidebar %}
{% include "partials/sidebar.html" %}
{% endblock %}

{% block content %}
<h1>{{ form_title }}</h1>

{% for err in errors %}
<div class="alert alert-error">{{ err }}</div>
{% endfor %}

<form method="post" action="{{ form_action }}" class="form-card">
    <input type="hidden" name="csrf_token" value="{{ ctx.csrf_token }}">

    <div class="form-group">
        <label for="name">Name</label>
        <input type="text" id="name" name="name"
               value="{% if let Some(t) = tor %}{{ t.name }}{% endif %}" required>
        <span class="hint">Internal identifier (e.g. budget_committee)</span>
    </div>

    <div class="form-group">
        <label for="label">Label</label>
        <input type="text" id="label" name="label"
               value="{% if let Some(t) = tor %}{{ t.label }}{% endif %}" required>
        <span class="hint">Display name (e.g. Budget Committee)</span>
    </div>

    <div class="form-group">
        <label for="description">Description</label>
        <textarea id="description" name="description" rows="3">{% if let Some(t) = tor %}{{ t.description }}{% endif %}</textarea>
    </div>

    <div class="form-group">
        <label for="status">Status</label>
        <select id="status" name="status">
            <option value="active" {% if let Some(t) = tor %}{% if t.status.as_str() == "active" %}selected{% endif %}{% else %}selected{% endif %}>Active</option>
            <option value="archived" {% if let Some(t) = tor %}{% if t.status.as_str() == "archived" %}selected{% endif %}{% endif %}>Archived</option>
        </select>
    </div>

    <fieldset>
        <legend>Meeting Schedule</legend>

        <div class="form-group">
            <label for="meeting_cadence">Cadence</label>
            <select id="meeting_cadence" name="meeting_cadence">
                <option value="ad-hoc" {% if let Some(t) = tor %}{% if t.meeting_cadence.as_str() == "ad-hoc" %}selected{% endif %}{% else %}selected{% endif %}>Ad-hoc</option>
                <option value="weekly" {% if let Some(t) = tor %}{% if t.meeting_cadence.as_str() == "weekly" %}selected{% endif %}{% endif %}>Weekly</option>
                <option value="biweekly" {% if let Some(t) = tor %}{% if t.meeting_cadence.as_str() == "biweekly" %}selected{% endif %}{% endif %}>Biweekly</option>
                <option value="monthly" {% if let Some(t) = tor %}{% if t.meeting_cadence.as_str() == "monthly" %}selected{% endif %}{% endif %}>Monthly</option>
            </select>
        </div>

        <div class="form-group">
            <label for="cadence_day">Day</label>
            <select id="cadence_day" name="cadence_day">
                <option value="">—</option>
                <option value="monday" {% if let Some(t) = tor %}{% if t.cadence_day.as_str() == "monday" %}selected{% endif %}{% endif %}>Monday</option>
                <option value="tuesday" {% if let Some(t) = tor %}{% if t.cadence_day.as_str() == "tuesday" %}selected{% endif %}{% endif %}>Tuesday</option>
                <option value="wednesday" {% if let Some(t) = tor %}{% if t.cadence_day.as_str() == "wednesday" %}selected{% endif %}{% endif %}>Wednesday</option>
                <option value="thursday" {% if let Some(t) = tor %}{% if t.cadence_day.as_str() == "thursday" %}selected{% endif %}{% endif %}>Thursday</option>
                <option value="friday" {% if let Some(t) = tor %}{% if t.cadence_day.as_str() == "friday" %}selected{% endif %}{% endif %}>Friday</option>
            </select>
        </div>

        <div class="form-group">
            <label for="cadence_time">Time</label>
            <input type="time" id="cadence_time" name="cadence_time"
                   value="{% if let Some(t) = tor %}{{ t.cadence_time }}{% endif %}">
        </div>

        <div class="form-group">
            <label for="cadence_duration_minutes">Duration (minutes)</label>
            <input type="number" id="cadence_duration_minutes" name="cadence_duration_minutes"
                   value="{% if let Some(t) = tor %}{{ t.cadence_duration_minutes }}{% else %}60{% endif %}"
                   min="15" step="15">
        </div>
    </fieldset>

    <fieldset>
        <legend>Location & Resources</legend>

        <div class="form-group">
            <label for="default_location">Default Location</label>
            <input type="text" id="default_location" name="default_location"
                   value="{% if let Some(t) = tor %}{{ t.default_location }}{% endif %}">
            <span class="hint">Physical meeting room or place</span>
        </div>

        <div class="form-group">
            <label for="remote_url">Remote Meeting URL</label>
            <input type="url" id="remote_url" name="remote_url"
                   value="{% if let Some(t) = tor %}{{ t.remote_url }}{% endif %}"
                   placeholder="https://teams.microsoft.com/...">
            <span class="hint">Teams, Skype, Zoom, or other video conference link</span>
        </div>

        <div class="form-group">
            <label for="background_repo_url">Background Documents URL</label>
            <input type="url" id="background_repo_url" name="background_repo_url"
                   value="{% if let Some(t) = tor %}{{ t.background_repo_url }}{% endif %}"
                   placeholder="https://sharepoint.com/...">
            <span class="hint">Link to shared repository for background documents</span>
        </div>
    </fieldset>

    <div class="form-actions">
        <button type="submit" class="btn btn-primary">Save</button>
        <a href="/tor" class="btn">Cancel</a>
    </div>
</form>
{% endblock %}
```

**Step 2: Commit**

```bash
git add templates/tor/form.html
git commit -m "feat(tor): add form template for create/edit"
```

---

### Task 8: Create ToR detail template

**Files:**
- Create: `templates/tor/detail.html`

**Step 1: Write the detail/management template**

```html
{% extends "base.html" %}

{% block title %}{{ tor.label }} — {{ ctx.app_name }}{% endblock %}

{% block nav %}
{% include "partials/nav.html" %}
{% endblock %}

{% block sidebar %}
{% include "partials/sidebar.html" %}
{% endblock %}

{% block content %}
{% if let Some(msg) = ctx.flash %}
<div class="alert alert-success">{{ msg }}</div>
{% endif %}

<div class="page-header">
    <h1>{{ tor.label }}</h1>
    <div class="page-actions">
        {% if ctx.permissions.has("tor.edit") %}
        <a href="/tor/{{ tor.id }}/edit" class="btn btn-sm">Edit</a>
        {% endif %}
        <a href="/tor" class="btn btn-sm">Back to List</a>
    </div>
</div>

<div class="detail-card">
    <div class="detail-row">
        <span class="detail-label">Name</span>
        <span class="detail-value"><code class="mono-type">{{ tor.name }}</code></span>
    </div>
    <div class="detail-row">
        <span class="detail-label">Status</span>
        <span class="detail-value">
            {% if tor.status.as_str() == "active" %}
            <span class="badge badge-success">Active</span>
            {% else %}
            <span class="badge badge-muted">{{ tor.status }}</span>
            {% endif %}
        </span>
    </div>
    {% if !tor.description.is_empty() %}
    <div class="detail-row">
        <span class="detail-label">Description</span>
        <span class="detail-value">{{ tor.description }}</span>
    </div>
    {% endif %}
    <div class="detail-row">
        <span class="detail-label">Cadence</span>
        <span class="detail-value">{{ tor.meeting_cadence }}
            {% if !tor.cadence_day.is_empty() %} — {{ tor.cadence_day }}{% endif %}
            {% if !tor.cadence_time.is_empty() %} at {{ tor.cadence_time }}{% endif %}
        </span>
    </div>
    {% if !tor.default_location.is_empty() %}
    <div class="detail-row">
        <span class="detail-label">Location</span>
        <span class="detail-value">{{ tor.default_location }}</span>
    </div>
    {% endif %}
    {% if !tor.remote_url.is_empty() %}
    <div class="detail-row">
        <span class="detail-label">Remote Meeting</span>
        <span class="detail-value"><a href="{{ tor.remote_url }}" target="_blank">{{ tor.remote_url }}</a></span>
    </div>
    {% endif %}
    {% if !tor.background_repo_url.is_empty() %}
    <div class="detail-row">
        <span class="detail-label">Background Documents</span>
        <span class="detail-value"><a href="{{ tor.background_repo_url }}" target="_blank">{{ tor.background_repo_url }}</a></span>
    </div>
    {% endif %}
</div>

<!-- Members Section -->
<section class="section">
    <div class="section-header">
        <h2>Members ({{ members.len() }})</h2>
    </div>

    {% if ctx.permissions.has("tor.manage_members") && !available_users.is_empty() %}
    <form method="post" action="/tor/{{ tor.id }}/members" class="inline-form">
        <input type="hidden" name="csrf_token" value="{{ ctx.csrf_token }}">
        <input type="hidden" name="action" value="add">
        <select name="user_id" required>
            <option value="">Select user to add...</option>
            {% for u in available_users %}
            <option value="{{ u.id }}">{{ u.label }} ({{ u.name }})</option>
            {% endfor %}
        </select>
        <button type="submit" class="btn btn-sm btn-primary">Add Member</button>
    </form>
    {% endif %}

    {% if members.is_empty() %}
    <p class="empty-hint">No members assigned yet.</p>
    {% else %}
    <table class="table">
        <thead>
            <tr>
                <th>Member</th>
                <th>Functions</th>
                {% if ctx.permissions.has("tor.manage_members") %}
                <th>Actions</th>
                {% endif %}
            </tr>
        </thead>
        <tbody>
        {% for member in members %}
            <tr>
                <td><strong>{{ member.user_label }}</strong> <code class="mono-type">{{ member.user_name }}</code></td>
                <td>
                    {% if member.functions.is_empty() %}
                    <span class="text-muted">None</span>
                    {% else %}
                    {% for func in member.functions %}
                    <span class="badge badge-role">{{ func.label }}</span>
                    {% endfor %}
                    {% endif %}
                </td>
                {% if ctx.permissions.has("tor.manage_members") %}
                <td class="actions">
                    <form method="post" action="/tor/{{ tor.id }}/members" class="inline"
                          onsubmit="return confirm('Remove this member?')">
                        <input type="hidden" name="csrf_token" value="{{ ctx.csrf_token }}">
                        <input type="hidden" name="action" value="remove">
                        <input type="hidden" name="user_id" value="{{ member.user_id }}">
                        <button type="submit" class="btn btn-sm btn-danger">Remove</button>
                    </form>
                </td>
                {% endif %}
            </tr>
        {% endfor %}
        </tbody>
    </table>
    {% endif %}
</section>

<!-- Functions Section -->
<section class="section">
    <div class="section-header">
        <h2>Functions ({{ functions.len() }})</h2>
        {% if ctx.permissions.has("tor.edit") %}
        <a href="/tor/{{ tor.id }}/functions/new" class="btn btn-sm btn-primary">New Function</a>
        {% endif %}
    </div>

    {% if functions.is_empty() %}
    <p class="empty-hint">No functions defined yet.</p>
    {% else %}
    <table class="table">
        <thead>
            <tr>
                <th>Function</th>
                <th>Category</th>
                <th>Assigned To</th>
            </tr>
        </thead>
        <tbody>
        {% for func in functions %}
            <tr>
                <td><strong>{{ func.label }}</strong> <code class="mono-type">{{ func.name }}</code></td>
                <td>{{ func.category }}</td>
                <td>
                    {% if func.assigned_to.is_empty() %}
                    <span class="text-muted">Unassigned</span>
                    {% else %}
                    {% for user_label in func.assigned_to %}
                    <span class="badge badge-user">{{ user_label }}</span>
                    {% endfor %}
                    {% endif %}
                </td>
            </tr>
        {% endfor %}
        </tbody>
    </table>
    {% endif %}
</section>
{% endblock %}
```

**Step 2: Verify templates compile**

```bash
cargo check
```

**Step 3: Commit**

```bash
git add templates/tor/detail.html
git commit -m "feat(tor): add detail template with members and functions"
```

---

### Task 9: Create ToR list handler

**Files:**
- Create: `src/handlers/tor_handlers/list.rs`

**Step 1: Write the list handler**

```rust
use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::models::tor;
use crate::auth::session::require_permission;
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, TorListTemplate};

pub async fn list(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.list")?;

    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/tor")?;
    let tors = tor::find_all_list_items(&conn)?;

    let tmpl = TorListTemplate { ctx, tors };
    render(tmpl)
}
```

**Step 2: Verify**

```bash
cargo check
```

**Step 3: Commit**

```bash
git add src/handlers/tor_handlers/list.rs
git commit -m "feat(tor): add list handler"
```

---

### Task 10: Create ToR CRUD handlers

**Files:**
- Create: `src/handlers/tor_handlers/crud.rs`

**Step 1: Write CRUD handlers**

```rust
use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::models::tor;
use crate::auth::csrf;
use crate::auth::session::require_permission;
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, TorFormTemplate, TorDetailTemplate, UserOption};

pub async fn new_form(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.create")?;

    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/tor")?;

    let tmpl = TorFormTemplate {
        ctx,
        form_action: "/tor".to_string(),
        form_title: "Create Terms of Reference".to_string(),
        tor: None,
        errors: vec![],
    };
    render(tmpl)
}

pub async fn create(
    pool: web::Data<DbPool>,
    session: Session,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.create")?;
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    let conn = pool.get()?;

    let name = form.get("name").map(|s| s.as_str()).unwrap_or("");
    let label = form.get("label").map(|s| s.as_str()).unwrap_or("");
    let description = form.get("description").map(|s| s.as_str()).unwrap_or("");
    let status = form.get("status").map(|s| s.as_str()).unwrap_or("active");
    let meeting_cadence = form.get("meeting_cadence").map(|s| s.as_str()).unwrap_or("ad-hoc");
    let cadence_day = form.get("cadence_day").map(|s| s.as_str()).unwrap_or("");
    let cadence_time = form.get("cadence_time").map(|s| s.as_str()).unwrap_or("");
    let cadence_duration = form.get("cadence_duration_minutes").map(|s| s.as_str()).unwrap_or("60");
    let default_location = form.get("default_location").map(|s| s.as_str()).unwrap_or("");
    let remote_url = form.get("remote_url").map(|s| s.as_str()).unwrap_or("");
    let background_repo_url = form.get("background_repo_url").map(|s| s.as_str()).unwrap_or("");

    // Validate
    let mut errors = vec![];
    if name.trim().is_empty() {
        errors.push("Name is required".to_string());
    }
    if label.trim().is_empty() {
        errors.push("Label is required".to_string());
    }

    if !errors.is_empty() {
        let ctx = PageContext::build(&session, &conn, "/tor")?;
        let tmpl = TorFormTemplate {
            ctx,
            form_action: "/tor".to_string(),
            form_title: "Create Terms of Reference".to_string(),
            tor: None,
            errors,
        };
        return render(tmpl);
    }

    match tor::create(&conn, name.trim(), label.trim(), description.trim(),
                      status, meeting_cadence, cadence_day, cadence_time, cadence_duration,
                      default_location, remote_url, background_repo_url) {
        Ok(tor_id) => {
            let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
            let details = serde_json::json!({
                "tor_name": name.trim(),
                "summary": format!("Created Terms of Reference '{}'", label.trim())
            });
            let _ = crate::audit::log(&conn, current_user_id, "tor.created", "tor", tor_id, details);

            let _ = session.insert("flash", "Terms of Reference created successfully");
            Ok(HttpResponse::SeeOther()
                .insert_header(("Location", format!("/tor/{tor_id}")))
                .finish())
        }
        Err(e) => {
            let msg = if e.to_string().contains("UNIQUE") {
                "A ToR with this name already exists".to_string()
            } else {
                format!("Error creating ToR: {e}")
            };
            let ctx = PageContext::build(&session, &conn, "/tor")?;
            let tmpl = TorFormTemplate {
                ctx,
                form_action: "/tor".to_string(),
                form_title: "Create Terms of Reference".to_string(),
                tor: None,
                errors: vec![msg],
            };
            render(tmpl)
        }
    }
}

pub async fn detail(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.list")?;

    let id = path.into_inner();
    let conn = pool.get()?;

    match tor::find_detail_by_id(&conn, id)? {
        Some(tor_detail) => {
            let ctx = PageContext::build(&session, &conn, "/tor")?;
            let members = tor::find_members(&conn, id)?;
            let functions = tor::find_functions(&conn, id)?;
            let non_members = tor::find_non_members(&conn, id)?;
            let available_users = non_members.into_iter()
                .map(|(id, name, label)| UserOption { id, name, label })
                .collect();

            let tmpl = TorDetailTemplate {
                ctx,
                tor: tor_detail,
                members,
                functions,
                available_users,
            };
            render(tmpl)
        }
        None => Err(AppError::NotFound),
    }
}

pub async fn edit_form(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;

    let id = path.into_inner();
    let conn = pool.get()?;

    match tor::find_detail_by_id(&conn, id)? {
        Some(t) => {
            let ctx = PageContext::build(&session, &conn, "/tor")?;
            let tmpl = TorFormTemplate {
                ctx,
                form_action: format!("/tor/{id}"),
                form_title: "Edit Terms of Reference".to_string(),
                tor: Some(t),
                errors: vec![],
            };
            render(tmpl)
        }
        None => Err(AppError::NotFound),
    }
}

pub async fn update(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    let id = path.into_inner();
    let conn = pool.get()?;

    let name = form.get("name").map(|s| s.as_str()).unwrap_or("");
    let label = form.get("label").map(|s| s.as_str()).unwrap_or("");
    let description = form.get("description").map(|s| s.as_str()).unwrap_or("");
    let status = form.get("status").map(|s| s.as_str()).unwrap_or("active");
    let meeting_cadence = form.get("meeting_cadence").map(|s| s.as_str()).unwrap_or("ad-hoc");
    let cadence_day = form.get("cadence_day").map(|s| s.as_str()).unwrap_or("");
    let cadence_time = form.get("cadence_time").map(|s| s.as_str()).unwrap_or("");
    let cadence_duration = form.get("cadence_duration_minutes").map(|s| s.as_str()).unwrap_or("60");
    let default_location = form.get("default_location").map(|s| s.as_str()).unwrap_or("");
    let remote_url = form.get("remote_url").map(|s| s.as_str()).unwrap_or("");
    let background_repo_url = form.get("background_repo_url").map(|s| s.as_str()).unwrap_or("");

    // Validate
    let mut errors = vec![];
    if name.trim().is_empty() {
        errors.push("Name is required".to_string());
    }
    if label.trim().is_empty() {
        errors.push("Label is required".to_string());
    }

    if !errors.is_empty() {
        let existing = tor::find_detail_by_id(&conn, id).ok().flatten();
        let ctx = PageContext::build(&session, &conn, "/tor")?;
        let tmpl = TorFormTemplate {
            ctx,
            form_action: format!("/tor/{id}"),
            form_title: "Edit Terms of Reference".to_string(),
            tor: existing,
            errors,
        };
        return render(tmpl);
    }

    match tor::update(&conn, id, name.trim(), label.trim(), description.trim(),
                      status, meeting_cadence, cadence_day, cadence_time, cadence_duration,
                      default_location, remote_url, background_repo_url) {
        Ok(_) => {
            let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
            let details = serde_json::json!({
                "tor_name": name.trim(),
                "summary": format!("Updated Terms of Reference '{}'", label.trim())
            });
            let _ = crate::audit::log(&conn, current_user_id, "tor.updated", "tor", id, details);

            let _ = session.insert("flash", "Terms of Reference updated successfully");
            Ok(HttpResponse::SeeOther()
                .insert_header(("Location", format!("/tor/{id}")))
                .finish())
        }
        Err(e) => {
            let msg = if e.to_string().contains("UNIQUE") {
                "A ToR with this name already exists".to_string()
            } else {
                format!("Error updating ToR: {e}")
            };
            let existing = tor::find_detail_by_id(&conn, id).ok().flatten();
            let ctx = PageContext::build(&session, &conn, "/tor")?;
            let tmpl = TorFormTemplate {
                ctx,
                form_action: format!("/tor/{id}"),
                form_title: "Edit Terms of Reference".to_string(),
                tor: existing,
                errors: vec![msg],
            };
            render(tmpl)
        }
    }
}

pub async fn delete(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<crate::handlers::auth_handlers::CsrfOnly>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let id = path.into_inner();
    let conn = pool.get()?;

    // Prevent deleting a ToR that has members
    let member_count = tor::count_members(&conn, id)?;
    if member_count > 0 {
        let _ = session.insert("flash", format!("Cannot delete ToR: {member_count} member(s) still assigned"));
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", "/tor"))
            .finish());
    }

    let tor_details = tor::find_detail_by_id(&conn, id).ok().flatten();

    match tor::delete(&conn, id) {
        Ok(_) => {
            let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
            if let Some(deleted) = tor_details {
                let details = serde_json::json!({
                    "tor_name": deleted.name,
                    "summary": format!("Deleted Terms of Reference '{}'", deleted.label)
                });
                let _ = crate::audit::log(&conn, current_user_id, "tor.deleted", "tor", id, details);
            }

            let _ = session.insert("flash", "Terms of Reference deleted");
            Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/tor"))
                .finish())
        }
        Err(_) => {
            let _ = session.insert("flash", "Error deleting Terms of Reference");
            Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/tor"))
                .finish())
        }
    }
}
```

**Step 2: Verify**

```bash
cargo check
```

**Step 3: Commit**

```bash
git add src/handlers/tor_handlers/crud.rs
git commit -m "feat(tor): add CRUD handlers"
```

---

### Task 11: Create member management handler

**Files:**
- Create: `src/handlers/tor_handlers/members.rs`

**Step 1: Write the member management handler**

```rust
use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::models::tor;
use crate::auth::csrf;
use crate::auth::session::require_permission;
use crate::errors::AppError;

pub async fn manage_members(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.manage_members")?;
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    let tor_id = path.into_inner();
    let conn = pool.get()?;

    let action = form.get("action").map(|s| s.as_str()).unwrap_or("");
    let user_id: i64 = form.get("user_id")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if user_id == 0 {
        let _ = session.insert("flash", "Please select a user");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", format!("/tor/{tor_id}")))
            .finish());
    }

    let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);

    match action {
        "add" => {
            tor::add_member(&conn, user_id, tor_id)?;
            let details = serde_json::json!({
                "user_id": user_id,
                "summary": format!("Added member to ToR")
            });
            let _ = crate::audit::log(&conn, current_user_id, "tor.member_added", "tor", tor_id, details);
            let _ = session.insert("flash", "Member added successfully");
        }
        "remove" => {
            tor::remove_member(&conn, user_id, tor_id)?;
            let details = serde_json::json!({
                "user_id": user_id,
                "summary": format!("Removed member from ToR")
            });
            let _ = crate::audit::log(&conn, current_user_id, "tor.member_removed", "tor", tor_id, details);
            let _ = session.insert("flash", "Member removed");
        }
        _ => {
            let _ = session.insert("flash", "Unknown action");
        }
    }

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}")))
        .finish())
}
```

**Step 2: Verify**

```bash
cargo check
```

**Step 3: Commit**

```bash
git add src/handlers/tor_handlers/members.rs
git commit -m "feat(tor): add member management handler"
```

---

### Task 12: Register handler module

**Files:**
- Create: `src/handlers/tor_handlers/mod.rs`
- Modify: `src/handlers/mod.rs`

**Step 1: Create tor_handlers/mod.rs**

```rust
pub mod list;
pub mod crud;
pub mod members;

pub use list::*;
pub use crud::*;
pub use members::*;
```

**Step 2: Add to handlers/mod.rs**

Add after existing modules:

```rust
pub mod tor_handlers;
```

**Step 3: Verify**

```bash
cargo check
```

**Step 4: Commit**

```bash
git add src/handlers/tor_handlers/mod.rs src/handlers/mod.rs
git commit -m "feat(tor): register tor handler module"
```

---

### Task 13: Wire routes

**Files:**
- Modify: `src/main.rs`

**Step 1: Add ToR routes to the protected scope**

After the Role CRUD routes block, add:

```rust
// ToR CRUD — /tor/new BEFORE /tor/{id}
.route("/tor", web::get().to(handlers::tor_handlers::list))
.route("/tor/new", web::get().to(handlers::tor_handlers::new_form))
.route("/tor", web::post().to(handlers::tor_handlers::create))
.route("/tor/{id}", web::get().to(handlers::tor_handlers::detail))
.route("/tor/{id}/edit", web::get().to(handlers::tor_handlers::edit_form))
.route("/tor/{id}", web::post().to(handlers::tor_handlers::update))
.route("/tor/{id}/delete", web::post().to(handlers::tor_handlers::delete))
// ToR member management
.route("/tor/{id}/members", web::post().to(handlers::tor_handlers::manage_members))
```

**Step 2: Verify build**

```bash
cargo build
```

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat(tor): wire ToR routes"
```

---

### Task 14: End-to-end verification

**Step 1: Reset database and start server**

```bash
rm data/app.db && cargo run
```

**Step 2: Manual test checklist**

1. Login as admin → see "Governance" in nav bar
2. Click "Governance" → ToR list page (empty state)
3. Click "New ToR" → create form
4. Create "budget_committee" / "Budget Committee" with weekly cadence
5. Redirected to detail page showing the new ToR
6. Click "Edit" → edit form pre-populated
7. Change description → save → redirected back to detail
8. Go back to list → see the ToR
9. On detail page, add a member via the dropdown
10. Member appears in the members table
11. Remove the member → confirmed removal
12. Check Ontology browser → `tor` entity type visible in graph
13. Check Audit log → tor.created, tor.updated, tor.member_added events

**Step 3: Final commit**

```bash
git add -A
git commit -m "feat(tor): Phase 1 complete — ToR foundation with membership"
```

---

## Notes for Subsequent Phases

- **Phase 2 (Item Pipeline):** Same pattern — new entity types (`suggestion`, `proposal`, `agenda_point`), model modules, handlers, templates. Add status transition logic in query functions.
- **Phase 3 (Meetings):** Add `meeting` entity type with recurring generation logic (a new function in `tor::queries` that creates meeting entities from cadence properties).
- **Phase 4 (Calendar):** New handler + template with no new entity types. Query meetings by date range. Consider adding index on `entity_properties(key, value)` for `scheduled_date` lookups.
- **Phase 5 (Delegation):** `delegation` entity type with date-bounded authority. Extend `require_permission` or add `require_tor_authority` helper.
- **Phase 6 (ABAC Prep):** Internal refactoring — create `src/auth/authority.rs` with policy evaluation functions.
