# ToR Expansion — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extend the ToR governance system with position-based membership, meeting protocol templates, inter-ToR dependencies, structured minutes, and presentation templates.

**Architecture:** Pure EAV — all new concepts are entity types with properties and relations. Extends existing `tor`, `tor_function`, `meeting` entities. Replaces `member_of`/`has_tor_role` relations with `fills_position`. New modules follow established patterns: model (types.rs + queries.rs), handlers (list.rs + crud.rs), Askama templates, route registration.

**Tech Stack:** Actix-web 4, Askama 0.14, rusqlite 0.32, r2d2_sqlite — same as existing codebase.

**Design doc:** `docs/plans/2026-02-18-tor-expansion-design.md`

---

## Phase 1b — Position-Based Membership Migration

### Task 1: Seed `fills_position` relation type

**GOAL:** Add `fills_position` relation type to seed data. After `rm data/dev/app.db && cargo run`, query `SELECT * FROM entities WHERE entity_type='relation_type' AND name='fills_position'` returns one row. `relation_properties` table already exists in schema.sql — no schema change needed.

**CONSTRAINTS:**
- Modify only `src/db.rs` (seed_ontology function)
- Keep `member_of` and `has_tor_role` in seed for backward compatibility during migration — mark with `// DEPRECATED: replaced by fills_position` comment
- No new Rust dependencies

**FORMAT:**
- Modify: `src/db.rs` — add `fills_position` relation type insert after the existing ToR relation types block (line ~74)

**Step 1: Add fills_position to seed_ontology**

In `src/db.rs`, after line 74 (`_belongs_to_tor_id`), add:

```rust
let _fills_position_id = insert_entity(&conn, "relation_type", "fills_position", "Fills Position", 0);
```

**Step 2: Add fills_position to seed_staging too**

Staging seed calls `seed_ontology` first so it inherits. No changes needed.

**Step 3: Add new permissions for minutes**

In the `perms` array (around line 143), add:

```rust
("minutes.generate", "Generate Meeting Minutes", "Governance"),
("minutes.edit", "Edit Meeting Minutes", "Governance"),
("minutes.approve", "Approve Meeting Minutes", "Governance"),
```

**Step 4: Verify**

```bash
rm data/dev/app.db && cargo run 2>&1 | head -20
```

Expected: "Seeded ontology:" log line, server starts.

**Step 5: Commit**

```bash
git add src/db.rs
git commit -m "feat(tor): seed fills_position relation type and minutes permissions"
```

**FAILURE CONDITIONS:**
- `fills_position` not seeded
- Server fails to start after DB reset
- Compilation errors

---

### Task 2: Update TorMember type for position-based model

**GOAL:** Replace person-centric `TorMember` struct with position-centric version. Position is primary, holder is optional (vacant positions). `cargo check` passes.

**CONSTRAINTS:**
- Modify only `src/models/tor/types.rs`
- Keep existing `TorFunctionRef`, `TorFunctionDetail`, `TorFunctionListItem` unchanged
- New `TorMember` must handle vacant positions (holder fields are `Option`)
- Follow existing derive pattern: `#[derive(Debug, Clone)]`

**FORMAT:**
- Modify: `src/models/tor/types.rs`

**Step 1: Replace TorMember struct**

Replace the existing `TorMember` and `TorFunctionRef` structs (lines 33-49) with:

```rust
/// A position in a ToR with its current holder (if any).
/// Position-based: authority flows from position, not person.
#[derive(Debug, Clone)]
pub struct TorMember {
    pub position_id: i64,
    pub position_name: String,
    pub position_label: String,
    pub membership_type: String, // "mandatory" or "optional"
    pub holder_id: Option<i64>,
    pub holder_name: Option<String>,
    pub holder_label: Option<String>,
}
```

**Step 2: Verify**

```bash
cargo check 2>&1 | tail -20
```

Expected: Errors in `queries.rs` and `crud.rs` referencing old `TorMember` fields — that's correct, we fix those in Task 3.

**Step 3: Commit**

```bash
git add src/models/tor/types.rs
git commit -m "feat(tor): position-based TorMember type (breaks queries — fixed next)"
```

**FAILURE CONDITIONS:**
- `TorMember` still has `user_id` as non-optional primary field
- No representation of vacant positions
- Missing `membership_type` field

---

### Task 3: Rewrite membership queries for position-based model

**GOAL:** Rewrite `find_members()`, `add_member()`, `remove_member()`, `count_members()`, `find_non_members()`, and `require_tor_membership()` in `src/models/tor/queries.rs` to use `fills_position` relation. Add `assign_to_position()` and `vacate_position()`. `cargo check` passes.

**CONSTRAINTS:**
- Derived membership: user is member if they fill any position belonging to the ToR
- `find_members()` returns positions (including vacant) with optional holder
- `assign_to_position()` creates `fills_position` relation + `relation_properties` entry for `membership_type`
- `vacate_position()` removes the `fills_position` relation pointing to a position
- `count_members()` counts positions with holders (not vacant)
- `require_tor_membership()` checks if user fills any position in the ToR
- `find_non_members()` returns users not filling any position in this ToR
- Use `relation_properties` table for `membership_type`

**FORMAT:**
- Modify: `src/models/tor/queries.rs` — rewrite 7 functions

**Step 1: Rewrite find_members**

Replace the existing `find_members` function (lines 207-262) with:

```rust
/// Find all positions in a ToR with their current holders.
/// Returns positions even when vacant (holder fields will be None).
pub fn find_members(conn: &Connection, tor_id: i64) -> rusqlite::Result<Vec<TorMember>> {
    let mut stmt = conn.prepare(
        "SELECT f.id AS position_id, f.name AS position_name, f.label AS position_label, \
                COALESCE(rp.value, 'optional') AS membership_type, \
                u.id AS holder_id, u.name AS holder_name, u.label AS holder_label \
         FROM entities f \
         JOIN relations r_tor ON f.id = r_tor.source_id \
         LEFT JOIN relations r_fills ON f.id = r_fills.target_id \
             AND r_fills.relation_type_id = ( \
                 SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position') \
         LEFT JOIN relation_properties rp ON r_fills.id = rp.relation_id AND rp.key = 'membership_type' \
         LEFT JOIN entities u ON r_fills.source_id = u.id AND u.entity_type = 'user' \
         WHERE r_tor.target_id = ?1 \
           AND r_tor.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
           AND f.entity_type = 'tor_function' \
         ORDER BY CASE WHEN rp.value = 'mandatory' THEN 0 ELSE 1 END, f.label",
    )?;

    let members = stmt
        .query_map(params![tor_id], |row| {
            let holder_id: Option<i64> = row.get("holder_id")?;
            Ok(TorMember {
                position_id: row.get("position_id")?,
                position_name: row.get("position_name")?,
                position_label: row.get("position_label")?,
                membership_type: row.get("membership_type")?,
                holder_id,
                holder_name: if holder_id.is_some() { row.get("holder_name")? } else { None },
                holder_label: if holder_id.is_some() { row.get("holder_label")? } else { None },
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(members)
}
```

**Step 2: Add assign_to_position and vacate_position**

After the rewritten `find_members`, add:

```rust
/// Assign a user to a position (fills_position relation + membership_type property).
pub fn assign_to_position(
    conn: &Connection,
    user_id: i64,
    position_id: i64,
    membership_type: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id) \
         VALUES ( \
             (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position'), \
             ?1, ?2)",
        params![user_id, position_id],
    )?;

    // Get the relation ID for the property
    let relation_id: i64 = conn.query_row(
        "SELECT id FROM relations WHERE source_id = ?1 AND target_id = ?2 \
         AND relation_type_id = ( \
             SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position')",
        params![user_id, position_id],
        |row| row.get(0),
    )?;

    conn.execute(
        "INSERT INTO relation_properties (relation_id, key, value) VALUES (?1, 'membership_type', ?2) \
         ON CONFLICT(relation_id, key) DO UPDATE SET value = excluded.value",
        params![relation_id, membership_type],
    )?;

    Ok(())
}

/// Remove the current holder from a position.
pub fn vacate_position(conn: &Connection, position_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM relations WHERE target_id = ?1 \
         AND relation_type_id = ( \
             SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position')",
        params![position_id],
    )?;
    Ok(())
}
```

**Step 3: Rewrite count_members, find_non_members, require_tor_membership**

Replace `count_members` (line ~360):

```rust
/// Count positions with holders in a ToR (not vacant positions).
pub fn count_members(conn: &Connection, tor_id: i64) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(DISTINCT r_fills.source_id) \
         FROM entities f \
         JOIN relations r_tor ON f.id = r_tor.source_id \
         JOIN relations r_fills ON f.id = r_fills.target_id \
         WHERE r_tor.target_id = ?1 \
           AND r_tor.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
           AND r_fills.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position') \
           AND f.entity_type = 'tor_function'",
        params![tor_id],
        |row| row.get(0),
    )
}
```

Replace `find_non_members` (line ~372):

```rust
/// Find users not currently filling any position in this ToR.
pub fn find_non_members(
    conn: &Connection,
    tor_id: i64,
) -> rusqlite::Result<Vec<(i64, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label \
         FROM entities e \
         WHERE e.entity_type = 'user' \
           AND e.is_active = 1 \
           AND e.id NOT IN ( \
               SELECT r_fills.source_id \
               FROM relations r_fills \
               JOIN relations r_tor ON r_fills.target_id = r_tor.source_id \
               WHERE r_tor.target_id = ?1 \
                 AND r_tor.relation_type_id = ( \
                     SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
                 AND r_fills.relation_type_id = ( \
                     SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position')) \
         ORDER BY e.label",
    )?;

    let users = stmt
        .query_map(params![tor_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(users)
}
```

Replace `require_tor_membership` (line ~399):

```rust
/// Verify user fills a position in the given ToR. Returns AppError::PermissionDenied if not.
pub fn require_tor_membership(
    conn: &Connection,
    user_id: i64,
    tor_id: i64,
) -> Result<(), AppError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) \
         FROM relations r_fills \
         JOIN relations r_tor ON r_fills.target_id = r_tor.source_id \
         WHERE r_fills.source_id = ?1 \
           AND r_tor.target_id = ?2 \
           AND r_fills.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position') \
           AND r_tor.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor')",
        params![user_id, tor_id],
        |row| row.get(0),
    )?;

    if count == 0 {
        return Err(AppError::PermissionDenied("Not a member of this ToR".into()));
    }
    Ok(())
}
```

**Step 4: Remove deprecated functions**

Remove `add_member` and `remove_member` functions entirely. They are replaced by `assign_to_position` and `vacate_position`.

**Step 5: Verify**

```bash
cargo check 2>&1 | tail -20
```

Expected: Errors in handlers referencing old `add_member`/`remove_member` — fixed in Task 4.

**Step 6: Commit**

```bash
git add src/models/tor/queries.rs
git commit -m "feat(tor): position-based membership queries (breaks handlers — fixed next)"
```

**FAILURE CONDITIONS:**
- Still queries `member_of` relation type
- Still queries `has_tor_role` relation type
- Vacant positions not returned by `find_members()`
- `membership_type` not read from `relation_properties`

---

### Task 4: Update handlers and templates for position-based membership

**GOAL:** Update ToR detail page and member management handlers to show positions with holders. The "Add Member" flow becomes "Assign Person to Position". `cargo build` passes. Manual test: ToR detail page shows positions with holder names and mandatory/optional badges.

**CONSTRAINTS:**
- ToR detail shows positions table: Position | Type | Current Holder | Actions
- "Assign" dropdown of users not already in this ToR
- "Vacate" button removes person from position
- Mandatory positions without holder show warning badge
- CSRF on all mutations
- Follow existing Askama patterns

**FORMAT:**
- Modify: `src/handlers/tor_handlers/members.rs`
- Modify: `templates/tor/detail.html`

**Step 1: Rewrite manage_members handler**

Replace `src/handlers/tor_handlers/members.rs` contents with:

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
    let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);

    match action {
        "assign" => {
            let user_id: i64 = form.get("user_id")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let position_id: i64 = form.get("position_id")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let membership_type = form.get("membership_type")
                .map(|s| s.as_str())
                .unwrap_or("optional");

            if user_id == 0 || position_id == 0 {
                let _ = session.insert("flash", "Please select a user and position");
                return Ok(HttpResponse::SeeOther()
                    .insert_header(("Location", format!("/tor/{tor_id}")))
                    .finish());
            }

            tor::assign_to_position(&conn, user_id, position_id, membership_type)?;
            let details = serde_json::json!({
                "user_id": user_id,
                "position_id": position_id,
                "membership_type": membership_type,
                "summary": "Assigned user to position"
            });
            let _ = crate::audit::log(&conn, current_user_id, "tor.position_assigned", "tor", tor_id, details);
            let _ = session.insert("flash", "User assigned to position");
        }
        "vacate" => {
            let position_id: i64 = form.get("position_id")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            if position_id == 0 {
                let _ = session.insert("flash", "Invalid position");
                return Ok(HttpResponse::SeeOther()
                    .insert_header(("Location", format!("/tor/{tor_id}")))
                    .finish());
            }

            tor::vacate_position(&conn, position_id)?;
            let details = serde_json::json!({
                "position_id": position_id,
                "summary": "Vacated position"
            });
            let _ = crate::audit::log(&conn, current_user_id, "tor.position_vacated", "tor", tor_id, details);
            let _ = session.insert("flash", "Position vacated");
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

**Step 2: Update the Members section in templates/tor/detail.html**

Replace the `<!-- Members Section -->` block with a positions table. The table columns: Position | Type | Holder | Actions. Mandatory positions without holders get a warning badge.

Find the section starting `<!-- Members Section -->` and replace through `</section>` with:

```html
<!-- Positions Section -->
<section class="section">
    <div class="section-header">
        <h2>Positions ({{ members.len() }})</h2>
    </div>

    {% if members.is_empty() %}
    <p class="empty-hint">No positions defined yet. Add functions first, then assign people.</p>
    {% else %}
    <table class="table">
        <thead>
            <tr>
                <th>Position</th>
                <th>Type</th>
                <th>Current Holder</th>
                {% if ctx.permissions.has("tor.manage_members") %}
                <th>Actions</th>
                {% endif %}
            </tr>
        </thead>
        <tbody>
        {% for member in members %}
            <tr>
                <td><strong>{{ member.position_label }}</strong> <code class="mono-type">{{ member.position_name }}</code></td>
                <td>
                    {% if member.membership_type.as_str() == "mandatory" %}
                    <span class="badge badge-error">Mandatory</span>
                    {% else %}
                    <span class="badge badge-muted">Optional</span>
                    {% endif %}
                </td>
                <td>
                    {% if let Some(label) = member.holder_label %}
                    <strong>{{ label }}</strong>
                    {% else %}
                        {% if member.membership_type.as_str() == "mandatory" %}
                        <span class="badge badge-warning">Vacant — mandatory position</span>
                        {% else %}
                        <span class="text-muted">Vacant</span>
                        {% endif %}
                    {% endif %}
                </td>
                {% if ctx.permissions.has("tor.manage_members") %}
                <td class="actions">
                    {% if member.holder_id.is_some() %}
                    <form method="post" action="/tor/{{ tor.id }}/members" class="inline"
                          onsubmit="return confirm('Vacate this position?')">
                        <input type="hidden" name="csrf_token" value="{{ ctx.csrf_token }}">
                        <input type="hidden" name="action" value="vacate">
                        <input type="hidden" name="position_id" value="{{ member.position_id }}">
                        <button type="submit" class="btn btn-sm btn-danger">Vacate</button>
                    </form>
                    {% else %}
                    <form method="post" action="/tor/{{ tor.id }}/members" class="inline-form">
                        <input type="hidden" name="csrf_token" value="{{ ctx.csrf_token }}">
                        <input type="hidden" name="action" value="assign">
                        <input type="hidden" name="position_id" value="{{ member.position_id }}">
                        <input type="hidden" name="membership_type" value="{{ member.membership_type }}">
                        <select name="user_id" required>
                            <option value="">Assign...</option>
                            {% for u in available_users %}
                            <option value="{{ u.id }}">{{ u.label }}</option>
                            {% endfor %}
                        </select>
                        <button type="submit" class="btn btn-sm btn-primary">Assign</button>
                    </form>
                    {% endif %}
                </td>
                {% endif %}
            </tr>
        {% endfor %}
        </tbody>
    </table>
    {% endif %}
</section>
```

**Step 3: Verify**

```bash
cargo build 2>&1 | tail -5
```

Expected: "Finished" with no errors.

**Step 4: Manual test**

```bash
rm data/dev/app.db && cargo run
```

1. Login as admin
2. Create a ToR, add functions
3. Check detail page shows positions table
4. Assign a user to a position
5. Vacate the position

**Step 5: Commit**

```bash
git add src/handlers/tor_handlers/members.rs templates/tor/detail.html
git commit -m "feat(tor): position-based membership UI with assign/vacate"
```

**FAILURE CONDITIONS:**
- Still shows old flat "Members" table
- No mandatory/optional distinction visible
- Vacant positions not shown
- Missing CSRF on assign/vacate
- `cargo build` fails

---

### Task 5: Update find_functions to show current holder via fills_position

**GOAL:** The `TorFunctionListItem.assigned_to` field currently uses `has_tor_role` relation. Update it to use `fills_position`. `cargo check` passes.

**CONSTRAINTS:**
- Modify only `src/models/tor/queries.rs`, function `find_functions`
- `assigned_to: Vec<String>` stays — it now lists users who fill each position

**FORMAT:**
- Modify: `src/models/tor/queries.rs` — `find_functions()` inner query

**Step 1: Update the user lookup query inside find_functions**

Replace the `user_stmt` prepare block (lines ~289-298) with:

```rust
    let mut user_stmt = conn.prepare(
        "SELECT u.label \
         FROM relations r \
         JOIN entities u ON r.source_id = u.id \
         WHERE r.target_id = ?1 \
           AND r.relation_type_id = ( \
               SELECT id FROM entities \
               WHERE entity_type = 'relation_type' AND name = 'fills_position') \
         ORDER BY u.label",
    )?;
```

**Step 2: Verify**

```bash
cargo check 2>&1 | tail -5
```

**Step 3: Commit**

```bash
git add src/models/tor/queries.rs
git commit -m "feat(tor): find_functions uses fills_position for holder lookup"
```

**FAILURE CONDITIONS:**
- Still references `has_tor_role`
- `cargo check` fails

---

## Phase 3a — Meeting Protocol Templates

### Task 6: Create protocol model module

**GOAL:** New `src/models/protocol/` module with types and CRUD queries for protocol steps. Seed `protocol_of` relation type. `cargo check` passes.

**CONSTRAINTS:**
- Properties: `step_type`, `sequence_order`, `default_duration_minutes`, `description`, `is_required`
- Queries scoped to ToR via `protocol_of` relation
- Ordered by `sequence_order`
- `reorder_steps()` swaps two steps' sequence_order values

**FORMAT:**
- Modify: `src/db.rs` — seed `protocol_of` relation type
- Create: `src/models/protocol/types.rs`
- Create: `src/models/protocol/queries.rs`
- Create: `src/models/protocol/mod.rs`
- Modify: `src/models/mod.rs` — add `pub mod protocol`

**Step 1: Seed protocol_of relation type**

In `src/db.rs`, after the `_fills_position_id` line, add:

```rust
// --- Protocol relation types ---
let _protocol_of_id = insert_entity(&conn, "relation_type", "protocol_of", "Protocol Of", 0);
```

**Step 2: Create types.rs**

```rust
#[derive(Debug, Clone)]
pub struct ProtocolStep {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub step_type: String,       // "procedural", "agenda_slot", "fixed"
    pub sequence_order: i64,
    pub default_duration_minutes: Option<i64>,
    pub description: String,
    pub is_required: bool,
}
```

**Step 3: Create queries.rs**

```rust
use rusqlite::{Connection, params};
use super::types::*;

pub fn find_steps_for_tor(conn: &Connection, tor_id: i64) -> rusqlite::Result<Vec<ProtocolStep>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label, \
                COALESCE(p_type.value, 'procedural') AS step_type, \
                CAST(COALESCE(p_order.value, '0') AS INTEGER) AS sequence_order, \
                CASE WHEN p_dur.value IS NOT NULL THEN CAST(p_dur.value AS INTEGER) ELSE NULL END AS duration, \
                COALESCE(p_desc.value, '') AS description, \
                COALESCE(p_req.value, 'true') AS is_required \
         FROM entities e \
         JOIN relations r ON e.id = r.source_id \
         LEFT JOIN entity_properties p_type ON e.id = p_type.entity_id AND p_type.key = 'step_type' \
         LEFT JOIN entity_properties p_order ON e.id = p_order.entity_id AND p_order.key = 'sequence_order' \
         LEFT JOIN entity_properties p_dur ON e.id = p_dur.entity_id AND p_dur.key = 'default_duration_minutes' \
         LEFT JOIN entity_properties p_desc ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
         LEFT JOIN entity_properties p_req ON e.id = p_req.entity_id AND p_req.key = 'is_required' \
         WHERE r.target_id = ?1 \
           AND r.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'protocol_of') \
           AND e.entity_type = 'protocol_step' \
         ORDER BY CAST(COALESCE(p_order.value, '0') AS INTEGER)",
    )?;

    let steps = stmt
        .query_map(params![tor_id], |row| {
            Ok(ProtocolStep {
                id: row.get("id")?,
                name: row.get("name")?,
                label: row.get("label")?,
                step_type: row.get("step_type")?,
                sequence_order: row.get("sequence_order")?,
                default_duration_minutes: row.get("duration")?,
                description: row.get("description")?,
                is_required: row.get::<_, String>("is_required")? == "true",
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(steps)
}

pub fn create_step(
    conn: &Connection,
    tor_id: i64,
    name: &str,
    label: &str,
    step_type: &str,
    sequence_order: i64,
    default_duration_minutes: Option<i64>,
    description: &str,
    is_required: bool,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('protocol_step', ?1, ?2)",
        params![name, label],
    )?;
    let step_id = conn.last_insert_rowid();

    let props: Vec<(&str, String)> = vec![
        ("step_type", step_type.to_string()),
        ("sequence_order", sequence_order.to_string()),
        ("description", description.to_string()),
        ("is_required", if is_required { "true" } else { "false" }.to_string()),
    ];

    for (key, value) in &props {
        if !value.is_empty() {
            conn.execute(
                "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
                params![step_id, key, value],
            )?;
        }
    }

    if let Some(dur) = default_duration_minutes {
        conn.execute(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'default_duration_minutes', ?2)",
            params![step_id, dur.to_string()],
        )?;
    }

    // Link to ToR via protocol_of relation
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'protocol_of'), ?1, ?2)",
        params![step_id, tor_id],
    )?;

    Ok(step_id)
}

pub fn delete_step(conn: &Connection, step_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM entities WHERE id = ?1 AND entity_type = 'protocol_step'",
        params![step_id],
    )?;
    Ok(())
}

/// Swap sequence_order of two steps.
pub fn reorder_steps(conn: &Connection, step_a_id: i64, step_b_id: i64) -> rusqlite::Result<()> {
    let order_a: String = conn.query_row(
        "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = 'sequence_order'",
        params![step_a_id],
        |row| row.get(0),
    )?;
    let order_b: String = conn.query_row(
        "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = 'sequence_order'",
        params![step_b_id],
        |row| row.get(0),
    )?;

    conn.execute(
        "UPDATE entity_properties SET value = ?1 WHERE entity_id = ?2 AND key = 'sequence_order'",
        params![order_b, step_a_id],
    )?;
    conn.execute(
        "UPDATE entity_properties SET value = ?1 WHERE entity_id = ?2 AND key = 'sequence_order'",
        params![order_a, step_b_id],
    )?;

    Ok(())
}
```

**Step 4: Create mod.rs**

```rust
pub mod types;
pub mod queries;

pub use types::*;
pub use queries::*;
```

**Step 5: Register in models/mod.rs**

Add after `pub mod proposal;`:

```rust
pub mod protocol;
```

**Step 6: Verify**

```bash
cargo check 2>&1 | tail -5
```

**Step 7: Commit**

```bash
git add src/db.rs src/models/protocol/ src/models/mod.rs
git commit -m "feat(protocol): add protocol step model with CRUD queries"
```

**FAILURE CONDITIONS:**
- `protocol_of` relation type not seeded
- Steps not ordered by `sequence_order`
- Missing `reorder_steps()` function
- Module not registered
- `cargo check` fails

---

### Task 7: Protocol template management UI

**GOAL:** ToR detail page gets a "Meeting Protocol" section where users with `tor.edit` permission can view, add, reorder, and remove protocol steps. Verify: manual test — add 5 protocol steps, reorder them, delete one.

**CONSTRAINTS:**
- Section on existing ToR detail page
- Table: # | Label | Type | Duration | Required | Actions
- "Add Step" form inline on page
- Reorder via up/down POST actions
- Delete with confirmation (only non-required steps deletable)
- Permission gated: `tor.edit`
- CSRF on all mutations

**FORMAT:**
- Modify: `templates/tor/detail.html` — add Protocol section after Functions section
- Modify: `src/handlers/tor_handlers/crud.rs` — detail handler loads protocol steps
- Create: `src/handlers/tor_handlers/protocol.rs` — add_step, delete_step, move_step handlers
- Modify: `src/handlers/tor_handlers/mod.rs` — add `pub mod protocol; pub use protocol::*;`
- Modify: `src/main.rs` — wire protocol routes
- Modify: `src/templates_structs.rs` — add `protocol_steps: Vec<ProtocolStep>` to TorDetailTemplate

**Step 1: Add ProtocolStep import and field to TorDetailTemplate**

In `src/templates_structs.rs`, add to the imports:

```rust
use crate::models::protocol::ProtocolStep;
```

Add to `TorDetailTemplate`:

```rust
pub protocol_steps: Vec<ProtocolStep>,
```

**Step 2: Update detail handler to load protocol steps**

In `src/handlers/tor_handlers/crud.rs`, add import:

```rust
use crate::models::protocol;
```

In the `detail` function, after `let functions = tor::find_functions(...)`, add:

```rust
let protocol_steps = protocol::find_steps_for_tor(&conn, id)?;
```

And add `protocol_steps` to the TorDetailTemplate construction.

**Step 3: Create protocol handlers**

Create `src/handlers/tor_handlers/protocol.rs`:

```rust
use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::models::protocol;
use crate::auth::csrf;
use crate::auth::session::require_permission;
use crate::errors::AppError;

pub async fn add_step(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    let tor_id = path.into_inner();
    let conn = pool.get()?;

    let name = form.get("name").map(|s| s.as_str()).unwrap_or("");
    let label = form.get("label").map(|s| s.as_str()).unwrap_or("");
    let step_type = form.get("step_type").map(|s| s.as_str()).unwrap_or("procedural");
    let sequence_order: i64 = form.get("sequence_order")
        .and_then(|s| s.parse().ok())
        .unwrap_or(99);
    let duration: Option<i64> = form.get("default_duration_minutes")
        .and_then(|s| s.parse().ok());
    let description = form.get("description").map(|s| s.as_str()).unwrap_or("");
    let is_required = form.get("is_required").map(|s| s.as_str()) == Some("true");

    if name.trim().is_empty() || label.trim().is_empty() {
        let _ = session.insert("flash", "Name and label are required");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", format!("/tor/{tor_id}")))
            .finish());
    }

    protocol::create_step(&conn, tor_id, name.trim(), label.trim(), step_type,
                          sequence_order, duration, description, is_required)?;

    let _ = session.insert("flash", "Protocol step added");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}")))
        .finish())
}

pub async fn delete_step(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<crate::handlers::auth_handlers::CsrfOnly>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, step_id) = path.into_inner();
    let conn = pool.get()?;

    protocol::delete_step(&conn, step_id)?;

    let _ = session.insert("flash", "Protocol step removed");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}")))
        .finish())
}

pub async fn move_step(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    let (tor_id, step_id) = path.into_inner();
    let conn = pool.get()?;

    let swap_with: i64 = form.get("swap_with")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if swap_with > 0 {
        protocol::reorder_steps(&conn, step_id, swap_with)?;
    }

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}")))
        .finish())
}
```

**Step 4: Register in tor_handlers/mod.rs**

Add:

```rust
pub mod protocol;
pub use protocol::*;
```

**Step 5: Wire routes in main.rs**

After the `// ToR member management` line, add:

```rust
// ToR protocol management
.route("/tor/{id}/protocol", web::post().to(handlers::tor_handlers::add_step))
.route("/tor/{id}/protocol/{step_id}/delete", web::post().to(handlers::tor_handlers::delete_step))
.route("/tor/{id}/protocol/{step_id}/move", web::post().to(handlers::tor_handlers::move_step))
```

**Step 6: Add Protocol section to detail.html**

After the Functions `</section>`, add the protocol table HTML. (This is a large template block — the implementer should follow the same pattern as the Functions and Positions sections: table with rows, action forms for add/delete/reorder, permission-gated.)

**Step 7: Verify**

```bash
cargo build 2>&1 | tail -5
```

**Step 8: Manual test**

Reset DB, create a ToR, add protocol steps, verify ordering and delete.

**Step 9: Commit**

```bash
git add src/handlers/tor_handlers/protocol.rs src/handlers/tor_handlers/mod.rs \
        src/handlers/tor_handlers/crud.rs src/templates_structs.rs \
        src/main.rs templates/tor/detail.html
git commit -m "feat(protocol): protocol template management UI on ToR detail page"
```

**FAILURE CONDITIONS:**
- Protocol section missing from ToR detail page
- Can delete required steps
- Reorder doesn't persist
- Missing CSRF or permission check
- `cargo build` fails

---

## Phase 3c — Meeting Dependencies

### Task 8: Meeting dependency relations and UI

**GOAL:** Add `feeds_into` and `escalates_to` relation types between ToRs. ToR detail page shows Dependencies section. Verify: create 3 ToRs, add dependencies between them, see correct display.

**CONSTRAINTS:**
- `feeds_into` and `escalates_to` relation types seeded
- Relation properties: `output_types`, `description`, `is_blocking`
- Self-referencing prevented
- Permission gated: `tor.edit`
- CSRF on all mutations

**FORMAT:**
- Modify: `src/db.rs` — seed relation types
- Create: `src/models/tor/dependencies.rs`
- Modify: `src/models/tor/mod.rs`
- Modify: `templates/tor/detail.html` — Dependencies section
- Create: `src/handlers/tor_handlers/dependencies.rs`
- Modify: `src/handlers/tor_handlers/mod.rs`
- Modify: `src/main.rs`
- Modify: `src/templates_structs.rs`

This task follows the same pattern as Tasks 6-7 but for inter-ToR relations. The implementer should:

1. Seed `feeds_into` and `escalates_to` relation types in `src/db.rs`
2. Create `TorDependency` type with fields: `id`, `relation_type` (feeds_into/escalates_to), `other_tor_id`, `other_tor_label`, `output_types`, `description`, `is_blocking`
3. Write `find_upstream()`, `find_downstream()`, `add_dependency()`, `remove_dependency()` queries
4. Create handler with add/remove actions
5. Add Dependencies section to ToR detail template
6. Wire routes

**FAILURE CONDITIONS:**
- Self-referencing allowed
- Relation properties not stored
- Missing upstream or downstream direction
- Missing CSRF or permission check
- `cargo build` fails

---

## Phase 3d — Minutes Generation

### Task 9: Minutes model and auto-scaffold

**GOAL:** Add `minutes` and `minutes_section` entity types with `generate_minutes_scaffold()` function. Verify: `cargo check` passes.

**CONSTRAINTS:**
- `minutes_of` and `section_of` relation types seeded
- Scaffold generates: attendance, protocol, agenda_item, decision, action_item sections
- Each section has `is_auto_generated = true`
- Attendance flags vacant mandatory positions

**FORMAT:**
- Modify: `src/db.rs` — seed relation types
- Create: `src/models/minutes/types.rs`
- Create: `src/models/minutes/queries.rs`
- Create: `src/models/minutes/mod.rs`
- Modify: `src/models/mod.rs`

This follows the same module pattern. Key query: `generate_minutes_scaffold(conn, meeting_id, tor_id)` creates the minutes entity, then iterates meeting steps and agenda points to auto-generate sections.

**FAILURE CONDITIONS:**
- Scaffold missing any section type
- Vacant mandatory positions not flagged
- `is_auto_generated` not set
- `cargo check` fails

---

### Task 10: Minutes UI

**GOAL:** Minutes editing and approval workflow. Verify: generate minutes for a completed meeting, edit sections, approve.

**CONSTRAINTS:**
- "Generate Minutes" only on completed meetings without existing minutes
- Editable sections for users with `minutes.edit` permission
- Status transitions: draft → pending_approval → approved
- Audit logging on status changes
- CSRF on all mutations

**FORMAT:**
- Create: `templates/minutes/view.html`
- Create: `src/handlers/minutes_handlers/mod.rs`
- Create: `src/handlers/minutes_handlers/crud.rs`
- Modify: `src/handlers/mod.rs`
- Modify: `src/main.rs`
- Modify: `src/templates_structs.rs`

**FAILURE CONDITIONS:**
- Can generate minutes for non-completed meeting
- Approved minutes still editable
- Missing audit logging
- `cargo build` fails

---

## Phase 3e — Presentation Templates

### Task 11: Presentation template model

**GOAL:** Add `presentation_template` and `template_slide` entity types with CRUD queries. Verify: `cargo check` passes.

**CONSTRAINTS:**
- `template_of`, `slide_of`, `requires_template` relation types seeded
- Slides ordered by `slide_order`
- Templates scoped to ToR via `template_of`

**FORMAT:**
- Modify: `src/db.rs` — seed relation types
- Create: `src/models/presentation_template/types.rs`
- Create: `src/models/presentation_template/queries.rs`
- Create: `src/models/presentation_template/mod.rs`
- Modify: `src/models/mod.rs`

**FAILURE CONDITIONS:**
- Slides not ordered
- Templates not scoped to ToR
- `cargo check` fails

---

### Task 12: Presentation template management UI

**GOAL:** ToR admin gets template management. Agenda points can link templates. Verify: create template, link to agenda point, see requirements.

**CONSTRAINTS:**
- Template CRUD on ToR detail or sub-page
- Slide management: add, edit, reorder, delete
- `requires_template` relation links agenda point → template
- Permission gated: `tor.edit`

**FORMAT:**
- Create: `templates/tor/presentation_templates.html`
- Create: `src/handlers/tor_handlers/presentation.rs`
- Modify: `src/handlers/tor_handlers/mod.rs`
- Modify: `src/main.rs`

**FAILURE CONDITIONS:**
- Slides not manageable
- Template not linkable to agenda points
- `cargo build` fails

---

## Phase 3f — Governance Map

### Task 13: Governance map page

**GOAL:** Dedicated page at `/governance/map` showing all ToRs and their dependency relationships. Verify: 4 ToRs with various relations render correctly.

**CONSTRAINTS:**
- Route: `GET /governance/map`
- Table/matrix format
- Color-coded: blue for feeds_into, orange for escalates_to
- Click ToR name → navigate to detail
- Permission gated: `tor.list`
- Seed nav item

**FORMAT:**
- Create: `templates/governance/map.html`
- Create: `src/handlers/governance_handlers/map.rs`
- Create: `src/handlers/governance_handlers/mod.rs`
- Modify: `src/handlers/mod.rs`
- Modify: `src/main.rs`
- Modify: `src/db.rs` — seed nav item
- Modify: `src/templates_structs.rs`

**FAILURE CONDITIONS:**
- Missing either relationship type visualization
- Not clickable to detail
- `cargo build` fails

---

## Execution Order & Dependencies

```
Task 1 (seed) → Task 2 (types) → Task 3 (queries) → Task 4 (UI) → Task 5 (functions fix)
                                                                        ↓
Task 6 (protocol model) → Task 7 (protocol UI)
                                                                        ↓
Task 8 (dependencies) ────────────────────────────────→ Task 13 (governance map)
                                                                        ↓
Task 9 (minutes model) → Task 10 (minutes UI)
                                                                        ↓
Task 11 (presentation model) → Task 12 (presentation UI)
```

Tasks 1-5 must be sequential (each depends on previous). After Task 5, Tasks 6-7, 8, 9-10, 11-12 can be done in any order. Task 13 depends on Task 8.
