# Phase 2b: Agenda Points, COAs & Data-Driven Workflows — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add agenda points, courses of action (COAs), opinion recording, decision making, and a data-driven workflow engine to the governance system. Rename "Pipeline" to "Workflow" throughout.

**Architecture:** EAV-based entities for all new types (agenda_point, coa, coa_section, opinion, workflow_status, workflow_transition). New `relation_properties` table extends EAV to relations. Workflow engine replaces hardcoded status transitions for all entity types. Existing patterns: AppError, render() helper, PageContext, session helpers, entity/relation helpers.

**Tech Stack:** Rust, Actix-web 4, Askama 0.14, SQLite (rusqlite + r2d2), serde_json

**Design doc:** `docs/plans/2026-02-14-phase2b-agenda-points-design.md`

---

## Context for Implementer

### Key Files to Understand

- `src/db.rs` — Database schema (MIGRATIONS const) + seed_ontology() function
- `src/models/entity.rs` — Entity CRUD helpers (create, set_property, get_property, etc.)
- `src/models/relation.rs` — Relation helpers (create, find_targets, find_sources, delete)
- `src/models/suggestion/` — Model pattern: types.rs (structs) + queries.rs (DB functions) + mod.rs
- `src/models/proposal/` — Same pattern, more complex (auto_create_from_suggestion)
- `src/handlers/pipeline_handlers.rs` — Pipeline view handler pattern
- `src/handlers/suggestion_handlers.rs` — CRUD + status workflow handler pattern
- `src/handlers/proposal_handlers.rs` — Full CRUD + multi-status workflow pattern
- `src/templates_structs.rs` — All Askama template structs with PageContext
- `templates/pipeline/view.html` — Tab-based view with suggestions/proposals tables
- `src/errors.rs` — AppError enum + render() helper
- `src/auth/session.rs` — require_permission(), get_user_id(), Permissions struct

### Patterns to Follow

**Handler pattern:** All handlers return `Result<HttpResponse, AppError>`. Start with `require_permission()`, then `get_user_id()`, then `pool.get()`, then `require_tor_membership()`, then business logic, then `render(tmpl)` or redirect.

**Model pattern:** Each model has `types.rs` (Serialize/Deserialize structs), `queries.rs` (DB functions using entity/relation helpers), `mod.rs` (pub use).

**Entity creation:** Use `entity::create()` + `entity::set_property()` for each property. Use `relation::create()` for relations. All wrapped in `Result<_, AppError>`.

**Template pattern:** Struct with `#[derive(Template)]` and `#[template(path = "...")]`. Must carry `ctx: PageContext` field. Use `.as_str()` for string comparisons in Askama.

**Seed pattern:** In `seed_ontology()` in `src/db.rs`, use `insert_entity()` + `insert_prop()` + `insert_relation()` helpers.

---

## Task 1: Database Migration — relation_properties Table

**Files:**
- Modify: `src/db.rs` (MIGRATIONS const, around line 7-40)

**What to do:**

Add `relation_properties` table to the MIGRATIONS const, after the existing `relations` table:

```sql
CREATE TABLE IF NOT EXISTS relation_properties (
    relation_id INTEGER NOT NULL REFERENCES relations(id) ON DELETE CASCADE,
    key         TEXT NOT NULL,
    value       TEXT NOT NULL,
    PRIMARY KEY (relation_id, key)
);

CREATE INDEX IF NOT EXISTS idx_relation_properties ON relation_properties(relation_id);
```

Also add a helper module for relation properties.

**Files:**
- Modify: `src/models/relation.rs` — Add relation property helpers

Add these functions to `src/models/relation.rs`:

```rust
/// Create a relation and return its id.
pub fn create_with_id(conn: &Connection, relation_type_name: &str, source_id: i64, target_id: i64) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = ?1), ?2, ?3)",
        params![relation_type_name, source_id, target_id],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Get a property value from a relation.
pub fn get_relation_property(conn: &Connection, relation_id: i64, key: &str) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT value FROM relation_properties WHERE relation_id = ?1 AND key = ?2"
    )?;
    let mut rows = stmt.query_map(params![relation_id, key], |row| row.get::<_, String>(0))?;
    match rows.next() {
        Some(val) => Ok(Some(val?)),
        None => Ok(None),
    }
}

/// Set a relation property (upsert).
pub fn set_relation_property(conn: &Connection, relation_id: i64, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO relation_properties (relation_id, key, value) VALUES (?1, ?2, ?3) \
         ON CONFLICT(relation_id, key) DO UPDATE SET value = excluded.value",
        params![relation_id, key, value],
    )?;
    Ok(())
}

/// Find a relation id between two entities of a given type.
pub fn find_relation_id(conn: &Connection, relation_type_name: &str, source_id: i64, target_id: i64) -> rusqlite::Result<Option<i64>> {
    let mut stmt = conn.prepare(
        "SELECT r.id FROM relations r \
         WHERE r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = ?1) \
         AND r.source_id = ?2 AND r.target_id = ?3"
    )?;
    let mut rows = stmt.query_map(params![relation_type_name, source_id, target_id], |row| row.get::<_, i64>(0))?;
    match rows.next() {
        Some(val) => Ok(Some(val?)),
        None => Ok(None),
    }
}
```

**Verify:** `cargo check` — should compile with 0 errors.

**Commit:** `git commit -m "feat(db): add relation_properties table and helpers"`

---

## Task 2: Seed Data — New Relation Types, Permissions, Nav Rename

**Files:**
- Modify: `src/db.rs` (seed_ontology function)

**What to do:**

In `seed_ontology()`, after the existing item pipeline relation types (around line 112):

1. Add new relation types for Phase 2b:

```rust
// --- Phase 2b relation types ---
let _transition_from_id = insert_entity(&conn, "relation_type", "transition_from", "Transition From", 0);
let _transition_to_id = insert_entity(&conn, "relation_type", "transition_to", "Transition To", 0);
let _considers_coa_id = insert_entity(&conn, "relation_type", "considers_coa", "Considers COA", 0);
let _originates_from_id = insert_entity(&conn, "relation_type", "originates_from", "Originates From", 0);
let _has_section_id = insert_entity(&conn, "relation_type", "has_section", "Has Section", 0);
let _has_subsection_id = insert_entity(&conn, "relation_type", "has_subsection", "Has Subsection", 0);
let _agenda_submitted_to_id = insert_entity(&conn, "relation_type", "agenda_submitted_to", "Agenda Submitted To", 0);
let _spawns_agenda_point_id = insert_entity(&conn, "relation_type", "spawns_agenda_point", "Spawns Agenda Point", 0);
let _opinion_by_id = insert_entity(&conn, "relation_type", "opinion_by", "Opinion By", 0);
let _opinion_on_id = insert_entity(&conn, "relation_type", "opinion_on", "Opinion On", 0);
let _prefers_coa_id = insert_entity(&conn, "relation_type", "prefers_coa", "Prefers COA", 0);
let _presents_id = insert_entity(&conn, "relation_type", "presents", "Presents", 0);
```

2. Add new permissions to the `perms` array (after the existing Pipeline permissions):

```rust
// Phase 2b - Agenda permissions
("agenda.view", "View agenda points in ToR workflow", "Workflow"),
("agenda.create", "Create agenda points", "Workflow"),
("agenda.queue", "Mark proposals as ready for agenda", "Workflow"),
("agenda.manage", "Progress agenda point status", "Workflow"),
("agenda.participate", "Record opinions on agenda items", "Workflow"),
("agenda.decide", "Record final decisions", "Workflow"),
("coa.create", "Create courses of action", "Workflow"),
("coa.edit", "Edit COA content and sections", "Workflow"),
("workflow.manage", "Manage workflow definitions", "Workflow"),
```

3. Rename the nav item from "Item Pipeline" to "Item Workflow" and update the URL. Find the line with `governance.pipeline` (around line 318) and change:

```rust
// OLD:
let nav_gov_pipeline_id = insert_entity(&conn, "nav_item", "governance.pipeline", "Item Pipeline", 2);
insert_prop(&conn, nav_gov_pipeline_id, "url", "/pipeline");

// NEW:
let nav_gov_workflow_id = insert_entity(&conn, "nav_item", "governance.workflow", "Item Workflow", 2);
insert_prop(&conn, nav_gov_workflow_id, "url", "/workflow");
```

Also update the permission relation to use the new nav item variable name, and update the log message at the end to reflect the new counts.

**Verify:** `cargo check` — should compile with 0 errors.

**Commit:** `git commit -m "feat(seed): add Phase 2b relation types, permissions, and rename pipeline to workflow"`

---

## Task 3: Workflow Engine Model — Types + Queries

**Files:**
- Create: `src/models/workflow/mod.rs`
- Create: `src/models/workflow/types.rs`
- Create: `src/models/workflow/queries.rs`
- Modify: `src/models/mod.rs` — Add `pub mod workflow;`

**types.rs:**

```rust
use serde::{Deserialize, Serialize};

/// A workflow status (state) for a given entity type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStatus {
    pub id: i64,
    pub entity_type_scope: String,
    pub status_code: String,
    pub label: String,
    pub is_initial: bool,
    pub is_terminal: bool,
}

/// A valid transition between two statuses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTransition {
    pub id: i64,
    pub from_status_code: String,
    pub to_status_code: String,
    pub required_permission: String,
    pub condition: Option<String>,
    pub requires_outcome: bool,
    pub transition_label: String,
}

/// Information about an available transition for UI rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableTransition {
    pub to_status_code: String,
    pub transition_label: String,
    pub requires_outcome: bool,
}
```

**queries.rs:**

```rust
use rusqlite::{Connection, params};
use crate::errors::AppError;
use crate::auth::session::Permissions;
use super::types::*;

/// Find all workflow statuses for a given entity type scope.
pub fn find_statuses_for_type(conn: &Connection, entity_type_scope: &str) -> Result<Vec<WorkflowStatus>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.label, \
                COALESCE(p_scope.value, '') AS entity_type_scope, \
                COALESCE(p_code.value, '') AS status_code, \
                COALESCE(p_initial.value, 'false') AS is_initial, \
                COALESCE(p_terminal.value, 'false') AS is_terminal \
         FROM entities e \
         LEFT JOIN entity_properties p_scope ON e.id = p_scope.entity_id AND p_scope.key = 'entity_type_scope' \
         LEFT JOIN entity_properties p_code ON e.id = p_code.entity_id AND p_code.key = 'status_code' \
         LEFT JOIN entity_properties p_initial ON e.id = p_initial.entity_id AND p_initial.key = 'is_initial' \
         LEFT JOIN entity_properties p_terminal ON e.id = p_terminal.entity_id AND p_terminal.key = 'is_terminal' \
         WHERE e.entity_type = 'workflow_status' \
           AND p_scope.value = ?1 \
         ORDER BY e.sort_order, e.id"
    )?;

    let items = stmt.query_map(params![entity_type_scope], |row| {
        let is_initial_str: String = row.get("is_initial")?;
        let is_terminal_str: String = row.get("is_terminal")?;
        Ok(WorkflowStatus {
            id: row.get("id")?,
            entity_type_scope: row.get("entity_type_scope")?,
            status_code: row.get("status_code")?,
            label: row.get("label")?,
            is_initial: is_initial_str == "true",
            is_terminal: is_terminal_str == "true",
        })
    })?.collect::<Result<Vec<_>, _>>()?;

    Ok(items)
}

/// Get the initial status code for an entity type.
pub fn get_initial_status(conn: &Connection, entity_type_scope: &str) -> Result<String, AppError> {
    let statuses = find_statuses_for_type(conn, entity_type_scope)?;
    statuses.into_iter()
        .find(|s| s.is_initial)
        .map(|s| s.status_code)
        .ok_or_else(|| AppError::Session(format!("No initial workflow status for {}", entity_type_scope)))
}

/// Get the label for a status code of a given entity type.
pub fn get_status_label(conn: &Connection, entity_type_scope: &str, status_code: &str) -> Result<String, AppError> {
    let statuses = find_statuses_for_type(conn, entity_type_scope)?;
    statuses.into_iter()
        .find(|s| s.status_code == status_code)
        .map(|s| s.label)
        .ok_or_else(|| AppError::Session(format!("Unknown status '{}' for {}", status_code, entity_type_scope)))
}

/// Find all available transitions from the current status,
/// filtered by user permissions and entity properties (conditions).
pub fn find_available_transitions(
    conn: &Connection,
    entity_type_scope: &str,
    current_status: &str,
    user_permissions: &Permissions,
    entity_properties: &std::collections::HashMap<String, String>,
) -> Result<Vec<AvailableTransition>, AppError> {
    // Find all transitions where transition_from matches current_status and entity_type_scope
    let mut stmt = conn.prepare(
        "SELECT t.id, t.label AS transition_label, \
                COALESCE(p_perm.value, '') AS required_permission, \
                p_cond.value AS condition, \
                COALESCE(p_outcome.value, 'false') AS requires_outcome, \
                COALESCE(p_to_code.value, '') AS to_status_code \
         FROM entities t \
         JOIN relations r_from ON t.id = r_from.source_id \
         JOIN entities rt_from ON r_from.relation_type_id = rt_from.id AND rt_from.name = 'transition_from' \
         JOIN entities s_from ON r_from.target_id = s_from.id \
         JOIN entity_properties sp_from_code ON s_from.id = sp_from_code.entity_id AND sp_from_code.key = 'status_code' \
         JOIN entity_properties sp_from_scope ON s_from.id = sp_from_scope.entity_id AND sp_from_scope.key = 'entity_type_scope' \
         JOIN relations r_to ON t.id = r_to.source_id \
         JOIN entities rt_to ON r_to.relation_type_id = rt_to.id AND rt_to.name = 'transition_to' \
         JOIN entities s_to ON r_to.target_id = s_to.id \
         JOIN entity_properties p_to_code ON s_to.id = p_to_code.entity_id AND p_to_code.key = 'status_code' \
         LEFT JOIN entity_properties p_perm ON t.id = p_perm.entity_id AND p_perm.key = 'required_permission' \
         LEFT JOIN entity_properties p_cond ON t.id = p_cond.entity_id AND p_cond.key = 'condition' \
         LEFT JOIN entity_properties p_outcome ON t.id = p_outcome.entity_id AND p_outcome.key = 'requires_outcome' \
         WHERE t.entity_type = 'workflow_transition' \
           AND sp_from_code.value = ?1 \
           AND sp_from_scope.value = ?2"
    )?;

    let all_transitions = stmt.query_map(params![current_status, entity_type_scope], |row| {
        let requires_outcome_str: String = row.get("requires_outcome")?;
        Ok((
            row.get::<_, String>("required_permission")?,
            row.get::<_, Option<String>>("condition")?,
            AvailableTransition {
                to_status_code: row.get("to_status_code")?,
                transition_label: row.get("transition_label")?,
                requires_outcome: requires_outcome_str == "true",
            },
        ))
    })?.collect::<Result<Vec<_>, _>>()?;

    // Filter by permission and condition
    let mut available = Vec::new();
    for (required_perm, condition, transition) in all_transitions {
        // Check permission
        if !required_perm.is_empty() && !user_permissions.has(&required_perm) {
            continue;
        }

        // Check condition (format: "key=value")
        if let Some(cond) = &condition {
            if let Some((key, value)) = cond.split_once('=') {
                let actual = entity_properties.get(key).map(|s| s.as_str()).unwrap_or("");
                if actual != value {
                    continue;
                }
            }
        }

        available.push(transition);
    }

    Ok(available)
}

/// Validate a specific transition and return its info.
/// Returns error if transition is not valid or user lacks permission.
pub fn validate_transition(
    conn: &Connection,
    entity_type_scope: &str,
    current_status: &str,
    new_status: &str,
    user_permissions: &Permissions,
    entity_properties: &std::collections::HashMap<String, String>,
) -> Result<AvailableTransition, AppError> {
    let available = find_available_transitions(
        conn, entity_type_scope, current_status, user_permissions, entity_properties,
    )?;

    available.into_iter()
        .find(|t| t.to_status_code == new_status)
        .ok_or_else(|| AppError::PermissionDenied(
            format!("Invalid or unauthorized transition: {} -> {} for {}", current_status, new_status, entity_type_scope)
        ))
}
```

**mod.rs:**

```rust
pub mod types;
pub mod queries;

pub use types::*;
pub use queries::*;
```

**Verify:** `cargo check` — should compile with 0 errors (warnings OK).

**Commit:** `git commit -m "feat: add data-driven workflow engine model"`

---

## Task 4: Seed Data — Workflow Status + Transition Entities

**Files:**
- Modify: `src/db.rs` (seed_ontology function, after permissions section)

**What to do:**

After the permissions and nav sections in `seed_ontology()`, add workflow status and transition entities. This seeds the complete workflow definitions for suggestion, proposal, and agenda_point entity types.

Add a new section in seed_ontology() with a helper approach. The key pattern is:
1. Create `workflow_status` entities with properties (entity_type_scope, status_code, is_initial, is_terminal)
2. Create `workflow_transition` entities with properties (required_permission, condition, requires_outcome, transition_label)
3. Create `transition_from` and `transition_to` relations linking transitions to statuses

```rust
// --- Workflow Status + Transition entities ---

// Get transition relation type IDs
let transition_from_rel_id: i64 = conn.query_row(
    "SELECT id FROM entities WHERE entity_type='relation_type' AND name='transition_from'",
    [], |row| row.get(0),
).unwrap();
let transition_to_rel_id: i64 = conn.query_row(
    "SELECT id FROM entities WHERE entity_type='relation_type' AND name='transition_to'",
    [], |row| row.get(0),
).unwrap();

// --- Suggestion workflow ---
let s_open = insert_entity(&conn, "workflow_status", "suggestion.open", "Open", 1);
insert_prop(&conn, s_open, "entity_type_scope", "suggestion");
insert_prop(&conn, s_open, "status_code", "open");
insert_prop(&conn, s_open, "is_initial", "true");
insert_prop(&conn, s_open, "is_terminal", "false");

let s_accepted = insert_entity(&conn, "workflow_status", "suggestion.accepted", "Accepted", 2);
insert_prop(&conn, s_accepted, "entity_type_scope", "suggestion");
insert_prop(&conn, s_accepted, "status_code", "accepted");
insert_prop(&conn, s_accepted, "is_initial", "false");
insert_prop(&conn, s_accepted, "is_terminal", "true");

let s_rejected = insert_entity(&conn, "workflow_status", "suggestion.rejected", "Rejected", 3);
insert_prop(&conn, s_rejected, "entity_type_scope", "suggestion");
insert_prop(&conn, s_rejected, "status_code", "rejected");
insert_prop(&conn, s_rejected, "is_initial", "false");
insert_prop(&conn, s_rejected, "is_terminal", "true");

// Suggestion transitions
let st_accept = insert_entity(&conn, "workflow_transition", "suggestion.open_to_accepted", "Accept", 0);
insert_prop(&conn, st_accept, "required_permission", "suggestion.review");
insert_prop(&conn, st_accept, "requires_outcome", "false");
insert_prop(&conn, st_accept, "transition_label", "Accept");
insert_relation(&conn, transition_from_rel_id, st_accept, s_open);
insert_relation(&conn, transition_to_rel_id, st_accept, s_accepted);

let st_reject = insert_entity(&conn, "workflow_transition", "suggestion.open_to_rejected", "Reject", 0);
insert_prop(&conn, st_reject, "required_permission", "suggestion.review");
insert_prop(&conn, st_reject, "requires_outcome", "false");
insert_prop(&conn, st_reject, "transition_label", "Reject");
insert_relation(&conn, transition_from_rel_id, st_reject, s_open);
insert_relation(&conn, transition_to_rel_id, st_reject, s_rejected);

// --- Proposal workflow ---
let p_draft = insert_entity(&conn, "workflow_status", "proposal.draft", "Draft", 1);
insert_prop(&conn, p_draft, "entity_type_scope", "proposal");
insert_prop(&conn, p_draft, "status_code", "draft");
insert_prop(&conn, p_draft, "is_initial", "true");
insert_prop(&conn, p_draft, "is_terminal", "false");

let p_submitted = insert_entity(&conn, "workflow_status", "proposal.submitted", "Submitted", 2);
insert_prop(&conn, p_submitted, "entity_type_scope", "proposal");
insert_prop(&conn, p_submitted, "status_code", "submitted");
insert_prop(&conn, p_submitted, "is_initial", "false");
insert_prop(&conn, p_submitted, "is_terminal", "false");

let p_under_review = insert_entity(&conn, "workflow_status", "proposal.under_review", "Under Review", 3);
insert_prop(&conn, p_under_review, "entity_type_scope", "proposal");
insert_prop(&conn, p_under_review, "status_code", "under_review");
insert_prop(&conn, p_under_review, "is_initial", "false");
insert_prop(&conn, p_under_review, "is_terminal", "false");

let p_approved = insert_entity(&conn, "workflow_status", "proposal.approved", "Approved", 4);
insert_prop(&conn, p_approved, "entity_type_scope", "proposal");
insert_prop(&conn, p_approved, "status_code", "approved");
insert_prop(&conn, p_approved, "is_initial", "false");
insert_prop(&conn, p_approved, "is_terminal", "true");

let p_rejected = insert_entity(&conn, "workflow_status", "proposal.rejected", "Rejected", 5);
insert_prop(&conn, p_rejected, "entity_type_scope", "proposal");
insert_prop(&conn, p_rejected, "status_code", "rejected");
insert_prop(&conn, p_rejected, "is_initial", "false");
insert_prop(&conn, p_rejected, "is_terminal", "false");

// Proposal transitions
let pt_submit = insert_entity(&conn, "workflow_transition", "proposal.draft_to_submitted", "Submit", 0);
insert_prop(&conn, pt_submit, "required_permission", "proposal.submit");
insert_prop(&conn, pt_submit, "requires_outcome", "false");
insert_prop(&conn, pt_submit, "transition_label", "Submit");
insert_relation(&conn, transition_from_rel_id, pt_submit, p_draft);
insert_relation(&conn, transition_to_rel_id, pt_submit, p_submitted);

let pt_review = insert_entity(&conn, "workflow_transition", "proposal.submitted_to_under_review", "Start Review", 0);
insert_prop(&conn, pt_review, "required_permission", "proposal.review");
insert_prop(&conn, pt_review, "requires_outcome", "false");
insert_prop(&conn, pt_review, "transition_label", "Start Review");
insert_relation(&conn, transition_from_rel_id, pt_review, p_submitted);
insert_relation(&conn, transition_to_rel_id, pt_review, p_under_review);

let pt_approve = insert_entity(&conn, "workflow_transition", "proposal.under_review_to_approved", "Approve", 0);
insert_prop(&conn, pt_approve, "required_permission", "proposal.approve");
insert_prop(&conn, pt_approve, "requires_outcome", "false");
insert_prop(&conn, pt_approve, "transition_label", "Approve");
insert_relation(&conn, transition_from_rel_id, pt_approve, p_under_review);
insert_relation(&conn, transition_to_rel_id, pt_approve, p_approved);

let pt_reject = insert_entity(&conn, "workflow_transition", "proposal.under_review_to_rejected", "Reject", 0);
insert_prop(&conn, pt_reject, "required_permission", "proposal.approve");
insert_prop(&conn, pt_reject, "requires_outcome", "false");
insert_prop(&conn, pt_reject, "transition_label", "Reject");
insert_relation(&conn, transition_from_rel_id, pt_reject, p_under_review);
insert_relation(&conn, transition_to_rel_id, pt_reject, p_rejected);

let pt_resubmit = insert_entity(&conn, "workflow_transition", "proposal.rejected_to_submitted", "Resubmit", 0);
insert_prop(&conn, pt_resubmit, "required_permission", "proposal.submit");
insert_prop(&conn, pt_resubmit, "requires_outcome", "false");
insert_prop(&conn, pt_resubmit, "transition_label", "Resubmit");
insert_relation(&conn, transition_from_rel_id, pt_resubmit, p_rejected);
insert_relation(&conn, transition_to_rel_id, pt_resubmit, p_submitted);

// --- Agenda Point workflow ---
let a_scheduled = insert_entity(&conn, "workflow_status", "agenda_point.scheduled", "Scheduled", 1);
insert_prop(&conn, a_scheduled, "entity_type_scope", "agenda_point");
insert_prop(&conn, a_scheduled, "status_code", "scheduled");
insert_prop(&conn, a_scheduled, "is_initial", "true");
insert_prop(&conn, a_scheduled, "is_terminal", "false");

let a_in_progress = insert_entity(&conn, "workflow_status", "agenda_point.in_progress", "In Progress", 2);
insert_prop(&conn, a_in_progress, "entity_type_scope", "agenda_point");
insert_prop(&conn, a_in_progress, "status_code", "in_progress");
insert_prop(&conn, a_in_progress, "is_initial", "false");
insert_prop(&conn, a_in_progress, "is_terminal", "false");

let a_voted = insert_entity(&conn, "workflow_status", "agenda_point.voted", "Voted", 3);
insert_prop(&conn, a_voted, "entity_type_scope", "agenda_point");
insert_prop(&conn, a_voted, "status_code", "voted");
insert_prop(&conn, a_voted, "is_initial", "false");
insert_prop(&conn, a_voted, "is_terminal", "false");

let a_completed = insert_entity(&conn, "workflow_status", "agenda_point.completed", "Completed", 4);
insert_prop(&conn, a_completed, "entity_type_scope", "agenda_point");
insert_prop(&conn, a_completed, "status_code", "completed");
insert_prop(&conn, a_completed, "is_initial", "false");
insert_prop(&conn, a_completed, "is_terminal", "true");

// Agenda Point transitions
let at_start = insert_entity(&conn, "workflow_transition", "agenda_point.scheduled_to_in_progress", "Start Discussion", 0);
insert_prop(&conn, at_start, "required_permission", "agenda.manage");
insert_prop(&conn, at_start, "requires_outcome", "false");
insert_prop(&conn, at_start, "transition_label", "Start Discussion");
insert_relation(&conn, transition_from_rel_id, at_start, a_scheduled);
insert_relation(&conn, transition_to_rel_id, at_start, a_in_progress);

let at_record_opinions = insert_entity(&conn, "workflow_transition", "agenda_point.in_progress_to_voted", "Record Opinions", 0);
insert_prop(&conn, at_record_opinions, "required_permission", "agenda.manage");
insert_prop(&conn, at_record_opinions, "condition", "item_type=decision");
insert_prop(&conn, at_record_opinions, "requires_outcome", "false");
insert_prop(&conn, at_record_opinions, "transition_label", "Record Opinions");
insert_relation(&conn, transition_from_rel_id, at_record_opinions, a_in_progress);
insert_relation(&conn, transition_to_rel_id, at_record_opinions, a_voted);

let at_complete_info = insert_entity(&conn, "workflow_transition", "agenda_point.in_progress_to_completed", "Complete", 0);
insert_prop(&conn, at_complete_info, "required_permission", "agenda.manage");
insert_prop(&conn, at_complete_info, "condition", "item_type=informative");
insert_prop(&conn, at_complete_info, "requires_outcome", "true");
insert_prop(&conn, at_complete_info, "transition_label", "Complete");
insert_relation(&conn, transition_from_rel_id, at_complete_info, a_in_progress);
insert_relation(&conn, transition_to_rel_id, at_complete_info, a_completed);

let at_decide = insert_entity(&conn, "workflow_transition", "agenda_point.voted_to_completed", "Record Decision", 0);
insert_prop(&conn, at_decide, "required_permission", "agenda.decide");
insert_prop(&conn, at_decide, "requires_outcome", "true");
insert_prop(&conn, at_decide, "transition_label", "Record Decision");
insert_relation(&conn, transition_from_rel_id, at_decide, a_voted);
insert_relation(&conn, transition_to_rel_id, at_decide, a_completed);
```

**Verify:** Delete `data/app.db`, run `cargo run`, confirm server starts and seeding completes without errors. Check log for correct entity counts.

**Commit:** `git commit -m "feat(seed): add workflow status and transition entities for all entity types"`

---

## Task 5: Rename Pipeline → Workflow Throughout

**Files:**
- Rename: `templates/pipeline/view.html` → `templates/workflow/view.html`
- Rename: `src/handlers/pipeline_handlers.rs` → `src/handlers/workflow_handlers.rs`
- Modify: `src/handlers/mod.rs` — Change `pub mod pipeline_handlers` to `pub mod workflow_handlers`
- Modify: `src/handlers/suggestion_handlers.rs` — Update all redirect URLs from `/pipeline` to `/workflow`
- Modify: `src/handlers/proposal_handlers.rs` — Update all redirect URLs from `/pipeline` to `/workflow`, update `PageContext::build` paths
- Modify: `src/handlers/workflow_handlers.rs` (was pipeline) — Update struct name, import path, redirect URLs
- Modify: `src/templates_structs.rs` — Rename `PipelineTemplate` to `WorkflowTemplate`, update template path
- Modify: `src/main.rs` — Update all `/pipeline` routes to `/workflow`, update handler module reference
- Modify: `templates/workflow/view.html` — Update all `/pipeline` URLs to `/workflow`, rename heading

**Key changes in the view template:**
- Replace all `/tor/{{ tor_id }}/pipeline` with `/tor/{{ tor_id }}/workflow`
- Change heading from "Item Pipeline" to "Item Workflow"
- Change disabled tab from "Agenda Points (Phase 3)" to active link

**Key changes in handlers:**
- All redirect URLs: `/tor/{tor_id}/pipeline?tab=...` → `/tor/{tor_id}/workflow?tab=...`
- PageContext::build paths: `"/pipeline"` → `"/workflow"`

**Key changes in main.rs:**
- `.route("/tor/{id}/pipeline", ...)` → `.route("/tor/{id}/workflow", ...)`
- `handlers::pipeline_handlers::view` → `handlers::workflow_handlers::view`

**Verify:** `cargo check` — should compile. Delete `data/app.db` and test that `/tor/{id}/workflow` loads correctly.

**Commit:** `git commit -m "refactor: rename pipeline to workflow throughout application"`

---

## Task 6: Agenda Point Model — Types + Queries

**Files:**
- Create: `src/models/agenda_point/mod.rs`
- Create: `src/models/agenda_point/types.rs`
- Create: `src/models/agenda_point/queries.rs`
- Modify: `src/models/mod.rs` — Add `pub mod agenda_point;`

Follow the same pattern as `src/models/proposal/` but for agenda_point entities.

**types.rs:** Define `AgendaPointListItem` (id, title, item_type, scheduled_date, scheduled_order, status, presenter_name, time_allocation_minutes, created_from_proposal_id), `AgendaPointDetail` (full details including description, outcome_summary, decided_by, decision_date, selected_coa_id, created_by_name), `AgendaPointForm` (title, description, item_type, scheduled_date, scheduled_order, presenter_id, time_allocation_minutes, csrf_token).

**queries.rs:** Implement `find_all_for_tor()`, `find_by_id()`, `create()`, `update()`, `update_status()`. Use `agenda_submitted_to` relation to link to ToR. Use `entity::create()` and `entity::set_property()` helpers. `find_all_for_tor` queries via `agenda_submitted_to` relation (same pattern as proposal's `submitted_to`).

**Verify:** `cargo check`

**Commit:** `git commit -m "feat: add agenda point model — types and queries"`

---

## Task 7: COA Model — Types + Queries

**Files:**
- Create: `src/models/coa/mod.rs`
- Create: `src/models/coa/types.rs`
- Create: `src/models/coa/queries.rs`
- Modify: `src/models/mod.rs` — Add `pub mod coa;`

**types.rs:** Define `CoaListItem` (id, title, description, coa_type, coa_order, created_by_name), `CoaDetail` (full details), `CoaSectionItem` (id, section_number, section_title, content, section_order, children: Vec<CoaSectionItem>), `CoaForm`, `CoaSectionForm`.

**queries.rs:** Implement `find_all_for_agenda_point()` (via `considers_coa` relation), `find_by_id()`, `create()`, `update()`, `delete()`. For sections: `find_sections_for_coa()` (via `has_section` relation, with recursive `has_subsection` resolution to build tree), `create_section()`, `update_section()`, `delete_section()`. Use `originates_from` relation for COA-to-proposal traceability.

**Verify:** `cargo check`

**Commit:** `git commit -m "feat: add COA model — types and queries with nested sections"`

---

## Task 8: Opinion Model — Types + Queries

**Files:**
- Create: `src/models/opinion/mod.rs`
- Create: `src/models/opinion/types.rs`
- Create: `src/models/opinion/queries.rs`
- Modify: `src/models/mod.rs` — Add `pub mod opinion;`

**types.rs:** Define `OpinionListItem` (id, user_name, preferred_coa_id, preferred_coa_title, comment, recorded_date), `OpinionSummary` (coa_id, coa_title, preference_count, opinions: Vec<OpinionListItem>), `OpinionForm` (preferred_coa_id, comment, csrf_token).

**queries.rs:** Implement `find_all_for_agenda_point()` (via `opinion_on` relation), `find_by_user_and_agenda_point()` (check if user already recorded opinion), `create()`, `update()`. Create uses `opinion_by`, `opinion_on`, and optionally `prefers_coa` relations. `get_opinion_summary()` aggregates by COA.

**Verify:** `cargo check`

**Commit:** `git commit -m "feat: add opinion model — types and queries"`

---

## Task 9: Proposal Queue Enhancement

**Files:**
- Modify: `src/models/proposal/queries.rs` — Add `find_queued_for_tor()` and `set_ready_for_agenda()`
- Modify: `src/models/proposal/types.rs` — Add `ready_for_agenda` field to `ProposalListItem`

**queries.rs additions:**

```rust
/// Find all approved proposals for a ToR that are marked ready for agenda.
pub fn find_queued_for_tor(conn: &Connection, tor_id: i64) -> Result<Vec<ProposalListItem>, AppError> {
    // Same query as find_all_for_tor but with additional filter:
    // AND p_status.value = 'approved' AND p_ready.value = 'true'
    // LEFT JOIN entity_properties p_ready ON e.id = p_ready.entity_id AND p_ready.key = 'ready_for_agenda'
}

/// Mark/unmark a proposal as ready for agenda.
pub fn set_ready_for_agenda(conn: &Connection, proposal_id: i64, ready: bool) -> Result<(), AppError> {
    entity::set_property(conn, proposal_id, "ready_for_agenda", if ready { "true" } else { "false" })?;
    Ok(())
}
```

**Verify:** `cargo check`

**Commit:** `git commit -m "feat: add proposal queue — ready_for_agenda flag and queries"`

---

## Task 10: Template Structs for All New Pages

**Files:**
- Modify: `src/templates_structs.rs`

Add new template structs. Import the new model types at the top:

```rust
use crate::models::agenda_point::{AgendaPointListItem, AgendaPointDetail};
use crate::models::coa::{CoaListItem, CoaDetail, CoaSectionItem};
use crate::models::opinion::{OpinionListItem, OpinionSummary};
use crate::models::workflow::AvailableTransition;
```

Add these template structs:

```rust
// Update WorkflowTemplate (renamed from PipelineTemplate) to include agenda_points
#[derive(Template)]
#[template(path = "workflow/view.html")]
pub struct WorkflowTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub tor_name: String,
    pub active_tab: String,  // "suggestions", "proposals", or "agenda"
    pub suggestions: Vec<SuggestionListItem>,
    pub proposals: Vec<ProposalListItem>,
    pub agenda_points: Vec<AgendaPointListItem>,
}

// Queue view
#[derive(Template)]
#[template(path = "workflow/queue.html")]
pub struct QueueTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub tor_name: String,
    pub queued_proposals: Vec<ProposalListItem>,
}

// Agenda point form
#[derive(Template)]
#[template(path = "agenda/form.html")]
pub struct AgendaPointFormTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub tor_name: String,
    pub form_action: String,
    pub form_title: String,
    pub agenda_point: Option<AgendaPointDetail>,
    pub errors: Vec<String>,
}

// Agenda point detail
#[derive(Template)]
#[template(path = "agenda/detail.html")]
pub struct AgendaPointDetailTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub tor_name: String,
    pub agenda_point: AgendaPointDetail,
    pub coas: Vec<CoaDetail>,
    pub opinions: Vec<OpinionSummary>,
    pub available_transitions: Vec<AvailableTransition>,
}

// COA form
#[derive(Template)]
#[template(path = "coa/form.html")]
pub struct CoaFormTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub agenda_point_id: i64,
    pub form_action: String,
    pub form_title: String,
    pub coa: Option<CoaDetail>,
    pub errors: Vec<String>,
}

// COA section form
#[derive(Template)]
#[template(path = "coa/section_form.html")]
pub struct CoaSectionFormTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub agenda_point_id: i64,
    pub coa_id: i64,
    pub form_action: String,
    pub form_title: String,
    pub sections: Vec<CoaSectionItem>,
    pub errors: Vec<String>,
}

// Opinion form
#[derive(Template)]
#[template(path = "opinion/form.html")]
pub struct OpinionFormTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub agenda_point_id: i64,
    pub coas: Vec<CoaListItem>,
    pub existing_opinion: Option<OpinionListItem>,
    pub errors: Vec<String>,
}

// Decision form
#[derive(Template)]
#[template(path = "agenda/decision_form.html")]
pub struct DecisionFormTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub agenda_point: AgendaPointDetail,
    pub coas: Vec<CoaDetail>,
    pub opinions: Vec<OpinionSummary>,
    pub errors: Vec<String>,
}
```

**Verify:** `cargo check` — Will show template-not-found errors until templates are created, but the Rust code should parse correctly.

**Commit:** `git commit -m "feat: add template structs for all Phase 2b pages"`

---

## Task 11: Workflow View Template — Rename + Agenda Tab

**Files:**
- Modify: `templates/workflow/view.html`

Update the existing pipeline view template:
1. Replace all `/pipeline` URLs with `/workflow`
2. Change heading from "Item Pipeline" to "Item Workflow"
3. Replace the disabled "Agenda Points (Phase 3)" span with an active link to `?tab=agenda`
4. Add agenda points tab content (similar to proposals tab but with agenda-specific columns)
5. Add "Add to Queue" button on approved proposals (if user has `agenda.queue` permission)
6. Use data from `agenda_points` variable for the agenda tab

Agenda Points tab content:
- Table: Order (#) | Title | Item Type (badge) | Scheduled Date | Status (badge) | Actions
- Item type badges: "Informative" (blue), "Decision" (amber)
- Actions rendered from available_transitions (data-driven) — for now, use static buttons matching the workflow statuses since the template doesn't have per-row transition data; that will be refined in the handler
- "New Agenda Point" button (if `agenda.create` permission)
- "Schedule from Queue" link to queue view (if `agenda.queue` permission)
- Empty state: "No agenda points yet"

**Verify:** Delete `data/app.db`, run server, navigate to `/tor/{id}/workflow` — all three tabs should render.

**Commit:** `git commit -m "feat: update workflow view template with agenda points tab"`

---

## Task 12: Remaining Templates — Queue, Agenda, COA, Opinion, Decision

**Files:**
- Create: `templates/workflow/queue.html`
- Create: `templates/agenda/form.html`
- Create: `templates/agenda/detail.html`
- Create: `templates/agenda/decision_form.html`
- Create: `templates/coa/form.html`
- Create: `templates/coa/section_form.html`
- Create: `templates/opinion/form.html`

All templates extend `base.html`, include `partials/nav.html` and `partials/sidebar.html`, and follow the existing form/detail patterns.

**Queue template:** Checkbox table of queued proposals, bulk action form with date picker, scheduling modal (can be inline form).

**Agenda form:** Title, description, item_type select (informative/decision), scheduled_date, scheduled_order, presenter (optional), time_allocation_minutes (optional).

**Agenda detail:** Header with title + status + item type badges. Metadata section. COAs section (expandable, show sections tree for complex COAs). Member Input section (opinion summary per COA). Decision section (if completed). Action buttons from available_transitions.

**Decision form:** Shows opinion summary, COA options with radio/text, outcome_summary textarea. Submit records decision.

**COA form:** Title, description, coa_type select (simple/complex). For complex, redirect to section editor after creation.

**COA section form:** List of existing sections with edit/delete, add new section form (section_number, section_title, content, section_order).

**Opinion form:** Radio buttons for COA preference (listing each COA), comment textarea.

**Verify:** `cargo check` — templates should compile.

**Commit:** `git commit -m "feat: add all Phase 2b templates — queue, agenda, COA, opinion, decision"`

---

## Task 13: Agenda Point Handlers

**Files:**
- Create: `src/handlers/agenda_handlers.rs`
- Modify: `src/handlers/mod.rs` — Add `pub mod agenda_handlers;`

Implement handlers following the existing proposal_handlers pattern:

- `GET /tor/{id}/workflow/agenda/new` → `new_form()`
- `POST /tor/{id}/workflow/agenda` → `create()`
- `GET /tor/{id}/workflow/agenda/{agenda_id}` → `detail()`
- `GET /tor/{id}/workflow/agenda/{agenda_id}/edit` → `edit_form()`
- `POST /tor/{id}/workflow/agenda/{agenda_id}` → `update()`
- `POST /tor/{id}/workflow/agenda/{agenda_id}/transition` → `transition()` — Generic transition handler using workflow engine

The `transition()` handler:
1. Parse `new_status` from form data
2. Get current agenda point properties (including item_type)
3. Call `workflow::validate_transition()` with current status, new status, user permissions, entity properties
4. If valid, update status via `entity::set_property()`
5. If `requires_outcome`, require `outcome_summary` from form
6. Audit log the transition
7. Redirect back to detail page

**Verify:** `cargo check`

**Commit:** `git commit -m "feat: add agenda point handlers with data-driven transitions"`

---

## Task 14: Queue Handlers

**Files:**
- Create: `src/handlers/queue_handlers.rs`
- Modify: `src/handlers/mod.rs` — Add `pub mod queue_handlers;`

Implement:
- `GET /tor/{id}/workflow/queue` → `view()` — Shows queued proposals
- `POST /tor/{id}/workflow/queue/add` → `add_to_queue()` — Marks proposal as ready_for_agenda
- `POST /tor/{id}/workflow/queue/remove` → `remove_from_queue()` — Unmarks proposal
- `POST /tor/{id}/workflow/queue/schedule` → `bulk_schedule()` — Creates agenda points from selected proposals

`bulk_schedule()`:
1. Parse form: proposal_ids[], scheduled_date, item_type (default)
2. For each proposal_id:
   a. Create agenda_point entity with properties copied from proposal
   b. Create `spawns_agenda_point` relation (proposal → agenda_point)
   c. Create `agenda_submitted_to` relation (agenda_point → tor)
   d. Set `ready_for_agenda = false` on proposal
   e. Auto-increment scheduled_order
3. Flash message: "Created N agenda points for {date}"
4. Audit log each creation

**Verify:** `cargo check`

**Commit:** `git commit -m "feat: add queue handlers — add/remove/bulk schedule"`

---

## Task 15: COA Handlers

**Files:**
- Create: `src/handlers/coa_handlers.rs`
- Modify: `src/handlers/mod.rs` — Add `pub mod coa_handlers;`

Implement:
- `GET /tor/{id}/workflow/agenda/{agenda_id}/coa/new` → `new_form()`
- `POST /tor/{id}/workflow/agenda/{agenda_id}/coa` → `create()`
- `GET /tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}/edit` → `edit_form()`
- `POST /tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}` → `update()`
- `POST /tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}/delete` → `delete()`
- `POST /tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}/sections` → `add_section()`
- `POST /tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}/sections/{section_id}` → `update_section()`
- `POST /tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}/sections/{section_id}/delete` → `delete_section()`

COA creation also creates `considers_coa` relation (agenda_point → coa). If originating from proposal, creates `originates_from` relation.

**Verify:** `cargo check`

**Commit:** `git commit -m "feat: add COA handlers — CRUD with section management"`

---

## Task 16: Opinion + Decision Handlers

**Files:**
- Create: `src/handlers/opinion_handlers.rs`
- Modify: `src/handlers/mod.rs` — Add `pub mod opinion_handlers;`

Implement:
- `GET /tor/{id}/workflow/agenda/{agenda_id}/input` → `form()` — Shows opinion form with COA options
- `POST /tor/{id}/workflow/agenda/{agenda_id}/input` → `submit()` — Records or updates opinion
- `GET /tor/{id}/workflow/agenda/{agenda_id}/decide` → `decision_form()` — Shows decision recording form
- `POST /tor/{id}/workflow/agenda/{agenda_id}/decide` → `record_decision()` — Records final decision

`submit()`:
1. Check existing opinion via `opinion::find_by_user_and_agenda_point()`
2. If exists, update. If not, create new opinion entity + relations
3. Audit log

`record_decision()`:
1. Require `agenda.decide` permission
2. Record `outcome_summary`, `decided_by_id`, `decision_date`, optionally `selected_coa_id` on agenda_point
3. Trigger workflow transition to `completed` via workflow engine
4. Audit log (important event)

**Verify:** `cargo check`

**Commit:** `git commit -m "feat: add opinion and decision handlers"`

---

## Task 17: Route Wiring in main.rs

**Files:**
- Modify: `src/main.rs`

Add all new routes in the protected scope, organized by feature area. Place after existing ToR and workflow routes:

```rust
// Workflow queue
.route("/tor/{id}/workflow/queue", web::get().to(handlers::queue_handlers::view))
.route("/tor/{id}/workflow/queue/add", web::post().to(handlers::queue_handlers::add_to_queue))
.route("/tor/{id}/workflow/queue/remove", web::post().to(handlers::queue_handlers::remove_from_queue))
.route("/tor/{id}/workflow/queue/schedule", web::post().to(handlers::queue_handlers::bulk_schedule))
// Agenda points — /new BEFORE /{agenda_id}
.route("/tor/{id}/workflow/agenda/new", web::get().to(handlers::agenda_handlers::new_form))
.route("/tor/{id}/workflow/agenda", web::post().to(handlers::agenda_handlers::create))
.route("/tor/{id}/workflow/agenda/{agenda_id}", web::get().to(handlers::agenda_handlers::detail))
.route("/tor/{id}/workflow/agenda/{agenda_id}/edit", web::get().to(handlers::agenda_handlers::edit_form))
.route("/tor/{id}/workflow/agenda/{agenda_id}", web::post().to(handlers::agenda_handlers::update))
.route("/tor/{id}/workflow/agenda/{agenda_id}/transition", web::post().to(handlers::agenda_handlers::transition))
// COAs — /new BEFORE /{coa_id}
.route("/tor/{id}/workflow/agenda/{agenda_id}/coa/new", web::get().to(handlers::coa_handlers::new_form))
.route("/tor/{id}/workflow/agenda/{agenda_id}/coa", web::post().to(handlers::coa_handlers::create))
.route("/tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}/edit", web::get().to(handlers::coa_handlers::edit_form))
.route("/tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}", web::post().to(handlers::coa_handlers::update))
.route("/tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}/delete", web::post().to(handlers::coa_handlers::delete))
.route("/tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}/sections", web::post().to(handlers::coa_handlers::add_section))
.route("/tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}/sections/{section_id}", web::post().to(handlers::coa_handlers::update_section))
.route("/tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}/sections/{section_id}/delete", web::post().to(handlers::coa_handlers::delete_section))
// Opinions + Decisions
.route("/tor/{id}/workflow/agenda/{agenda_id}/input", web::get().to(handlers::opinion_handlers::form))
.route("/tor/{id}/workflow/agenda/{agenda_id}/input", web::post().to(handlers::opinion_handlers::submit))
.route("/tor/{id}/workflow/agenda/{agenda_id}/decide", web::get().to(handlers::opinion_handlers::decision_form))
.route("/tor/{id}/workflow/agenda/{agenda_id}/decide", web::post().to(handlers::opinion_handlers::record_decision))
```

**Important:** Register `/new` routes BEFORE `/{id}` routes to avoid path parameter conflicts (see CLAUDE.md).

**Verify:** `cargo check` — should compile with 0 errors.

**Commit:** `git commit -m "feat: wire all Phase 2b routes in main.rs"`

---

## Task 18: Migrate Suggestion + Proposal Handlers to Workflow Engine

**Files:**
- Modify: `src/handlers/suggestion_handlers.rs`
- Modify: `src/handlers/proposal_handlers.rs`

Replace hardcoded status strings with workflow engine calls:

**Suggestion handlers (accept/reject):**
- Before: `suggestion::update_status(&conn, suggestion_id, "accepted", None)?;`
- After: First validate transition via `workflow::validate_transition(&conn, "suggestion", current_status, "accepted", &permissions, &props)?;`, then update status. This ensures the workflow definition is respected.

**Proposal handlers (submit/review/approve/reject):**
- Same pattern: Validate transition first, then update status.
- Remove hardcoded status checks and use `workflow::validate_transition()` instead.
- The workflow engine already checks permissions, so the `require_permission()` calls can be replaced by the engine's permission check (or kept as defense-in-depth).

**Minimal change approach:** Keep the existing handler structure but add `workflow::validate_transition()` call before each status update. This ensures backward compatibility while enabling the data-driven workflow.

**Verify:** Delete `data/app.db`, cargo run, test full suggestion→proposal workflow still works.

**Commit:** `git commit -m "refactor: migrate suggestion and proposal handlers to workflow engine"`

---

## Task 19: Manual E2E Testing

**No files to create/modify.**

Test the complete Phase 2b workflow:

1. Delete `data/app.db` and restart server (fresh seed)
2. Login as admin
3. Create a ToR and add admin as member
4. **Test suggestions** (should still work as before via workflow engine)
5. **Test proposals** (should still work, plus "Add to Queue" on approved)
6. **Test queue:** Mark approved proposal as ready → appears in queue view
7. **Test bulk schedule:** Select proposals in queue → create agenda points for a date
8. **Test agenda point detail:** View agenda point with COAs
9. **Test COA creation:** Create simple COA, create complex COA with sections
10. **Test opinion recording:** Record opinion with COA preference + comment
11. **Test decision recording:** Record decision as user with `agenda.decide` permission
12. **Test workflow transitions:** Progress agenda point through all statuses
13. **Test from scratch:** Create agenda point without queue (routine item)
14. Verify audit log captures all events

**Commit:** No commit needed for testing.

---

## Implementation Notes

### Delete data/app.db Before Testing

The workflow engine adds many new seed entities. Always delete the database and restart when testing to get fresh seed data.

### Askama Gotchas

- Use `.as_str()` for string comparisons in templates: `{% if status.as_str() == "scheduled" %}`
- No `ref` in `if let`: use `{% if let Some(x) = val %}`
- Included templates share parent scope — all fields must be on the template struct

### Route Order

Always register specific routes (`/new`, `/edit`) before parameterized routes (`/{id}`) to avoid conflicts.

### Permission Group

Use "Workflow" as the `group_name` for new permissions (replacing "Pipeline" for consistency with the rename).
