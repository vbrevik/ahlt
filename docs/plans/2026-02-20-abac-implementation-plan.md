# ABAC Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add attribute-based access control (ABAC) so ToR members can perform meeting lifecycle operations based on their function's `can_*` capability flags, without requiring global `tor.edit`.

**Architecture:** A new `src/auth/abac.rs` module provides three functions: `has_resource_capability` (generic EAV graph query), `load_tor_capabilities` (bulk capability loader for template context), and `require_tor_capability` (handler helper with global bypass). Meeting and minutes handlers replace `require_permission("tor.edit")` calls with `require_tor_capability`. The meeting detail template gets a `tor_capabilities: Permissions` field so buttons show/hide correctly for members.

**Tech Stack:** Rust, rusqlite, actix-web 4, Askama 0.14. No new dependencies.

**Design doc:** `docs/plans/2026-02-20-abac-design.md`

---

### Task 1: Write failing tests for `has_resource_capability`

**Files:**
- Create: `tests/abac_test.rs`

Understand the test pattern by reading `tests/common/mod.rs` (especially `setup_test_db` and `seed_base_entities`) before writing tests. The test DB already seeds `fills_position` and `belongs_to_tor` relation types.

**Step 1: Create the test file**

```rust
// tests/abac_test.rs
mod common;

use ahlt::auth::abac;
use rusqlite::params;

// Helper: create a tor_function entity with given can_* property
fn create_function(conn: &rusqlite::Connection, name: &str, capability: &str, value: &str) -> i64 {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('tor_function', ?1, ?1)",
        [name],
    ).unwrap();
    let func_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE name = ?1 AND entity_type = 'tor_function'",
        [name],
        |r| r.get(0),
    ).unwrap();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
        params![func_id, capability, value],
    ).unwrap();
    func_id
}

// Helper: create a user entity
fn create_user(conn: &rusqlite::Connection, name: &str) -> i64 {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('user', ?1, ?1)",
        [name],
    ).unwrap();
    conn.query_row(
        "SELECT id FROM entities WHERE name = ?1 AND entity_type = 'user'",
        [name],
        |r| r.get(0),
    ).unwrap()
}

// Helper: create a tor entity
fn create_tor(conn: &rusqlite::Connection, name: &str) -> i64 {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('tor', ?1, ?1)",
        [name],
    ).unwrap();
    conn.query_row(
        "SELECT id FROM entities WHERE name = ?1 AND entity_type = 'tor'",
        [name],
        |r| r.get(0),
    ).unwrap()
}

// Helper: get relation type id
fn rel_type(conn: &rusqlite::Connection, name: &str) -> i64 {
    conn.query_row(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = ?1",
        [name],
        |r| r.get(0),
    ).unwrap()
}

// Helper: link user fills_position function
fn fills_position(conn: &rusqlite::Connection, user_id: i64, func_id: i64) {
    let rt = rel_type(conn, "fills_position");
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
        params![rt, user_id, func_id],
    ).unwrap();
}

// Helper: link function belongs_to_tor tor
fn belongs_to_tor(conn: &rusqlite::Connection, func_id: i64, tor_id: i64) {
    let rt = rel_type(conn, "belongs_to_tor");
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
        params![rt, func_id, tor_id],
    ).unwrap();
}

#[test]
fn test_has_capability_true() {
    let (_dir, conn) = common::setup_test_db();
    let user_id = create_user(&conn, "alice");
    let tor_id = create_tor(&conn, "tor_alpha");
    let func_id = create_function(&conn, "chair_alpha", "can_call_meetings", "true");
    fills_position(&conn, user_id, func_id);
    belongs_to_tor(&conn, func_id, tor_id);

    let result = abac::has_resource_capability(
        &conn, user_id, tor_id, "belongs_to_tor", "can_call_meetings"
    ).unwrap();
    assert!(result);
}

#[test]
fn test_has_capability_false_when_flag_is_false() {
    let (_dir, conn) = common::setup_test_db();
    let user_id = create_user(&conn, "bob");
    let tor_id = create_tor(&conn, "tor_beta");
    let func_id = create_function(&conn, "member_beta", "can_call_meetings", "false");
    fills_position(&conn, user_id, func_id);
    belongs_to_tor(&conn, func_id, tor_id);

    let result = abac::has_resource_capability(
        &conn, user_id, tor_id, "belongs_to_tor", "can_call_meetings"
    ).unwrap();
    assert!(!result);
}

#[test]
fn test_has_capability_false_when_not_member() {
    let (_dir, conn) = common::setup_test_db();
    let user_id = create_user(&conn, "charlie");
    let tor_id = create_tor(&conn, "tor_gamma");
    // No fills_position or belongs_to_tor

    let result = abac::has_resource_capability(
        &conn, user_id, tor_id, "belongs_to_tor", "can_call_meetings"
    ).unwrap();
    assert!(!result);
}

#[test]
fn test_boundary_isolation_different_tor() {
    let (_dir, conn) = common::setup_test_db();
    let user_id = create_user(&conn, "diana");
    let tor_a = create_tor(&conn, "tor_a");
    let tor_b = create_tor(&conn, "tor_b");
    let func_id = create_function(&conn, "chair_a", "can_call_meetings", "true");
    fills_position(&conn, user_id, func_id);
    belongs_to_tor(&conn, func_id, tor_a); // member of A, not B

    let result = abac::has_resource_capability(
        &conn, user_id, tor_b, "belongs_to_tor", "can_call_meetings"
    ).unwrap();
    assert!(!result, "capability in tor_a should not grant access to tor_b");
}

#[test]
fn test_missing_capability_key_returns_false() {
    let (_dir, conn) = common::setup_test_db();
    let user_id = create_user(&conn, "eve");
    let tor_id = create_tor(&conn, "tor_delta");
    // Function only has can_manage_agenda, not can_call_meetings
    let func_id = create_function(&conn, "secretary_delta", "can_manage_agenda", "true");
    fills_position(&conn, user_id, func_id);
    belongs_to_tor(&conn, func_id, tor_id);

    let result = abac::has_resource_capability(
        &conn, user_id, tor_id, "belongs_to_tor", "can_call_meetings"
    ).unwrap();
    assert!(!result);
}

#[test]
fn test_load_tor_capabilities_returns_all_true_flags() {
    let (_dir, conn) = common::setup_test_db();
    let user_id = create_user(&conn, "frank");
    let tor_id = create_tor(&conn, "tor_epsilon");

    // Function with multiple caps, one false
    let func_id = create_function(&conn, "chair_epsilon", "can_call_meetings", "true");
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'can_manage_agenda', 'true')",
        params![func_id],
    ).unwrap();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'can_record_decisions', 'false')",
        params![func_id],
    ).unwrap();
    fills_position(&conn, user_id, func_id);
    belongs_to_tor(&conn, func_id, tor_id);

    let caps = abac::load_tor_capabilities(&conn, user_id, tor_id).unwrap();
    assert!(caps.has("can_call_meetings"));
    assert!(caps.has("can_manage_agenda"));
    assert!(!caps.has("can_record_decisions"));
}

#[test]
fn test_load_tor_capabilities_empty_for_non_member() {
    let (_dir, conn) = common::setup_test_db();
    let user_id = create_user(&conn, "grace");
    let tor_id = create_tor(&conn, "tor_zeta");

    let caps = abac::load_tor_capabilities(&conn, user_id, tor_id).unwrap();
    assert!(!caps.has("can_call_meetings"));
}
```

**Step 2: Run tests to verify they fail**

```bash
cargo test --test abac_test 2>&1 | tail -20
```

Expected: compile error `unresolved module abac` — confirms tests are written before implementation.

---

### Task 2: Implement `src/auth/abac.rs`

**Files:**
- Create: `src/auth/abac.rs`
- Modify: `src/auth/mod.rs`

**Step 1: Add module declaration to `src/auth/mod.rs`**

Add `pub mod abac;` as the first line of `src/auth/mod.rs`. The file currently has:
```
pub mod csrf;
pub mod middleware;
pub mod password;
pub mod rate_limit;
pub mod session;
pub mod validate;
```

Add `pub mod abac;` at the top.

**Step 2: Create `src/auth/abac.rs`**

```rust
//! Attribute-Based Access Control (ABAC) for resource-scoped capability checks.
//!
//! # How it works
//! Checks follow a two-phase pattern:
//! 1. Global bypass: if the user has a bypass permission (e.g. "tor.edit"), allow immediately.
//! 2. EAV graph: user fills a function entity that belongs to the resource AND has the
//!    required `can_*` property set to `"true"`.
//!
//! # Generalisation
//! Pass different `belongs_to_rel` strings to reuse `has_resource_capability` for other
//! entity types as they adopt ABAC (e.g. "belongs_to_governance").

use actix_session::Session;
use rusqlite::{params, Connection};

use crate::auth::session::{get_user_id, require_permission, Permissions};
use crate::errors::AppError;

/// Returns `true` if `user_id` fills a function in the given resource that has
/// `capability = "true"` in its entity_properties.
///
/// `belongs_to_rel` is the relation type name that connects the function entity
/// to the resource (e.g. `"belongs_to_tor"`).
pub fn has_resource_capability(
    conn: &Connection,
    user_id: i64,
    resource_id: i64,
    belongs_to_rel: &str,
    capability: &str,
) -> Result<bool, AppError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM relations r_fills
         JOIN relations r_belongs ON r_belongs.source_id = r_fills.target_id
         JOIN entity_properties ep ON ep.entity_id = r_fills.target_id
         WHERE r_fills.source_id = ?1
           AND r_belongs.target_id = ?2
           AND r_fills.relation_type_id = (
               SELECT id FROM entities
               WHERE entity_type = 'relation_type' AND name = 'fills_position')
           AND r_belongs.relation_type_id = (
               SELECT id FROM entities
               WHERE entity_type = 'relation_type' AND name = ?3)
           AND ep.key = ?4
           AND ep.value = 'true'",
        params![user_id, resource_id, belongs_to_rel, capability],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Returns a `Permissions` struct containing all `can_*` capability flags
/// that the user holds (value = `"true"`) in the given ToR.
///
/// Use this to populate template context in a single query instead of N calls
/// to `has_resource_capability`.
pub fn load_tor_capabilities(
    conn: &Connection,
    user_id: i64,
    tor_id: i64,
) -> Result<Permissions, AppError> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT ep.key
         FROM relations r_fills
         JOIN relations r_tor ON r_tor.source_id = r_fills.target_id
         JOIN entity_properties ep ON ep.entity_id = r_fills.target_id
         WHERE r_fills.source_id = ?1
           AND r_tor.target_id = ?2
           AND r_fills.relation_type_id = (
               SELECT id FROM entities
               WHERE entity_type = 'relation_type' AND name = 'fills_position')
           AND r_tor.relation_type_id = (
               SELECT id FROM entities
               WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor')
           AND ep.key LIKE 'can_%'
           AND ep.value = 'true'",
    )?;
    let keys: Vec<String> = stmt
        .query_map(params![user_id, tor_id], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(Permissions(keys))
}

/// Check global `tor.edit` bypass first, then ABAC membership capability.
///
/// Returns `Ok(())` if:
/// - The session holds `tor.edit`, OR
/// - The user fills a function in `tor_id` that has `capability = "true"`.
///
/// Use this in meeting and ToR lifecycle handlers (not structural ToR handlers,
/// which remain exclusively behind `require_permission("tor.edit")`).
pub fn require_tor_capability(
    conn: &Connection,
    session: &Session,
    tor_id: i64,
    capability: &str,
) -> Result<(), AppError> {
    if require_permission(session, "tor.edit").is_ok() {
        return Ok(());
    }
    let user_id = get_user_id(session)
        .ok_or_else(|| AppError::Session("No user_id in session".to_string()))?;
    let has_cap =
        has_resource_capability(conn, user_id, tor_id, "belongs_to_tor", capability)?;
    if has_cap {
        Ok(())
    } else {
        Err(AppError::PermissionDenied(format!(
            "Requires {} capability in this ToR",
            capability
        )))
    }
}
```

**Step 3: Run the failing tests — they should now pass**

```bash
cargo test --test abac_test 2>&1 | tail -20
```

Expected: all 7 tests pass.

**Step 4: Commit**

```bash
git add src/auth/abac.rs src/auth/mod.rs tests/abac_test.rs
git commit -m "feat(abac): add ABAC module with has_resource_capability and require_tor_capability"
```

---

### Task 3: Migrate `confirm` and `transition` handlers

**Files:**
- Modify: `src/handlers/meeting_handlers/crud.rs` (lines 6, 122, 327)

**Step 1: Add import**

In `crud.rs`, line 6 currently reads:
```rust
use crate::auth::session::{get_permissions, get_user_id, require_permission};
```

Change to:
```rust
use crate::auth::session::{get_permissions, get_user_id, require_permission};
use crate::auth::abac;
```

**Step 2: Update `confirm` handler (line 122)**

Replace:
```rust
    require_permission(&session, "tor.edit")?;
```
(the line inside `pub async fn confirm`) with:
```rust
    abac::require_tor_capability(&conn, &session, tor_id, "can_call_meetings")?;
```

Wait — `conn` is obtained on the line *after* the permission check. Reorder so `conn` comes first:

Replace the opening of `confirm` (lines 116–127):
```rust
pub async fn confirm(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<ConfirmForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let tor_id = path.into_inner();
    let conn = pool.get()?;
```
with:
```rust
pub async fn confirm(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<ConfirmForm>,
) -> Result<HttpResponse, AppError> {
    let tor_id = path.into_inner();
    let conn = pool.get()?;
    abac::require_tor_capability(&conn, &session, tor_id, "can_call_meetings")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;
```

**Step 3: Update `transition` handler (line 327)**

Same reorder pattern. Replace:
```rust
pub async fn transition(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<TransitionForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, mid) = path.into_inner();
    let conn = pool.get()?;
```
with:
```rust
pub async fn transition(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<TransitionForm>,
) -> Result<HttpResponse, AppError> {
    let (tor_id, mid) = path.into_inner();
    let conn = pool.get()?;
    abac::require_tor_capability(&conn, &session, tor_id, "can_call_meetings")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;
```

**Step 4: Build check**

```bash
cargo check 2>&1 | tail -15
```

Expected: no errors.

**Step 5: Commit**

```bash
git add src/handlers/meeting_handlers/crud.rs
git commit -m "feat(abac): gate confirm and transition handlers behind can_call_meetings"
```

---

### Task 4: Migrate `confirm_calendar` handler

**Files:**
- Modify: `src/handlers/meeting_handlers/crud.rs` (lines 189–313)

The `confirm_calendar` handler returns JSON on failure (not `AppError`), so we cannot use `require_tor_capability` with `?`. Use `has_resource_capability` directly.

**Step 1: Update `confirm_calendar` — extract `tor_id` and `conn` first, then ABAC check**

Replace lines 189–216 (the function opening and its permission/CSRF block):

```rust
pub async fn confirm_calendar(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<CalendarConfirmForm>,
) -> Result<HttpResponse, AppError> {
    // Check permission and CSRF first
    if require_permission(&session, "tor.edit").is_err() {
        return Ok(HttpResponse::Forbidden()
            .content_type("application/json")
            .body(serde_json::json!({"ok": false, "error": "Permission denied"}).to_string()));
    }

    if csrf::validate_csrf(&session, &form.csrf_token).is_err() {
        return Ok(HttpResponse::Forbidden()
            .content_type("application/json")
            .body(serde_json::json!({"ok": false, "error": "CSRF token invalid"}).to_string()));
    }

    let tor_id = path.into_inner();
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .body(serde_json::json!({"ok": false, "error": "Database error"}).to_string()));
        }
    };
    let current_user_id = get_user_id(&session).unwrap_or(0);
```

with:

```rust
pub async fn confirm_calendar(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<CalendarConfirmForm>,
) -> Result<HttpResponse, AppError> {
    let tor_id = path.into_inner();
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => {
            return Ok(HttpResponse::InternalServerError()
                .content_type("application/json")
                .body(serde_json::json!({"ok": false, "error": "Database error"}).to_string()));
        }
    };
    let current_user_id = get_user_id(&session).unwrap_or(0);

    // ABAC check: global tor.edit bypass OR member with can_call_meetings
    let has_access = require_permission(&session, "tor.edit").is_ok()
        || abac::has_resource_capability(
            &conn,
            current_user_id,
            tor_id,
            "belongs_to_tor",
            "can_call_meetings",
        )
        .unwrap_or(false);
    if !has_access {
        return Ok(HttpResponse::Forbidden()
            .content_type("application/json")
            .body(serde_json::json!({"ok": false, "error": "Permission denied"}).to_string()));
    }

    if csrf::validate_csrf(&session, &form.csrf_token).is_err() {
        return Ok(HttpResponse::Forbidden()
            .content_type("application/json")
            .body(serde_json::json!({"ok": false, "error": "CSRF token invalid"}).to_string()));
    }
```

**Step 2: Build check**

```bash
cargo check 2>&1 | tail -15
```

**Step 3: Commit**

```bash
git add src/handlers/meeting_handlers/crud.rs
git commit -m "feat(abac): gate confirm_calendar handler behind can_call_meetings"
```

---

### Task 5: Migrate agenda and roll call handlers

**Files:**
- Modify: `src/handlers/meeting_handlers/crud.rs` (lines 387–577)

**Step 1: Update `assign_agenda` (line 393)**

Replace:
```rust
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, mid) = path.into_inner();
    let conn = pool.get()?;
```
with:
```rust
    let (tor_id, mid) = path.into_inner();
    let conn = pool.get()?;
    abac::require_tor_capability(&conn, &session, tor_id, "can_manage_agenda")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;
```

**Step 2: Update `remove_agenda` (line 435)**

Same pattern as `assign_agenda` — replace the `require_permission` + path/conn ordering:
```rust
    let (tor_id, mid) = path.into_inner();
    let conn = pool.get()?;
    abac::require_tor_capability(&conn, &session, tor_id, "can_manage_agenda")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;
```

**Step 3: Update `save_roll_call` (line 547)**

`save_roll_call` already extracts `tor_id` after the permission check. Reorder:

Replace:
```rust
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;
    let (tor_id, meeting_id) = path.into_inner();
    let conn = pool.get()?;
```
with:
```rust
    let (tor_id, meeting_id) = path.into_inner();
    let conn = pool.get()?;
    abac::require_tor_capability(&conn, &session, tor_id, "can_record_decisions")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;
```

**Step 4: Update `generate_minutes` in `crud.rs` (line 477)**

This uses `require_permission("minutes.generate")` — replace with:
```rust
    let (tor_id, mid) = path.into_inner();
    let conn = pool.get()?;
    abac::require_tor_capability(&conn, &session, tor_id, "can_record_decisions")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;
```

Remove the old `require_permission` line and reorder `path.into_inner()` and `pool.get()` to come first.

**Step 5: Build check**

```bash
cargo check 2>&1 | tail -15
```

**Step 6: Commit**

```bash
git add src/handlers/meeting_handlers/crud.rs
git commit -m "feat(abac): gate agenda, roll call, and minutes generation behind ABAC capabilities"
```

---

### Task 6: Migrate minutes handlers (`save_attendance`, `save_action_items`)

**Files:**
- Modify: `src/handlers/minutes_handlers/crud.rs` (lines 244–307)
- Modify: add `use crate::models::meeting;` and `use crate::auth::abac;` imports

These handlers only receive `minutes_id`, not `tor_id`. Resolve via: `minutes → meeting_id → MeetingDetail.tor_id`.

**Step 1: Add missing imports** at the top of `minutes/crud.rs`:

```rust
use crate::auth::abac;
use crate::models::meeting;
```

**Step 2: Update `save_attendance` (line 244)**

Replace the body of `save_attendance` up through the existing `require_permission`:

```rust
pub async fn save_attendance(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<AttendanceForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "minutes.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;
    let minutes_id = path.into_inner();
    let conn = pool.get()?;
```

with:

```rust
pub async fn save_attendance(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<AttendanceForm>,
) -> Result<HttpResponse, AppError> {
    let minutes_id = path.into_inner();
    let conn = pool.get()?;
    // Resolve tor_id for ABAC check
    let mins = minutes::find_by_id(&conn, minutes_id)?.ok_or(AppError::NotFound)?;
    let meeting = meeting::find_by_id(&conn, mins.meeting_id)?.ok_or(AppError::NotFound)?;
    abac::require_tor_capability(&conn, &session, meeting.tor_id, "can_record_decisions")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;
```

**Step 3: Update `save_action_items` (line 277)**

Same pattern:

```rust
pub async fn save_action_items(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<ActionItemsForm>,
) -> Result<HttpResponse, AppError> {
    let minutes_id = path.into_inner();
    let conn = pool.get()?;
    // Resolve tor_id for ABAC check
    let mins = minutes::find_by_id(&conn, minutes_id)?.ok_or(AppError::NotFound)?;
    let meeting = meeting::find_by_id(&conn, mins.meeting_id)?.ok_or(AppError::NotFound)?;
    abac::require_tor_capability(&conn, &session, meeting.tor_id, "can_record_decisions")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;
```

**Step 4: Build check**

```bash
cargo check 2>&1 | tail -15
```

**Step 5: Commit**

```bash
git add src/handlers/minutes_handlers/crud.rs
git commit -m "feat(abac): gate attendance and action items handlers behind can_record_decisions"
```

---

### Task 7: Add `tor_capabilities` to meeting detail template context

**Files:**
- Modify: `src/templates_structs.rs` (line 518)
- Modify: `src/handlers/meeting_handlers/crud.rs` (the `detail` handler, lines 63–106)

**Step 1: Add `tor_capabilities` field to `MeetingDetailTemplate`**

In `src/templates_structs.rs`, the struct at line 518:

```rust
pub struct MeetingDetailTemplate {
    pub ctx: PageContext,
    pub meeting: MeetingDetail,
    pub agenda_points: Vec<MeetingAgendaPoint>,
    pub unassigned_points: Vec<MeetingAgendaPoint>,
    pub protocol_steps: Vec<ProtocolStep>,
    pub transitions: Vec<AvailableTransition>,
    pub minutes: Option<Minutes>,
    pub tor_id: i64,
}
```

Add the new field:

```rust
pub struct MeetingDetailTemplate {
    pub ctx: PageContext,
    pub meeting: MeetingDetail,
    pub agenda_points: Vec<MeetingAgendaPoint>,
    pub unassigned_points: Vec<MeetingAgendaPoint>,
    pub protocol_steps: Vec<ProtocolStep>,
    pub transitions: Vec<AvailableTransition>,
    pub minutes: Option<Minutes>,
    pub tor_id: i64,
    pub tor_capabilities: crate::auth::session::Permissions,
}
```

**Step 2: Populate `tor_capabilities` in the `detail` handler**

In `src/handlers/meeting_handlers/crud.rs`, the `detail` handler currently builds `MeetingDetailTemplate` at lines 95–104:

```rust
    let tmpl = MeetingDetailTemplate {
        ctx,
        meeting,
        agenda_points,
        unassigned_points,
        protocol_steps,
        transitions,
        minutes: existing_minutes,
        tor_id,
    };
```

Before this block, after `let existing_minutes = ...`, add:

```rust
    let user_id = get_user_id(&session).unwrap_or(0);
    let tor_capabilities = abac::load_tor_capabilities(&conn, user_id, tor_id)
        .unwrap_or_default();
```

Then update the struct initialisation to include the new field:

```rust
    let tmpl = MeetingDetailTemplate {
        ctx,
        meeting,
        agenda_points,
        unassigned_points,
        protocol_steps,
        transitions,
        minutes: existing_minutes,
        tor_id,
        tor_capabilities,
    };
```

**Step 3: Build check**

```bash
cargo check 2>&1 | tail -15
```

Expected: no errors (Askama compiles templates as part of `cargo check`).

**Step 4: Commit**

```bash
git add src/templates_structs.rs src/handlers/meeting_handlers/crud.rs
git commit -m "feat(abac): add tor_capabilities to meeting detail template context"
```

---

### Task 8: Update meeting detail template button visibility

**Files:**
- Modify: `templates/meetings/detail.html`

**Step 1: Find all `ctx.permissions.has("tor.edit")` guards**

```bash
grep -n 'tor\.edit' templates/meetings/detail.html
```

**Step 2: Update each guard to include ABAC**

For buttons that call meetings (confirm/transition actions), change:
```html
{% if ctx.permissions.has("tor.edit") %}
```
to:
```html
{% if ctx.permissions.has("tor.edit") || tor_capabilities.has("can_call_meetings") %}
```

For roll call section buttons (lines 295 and 305):
```html
{% if ctx.permissions.has("tor.edit") %}
```
to:
```html
{% if ctx.permissions.has("tor.edit") || tor_capabilities.has("can_record_decisions") %}
```

For agenda management buttons, change `tor.edit` guard to:
```html
{% if ctx.permissions.has("tor.edit") || tor_capabilities.has("can_manage_agenda") %}
```

**Note on Askama:** Askama does not support `||` in `{% if %}` conditions. Use nested `{% if %}` blocks:

```html
{% if ctx.permissions.has("tor.edit") %}
  <!-- button -->
{% else %}{% if tor_capabilities.has("can_call_meetings") %}
  <!-- button -->
{% endif %}{% endif %}
```

Or extract a local bool — but Askama 0.14 doesn't support `{% let %}`. The nested approach is the correct one. See `CLAUDE.md` critical rules: "No `&&` in Askama" — same applies to `||`.

The pattern to use:

```html
{% if ctx.permissions.has("tor.edit") %}
<button ...>Confirm</button>
{% else %}{% if tor_capabilities.has("can_call_meetings") %}
<button ...>Confirm</button>
{% endif %}{% endif %}
```

If the button block is long, extract into a block variable or accept the duplication for now.

**Step 3: Build check**

```bash
cargo check 2>&1 | tail -15
```

**Step 4: Commit**

```bash
git add templates/meetings/detail.html
git commit -m "feat(abac): update meeting detail template to show buttons for ABAC-capable members"
```

---

### Task 9: Run full test suite and verify

**Step 1: Run all tests**

```bash
cargo test 2>&1 | tail -20
```

Expected: all existing tests plus the 7 new ABAC tests pass. Count should be ≥159.

**Step 2: Verify with clippy**

```bash
cargo clippy 2>&1 | grep -E "^error|warning\[" | head -20
```

Fix any warnings in files touched during this implementation.

**Step 3: Final commit if any cleanups needed**

```bash
git add -p   # stage only clippy fixes
git commit -m "fix(clippy): resolve warnings in ABAC implementation"
```

---

## Files Changed Summary

| File | Change |
|---|---|
| `src/auth/abac.rs` | New — `has_resource_capability`, `load_tor_capabilities`, `require_tor_capability` |
| `src/auth/mod.rs` | Add `pub mod abac;` |
| `src/handlers/meeting_handlers/crud.rs` | Replace `require_permission("tor.edit")` in 6 handlers; add `tor_capabilities` to `detail` |
| `src/handlers/minutes_handlers/crud.rs` | Replace `require_permission` in `save_attendance`, `save_action_items` with ABAC |
| `src/templates_structs.rs` | Add `tor_capabilities: Permissions` to `MeetingDetailTemplate` |
| `templates/meetings/detail.html` | Add ABAC condition to action button visibility guards |
| `tests/abac_test.rs` | New — 7 unit/integration tests |

## Handlers NOT changed (remain behind global `tor.edit`)

- All ToR structural handlers: `tor_handlers/crud.rs`, `tor_handlers/members.rs`, `tor_handlers/protocol.rs`, `tor_handlers/dependencies.rs`, `tor_handlers/presentation.rs`
- Meeting list and calendar view handlers (read-only)
- Minutes view and status handlers (`view_minutes`, `update_minutes_status`, `update_section`)
