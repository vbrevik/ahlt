# Phase 2a Implementation Plan — Suggestions + Proposals Pipeline

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Implement Suggestions and Proposals pipeline with auto-conversion, status workflows, and cross-ToR transparency.

**Architecture:** Pure EAV entities with granular permissions, ToR-scoped membership validation, tabbed pipeline view with server-rendered Askama templates.

**Tech Stack:** Rust, Actix-web 4, Askama 0.14, SQLite (rusqlite), r2d2 connection pooling

---

## Task 1: Database Seed — Relation Types, Permissions, Nav Items

**Files:**
- Modify: `src/db.rs` (seed_ontology function)

**Step 1: Add 3 new relation types**

In `src/db.rs`, in the `seed_ontology()` function, after the existing `has_tor_role` relation type, add:

```rust
// Item pipeline relation types
let _suggested_to_id = insert_entity(&conn, "relation_type", "suggested_to", "Suggested To", 0);
let _spawns_proposal_id = insert_entity(&conn, "relation_type", "spawns_proposal", "Spawns Proposal", 0);
let _submitted_to_id = insert_entity(&conn, "relation_type", "submitted_to", "Submitted To", 0);
```

**Step 2: Add 9 new permissions in Pipeline group**

After the existing `tor.manage_members` permission, add:

```rust
// Pipeline permissions
("suggestion.view", "View suggestions in member ToRs", "Pipeline"),
("suggestion.create", "Submit new suggestions", "Pipeline"),
("suggestion.review", "Accept or reject suggestions", "Pipeline"),
("proposal.view", "View proposals in member ToRs", "Pipeline"),
("proposal.create", "Create new proposals", "Pipeline"),
("proposal.submit", "Submit draft proposals for review", "Pipeline"),
("proposal.edit", "Edit draft proposals", "Pipeline"),
("proposal.review", "Move proposals to under_review status", "Pipeline"),
("proposal.approve", "Approve or reject proposals under review", "Pipeline"),
```

**Step 3: Add pipeline nav item under Governance**

After the `governance.tor` nav item, add:

```rust
// Pipeline navigation (shows when user has any suggestion/proposal permission)
let nav_pipeline_id = insert_entity(&conn, "nav_item", "governance.pipeline", "Item Pipeline", 2);
insert_property(&conn, nav_pipeline_id, "url", "/pipeline");
insert_property(&conn, nav_pipeline_id, "parent", "governance");

// Link pipeline to suggestion.view permission
let pipeline_perm_link_id = create_relation(&conn, "requires_permission", nav_pipeline_id, suggestion_view_perm_id);
```

**Step 4: Update log message**

Update the log message at the end of `seed_ontology()`:

```rust
log::info!("Ontology seeded: users, roles, permissions, nav items, ToR foundation, pipeline foundation");
```

**Step 5: Verify compilation**

Run: `cargo check 2>&1 | tail -10`
Expected: "Finished" with no errors (warnings OK)

**Step 6: Commit**

```bash
git add src/db.rs
git commit -m "feat: add pipeline relation types, permissions, and nav items

- 3 new relation types: suggested_to, spawns_proposal, submitted_to
- 9 new permissions in Pipeline group
- governance.pipeline nav item

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 2: Suggestion Model — Types

**Files:**
- Create: `src/models/suggestion/types.rs`
- Create: `src/models/suggestion/mod.rs`
- Modify: `src/models/mod.rs`

**Step 1: Create suggestion types module**

Create `src/models/suggestion/types.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestionListItem {
    pub id: i64,
    pub description: String,
    pub description_preview: String,  // First 100 chars
    pub submitted_by_id: i64,
    pub submitted_by_name: String,
    pub submitted_date: String,  // YYYY-MM-DD
    pub status: String,  // open, accepted, rejected
    pub rejection_reason: Option<String>,
    pub spawned_proposal_id: Option<i64>,  // If accepted
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestionDetail {
    pub id: i64,
    pub description: String,
    pub submitted_by_id: i64,
    pub submitted_by_name: String,
    pub submitted_date: String,
    pub status: String,
    pub rejection_reason: Option<String>,
    pub spawned_proposal_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SuggestionForm {
    pub description: String,
    pub csrf_token: String,
}
```

**Step 2: Create mod.rs**

Create `src/models/suggestion/mod.rs`:

```rust
pub mod types;
pub mod queries;

pub use types::*;
pub use queries::*;
```

**Step 3: Register in models/mod.rs**

In `src/models/mod.rs`, add:

```rust
pub mod suggestion;
```

**Step 4: Verify compilation**

Run: `cargo check 2>&1 | tail -10`
Expected: "Finished" (may have unused warnings, that's OK)

**Step 5: Commit**

```bash
git add src/models/suggestion/
git add src/models/mod.rs
git commit -m "feat: add suggestion model types

- SuggestionListItem for pipeline table view
- SuggestionDetail for full view
- SuggestionForm for create form

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 3: Suggestion Model — Queries

**Files:**
- Create: `src/models/suggestion/queries.rs`

**Step 1: Create queries module with find_all_for_tor**

Create `src/models/suggestion/queries.rs`:

```rust
use rusqlite::{params, Connection, Result as SqliteResult};
use crate::errors::AppError;
use super::types::*;

pub fn find_all_for_tor(conn: &Connection, tor_id: i64) -> Result<Vec<SuggestionListItem>, AppError> {
    let mut stmt = conn.prepare("
        SELECT
            e.id,
            COALESCE(ep_desc.value, '') as description,
            COALESCE(ep_date.value, '') as submitted_date,
            COALESCE(ep_status.value, 'open') as status,
            COALESCE(ep_by.value, '0') as submitted_by_id,
            COALESCE(u.label, 'Unknown') as submitted_by_name,
            COALESCE(ep_reason.value, '') as rejection_reason,
            spawned.target_id as spawned_proposal_id
        FROM entities e
        JOIN relations r ON e.id = r.source_id
        JOIN entities rt ON r.relation_type_id = rt.id AND rt.name = 'suggested_to'
        LEFT JOIN entity_properties ep_desc ON e.id = ep_desc.entity_id AND ep_desc.key = 'description'
        LEFT JOIN entity_properties ep_date ON e.id = ep_date.entity_id AND ep_date.key = 'submitted_date'
        LEFT JOIN entity_properties ep_status ON e.id = ep_status.entity_id AND ep_status.key = 'status'
        LEFT JOIN entity_properties ep_by ON e.id = ep_by.entity_id AND ep_by.key = 'submitted_by_id'
        LEFT JOIN entity_properties ep_reason ON e.id = ep_reason.entity_id AND ep_reason.key = 'rejection_reason'
        LEFT JOIN entities u ON CAST(ep_by.value AS INTEGER) = u.id
        LEFT JOIN relations spawned ON e.id = spawned.source_id AND EXISTS (
            SELECT 1 FROM entities rt2 WHERE spawned.relation_type_id = rt2.id AND rt2.name = 'spawns_proposal'
        )
        WHERE e.entity_type = 'suggestion'
          AND r.target_id = ?1
        ORDER BY ep_date.value DESC
    ")?;

    let suggestions = stmt.query_map(params![tor_id], |row| {
        let description: String = row.get(1)?;
        let description_preview = if description.len() > 100 {
            format!("{}...", &description[..100])
        } else {
            description.clone()
        };

        Ok(SuggestionListItem {
            id: row.get(0)?,
            description,
            description_preview,
            submitted_date: row.get(2)?,
            status: row.get(3)?,
            submitted_by_id: row.get::<_, String>(4)?.parse().unwrap_or(0),
            submitted_by_name: row.get(5)?,
            rejection_reason: {
                let reason: String = row.get(6)?;
                if reason.is_empty() { None } else { Some(reason) }
            },
            spawned_proposal_id: row.get(7).ok(),
        })
    })?.collect::<SqliteResult<Vec<_>>>()?;

    Ok(suggestions)
}
```

**Step 2: Add find_by_id**

In the same file, add:

```rust
pub fn find_by_id(conn: &Connection, id: i64) -> Result<SuggestionDetail, AppError> {
    let mut stmt = conn.prepare("
        SELECT
            e.id,
            COALESCE(ep_desc.value, '') as description,
            COALESCE(ep_date.value, '') as submitted_date,
            COALESCE(ep_status.value, 'open') as status,
            COALESCE(ep_by.value, '0') as submitted_by_id,
            COALESCE(u.label, 'Unknown') as submitted_by_name,
            COALESCE(ep_reason.value, '') as rejection_reason,
            spawned.target_id as spawned_proposal_id
        FROM entities e
        LEFT JOIN entity_properties ep_desc ON e.id = ep_desc.entity_id AND ep_desc.key = 'description'
        LEFT JOIN entity_properties ep_date ON e.id = ep_date.entity_id AND ep_date.key = 'submitted_date'
        LEFT JOIN entity_properties ep_status ON e.id = ep_status.entity_id AND ep_status.key = 'status'
        LEFT JOIN entity_properties ep_by ON e.id = ep_by.entity_id AND ep_by.key = 'submitted_by_id'
        LEFT JOIN entity_properties ep_reason ON e.id = ep_reason.entity_id AND ep_reason.key = 'rejection_reason'
        LEFT JOIN entities u ON CAST(ep_by.value AS INTEGER) = u.id
        LEFT JOIN relations spawned ON e.id = spawned.source_id AND EXISTS (
            SELECT 1 FROM entities rt2 WHERE spawned.relation_type_id = rt2.id AND rt2.name = 'spawns_proposal'
        )
        WHERE e.entity_type = 'suggestion' AND e.id = ?1
    ")?;

    let suggestion = stmt.query_row(params![id], |row| {
        Ok(SuggestionDetail {
            id: row.get(0)?,
            description: row.get(1)?,
            submitted_date: row.get(2)?,
            status: row.get(3)?,
            submitted_by_id: row.get::<_, String>(4)?.parse().unwrap_or(0),
            submitted_by_name: row.get(5)?,
            rejection_reason: {
                let reason: String = row.get(6)?;
                if reason.is_empty() { None } else { Some(reason) }
            },
            spawned_proposal_id: row.get(7).ok(),
        })
    })?;

    Ok(suggestion)
}
```

**Step 3: Add create function**

```rust
use crate::models::entity::{insert_entity, insert_property};
use crate::models::relation::create_relation;

pub fn create(
    conn: &Connection,
    tor_id: i64,
    description: &str,
    submitted_by_id: i64,
    submitted_date: &str,
) -> Result<i64, AppError> {
    // Generate name and label
    let name = format!("suggestion_{}", submitted_date.replace("-", "_"));
    let label = if description.len() > 50 {
        format!("{}...", &description[..50])
    } else {
        description.to_string()
    };

    // Create suggestion entity
    let suggestion_id = insert_entity(conn, "suggestion", &name, &label, 0);

    // Set properties
    insert_property(conn, suggestion_id, "description", description);
    insert_property(conn, suggestion_id, "submitted_date", submitted_date);
    insert_property(conn, suggestion_id, "status", "open");
    insert_property(conn, suggestion_id, "submitted_by_id", &submitted_by_id.to_string());

    // Create suggested_to relation
    create_relation(conn, "suggested_to", suggestion_id, tor_id);

    Ok(suggestion_id)
}
```

**Step 4: Add update_status function**

```rust
pub fn update_status(
    conn: &Connection,
    suggestion_id: i64,
    new_status: &str,
    rejection_reason: Option<&str>,
) -> Result<(), AppError> {
    // Update status property
    conn.execute(
        "INSERT OR REPLACE INTO entity_properties (entity_id, key, value) VALUES (?1, 'status', ?2)",
        params![suggestion_id, new_status],
    )?;

    // Update rejection_reason if provided
    if let Some(reason) = rejection_reason {
        conn.execute(
            "INSERT OR REPLACE INTO entity_properties (entity_id, key, value) VALUES (?1, 'rejection_reason', ?2)",
            params![suggestion_id, reason],
        )?;
    }

    Ok(())
}
```

**Step 5: Verify compilation**

Run: `cargo check 2>&1 | tail -10`
Expected: "Finished"

**Step 6: Commit**

```bash
git add src/models/suggestion/queries.rs
git commit -m "feat: add suggestion query functions

- find_all_for_tor: list suggestions for pipeline view
- find_by_id: get suggestion detail
- create: insert new suggestion with EAV properties
- update_status: transition suggestion status

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 4: Proposal Model — Types

**Files:**
- Create: `src/models/proposal/types.rs`
- Create: `src/models/proposal/mod.rs`
- Modify: `src/models/mod.rs`

**Step 1: Create proposal types module**

Create `src/models/proposal/types.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalListItem {
    pub id: i64,
    pub title: String,
    pub submitted_by_id: i64,
    pub submitted_by_name: String,
    pub submitted_date: String,  // YYYY-MM-DD
    pub status: String,  // draft, submitted, under_review, approved, rejected
    pub rejection_reason: Option<String>,
    pub related_suggestion_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalDetail {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub rationale: String,
    pub submitted_by_id: i64,
    pub submitted_by_name: String,
    pub submitted_date: String,
    pub status: String,
    pub rejection_reason: Option<String>,
    pub related_suggestion_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProposalForm {
    pub title: String,
    pub description: String,
    pub rationale: String,
    pub related_suggestion_id: Option<String>,  // Optional link to suggestion
    pub csrf_token: String,
}
```

**Step 2: Create mod.rs**

Create `src/models/proposal/mod.rs`:

```rust
pub mod types;
pub mod queries;

pub use types::*;
pub use queries::*;
```

**Step 3: Register in models/mod.rs**

In `src/models/mod.rs`, add:

```rust
pub mod proposal;
```

**Step 4: Verify compilation**

Run: `cargo check 2>&1 | tail -10`
Expected: "Finished"

**Step 5: Commit**

```bash
git add src/models/proposal/
git add src/models/mod.rs
git commit -m "feat: add proposal model types

- ProposalListItem for pipeline table view
- ProposalDetail for full view
- ProposalForm for create/edit forms

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 5: Proposal Model — Queries (Part 1)

**Files:**
- Create: `src/models/proposal/queries.rs`

**Step 1: Create find_all_for_tor**

Create `src/models/proposal/queries.rs`:

```rust
use rusqlite::{params, Connection, Result as SqliteResult};
use crate::errors::AppError;
use crate::models::entity::{insert_entity, insert_property};
use crate::models::relation::create_relation;
use super::types::*;

pub fn find_all_for_tor(conn: &Connection, tor_id: i64) -> Result<Vec<ProposalListItem>, AppError> {
    let mut stmt = conn.prepare("
        SELECT
            e.id,
            COALESCE(ep_title.value, '') as title,
            COALESCE(ep_date.value, '') as submitted_date,
            COALESCE(ep_status.value, 'draft') as status,
            COALESCE(ep_by.value, '0') as submitted_by_id,
            COALESCE(u.label, 'Unknown') as submitted_by_name,
            COALESCE(ep_reason.value, '') as rejection_reason,
            rel_sugg.source_id as related_suggestion_id
        FROM entities e
        JOIN relations r ON e.id = r.source_id
        JOIN entities rt ON r.relation_type_id = rt.id AND rt.name = 'submitted_to'
        LEFT JOIN entity_properties ep_title ON e.id = ep_title.entity_id AND ep_title.key = 'title'
        LEFT JOIN entity_properties ep_date ON e.id = ep_date.entity_id AND ep_date.key = 'submitted_date'
        LEFT JOIN entity_properties ep_status ON e.id = ep_status.entity_id AND ep_status.key = 'status'
        LEFT JOIN entity_properties ep_by ON e.id = ep_by.entity_id AND ep_by.key = 'submitted_by_id'
        LEFT JOIN entity_properties ep_reason ON e.id = ep_reason.entity_id AND ep_reason.key = 'rejection_reason'
        LEFT JOIN entities u ON CAST(ep_by.value AS INTEGER) = u.id
        LEFT JOIN relations rel_sugg ON e.id = rel_sugg.target_id AND EXISTS (
            SELECT 1 FROM entities rt2 WHERE rel_sugg.relation_type_id = rt2.id AND rt2.name = 'spawns_proposal'
        )
        WHERE e.entity_type = 'proposal'
          AND r.target_id = ?1
        ORDER BY ep_date.value DESC
    ")?;

    let proposals = stmt.query_map(params![tor_id], |row| {
        Ok(ProposalListItem {
            id: row.get(0)?,
            title: row.get(1)?,
            submitted_date: row.get(2)?,
            status: row.get(3)?,
            submitted_by_id: row.get::<_, String>(4)?.parse().unwrap_or(0),
            submitted_by_name: row.get(5)?,
            rejection_reason: {
                let reason: String = row.get(6)?;
                if reason.is_empty() { None } else { Some(reason) }
            },
            related_suggestion_id: row.get(7).ok(),
        })
    })?.collect::<SqliteResult<Vec<_>>>()?;

    Ok(proposals)
}
```

**Step 2: Add find_by_id**

```rust
pub fn find_by_id(conn: &Connection, id: i64) -> Result<ProposalDetail, AppError> {
    let mut stmt = conn.prepare("
        SELECT
            e.id,
            COALESCE(ep_title.value, '') as title,
            COALESCE(ep_desc.value, '') as description,
            COALESCE(ep_rat.value, '') as rationale,
            COALESCE(ep_date.value, '') as submitted_date,
            COALESCE(ep_status.value, 'draft') as status,
            COALESCE(ep_by.value, '0') as submitted_by_id,
            COALESCE(u.label, 'Unknown') as submitted_by_name,
            COALESCE(ep_reason.value, '') as rejection_reason,
            rel_sugg.source_id as related_suggestion_id
        FROM entities e
        LEFT JOIN entity_properties ep_title ON e.id = ep_title.entity_id AND ep_title.key = 'title'
        LEFT JOIN entity_properties ep_desc ON e.id = ep_desc.entity_id AND ep_desc.key = 'description'
        LEFT JOIN entity_properties ep_rat ON e.id = ep_rat.entity_id AND ep_rat.key = 'rationale'
        LEFT JOIN entity_properties ep_date ON e.id = ep_date.entity_id AND ep_date.key = 'submitted_date'
        LEFT JOIN entity_properties ep_status ON e.id = ep_status.entity_id AND ep_status.key = 'status'
        LEFT JOIN entity_properties ep_by ON e.id = ep_by.entity_id AND ep_by.key = 'submitted_by_id'
        LEFT JOIN entity_properties ep_reason ON e.id = ep_reason.entity_id AND ep_reason.key = 'rejection_reason'
        LEFT JOIN entities u ON CAST(ep_by.value AS INTEGER) = u.id
        LEFT JOIN relations rel_sugg ON e.id = rel_sugg.target_id AND EXISTS (
            SELECT 1 FROM entities rt2 WHERE rel_sugg.relation_type_id = rt2.id AND rt2.name = 'spawns_proposal'
        )
        WHERE e.entity_type = 'proposal' AND e.id = ?1
    ")?;

    let proposal = stmt.query_row(params![id], |row| {
        Ok(ProposalDetail {
            id: row.get(0)?,
            title: row.get(1)?,
            description: row.get(2)?,
            rationale: row.get(3)?,
            submitted_date: row.get(4)?,
            status: row.get(5)?,
            submitted_by_id: row.get::<_, String>(6)?.parse().unwrap_or(0),
            submitted_by_name: row.get(7)?,
            rejection_reason: {
                let reason: String = row.get(8)?;
                if reason.is_empty() { None } else { Some(reason) }
            },
            related_suggestion_id: row.get(9).ok(),
        })
    })?;

    Ok(proposal)
}
```

**Step 3: Verify compilation**

Run: `cargo check 2>&1 | tail -10`
Expected: "Finished"

**Step 4: Commit**

```bash
git add src/models/proposal/queries.rs
git commit -m "feat: add proposal query functions (part 1)

- find_all_for_tor: list proposals for pipeline view
- find_by_id: get proposal detail with all properties

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 6: Proposal Model — Queries (Part 2)

**Files:**
- Modify: `src/models/proposal/queries.rs`

**Step 1: Add create function**

In `src/models/proposal/queries.rs`, add:

```rust
pub fn create(
    conn: &Connection,
    tor_id: i64,
    title: &str,
    description: &str,
    rationale: &str,
    submitted_by_id: i64,
    submitted_date: &str,
    related_suggestion_id: Option<i64>,
) -> Result<i64, AppError> {
    // Generate name from title (sanitized)
    let name = title
        .to_lowercase()
        .replace(" ", "_")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect::<String>();

    // Create proposal entity
    let proposal_id = insert_entity(conn, "proposal", &name, title, 0);

    // Set properties
    insert_property(conn, proposal_id, "title", title);
    insert_property(conn, proposal_id, "description", description);
    insert_property(conn, proposal_id, "rationale", rationale);
    insert_property(conn, proposal_id, "submitted_date", submitted_date);
    insert_property(conn, proposal_id, "status", "draft");
    insert_property(conn, proposal_id, "submitted_by_id", &submitted_by_id.to_string());

    // Create submitted_to relation
    create_relation(conn, "submitted_to", proposal_id, tor_id);

    // Create spawns_proposal relation if linked to suggestion
    if let Some(suggestion_id) = related_suggestion_id {
        create_relation(conn, "spawns_proposal", suggestion_id, proposal_id);
    }

    Ok(proposal_id)
}
```

**Step 2: Add update function**

```rust
pub fn update(
    conn: &Connection,
    proposal_id: i64,
    title: &str,
    description: &str,
    rationale: &str,
) -> Result<(), AppError> {
    // Update entity label
    conn.execute(
        "UPDATE entities SET label = ?1 WHERE id = ?2",
        params![title, proposal_id],
    )?;

    // Update properties
    conn.execute(
        "INSERT OR REPLACE INTO entity_properties (entity_id, key, value) VALUES (?1, 'title', ?2)",
        params![proposal_id, title],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO entity_properties (entity_id, key, value) VALUES (?1, 'description', ?2)",
        params![proposal_id, description],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO entity_properties (entity_id, key, value) VALUES (?1, 'rationale', ?2)",
        params![proposal_id, rationale],
    )?;

    Ok(())
}
```

**Step 3: Add update_status function**

```rust
pub fn update_status(
    conn: &Connection,
    proposal_id: i64,
    new_status: &str,
    rejection_reason: Option<&str>,
) -> Result<(), AppError> {
    // Update status property
    conn.execute(
        "INSERT OR REPLACE INTO entity_properties (entity_id, key, value) VALUES (?1, 'status', ?2)",
        params![proposal_id, new_status],
    )?;

    // Update or clear rejection_reason
    if let Some(reason) = rejection_reason {
        conn.execute(
            "INSERT OR REPLACE INTO entity_properties (entity_id, key, value) VALUES (?1, 'rejection_reason', ?2)",
            params![proposal_id, reason],
        )?;
    } else if new_status != "rejected" {
        // Clear rejection_reason when not rejected (e.g., resubmission)
        conn.execute(
            "DELETE FROM entity_properties WHERE entity_id = ?1 AND key = 'rejection_reason'",
            params![proposal_id],
        )?;
    }

    Ok(())
}
```

**Step 4: Add auto_create_from_suggestion function**

```rust
pub fn auto_create_from_suggestion(
    conn: &Connection,
    suggestion_id: i64,
    tor_id: i64,
) -> Result<i64, AppError> {
    // Get suggestion details
    let suggestion = crate::models::suggestion::find_by_id(conn, suggestion_id)?;

    // Create proposal with description from suggestion
    let title = if suggestion.description.len() > 100 {
        format!("{}...", &suggestion.description[..100])
    } else {
        suggestion.description.clone()
    };

    let proposal_id = create(
        conn,
        tor_id,
        &title,
        &suggestion.description,
        "Auto-created from accepted suggestion",
        suggestion.submitted_by_id,
        &suggestion.submitted_date,
        Some(suggestion_id),
    )?;

    Ok(proposal_id)
}
```

**Step 5: Verify compilation**

Run: `cargo check 2>&1 | tail -10`
Expected: "Finished"

**Step 6: Commit**

```bash
git add src/models/proposal/queries.rs
git commit -m "feat: add proposal mutation functions (part 2)

- create: insert new proposal with EAV properties
- update: modify existing proposal (draft only)
- update_status: transition proposal status
- auto_create_from_suggestion: accept workflow

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 7: ToR Helper — Membership Validation

**Files:**
- Modify: `src/models/tor/queries.rs`

**Step 1: Add require_tor_membership helper**

In `src/models/tor/queries.rs`, after the existing query functions, add:

```rust
use crate::errors::AppError;

pub fn require_tor_membership(
    conn: &Connection,
    user_id: i64,
    tor_id: i64,
) -> Result<(), AppError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM relations r
         JOIN entities rt ON r.relation_type_id = rt.id
         WHERE rt.name = 'member_of'
           AND r.source_id = ?1
           AND r.target_id = ?2",
        params![user_id, tor_id],
        |row| row.get(0),
    )?;

    if count == 0 {
        return Err(AppError::PermissionDenied("Not a member of this ToR".into()));
    }
    Ok(())
}
```

**Step 2: Add get_tor_name helper**

```rust
pub fn get_tor_name(conn: &Connection, tor_id: i64) -> Result<String, AppError> {
    let name = conn.query_row(
        "SELECT label FROM entities WHERE id = ?1 AND entity_type = 'tor'",
        params![tor_id],
        |row| row.get(0),
    )?;
    Ok(name)
}
```

**Step 3: Verify compilation**

Run: `cargo check 2>&1 | tail -10`
Expected: "Finished"

**Step 4: Commit**

```bash
git add src/models/tor/queries.rs
git commit -m "feat: add ToR membership validation helpers

- require_tor_membership: enforce ToR scoping
- get_tor_name: fetch ToR label for audit logs

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 8: Template Structs — Pipeline Views

**Files:**
- Modify: `src/templates_structs.rs`

**Step 1: Add imports**

At the top of `src/templates_structs.rs`, add:

```rust
use crate::models::suggestion::{SuggestionListItem};
use crate::models::proposal::{ProposalListItem};
```

**Step 2: Add PipelineTemplate**

At the end of the file, add:

```rust
#[derive(Template)]
#[template(path = "pipeline/view.html")]
pub struct PipelineTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub tor_name: String,
    pub active_tab: String,  // "suggestions" | "proposals" | "agenda"
    pub suggestions: Vec<SuggestionListItem>,
    pub proposals: Vec<ProposalListItem>,
}

#[derive(Template)]
#[template(path = "suggestions/form.html")]
pub struct SuggestionFormTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub tor_name: String,
    pub errors: Vec<String>,
}

#[derive(Template)]
#[template(path = "proposals/form.html")]
pub struct ProposalFormTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub tor_name: String,
    pub form_action: String,
    pub form_title: String,
    pub proposal: Option<crate::models::proposal::ProposalDetail>,
    pub errors: Vec<String>,
}
```

**Step 3: Verify compilation**

Run: `cargo check 2>&1 | tail -10`
Expected: "Finished" (may have unused struct warnings)

**Step 4: Commit**

```bash
git add src/templates_structs.rs
git commit -m "feat: add pipeline template structs

- PipelineTemplate for tabbed view
- SuggestionFormTemplate for create form
- ProposalFormTemplate for create/edit forms

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

**Due to length constraints, I'll continue with the remaining tasks (9-20) in a structured format. The full plan would include:**

## Task 9: Pipeline View Template
- Create `templates/pipeline/view.html` with tab navigation

## Task 10: Suggestions Tab Template
- Create `templates/pipeline/suggestions_tab.html` with table

## Task 11: Proposals Tab Template
- Create `templates/pipeline/proposals_tab.html` with table

## Task 12: Suggestion Form Template
- Create `templates/suggestions/form.html`

## Task 13: Proposal Form Template
- Create `templates/proposals/form.html`

## Task 14: Pipeline Handler Module
- Create `src/handlers/pipeline_handlers/mod.rs` with view handler

## Task 15: Suggestion Handlers
- Create `src/handlers/suggestion_handlers/` with CRUD + accept/reject

## Task 16: Proposal Handlers (Part 1)
- Create `src/handlers/proposal_handlers/` with CRUD handlers

## Task 17: Proposal Handlers (Part 2)
- Add submit/review/approve/reject handlers

## Task 18: Register Handler Modules
- Modify `src/handlers/mod.rs` to register new modules

## Task 19: Route Wiring
- Modify `src/main.rs` to wire all routes

## Task 20: Manual E2E Testing
- Test full workflow: create suggestion → accept → edit proposal → submit → approve

---

**Would you like me to:**
1. **Complete the full plan** with all 20 tasks in detail (will be ~1500 lines)?
2. **Proceed with implementation** using the tasks I've outlined so far?
3. **Adjust the task breakdown** based on your feedback?
