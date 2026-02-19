# Meeting Lifecycle Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement persisted meeting entities with workflow lifecycle (projected → confirmed → in_progress → completed), agenda assignment, and minutes generation wiring.

**Architecture:** Meetings are EAV entities using the existing workflow engine for lifecycle transitions. The calendar (`tor/calendar.rs`) computes projected meetings from cadence rules; users confirm them to persist as entities. Agenda points relate to meetings via `scheduled_for_meeting` relation. Minutes scaffold generation is wired to meeting entities, fixing the existing broken pipeline.

**Tech Stack:** Rust, Actix-web 4, Askama 0.14, rusqlite, SQLite EAV pattern

**Design doc:** `docs/plans/2026-02-19-meeting-lifecycle-design.md`

---

## Phase 1: Seed Data

### Task 1: Seed meeting relation types, permission, and nav item

**GOAL:** Add the `scheduled_for_meeting` relation type, `meetings.view` permission, and `governance.meetings` nav item to `ontology.json` so the data manager seeds them on next DB reset. Success = `APP_ENV=staging cargo run` starts without errors after deleting the DB.

**CONSTRAINTS:**
- Only modify `data/seed/ontology.json` — no Rust code changes
- Follow the exact JSON structure of existing entries (see `belongs_to_tor` for relation types, `workflow.manage` for permissions, `governance.workflow_builder` for nav items)
- `belongs_to_tor` and `minutes_of` relation types already exist — do NOT duplicate them
- Nav item `sort_order: 6` (after workflow_builder at 5)
- Grant `meetings.view` to the `admin` role via `has_permission` relation

**FORMAT:**
Add to `data/seed/ontology.json`:
1. One `relation_type` entity: `scheduled_for_meeting`
2. One `permission` entity: `meetings.view` (group: Governance)
3. One `nav_item` entity: `governance.meetings` (url: `/meetings`, parent: `governance`)
4. One `requires_permission` relation: nav_item → permission
5. One `has_permission` relation: admin role → permission

**FAILURE CONDITIONS:**
- Missing any of the 5 entries listed above
- JSON syntax error in ontology.json (breaks all seeding)
- Duplicating `belongs_to_tor` or `minutes_of` relation types
- Nav item missing `parent` property (would render as top-level module instead of sidebar child)
- Permission not granted to admin role (admin can't see the nav item)
- `APP_ENV=staging cargo run` fails after DB delete

**Reference — exact JSON patterns to follow:**

Relation type:
```json
{
  "entity_type": "relation_type",
  "name": "scheduled_for_meeting",
  "label": "Scheduled For Meeting",
  "sort_order": 0,
  "properties": {}
}
```

Permission:
```json
{
  "entity_type": "permission",
  "name": "meetings.view",
  "label": "View Meetings",
  "sort_order": 0,
  "properties": {
    "group_name": "Governance"
  }
}
```

Nav item:
```json
{
  "entity_type": "nav_item",
  "name": "governance.meetings",
  "label": "Meetings",
  "sort_order": 6,
  "properties": {
    "url": "/meetings",
    "parent": "governance"
  }
}
```

Relations:
```json
{
  "relation_type": "requires_permission",
  "source": "nav_item:governance.meetings",
  "target": "permission:meetings.view"
},
{
  "relation_type": "has_permission",
  "source": "role:admin",
  "target": "permission:meetings.view"
}
```

**Commit:** `git add -f data/seed/ontology.json && git commit -m "feat(seed): add meeting relation type, permission, and nav item"`

---

### Task 2: Seed meeting workflow statuses and transitions

**GOAL:** Add the meeting workflow to `ontology.json`: 5 statuses (projected, confirmed, in_progress, completed, cancelled) and 5 transitions with their `transition_from`/`transition_to` relations. Success = `APP_ENV=staging cargo run` starts without errors after DB delete, and the workflow engine can query meeting transitions.

**CONSTRAINTS:**
- Only modify `data/seed/ontology.json`
- `entity_type_scope` must be `"meeting"` on all statuses and transitions (this is how the workflow engine scopes queries)
- Every transition needs BOTH a `transition_from` AND `transition_to` relation entry (10 relations total for 5 transitions)
- All transitions require `tor.edit` permission
- `requires_outcome: "false"` on all transitions (meetings don't have outcome recording)
- `projected` must have `is_initial: "true"`; `completed` and `cancelled` must have `is_terminal: "true"`

**FORMAT:**
Add to `data/seed/ontology.json`:
1. Five `workflow_status` entities with `entity_type_scope: "meeting"`
2. Five `workflow_transition` entities with `entity_type_scope: "meeting"`
3. Ten `transition_from`/`transition_to` relations (2 per transition)

**FAILURE CONDITIONS:**
- Missing `entity_type_scope: "meeting"` on any status/transition (workflow engine won't find them)
- Missing `transition_from` or `transition_to` relation for any transition (engine uses JOINs on both)
- `status_code` values don't match between transitions and statuses (e.g., `from_status_code: "confirmed"` must match a status with `status_code: "confirmed"`)
- Entity names don't follow `meeting.{status_code}` / `meeting.{from}_to_{to}` pattern
- Relation `source`/`target` references use wrong `entity_type:name` format
- JSON syntax error

**Reference — statuses:**
```json
{
  "entity_type": "workflow_status",
  "name": "meeting.projected",
  "label": "Projected",
  "sort_order": 1,
  "properties": {
    "order": "1",
    "status_code": "projected",
    "is_initial": "true",
    "label": "Projected",
    "entity_type_scope": "meeting"
  }
},
{
  "entity_type": "workflow_status",
  "name": "meeting.confirmed",
  "label": "Confirmed",
  "sort_order": 2,
  "properties": {
    "order": "2",
    "status_code": "confirmed",
    "label": "Confirmed",
    "entity_type_scope": "meeting"
  }
},
{
  "entity_type": "workflow_status",
  "name": "meeting.in_progress",
  "label": "In Progress",
  "sort_order": 3,
  "properties": {
    "order": "3",
    "status_code": "in_progress",
    "label": "In Progress",
    "entity_type_scope": "meeting"
  }
},
{
  "entity_type": "workflow_status",
  "name": "meeting.completed",
  "label": "Completed",
  "sort_order": 4,
  "properties": {
    "order": "4",
    "status_code": "completed",
    "is_terminal": "true",
    "label": "Completed",
    "entity_type_scope": "meeting"
  }
},
{
  "entity_type": "workflow_status",
  "name": "meeting.cancelled",
  "label": "Cancelled",
  "sort_order": 5,
  "properties": {
    "order": "5",
    "status_code": "cancelled",
    "is_terminal": "true",
    "label": "Cancelled",
    "entity_type_scope": "meeting"
  }
}
```

**Reference — transitions:**
```json
{
  "entity_type": "workflow_transition",
  "name": "meeting.projected_to_confirmed",
  "label": "Confirm",
  "sort_order": 0,
  "properties": {
    "from_status_code": "projected",
    "to_status_code": "confirmed",
    "required_permission": "tor.edit",
    "entity_type_scope": "meeting",
    "requires_outcome": "false",
    "transition_label": "Confirm"
  }
},
{
  "entity_type": "workflow_transition",
  "name": "meeting.projected_to_cancelled",
  "label": "Cancel",
  "sort_order": 0,
  "properties": {
    "from_status_code": "projected",
    "to_status_code": "cancelled",
    "required_permission": "tor.edit",
    "entity_type_scope": "meeting",
    "requires_outcome": "false",
    "transition_label": "Cancel"
  }
},
{
  "entity_type": "workflow_transition",
  "name": "meeting.confirmed_to_in_progress",
  "label": "Start Meeting",
  "sort_order": 0,
  "properties": {
    "from_status_code": "confirmed",
    "to_status_code": "in_progress",
    "required_permission": "tor.edit",
    "entity_type_scope": "meeting",
    "requires_outcome": "false",
    "transition_label": "Start Meeting"
  }
},
{
  "entity_type": "workflow_transition",
  "name": "meeting.confirmed_to_cancelled",
  "label": "Cancel",
  "sort_order": 0,
  "properties": {
    "from_status_code": "confirmed",
    "to_status_code": "cancelled",
    "required_permission": "tor.edit",
    "entity_type_scope": "meeting",
    "requires_outcome": "false",
    "transition_label": "Cancel"
  }
},
{
  "entity_type": "workflow_transition",
  "name": "meeting.in_progress_to_completed",
  "label": "End Meeting",
  "sort_order": 0,
  "properties": {
    "from_status_code": "in_progress",
    "to_status_code": "completed",
    "required_permission": "tor.edit",
    "entity_type_scope": "meeting",
    "requires_outcome": "false",
    "transition_label": "End Meeting"
  }
}
```

**Reference — transition relations (10 total):**
```json
{
  "relation_type": "transition_from",
  "source": "workflow_transition:meeting.projected_to_confirmed",
  "target": "workflow_status:meeting.projected"
},
{
  "relation_type": "transition_to",
  "source": "workflow_transition:meeting.projected_to_confirmed",
  "target": "workflow_status:meeting.confirmed"
},
{
  "relation_type": "transition_from",
  "source": "workflow_transition:meeting.projected_to_cancelled",
  "target": "workflow_status:meeting.projected"
},
{
  "relation_type": "transition_to",
  "source": "workflow_transition:meeting.projected_to_cancelled",
  "target": "workflow_status:meeting.cancelled"
},
{
  "relation_type": "transition_from",
  "source": "workflow_transition:meeting.confirmed_to_in_progress",
  "target": "workflow_status:meeting.confirmed"
},
{
  "relation_type": "transition_to",
  "source": "workflow_transition:meeting.confirmed_to_in_progress",
  "target": "workflow_status:meeting.in_progress"
},
{
  "relation_type": "transition_from",
  "source": "workflow_transition:meeting.confirmed_to_cancelled",
  "target": "workflow_status:meeting.confirmed"
},
{
  "relation_type": "transition_to",
  "source": "workflow_transition:meeting.confirmed_to_cancelled",
  "target": "workflow_status:meeting.cancelled"
},
{
  "relation_type": "transition_from",
  "source": "workflow_transition:meeting.in_progress_to_completed",
  "target": "workflow_status:meeting.in_progress"
},
{
  "relation_type": "transition_to",
  "source": "workflow_transition:meeting.in_progress_to_completed",
  "target": "workflow_status:meeting.completed"
}
```

**Verification:** Delete DB, run `APP_ENV=staging cargo run`, verify startup with no errors.

**Commit:** `git add -f data/seed/ontology.json && git commit -m "feat(seed): add meeting workflow statuses and transitions"`

---

## Phase 2: Model Layer (TDD)

### Task 3: Create meeting model module with types

**GOAL:** Create `src/models/meeting/` module with `MeetingListItem`, `MeetingDetail` types and empty `queries.rs`. Success = `cargo check` compiles with no errors (unused import warnings are OK).

**CONSTRAINTS:**
- Follow existing model module pattern: `mod.rs` with `pub use` re-exports, separate `types.rs` and `queries.rs`
- Register in `src/models/mod.rs` — add `pub mod meeting;`
- Types must match the EAV query patterns used throughout the project (all fields are `String` for properties, `i64` for IDs/counts, `bool` for existence checks)
- No business logic in types — they are plain structs

**FORMAT:**
1. Create `src/models/meeting/types.rs`
2. Create `src/models/meeting/queries.rs` (skeleton with imports only)
3. Create `src/models/meeting/mod.rs`
4. Modify `src/models/mod.rs` — add `pub mod meeting;`

**FAILURE CONDITIONS:**
- Module not registered in `src/models/mod.rs` (tests can't import `ahlt::models::meeting`)
- Types use `Option<String>` where `COALESCE` will always produce a value (should be `String`)
- `has_minutes` is `String` instead of `bool`
- `cargo check` fails

**Types:**
```rust
// src/models/meeting/types.rs

/// A meeting linked to a ToR — used in cross-ToR list views.
pub struct MeetingListItem {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub meeting_date: String,
    pub status: String,
    pub tor_id: i64,
    pub tor_name: String,
    pub tor_label: String,
    pub agenda_count: i64,
    pub has_minutes: bool,
}

/// Full meeting detail for the detail page.
pub struct MeetingDetail {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub meeting_date: String,
    pub status: String,
    pub location: String,
    pub notes: String,
    pub tor_id: i64,
    pub tor_name: String,
    pub tor_label: String,
}
```

**Commit:** `git add src/models/meeting/ src/models/mod.rs && git commit -m "feat(model): add meeting types and module skeleton"`

---

### Task 4: Implement meeting create + find_by_id (TDD)

**GOAL:** Implement `meeting::create()` and `meeting::find_by_id()` queries with 3 passing tests. Success = `cargo test --test meeting_test` passes 3 tests: `test_create_meeting`, `test_find_meeting_by_id`, `test_find_meeting_by_id_not_found`.

**CONSTRAINTS:**
- Write tests FIRST in `tests/meeting_test.rs`, run to verify they fail, then implement
- Test infrastructure: copy `setup_test_db()` + `insert_entity/insert_prop/insert_relation` helpers from existing test files (each test file has its own copy — this is the project convention)
- `create()` must: insert entity with `entity_type='meeting'`, set `status` property to `"projected"`, set `meeting_date` property, create `belongs_to_tor` relation
- `find_by_id()` must: LEFT JOIN all properties + `belongs_to_tor` relation to get ToR info, return `Option<MeetingDetail>`
- Use `COALESCE` for all property JOINs (rusqlite fails on NULL for non-Option types)
- Entity `name` format: `{tor_name_slugified}-{date}`, `label` format: `{tor_name} — {date}`

**FORMAT:**
1. Create `tests/meeting_test.rs` with helpers + 3 tests
2. Implement `create()` and `find_by_id()` in `src/models/meeting/queries.rs`

**FAILURE CONDITIONS:**
- Tests pass without failing first (means tests don't actually exercise the code)
- `create()` doesn't set `status` to `"projected"` (workflow engine won't find initial status)
- `create()` doesn't create `belongs_to_tor` relation (listing queries will miss this meeting)
- `find_by_id()` doesn't filter by `entity_type = 'meeting'` (could return non-meeting entities)
- Empty `location`/`notes` stored as empty-string properties instead of being skipped (follow ToR `create` pattern: skip empty values except `status`)
- Test file doesn't compile because of missing `ahlt::models::meeting` import path

**Test code:**
```rust
use tempfile::TempDir;

const MIGRATIONS: &str = include_str!("../src/schema.sql");

fn setup_test_db() -> (TempDir, rusqlite::Connection) {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = dir.path().join("test.db");
    let conn = rusqlite::Connection::open(&db_path).expect("Failed to open test DB");
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;")
        .expect("Failed to set pragmas");
    conn.execute_batch(MIGRATIONS)
        .expect("Failed to run migrations");
    (dir, conn)
}

fn insert_entity(conn: &rusqlite::Connection, entity_type: &str, name: &str, label: &str) -> i64 {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES (?, ?, ?)",
        [entity_type, name, label],
    ).expect("Failed to insert entity");
    conn.last_insert_rowid()
}

fn insert_prop(conn: &rusqlite::Connection, entity_id: i64, key: &str, value: &str) {
    conn.execute(
        "INSERT OR REPLACE INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
        rusqlite::params![entity_id, key, value],
    ).expect("Failed to insert property");
}

fn insert_relation(conn: &rusqlite::Connection, relation_type_id: i64, source_id: i64, target_id: i64) -> i64 {
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
        rusqlite::params![relation_type_id, source_id, target_id],
    ).expect("Failed to insert relation");
    conn.last_insert_rowid()
}

/// Helper: create a ToR and required relation types
fn setup_tor_with_relation_types(conn: &rusqlite::Connection) -> (i64, i64, i64) {
    let belongs_to_tor_rt = insert_entity(conn, "relation_type", "belongs_to_tor", "Belongs to ToR");
    let scheduled_for_meeting_rt = insert_entity(conn, "relation_type", "scheduled_for_meeting", "Scheduled For Meeting");
    let tor_id = insert_entity(conn, "tor", "test-tor", "Test ToR");
    insert_prop(conn, tor_id, "meeting_cadence", "weekly");
    insert_prop(conn, tor_id, "status", "active");
    (tor_id, belongs_to_tor_rt, scheduled_for_meeting_rt)
}

#[test]
fn test_create_meeting() {
    let (_dir, conn) = setup_test_db();
    let (tor_id, belongs_to_tor_rt, _) = setup_tor_with_relation_types(&conn);

    let meeting_id = ahlt::models::meeting::create(
        &conn, tor_id, "2026-03-01", "Test ToR", "", "",
    ).expect("Failed to create meeting");

    assert!(meeting_id > 0);

    let entity_type: String = conn.query_row(
        "SELECT entity_type FROM entities WHERE id = ?1",
        [meeting_id], |row| row.get(0),
    ).unwrap();
    assert_eq!(entity_type, "meeting");

    let status: String = conn.query_row(
        "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = 'status'",
        [meeting_id], |row| row.get(0),
    ).unwrap();
    assert_eq!(status, "projected");

    let date: String = conn.query_row(
        "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = 'meeting_date'",
        [meeting_id], |row| row.get(0),
    ).unwrap();
    assert_eq!(date, "2026-03-01");

    let rel_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM relations WHERE source_id = ?1 AND target_id = ?2 AND relation_type_id = ?3",
        rusqlite::params![meeting_id, tor_id, belongs_to_tor_rt],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(rel_count, 1);
}

#[test]
fn test_find_meeting_by_id() {
    let (_dir, conn) = setup_test_db();
    let (tor_id, _, _) = setup_tor_with_relation_types(&conn);

    let meeting_id = ahlt::models::meeting::create(
        &conn, tor_id, "2026-03-15", "Test ToR", "Room A", "Discussion notes",
    ).expect("Failed to create meeting");

    let detail = ahlt::models::meeting::find_by_id(&conn, meeting_id)
        .expect("Query failed")
        .expect("Meeting not found");

    assert_eq!(detail.id, meeting_id);
    assert_eq!(detail.meeting_date, "2026-03-15");
    assert_eq!(detail.status, "projected");
    assert_eq!(detail.location, "Room A");
    assert_eq!(detail.notes, "Discussion notes");
    assert_eq!(detail.tor_id, tor_id);
}

#[test]
fn test_find_meeting_by_id_not_found() {
    let (_dir, conn) = setup_test_db();
    let result = ahlt::models::meeting::find_by_id(&conn, 99999)
        .expect("Query failed");
    assert!(result.is_none());
}
```

**Implementation — `create()`:**
```rust
pub fn create(
    conn: &Connection,
    tor_id: i64,
    meeting_date: &str,
    tor_name: &str,
    location: &str,
    notes: &str,
) -> rusqlite::Result<i64> {
    let name = format!("{}-{}", tor_name.to_lowercase().replace(' ', "-"), meeting_date);
    let label = format!("{} — {}", tor_name, meeting_date);

    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('meeting', ?1, ?2)",
        params![name, label],
    )?;
    let meeting_id = conn.last_insert_rowid();

    let props: Vec<(&str, &str)> = vec![
        ("meeting_date", meeting_date),
        ("status", "projected"),
        ("location", location),
        ("notes", notes),
    ];
    for (key, value) in props {
        if !value.is_empty() || key == "status" || key == "meeting_date" {
            conn.execute(
                "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
                params![meeting_id, key, value],
            )?;
        }
    }

    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor'), ?1, ?2)",
        params![meeting_id, tor_id],
    )?;

    Ok(meeting_id)
}
```

**Implementation — `find_by_id()`:**
```rust
pub fn find_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<MeetingDetail>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label, \
                COALESCE(p_date.value, '') AS meeting_date, \
                COALESCE(p_status.value, 'projected') AS status, \
                COALESCE(p_loc.value, '') AS location, \
                COALESCE(p_notes.value, '') AS notes, \
                COALESCE(tor.id, 0) AS tor_id, \
                COALESCE(tor.name, '') AS tor_name, \
                COALESCE(tor.label, '') AS tor_label \
         FROM entities e \
         LEFT JOIN entity_properties p_date ON e.id = p_date.entity_id AND p_date.key = 'meeting_date' \
         LEFT JOIN entity_properties p_status ON e.id = p_status.entity_id AND p_status.key = 'status' \
         LEFT JOIN entity_properties p_loc ON e.id = p_loc.entity_id AND p_loc.key = 'location' \
         LEFT JOIN entity_properties p_notes ON e.id = p_notes.entity_id AND p_notes.key = 'notes' \
         LEFT JOIN relations r_tor ON e.id = r_tor.source_id \
             AND r_tor.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
         LEFT JOIN entities tor ON r_tor.target_id = tor.id \
         WHERE e.id = ?1 AND e.entity_type = 'meeting'",
    )?;

    let mut rows = stmt.query_map(params![id], |row| {
        Ok(MeetingDetail {
            id: row.get("id")?,
            name: row.get("name")?,
            label: row.get("label")?,
            meeting_date: row.get("meeting_date")?,
            status: row.get("status")?,
            location: row.get("location")?,
            notes: row.get("notes")?,
            tor_id: row.get("tor_id")?,
            tor_name: row.get("tor_name")?,
            tor_label: row.get("tor_label")?,
        })
    })?;

    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}
```

**Verify:** `cargo test --test meeting_test` — 3 PASS

**Commit:** `git add src/models/meeting/queries.rs tests/meeting_test.rs && git commit -m "feat(model): meeting create + find_by_id with 3 tests"`

---

### Task 5: Implement meeting listing queries (TDD)

**GOAL:** Implement `meeting::find_by_tor()` and `meeting::find_upcoming_all()` with 2 passing tests. Success = `cargo test --test meeting_test` passes 5 total tests.

**CONSTRAINTS:**
- Write tests FIRST, run to verify failure, then implement
- `find_by_tor()` returns meetings ordered by date DESCENDING (newest first) — for ToR detail page
- `find_upcoming_all()` takes a `from_date` cutoff string, returns meetings ordered by date ASCENDING (soonest first) — for cross-ToR list
- Both queries must include `agenda_count` (COUNT of `scheduled_for_meeting` relations) and `has_minutes` (EXISTS on `minutes_of` relation) — these are computed inline via subqueries, NOT via N+1
- Use INNER JOIN on `belongs_to_tor` (meetings always have a ToR) — unlike `find_by_id` which uses LEFT JOIN

**FORMAT:**
1. Add 2 tests to `tests/meeting_test.rs`
2. Add `find_by_tor()` and `find_upcoming_all()` to `src/models/meeting/queries.rs`

**FAILURE CONDITIONS:**
- N+1 query pattern (separate query per meeting for agenda/minutes counts)
- `find_by_tor()` sorted ascending instead of descending
- `find_upcoming_all()` includes past meetings (before `from_date`)
- Missing `agenda_count` or `has_minutes` fields in results
- Subquery references wrong relation type name (must be exact: `scheduled_for_meeting`, `minutes_of`)

**Tests:**
```rust
#[test]
fn test_find_meetings_by_tor() {
    let (_dir, conn) = setup_test_db();
    let (tor_id, _, _) = setup_tor_with_relation_types(&conn);

    ahlt::models::meeting::create(&conn, tor_id, "2026-04-01", "Test ToR", "", "").unwrap();
    ahlt::models::meeting::create(&conn, tor_id, "2026-04-08", "Test ToR", "", "").unwrap();
    ahlt::models::meeting::create(&conn, tor_id, "2025-01-01", "Test ToR", "", "").unwrap();

    let meetings = ahlt::models::meeting::find_by_tor(&conn, tor_id)
        .expect("Query failed");

    assert_eq!(meetings.len(), 3);
    assert_eq!(meetings[0].meeting_date, "2026-04-08");
    assert_eq!(meetings[1].meeting_date, "2026-04-01");
    assert_eq!(meetings[2].meeting_date, "2025-01-01");
}

#[test]
fn test_find_upcoming_all_cross_tor() {
    let (_dir, conn) = setup_test_db();
    let (tor_id1, _, _) = setup_tor_with_relation_types(&conn);
    let tor_id2 = insert_entity(&conn, "tor", "test-tor-2", "Test ToR 2");
    insert_prop(&conn, tor_id2, "status", "active");

    ahlt::models::meeting::create(&conn, tor_id1, "2026-04-01", "Test ToR", "", "").unwrap();
    ahlt::models::meeting::create(&conn, tor_id2, "2026-04-02", "Test ToR 2", "", "").unwrap();
    ahlt::models::meeting::create(&conn, tor_id1, "2025-01-01", "Test ToR", "", "").unwrap();

    let upcoming = ahlt::models::meeting::find_upcoming_all(&conn, "2026-03-01")
        .expect("Query failed");

    assert_eq!(upcoming.len(), 2);
    assert_eq!(upcoming[0].meeting_date, "2026-04-01");
    assert_eq!(upcoming[1].meeting_date, "2026-04-02");
}
```

**Implementation:** See previous version of this plan for the full `find_by_tor` and `find_upcoming_all` query bodies — they use the same inline-subquery pattern for `agenda_count` and `has_minutes`.

**Verify:** `cargo test --test meeting_test` — 5 PASS

**Commit:** `git add src/models/meeting/queries.rs tests/meeting_test.rs && git commit -m "feat(model): meeting find_by_tor + find_upcoming_all with tests"`

---

### Task 6: Implement agenda assignment + update_status queries (TDD)

**GOAL:** Implement `assign_agenda()`, `remove_agenda()`, `find_agenda_points()`, `find_unassigned_agenda_points()`, and `update_status()` with 5 passing tests. Success = `cargo test --test meeting_test` passes 10 total tests.

**CONSTRAINTS:**
- Write all 5 tests FIRST, verify they fail, then implement
- `assign_agenda()` uses `INSERT OR IGNORE` (idempotent — assigning same point twice is a no-op)
- `remove_agenda()` deletes the `scheduled_for_meeting` relation
- `find_agenda_points()` returns all agenda points linked to a meeting via `scheduled_for_meeting`
- `find_unassigned_agenda_points()` finds agenda points belonging to a ToR (via `belongs_to_tor`) that have NO `scheduled_for_meeting` relation to ANY meeting
- `update_status()` uses upsert pattern: `INSERT ... ON CONFLICT DO UPDATE`
- Relation direction for `scheduled_for_meeting`: source = agenda_point, target = meeting (agenda point is "scheduled for" the meeting)
- `MeetingAgendaPoint` struct must be defined in `queries.rs` (not types.rs — it's query-specific)

**FORMAT:**
1. Add 5 tests to `tests/meeting_test.rs`
2. Add 5 functions + `MeetingAgendaPoint` struct to `src/models/meeting/queries.rs`

**FAILURE CONDITIONS:**
- Relation direction reversed (source/target swapped for `scheduled_for_meeting`)
- `assign_agenda()` errors on duplicate instead of ignoring
- `find_unassigned_agenda_points()` returns points assigned to OTHER meetings (should exclude all assigned points, not just for a specific meeting)
- `update_status()` doesn't create the property if it doesn't exist yet (must use upsert, not just UPDATE)
- `MeetingAgendaPoint` defined in types.rs instead of queries.rs

**Tests:**
```rust
#[test]
fn test_assign_agenda_to_meeting() {
    let (_dir, conn) = setup_test_db();
    let (tor_id, _, scheduled_for_meeting_rt) = setup_tor_with_relation_types(&conn);

    let meeting_id = ahlt::models::meeting::create(&conn, tor_id, "2026-04-01", "Test ToR", "", "").unwrap();
    let agenda_id = insert_entity(&conn, "agenda_point", "test-agenda", "Test Agenda Point");

    ahlt::models::meeting::assign_agenda(&conn, meeting_id, agenda_id)
        .expect("Failed to assign agenda");

    let rel_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM relations WHERE source_id = ?1 AND target_id = ?2 AND relation_type_id = ?3",
        rusqlite::params![agenda_id, meeting_id, scheduled_for_meeting_rt],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(rel_count, 1);

    let meetings = ahlt::models::meeting::find_by_tor(&conn, tor_id).unwrap();
    assert_eq!(meetings[0].agenda_count, 1);
}

#[test]
fn test_remove_agenda_from_meeting() {
    let (_dir, conn) = setup_test_db();
    let (tor_id, _, _) = setup_tor_with_relation_types(&conn);

    let meeting_id = ahlt::models::meeting::create(&conn, tor_id, "2026-04-01", "Test ToR", "", "").unwrap();
    let agenda_id = insert_entity(&conn, "agenda_point", "test-agenda", "Test Agenda Point");

    ahlt::models::meeting::assign_agenda(&conn, meeting_id, agenda_id).unwrap();
    ahlt::models::meeting::remove_agenda(&conn, meeting_id, agenda_id)
        .expect("Failed to remove agenda");

    let meetings = ahlt::models::meeting::find_by_tor(&conn, tor_id).unwrap();
    assert_eq!(meetings[0].agenda_count, 0);
}

#[test]
fn test_find_meeting_agenda_points() {
    let (_dir, conn) = setup_test_db();
    let (tor_id, _, _) = setup_tor_with_relation_types(&conn);

    let meeting_id = ahlt::models::meeting::create(&conn, tor_id, "2026-04-01", "Test ToR", "", "").unwrap();
    let agenda1 = insert_entity(&conn, "agenda_point", "agenda-1", "First Point");
    let agenda2 = insert_entity(&conn, "agenda_point", "agenda-2", "Second Point");

    ahlt::models::meeting::assign_agenda(&conn, meeting_id, agenda1).unwrap();
    ahlt::models::meeting::assign_agenda(&conn, meeting_id, agenda2).unwrap();

    let points = ahlt::models::meeting::find_agenda_points(&conn, meeting_id)
        .expect("Query failed");
    assert_eq!(points.len(), 2);
}

#[test]
fn test_update_meeting_status() {
    let (_dir, conn) = setup_test_db();
    let (tor_id, _, _) = setup_tor_with_relation_types(&conn);

    let meeting_id = ahlt::models::meeting::create(&conn, tor_id, "2026-04-01", "Test ToR", "", "").unwrap();

    ahlt::models::meeting::update_status(&conn, meeting_id, "confirmed")
        .expect("Failed to update status");

    let detail = ahlt::models::meeting::find_by_id(&conn, meeting_id).unwrap().unwrap();
    assert_eq!(detail.status, "confirmed");
}

#[test]
fn test_find_unassigned_agenda_points() {
    let (_dir, conn) = setup_test_db();
    let (tor_id, belongs_to_tor_rt, _) = setup_tor_with_relation_types(&conn);

    let meeting_id = ahlt::models::meeting::create(&conn, tor_id, "2026-04-01", "Test ToR", "", "").unwrap();

    let agenda1 = insert_entity(&conn, "agenda_point", "agenda-1", "First Point");
    insert_relation(&conn, belongs_to_tor_rt, agenda1, tor_id);
    let agenda2 = insert_entity(&conn, "agenda_point", "agenda-2", "Second Point");
    insert_relation(&conn, belongs_to_tor_rt, agenda2, tor_id);

    ahlt::models::meeting::assign_agenda(&conn, meeting_id, agenda1).unwrap();

    let unassigned = ahlt::models::meeting::find_unassigned_agenda_points(&conn, tor_id)
        .expect("Query failed");
    assert_eq!(unassigned.len(), 1);
    assert_eq!(unassigned[0].id, agenda2);
}
```

**Implementation:** See previous version of this plan for the full function bodies of `assign_agenda`, `remove_agenda`, `find_agenda_points`, `find_unassigned_agenda_points`, `update_status`, and the `MeetingAgendaPoint` struct.

**Verify:** `cargo test --test meeting_test` — 10 PASS

**Commit:** `git add src/models/meeting/queries.rs tests/meeting_test.rs && git commit -m "feat(model): agenda assignment + update_status queries with 5 tests"`

---

## Phase 3: Handlers + Templates

### Task 7: Create meeting handler module skeleton and register routes

**GOAL:** Create the meeting handler module with stub handlers and register all 7 routes in `main.rs`. Success = `cargo check` compiles with no errors (stub handlers return placeholder responses).

**CONSTRAINTS:**
- Follow existing handler module pattern: `mod.rs` with `pub use` re-exports, `list.rs` for GET list, `crud.rs` for all other handlers
- Register `pub mod meeting_handlers;` in `src/handlers/mod.rs`
- Route `/tor/{id}/meetings/confirm` must be registered BEFORE `/tor/{id}/meetings/{mid}` (path param `{mid}` would swallow "confirm" — same Actix gotcha as `/users/new` vs `/users/{id}`)
- All routes go inside the existing protected scope (after minutes routes)
- Stub handlers must have correct signatures matching their route patterns (e.g., `web::Path<(i64, i64)>` for routes with both `{id}` and `{mid}`)

**FORMAT:**
1. Create `src/handlers/meeting_handlers/mod.rs`
2. Create `src/handlers/meeting_handlers/list.rs` — stub `list()` handler
3. Create `src/handlers/meeting_handlers/crud.rs` — stub `detail()`, `confirm()`, `transition()`, `assign_agenda()`, `remove_agenda()`, `generate_minutes()` handlers
4. Modify `src/handlers/mod.rs` — add `pub mod meeting_handlers;`
5. Modify `src/main.rs` — register 7 routes in protected scope

**FAILURE CONDITIONS:**
- Route ordering: `/tor/{id}/meetings/{mid}` registered before `/tor/{id}/meetings/confirm` (Actix will match `{mid}="confirm"`)
- Handler signatures don't match route parameters (e.g., `confirm` takes `web::Path<i64>` for single `{id}`, not `(i64, i64)`)
- Handlers not exported from `mod.rs` (main.rs can't resolve `handlers::meeting_handlers::list`)
- Routes registered outside the protected auth scope
- `cargo check` fails

**Routes to register (in this order):**
```rust
// Meeting management — confirm BEFORE {mid} to avoid path param conflict
.route("/meetings", web::get().to(handlers::meeting_handlers::list))
.route("/tor/{id}/meetings/confirm", web::post().to(handlers::meeting_handlers::confirm))
.route("/tor/{id}/meetings/{mid}", web::get().to(handlers::meeting_handlers::detail))
.route("/tor/{id}/meetings/{mid}/transition", web::post().to(handlers::meeting_handlers::transition))
.route("/tor/{id}/meetings/{mid}/agenda/assign", web::post().to(handlers::meeting_handlers::assign_agenda))
.route("/tor/{id}/meetings/{mid}/agenda/remove", web::post().to(handlers::meeting_handlers::remove_agenda))
.route("/tor/{id}/meetings/{mid}/minutes/generate", web::post().to(handlers::meeting_handlers::generate_minutes))
```

**Commit:** `git add src/handlers/meeting_handlers/ src/handlers/mod.rs src/main.rs && git commit -m "feat(handlers): meeting handler module skeleton with routes"`

---

### Task 8: Implement meeting list handler + template

**GOAL:** Replace the list stub with a real handler that queries upcoming + past meetings and renders an HTML page. Success = navigating to `/meetings` in the browser shows a page with "Upcoming" and "Past" meeting tables.

**CONSTRAINTS:**
- Handler requires `meetings.view` permission
- Use `chrono::Local::now().format("%Y-%m-%d")` for the date cutoff between upcoming/past
- Upcoming = `find_upcoming_all(conn, &today)`, Past = needs a new `find_past_all(conn, &today)` query OR filter `find_upcoming_all` results — prefer adding a simple query
- Template extends `base.html` and follows existing BEM CSS patterns
- Table rows link to `/tor/{tor_id}/meetings/{id}` (scoped detail URL)
- Empty state messages when no meetings exist
- `MeetingsListTemplate` struct in `src/templates_structs.rs` following `PageContext` pattern
- No `innerHTML` in templates — use Askama template logic only

**FORMAT:**
1. Add `MeetingsListTemplate` to `src/templates_structs.rs`
2. Implement `list()` in `src/handlers/meeting_handlers/list.rs`
3. Create `templates/meetings/list.html`

**FAILURE CONDITIONS:**
- Missing permission check (unauthenticated users can see meeting data)
- Template doesn't extend `base.html` (loses nav, styles, header)
- Date comparison done as numeric instead of string (ISO-8601 date strings sort correctly as strings in SQLite)
- Table links go to wrong URL pattern (must be `/tor/{tor_id}/meetings/{id}`, not `/meetings/{id}`)
- `cargo build` fails (Askama compiles templates at build time)

**Commit:** `git add src/handlers/meeting_handlers/list.rs templates/meetings/list.html src/templates_structs.rs && git commit -m "feat(ui): meeting list page with upcoming/past sections"`

---

### Task 9: Implement meeting detail handler + template

**GOAL:** Replace the detail stub with a real handler showing meeting info, agenda points, protocol steps, workflow transitions, and minutes status. Success = navigating to `/tor/{id}/meetings/{mid}` shows the full meeting detail page with all 4 sections.

**CONSTRAINTS:**
- Handler requires `meetings.view` permission
- Verify `meeting.tor_id == tor_id` from URL (prevent accessing meeting via wrong ToR)
- Load agenda via `meeting::find_agenda_points(conn, meeting_id)`
- Load unassigned points via `meeting::find_unassigned_agenda_points(conn, tor_id)`
- Load protocol steps via `protocol::find_steps_for_tor(conn, tor_id)` (referenced, NOT copied)
- Load transitions via `workflow::find_available_transitions("meeting", &status, &permissions, &HashMap::new())`
- Check minutes via `minutes::find_by_meeting(meeting_id)`
- Transition buttons are forms POSTing to `/tor/{tor_id}/meetings/{mid}/transition` with hidden `new_status` + CSRF
- "Generate Minutes" button only shown when status is `completed` AND no minutes exist

**FORMAT:**
1. Add `MeetingDetailTemplate` to `src/templates_structs.rs`
2. Implement `detail()` in `src/handlers/meeting_handlers/crud.rs`
3. Create `templates/meetings/detail.html`

**FAILURE CONDITIONS:**
- Missing ToR ID mismatch check (user could access meeting through wrong ToR context)
- Protocol steps copied into meeting entity (must be referenced from ToR template)
- Workflow transitions loaded with wrong `entity_type_scope` (must be `"meeting"`)
- "Generate Minutes" button shown for non-completed meetings
- Transition forms missing CSRF token
- Unassigned agenda dropdown shown for completed/cancelled meetings (should only show for confirmed/in_progress)
- `cargo build` fails

**Commit:** `git add src/handlers/meeting_handlers/crud.rs templates/meetings/detail.html src/templates_structs.rs && git commit -m "feat(ui): meeting detail page with agenda, protocol, and minutes"`

---

### Task 10: Implement mutation handlers (confirm, transition, agenda, minutes)

**GOAL:** Implement all 5 POST handlers: `confirm`, `transition`, `assign_agenda`, `remove_agenda`, `generate_minutes`. Success = full meeting lifecycle works: confirm a projected meeting → assign agenda → start meeting → end meeting → generate minutes.

**CONSTRAINTS:**
- Every handler: CSRF validation via `csrf::validate_csrf(&session, &token)?`, permission check, audit logging
- `confirm`: creates meeting entity directly in "confirmed" status (projected meetings are virtual — "confirm" = create + immediate transition to confirmed)
- `confirm`: reads `meeting_date`, `tor_name` from form. Also reads optional `location` and `notes`
- `transition`: calls `workflow::validate_transition("meeting", ...)` before updating (returns `AppError::PermissionDenied` on invalid/unauthorized transition)
- `transition`: reads `new_status` from form data
- `assign_agenda`/`remove_agenda`: reads `agenda_point_id` from form data
- `generate_minutes`: requires `minutes.generate` permission, verifies meeting status is `completed`, verifies no minutes exist yet
- `generate_minutes`: calls `minutes::generate_scaffold(conn, meeting_id, tor_id, &meeting.label)` — this is the existing function, now properly wired to a real meeting entity
- All handlers redirect to the meeting detail page after success (except `generate_minutes` which redirects to `/minutes/{id}`)
- Define `#[derive(Deserialize)] struct` for each form's expected fields

**FORMAT:**
1. Implement `confirm()`, `transition()`, `assign_agenda()`, `remove_agenda()`, `generate_minutes()` in `src/handlers/meeting_handlers/crud.rs`

**FAILURE CONDITIONS:**
- Any handler missing CSRF validation (security vulnerability)
- Any handler missing permission check
- Any mutation handler missing audit logging
- `confirm` stores meeting with status "projected" instead of "confirmed"
- `transition` bypasses the workflow engine and updates status directly
- `generate_minutes` called with `meeting_id` of 0 or missing (the whole point of this feature is wiring real meeting IDs)
- `generate_minutes` allows generation for non-completed meetings
- Redirect URL incorrect (meeting detail requires `/tor/{tor_id}/meetings/{mid}` not just `/meetings/{mid}`)
- `cargo build` fails

**Commit:** `git add src/handlers/meeting_handlers/crud.rs && git commit -m "feat(handlers): meeting confirm, transition, agenda, and minutes handlers"`

---

## Phase 4: ToR Detail Integration

### Task 11: Add meetings section to ToR detail page

**GOAL:** Add a "Meetings" section to the ToR detail page showing confirmed meetings and a "Confirm Meeting" form. Success = visiting a ToR detail page shows the meetings section after dependencies, with a form to confirm meetings by date.

**CONSTRAINTS:**
- Add `meetings: Vec<MeetingListItem>` field to existing `TorDetailTemplate` in `src/templates_structs.rs`
- Load meetings in `tor_handlers/crud.rs` `detail()` handler via `meeting::find_by_tor(&conn, id)?`
- Section goes AFTER the Dependencies section in `templates/tor/detail.html`
- "Confirm Meeting" form has: date input, hidden `tor_name` field (from `tor.label`), hidden CSRF token, POST to `/tor/{tor.id}/meetings/confirm`
- Show confirmed/in_progress meetings as "Upcoming" with links to detail
- Show completed/cancelled meetings as "Past" with minutes status badge
- Use existing BEM CSS patterns (no new CSS classes unless unavoidable)

**FORMAT:**
1. Modify `src/templates_structs.rs` — add `meetings` field to `TorDetailTemplate`
2. Modify `src/handlers/tor_handlers/crud.rs` — load meetings in `detail()` handler
3. Modify `templates/tor/detail.html` — add meetings section

**FAILURE CONDITIONS:**
- Forgot to pass `meetings` to template struct (Askama compile error)
- "Confirm Meeting" form missing CSRF token or hidden `tor_name` field
- Form POSTs to wrong URL (must be `/tor/{tor.id}/meetings/confirm`)
- Section placed before Dependencies (should be after)
- Meeting links go to wrong URL pattern
- `cargo build` fails

**Commit:** `git add src/handlers/tor_handlers/crud.rs src/templates_structs.rs templates/tor/detail.html && git commit -m "feat(ui): add meetings section to ToR detail page"`

---

## Phase 5: End-to-End Verification

### Task 12: Full flow verification and final cleanup

**GOAL:** Verify the complete meeting lifecycle works end-to-end, all tests pass, clippy is clean. Success = manual walkthrough of the full flow + `cargo test` passes all tests + `cargo clippy` has no warnings.

**CONSTRAINTS:**
- Must delete staging DB and restart fresh (`rm -f data/staging/app.db && APP_ENV=staging cargo run`)
- Login as admin (admin / admin123)
- Test every route: ToR detail → confirm meeting → meeting detail → assign agenda → transition workflow → generate minutes → view minutes
- Also test cross-ToR list at `/meetings`
- Run `cargo test` (all existing + new tests must pass)
- Run `cargo clippy` and fix any warnings
- Do NOT add new tests in this task — model tests were covered in Phase 2

**FORMAT:**
1. Manual E2E walkthrough (12 steps below)
2. `cargo test` — all pass
3. `cargo clippy` — clean
4. Fix any issues found
5. Final commit

**FAILURE CONDITIONS:**
- Any existing test regresses (broke something in ToR detail handler, templates, etc.)
- Clippy warnings unfixed
- Meeting detail page 404s from ToR detail links
- Minutes generation fails with database error (meeting_id not wired correctly)
- "Generate Minutes" button appears on non-completed meetings
- Cross-ToR list shows wrong data or errors

**E2E walkthrough:**
1. `rm -f data/staging/app.db && APP_ENV=staging cargo run`
2. Login as admin
3. Navigate to any ToR detail page (e.g., one with weekly cadence)
4. Verify "Meetings" section visible with "Confirm Meeting" form
5. Confirm a meeting for a future date
6. Verify redirect to meeting detail page
7. Verify protocol steps shown (from ToR template), agenda section with assign dropdown
8. Assign an agenda point (if any exist for this ToR)
9. Transition: Confirm → Start Meeting → End Meeting
10. Click "Generate Minutes" button
11. Verify redirect to minutes view with 5 auto-generated sections
12. Navigate to `/meetings` — verify the meeting appears in the cross-ToR list

**Commit:** `git add -A && git commit -m "feat: complete meeting lifecycle — entities, workflow, agenda, minutes"`

---

## Summary

| Phase | Tasks | Tests | Commits |
|-------|-------|-------|---------|
| 1. Seed Data | 1-2 | — | 2 |
| 2. Model Layer (TDD) | 3-6 | 10 | 4 |
| 3. Handlers + Templates | 7-10 | — | 4 |
| 4. ToR Integration | 11 | — | 1 |
| 5. Verification | 12 | all pass | 1 |
| **Total** | **12 tasks** | **~10 model tests** | **~12 commits** |
