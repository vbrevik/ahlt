# Users / Roles / Role Builder Separation — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Separate user management, role assignment, and role definition into three independent pages with distinct permissions, enabling multi-role support where users hold multiple roles simultaneously.

**Architecture:** Remove role handling from Users page. Create a dedicated Roles assignment page with By Role / By User tabs and menu preview. Make Role Builder the sole path for role CRUD. Change `has_role` from 1:1 to many-to-many. Aggregate permissions across all assigned roles in the session.

**Tech Stack:** Rust/Actix-web 4, Askama 0.14 templates, SQLite (rusqlite), EAV data model, BEM CSS

**Design doc:** `docs/plans/2026-02-20-users-roles-separation-design.md`

---

### Task 1: Add `roles.assign` Permission to Ontology Seed

**GOAL:** A new `roles.assign` permission entity exists in the seed data so the Roles assignment page can be gated independently from `roles.manage`.
Success = after DB reset + restart, `SELECT * FROM entities WHERE name = 'roles.assign'` returns a row.

**CONSTRAINTS:**
- Follow existing permission naming pattern: `{module}.{action}`
- Add to `data/seed/ontology.json` only (not staging.json)
- Use `group_name: "Roles"` property to match existing role permissions

**FORMAT:**
- Modify: `data/seed/ontology.json` — add permission entity + `group_name` property

**FAILURE CONDITIONS:**
- Permission name doesn't match `{module}.{action}` pattern
- Missing `group_name` property (would appear under "Other" in permission matrix)
- Added to staging.json instead of ontology.json

**Step 1: Add permission entity to ontology seed**

Open `data/seed/ontology.json` and find the permissions section (search for `"entity_type": "permission"`). Add after the last `roles.*` permission:

```json
{
  "entity_type": "permission",
  "name": "roles.assign",
  "label": "Assign Roles",
  "properties": { "group_name": "Roles" }
}
```

**Step 2: Verify seed format is valid JSON**

Run: `python3 -c "import json; json.load(open('data/seed/ontology.json'))"`
Expected: No output (valid JSON)

**Step 3: Delete staging DB and restart to verify**

```bash
rm -f data/staging/app.db
APP_ENV=staging cargo run &
sleep 3
# In another terminal or after Ctrl+C:
sqlite3 data/staging/app.db "SELECT id, name, label FROM entities WHERE name = 'roles.assign';"
```
Expected: One row with name `roles.assign` and label `Assign Roles`

**Step 4: Commit**

```bash
git add data/seed/ontology.json
git commit -m "feat: add roles.assign permission to ontology seed"
```

---

### Task 2: Add `nav_item` for Roles Assignment Page

**GOAL:** A "Roles" nav item appears in the Admin sidebar gated by `roles.assign`, separate from the existing "Role Builder" item gated by `roles.manage`.
Success = user with `roles.assign` (but not `roles.manage`) sees "Roles" in sidebar but not "Role Builder".

**CONSTRAINTS:**
- Nav items are entities with `requires_permission` relations — not a property (see MEMORY.md)
- Must be a child of the Admin module (has `parent` property pointing to Admin nav item)
- Must have correct `sort_order` so it appears between Users and Role Builder

**FORMAT:**
- Modify: `data/seed/ontology.json` — add nav_item entity + relation to `roles.assign` permission

**FAILURE CONDITIONS:**
- Uses `permission_required` property instead of `requires_permission` relation
- Nav item not linked to Admin parent module
- Sort order places it in wrong position

**Step 1: Find the Admin module nav_item in ontology seed**

Search `data/seed/ontology.json` for nav items with `"entity_type": "nav_item"`. Identify the Admin module's name and the existing "Role Builder" nav item to determine correct sort_order and parent.

**Step 2: Add "Roles" nav_item entity**

Add to the entities array:

```json
{
  "entity_type": "nav_item",
  "name": "nav_roles_assignment",
  "label": "Roles",
  "properties": {
    "url": "/roles",
    "icon": "users",
    "parent": "<admin-nav-item-name>",
    "sort_order": "<one less than role_builder sort_order>"
  }
}
```

**Step 3: Add `requires_permission` relation**

Add to the relations array:

```json
{
  "relation_type": "requires_permission",
  "source": "nav_roles_assignment",
  "target": "roles.assign"
}
```

**Step 4: Verify by resetting DB and checking navigation**

```bash
rm -f data/staging/app.db
APP_ENV=staging cargo run
```

Login as admin. Verify "Roles" appears in Admin sidebar. Check the nav_item query:

```sql
sqlite3 data/staging/app.db "SELECT e.name, e.label, ep.value FROM entities e JOIN entity_properties ep ON e.id = ep.entity_id WHERE e.name = 'nav_roles_assignment';"
```

**Step 5: Commit**

```bash
git add data/seed/ontology.json
git commit -m "feat: add Roles nav item under Admin module"
```

---

### Task 3: Multi-Role Permission Aggregation — Model Layer

**GOAL:** A new function `find_codes_by_user_id()` in `src/models/permission.rs` returns the union of all permission codes across ALL roles assigned to a user.
Success = if user has roles A (perms: `users.list`, `users.create`) and B (perms: `tor.view`, `users.list`), function returns `["tor.view", "users.create", "users.list"]` (sorted, deduplicated).

**CONSTRAINTS:**
- Use SQL DISTINCT to deduplicate at the database level
- Keep existing `find_codes_by_role_id()` — other code still uses it
- Return `Vec<String>` sorted by code name (ORDER BY)
- Query pattern from design doc's multi-role SQL

**FORMAT:**
- Modify: `src/models/permission.rs` — add `find_codes_by_user_id()` function
- Test: `tests/permission_test.rs` — new test file

**FAILURE CONDITIONS:**
- Modifies or removes `find_codes_by_role_id()`
- Doesn't deduplicate permissions shared across roles
- Doesn't handle users with zero roles (should return empty vec)

**Step 1: Write the failing test**

Create `tests/permission_test.rs`:

```rust
mod common;
use common::setup_test_db;
use ahlt::models::{entity, relation, permission};

#[test]
fn test_find_codes_by_user_id_multi_role() {
    let (_dir, conn) = setup_test_db();

    // Create two roles
    let role_a = entity::create(&conn, "role", "role_a", "Role A").unwrap();
    let role_b = entity::create(&conn, "role", "role_b", "Role B").unwrap();

    // Create permissions
    let perm1 = entity::create(&conn, "permission", "users.list", "List Users").unwrap();
    let perm2 = entity::create(&conn, "permission", "users.create", "Create Users").unwrap();
    let perm3 = entity::create(&conn, "permission", "tor.view", "View ToR").unwrap();

    // Assign permissions to roles
    relation::create(&conn, "has_permission", role_a, perm1).unwrap(); // role_a -> users.list
    relation::create(&conn, "has_permission", role_a, perm2).unwrap(); // role_a -> users.create
    relation::create(&conn, "has_permission", role_b, perm3).unwrap(); // role_b -> tor.view
    relation::create(&conn, "has_permission", role_b, perm1).unwrap(); // role_b -> users.list (overlap)

    // Create user with both roles
    let user_id = entity::create(&conn, "user", "testuser", "Test User").unwrap();
    relation::create(&conn, "has_role", user_id, role_a).unwrap();
    relation::create(&conn, "has_role", user_id, role_b).unwrap();

    // Should return union of all permissions, sorted, no duplicates
    let codes = permission::find_codes_by_user_id(&conn, user_id).unwrap();
    assert_eq!(codes, vec!["tor.view", "users.create", "users.list"]);
}

#[test]
fn test_find_codes_by_user_id_no_roles() {
    let (_dir, conn) = setup_test_db();

    let user_id = entity::create(&conn, "user", "norole", "No Role").unwrap();
    let codes = permission::find_codes_by_user_id(&conn, user_id).unwrap();
    assert!(codes.is_empty());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_find_codes_by_user_id -- --nocapture`
Expected: Compilation error — `find_codes_by_user_id` doesn't exist

**Step 3: Implement `find_codes_by_user_id()`**

Add to `src/models/permission.rs`:

```rust
/// Get all permission codes for a user across ALL assigned roles (multi-role union).
/// Returns sorted, deduplicated permission codes.
pub fn find_codes_by_user_id(conn: &Connection, user_id: i64) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT perm.name AS code \
         FROM relations r_role \
         JOIN relations r_perm ON r_perm.source_id = r_role.target_id \
         JOIN entities perm ON r_perm.target_id = perm.id \
         WHERE r_role.source_id = ?1 \
           AND r_role.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
           AND r_perm.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission') \
           AND perm.entity_type = 'permission' \
         ORDER BY perm.name"
    )?;
    let codes = stmt
        .query_map(params![user_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(codes)
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test test_find_codes_by_user_id -- --nocapture`
Expected: 2 tests pass

**Step 5: Commit**

```bash
git add src/models/permission.rs tests/permission_test.rs
git commit -m "feat: add find_codes_by_user_id for multi-role permission aggregation"
```

---

### Task 4: Update Login Handler for Multi-Role Permissions

**GOAL:** Login loads permissions from ALL roles a user has (not just one `role_id`). Session stores aggregated permission CSV.
Success = user with 2 roles sees union of both roles' permissions in their session.

**CONSTRAINTS:**
- Use the new `find_codes_by_user_id()` from Task 3
- Remove `role_id` and `role_label` from session — no longer meaningful with multi-role
- Keep backward compatibility: if user has exactly one role, behavior is identical to before
- Don't break logout or session middleware

**FORMAT:**
- Modify: `src/handlers/auth_handlers.rs:86-108` — login_submit success branch
- Modify: `src/models/user/types.rs` — remove `role_id` from `User` struct
- Modify: `src/models/user/queries.rs:138-170` — `find_by_username()` no longer needs role JOIN

**FAILURE CONDITIONS:**
- Keeps single `role_id` in session
- Breaks login for users with zero roles (should still login, just no permissions)
- Removes `find_by_username()` function entirely

**Step 1: Update `find_by_username()` to drop role JOIN**

In `src/models/user/queries.rs`, simplify `find_by_username()` to not JOIN the role table. Remove `role_id` from the returned `User` struct.

Update `src/models/user/types.rs` — remove `role_id: i64` from `User` struct.

**Step 2: Update login handler to use `find_codes_by_user_id()`**

In `src/handlers/auth_handlers.rs`, replace lines 92-105:

```rust
// OLD: single role
let role_label = role::find_by_id(&conn, u.role_id)?...
let perms = permission::find_codes_by_role_id(&conn, u.role_id)?;

// NEW: multi-role aggregation
let perms = permission::find_codes_by_user_id(&conn, u.id)?;
let perms_csv = perms.join(",");

let _ = session.insert("user_id", u.id);
let _ = session.insert("username", &u.username);
let _ = session.insert("permissions", &perms_csv);
```

Remove the `role_id` and `role_label` session inserts.

**Step 3: Check for `role_id` / `role_label` session reads elsewhere**

Search codebase for `session.get.*role` to find any code reading `role_id` or `role_label` from session. Update or remove those references.

**Step 4: Run tests**

Run: `cargo test`
Expected: All existing tests pass. Login functionality works.

**Step 5: Manual verification**

```bash
rm -f data/staging/app.db && APP_ENV=staging cargo run
```
Login as admin. Verify dashboard loads. Navigate to various pages to confirm permissions work.

**Step 6: Commit**

```bash
git add src/handlers/auth_handlers.rs src/models/user/types.rs src/models/user/queries.rs
git commit -m "feat: login loads permissions from all assigned roles (multi-role)"
```

---

### Task 5: Strip Role from Users Page — Model Layer

**GOAL:** `user::create()` no longer accepts or creates `has_role` relations. `user::update()` no longer touches `has_role` relations. A new `user::assign_default_role()` function assigns the "viewer" role to a user.
Success = creating a user via model API creates the entity + properties but no role relation. `assign_default_role()` creates a `has_role` relation to the viewer role.

**CONSTRAINTS:**
- Remove `role_id` from `NewUser` struct
- Remove `role_id` parameter from `update()` function
- Keep `count_by_role_id()` — it's still needed for role delete protection
- Default role name hardcoded to `"viewer"` for now

**FORMAT:**
- Modify: `src/models/user/types.rs` — remove `role_id` from `NewUser` and `UserForm`
- Modify: `src/models/user/queries.rs:182-255` — `create()` and `update()` functions
- Add: `assign_default_role()` function in `src/models/user/queries.rs`
- Test: Update existing tests in `tests/user_test.rs`

**FAILURE CONDITIONS:**
- `create()` still creates a `has_role` relation
- `update()` still deletes/creates `has_role` relations
- `assign_default_role()` doesn't check if viewer role exists
- Breaks existing tests that rely on `role_id` in `NewUser`

**Step 1: Update `NewUser` struct**

In `src/models/user/types.rs`, remove `role_id: i64` from `NewUser` struct. Remove `role_id: String` from `UserForm` struct.

**Step 2: Update `create()` function**

In `src/models/user/queries.rs`, remove the `has_role` INSERT from `create()` (lines 200-205).

**Step 3: Update `update()` function**

Remove `role_id: i64` parameter and the `has_role` DELETE + INSERT (lines 242-252).

New signature:
```rust
pub fn update(
    conn: &Connection,
    id: i64,
    username: &str,
    password: Option<&str>,
    email: &str,
    display_name: &str,
) -> rusqlite::Result<()> {
```

**Step 4: Add `assign_default_role()`**

```rust
/// Assign the default "viewer" role to a user. No-op if viewer role doesn't exist.
pub fn assign_default_role(conn: &Connection, user_id: i64) -> rusqlite::Result<()> {
    let viewer_id: Option<i64> = conn.query_row(
        "SELECT id FROM entities WHERE entity_type = 'role' AND name = 'viewer'",
        [],
        |row| row.get(0),
    ).optional()?;

    if let Some(role_id) = viewer_id {
        conn.execute(
            "INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id) \
             VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role'), ?1, ?2)",
            params![user_id, role_id],
        )?;
    }
    Ok(())
}
```

Note: Requires `use rusqlite::OptionalExtension;` import.

**Step 5: Fix all callers of `create()` and `update()`**

Search for all calls to `user::create` and `user::update` in handlers and tests. Remove `role_id` arguments. After `create()`, call `user::assign_default_role()`.

Key files:
- `src/handlers/user_handlers/crud.rs` — create handler (line 80-86), update handler (line 236-243)
- `tests/user_test.rs` — all test functions that call create/update
- `src/handlers/api_v1/users.rs` — API create/update if they exist

**Step 6: Run tests**

Run: `cargo test`
Expected: All tests pass after fixing callers.

**Step 7: Commit**

```bash
git add src/models/user/types.rs src/models/user/queries.rs src/handlers/user_handlers/crud.rs tests/user_test.rs
git commit -m "refactor: remove role handling from user create/update, add assign_default_role"
```

---

### Task 6: Strip Role from Users Page — Handler & Template Layer

**GOAL:** The user create/edit forms no longer show a role dropdown. Handlers no longer process role data. Creating a user auto-assigns the viewer role.
Success = `/users/new` form has no role select. Creating a user assigns viewer role automatically. `/users/{id}/edit` form has no role select.

**CONSTRAINTS:**
- Remove `roles` field from `UserFormTemplate` struct
- Remove `role::find_all_display()` calls from user handlers
- Remove role validation from create/update handlers
- Keep last-admin protection in delete handler (still check role relations in DB)
- Keep role column in users list table — will be removed in a later task if needed

**FORMAT:**
- Modify: `src/templates_structs.rs:96-105` — `UserFormTemplate` remove `roles` field
- Modify: `src/handlers/user_handlers/crud.rs` — all four handlers (new_form, create, edit_form, update)
- Modify: `templates/users/form.html:42-49` — remove role dropdown
- Modify: `src/handlers/user_handlers/crud.rs:35-137` — create handler calls `assign_default_role()` after create

**FAILURE CONDITIONS:**
- Role dropdown still visible on form
- Handler still references `form.role_id`
- New users get no role at all (must get viewer)
- Last-admin delete protection broken

**Step 1: Remove `roles` from `UserFormTemplate`**

In `src/templates_structs.rs`, remove `pub roles: Vec<RoleDisplay>` from `UserFormTemplate`.

**Step 2: Remove role dropdown from template**

In `templates/users/form.html`, delete lines 42-49 (the role `<div class="form-group">` containing the select).

**Step 3: Update `new_form` handler**

In `src/handlers/user_handlers/crud.rs`, remove `let roles = role::find_all_display(&conn)?;` and `roles` field from template construction.

**Step 4: Update `create` handler**

- Remove `role_id` parsing (lines 53-59)
- Remove `roles` from error template construction
- Remove `role_id` from `NewUser` construction
- After `user::create()` succeeds, call `user::assign_default_role(&conn, user_id)?;`
- Remove `role_id` from audit log details

**Step 5: Update `edit_form` handler**

Remove `let roles = role::find_all_display(&conn)?;` and `roles` field.

**Step 6: Update `update` handler**

- Remove `new_role_id` parsing (lines 190-196)
- Remove `roles` from error template construction
- Remove `new_role_id` from `user::update()` call
- Remove last-admin role change protection (lines 213-224) — role changes now happen on Roles page
- Remove `role_id` from audit log details

**Step 7: Update delete handler last-admin protection**

The delete handler's last-admin check (lines 308-318) currently uses `target.role_name == "admin"`. This still works because `UserDisplay` still has role info from the query. Keep this as-is for now — it checks via DB, not session.

**Step 8: Run tests**

Run: `cargo test`
Expected: All tests pass.

**Step 9: Manual verification**

```bash
APP_ENV=staging cargo run
```
- Go to `/users/new` — no role dropdown
- Create a user — check DB: user has `has_role` to viewer
- Go to `/users/{id}/edit` — no role dropdown

**Step 10: Commit**

```bash
git add src/templates_structs.rs src/handlers/user_handlers/crud.rs templates/users/form.html
git commit -m "feat: remove role assignment from user forms, auto-assign viewer role"
```

---

### Task 7: Remove Legacy Role Create Form

**GOAL:** The legacy `/roles/new` form and `/roles` POST create handler are removed. The "New Role" button on the roles list redirects to `/roles/builder`.
Success = `GET /roles/new` returns 404. `POST /roles` returns 404. Role Builder is the sole path for role CRUD.

**CONSTRAINTS:**
- Delete `templates/roles/form.html`
- Remove `new_form` and `create` functions from `src/handlers/role_handlers/crud.rs`
- Remove corresponding routes from `src/main.rs`
- Remove `RoleFormTemplate` from `src/templates_structs.rs`
- Keep `delete` function in crud.rs — still needed

**FORMAT:**
- Delete: `templates/roles/form.html`
- Modify: `src/handlers/role_handlers/crud.rs` — remove `new_form` and `create` functions
- Modify: `src/main.rs:145-146` — remove two routes
- Modify: `src/templates_structs.rs:114-123` — remove `RoleFormTemplate`
- Modify: `src/handlers/role_handlers/mod.rs` — remove re-exports if present

**FAILURE CONDITIONS:**
- Deletes the `delete` handler
- Removes Role Builder routes
- Leaves dead imports or unused code

**Step 1: Remove routes from main.rs**

In `src/main.rs`, delete lines:
```rust
.route("/roles/new", web::get().to(handlers::role_handlers::new_form))
.route("/roles", web::post().to(handlers::role_handlers::create))
```

**Step 2: Remove handler functions**

In `src/handlers/role_handlers/crud.rs`, delete `new_form` (lines 14-34) and `create` (lines 36-111). Keep `delete` (lines 113-160).

**Step 3: Remove `RoleFormTemplate`**

In `src/templates_structs.rs`, delete lines 114-123 (the `RoleFormTemplate` struct).

**Step 4: Delete legacy template file**

```bash
rm templates/roles/form.html
```

**Step 5: Update handler mod.rs**

In `src/handlers/role_handlers/mod.rs`, remove re-exports for `new_form` and `create`.

**Step 6: Clean up imports**

Remove unused imports in modified files (the helpers module import in crud.rs if `parse_form_body` etc. are no longer used).

**Step 7: Run tests**

Run: `cargo test`
Expected: All tests pass. `cargo check` clean.

**Step 8: Commit**

```bash
git add -A
git commit -m "refactor: remove legacy role create form, Role Builder is sole path"
```

---

### Task 8: Role Assignment Handlers — Assign and Unassign

**GOAL:** New POST handlers at `/roles/assign` and `/roles/unassign` manage `has_role` relations. These are the sole entry points for changing user-role assignments.
Success = POST to `/roles/assign` with `user_id` and `role_id` creates a `has_role` relation. POST to `/roles/unassign` removes it. Both require `roles.assign` permission.

**CONSTRAINTS:**
- Permission check: `roles.assign`
- CSRF validation on both handlers
- Assign: INSERT OR IGNORE to prevent duplicates
- Unassign: Must prevent removing ALL roles from last admin
- Both redirect back to `/roles` with flash message
- Audit log both operations

**FORMAT:**
- Create: `src/handlers/role_handlers/assignment.rs` — new file with `assign` and `unassign` handlers
- Modify: `src/handlers/role_handlers/mod.rs` — declare new module
- Modify: `src/main.rs` — add routes
- Test: Manual E2E test via browser

**FAILURE CONDITIONS:**
- Missing CSRF validation
- Missing permission check
- Allows duplicate `has_role` relations
- Allows removing last admin's admin role
- No audit logging

**Step 1: Create assignment handler file**

Create `src/handlers/role_handlers/assignment.rs`:

```rust
use actix_session::Session;
use actix_web::{web, HttpResponse};
use serde::Deserialize;

use crate::db::DbPool;
use crate::auth::csrf;
use crate::auth::session::require_permission;
use crate::errors::AppError;

#[derive(Deserialize)]
pub struct AssignForm {
    pub user_id: i64,
    pub role_id: i64,
    pub csrf_token: String,
}

pub async fn assign(
    pool: web::Data<DbPool>,
    session: Session,
    form: web::Form<AssignForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.assign")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let conn = pool.get()?;

    // INSERT OR IGNORE prevents duplicate relations
    conn.execute(
        "INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role'), ?1, ?2)",
        rusqlite::params![form.user_id, form.role_id],
    )?;

    // Audit
    let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "user_id": form.user_id,
        "role_id": form.role_id,
        "summary": "Assigned role to user"
    });
    let _ = crate::audit::log(&conn, current_user_id, "role.assigned", "role", form.role_id, details);

    let _ = session.insert("flash", "Role assigned");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/roles"))
        .finish())
}

#[derive(Deserialize)]
pub struct UnassignForm {
    pub user_id: i64,
    pub role_id: i64,
    pub csrf_token: String,
}

pub async fn unassign(
    pool: web::Data<DbPool>,
    session: Session,
    form: web::Form<UnassignForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.assign")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let conn = pool.get()?;

    // Last-admin protection: don't allow removing admin role if this is the last admin
    let is_admin_role: bool = conn.query_row(
        "SELECT name = 'admin' FROM entities WHERE id = ?1 AND entity_type = 'role'",
        rusqlite::params![form.role_id],
        |row| row.get(0),
    ).unwrap_or(false);

    if is_admin_role {
        let admin_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM relations \
             WHERE relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
             AND target_id = ?1",
            rusqlite::params![form.role_id],
            |row| row.get(0),
        ).unwrap_or(0);

        if admin_count <= 1 {
            let _ = session.insert("flash", "Cannot remove role: this is the last administrator");
            return Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/roles"))
                .finish());
        }
    }

    conn.execute(
        "DELETE FROM relations \
         WHERE relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
         AND source_id = ?1 AND target_id = ?2",
        rusqlite::params![form.user_id, form.role_id],
    )?;

    // Audit
    let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "user_id": form.user_id,
        "role_id": form.role_id,
        "summary": "Unassigned role from user"
    });
    let _ = crate::audit::log(&conn, current_user_id, "role.unassigned", "role", form.role_id, details);

    let _ = session.insert("flash", "Role unassigned");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/roles"))
        .finish())
}
```

**Step 2: Register module and routes**

In `src/handlers/role_handlers/mod.rs`, add: `pub mod assignment;`

In `src/main.rs`, add routes (after the existing `/roles` GET):
```rust
.route("/roles/assign", web::post().to(handlers::role_handlers::assignment::assign))
.route("/roles/unassign", web::post().to(handlers::role_handlers::assignment::unassign))
```

**Step 3: Run tests**

Run: `cargo test`
Expected: All tests pass. `cargo check` clean.

**Step 4: Commit**

```bash
git add src/handlers/role_handlers/assignment.rs src/handlers/role_handlers/mod.rs src/main.rs
git commit -m "feat: add role assign/unassign handlers with audit + admin protection"
```

---

### Task 9: Role Assignment Page — Query Functions

**GOAL:** New query functions provide data for the role assignment page: users grouped by role, roles grouped by user, and user list for the "add user" dropdown.
Success = `find_users_by_role()` returns all users with a given role. `find_roles_by_user()` returns all roles for a given user. `find_users_not_in_role()` returns users who don't have a given role.

**CONSTRAINTS:**
- Add functions to `src/models/role/queries.rs` (role-centric) and `src/models/user/queries.rs` (user-centric)
- Use existing EAV patterns (JOIN on `has_role` relation type)
- Return simple display structs, not full entities

**FORMAT:**
- Modify: `src/models/role/queries.rs` — add `find_users_by_role()`, `find_users_not_in_role()`
- Modify: `src/models/user/queries.rs` — add `find_roles_for_user()`
- Add types if needed to `src/models/role/types.rs` or `src/models/user/types.rs`
- Test: Add to `tests/permission_test.rs`

**FAILURE CONDITIONS:**
- Queries don't use the `has_role` relation type correctly
- Missing ORDER BY for consistent display order
- Returns password hashes in user data

**Step 1: Add `RoleMember` type**

In `src/models/role/types.rs`, add:

```rust
/// A user assigned to a role — for the assignment page member list.
pub struct RoleMember {
    pub user_id: i64,
    pub username: String,
    pub display_name: String,
}
```

**Step 2: Add `find_users_by_role()` and `find_users_not_in_role()`**

In `src/models/role/queries.rs`:

```rust
use super::types::RoleMember;

/// Find all users assigned to a specific role.
pub fn find_users_by_role(conn: &Connection, role_id: i64) -> rusqlite::Result<Vec<RoleMember>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name AS username, e.label AS display_name \
         FROM entities e \
         JOIN relations r ON r.source_id = e.id AND r.target_id = ?1 \
           AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
         WHERE e.entity_type = 'user' \
         ORDER BY e.label, e.name"
    )?;
    let members = stmt.query_map(params![role_id], |row| {
        Ok(RoleMember {
            user_id: row.get("id")?,
            username: row.get("username")?,
            display_name: row.get("display_name")?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(members)
}

/// Find users NOT assigned to a specific role (for "Add User" dropdown).
pub fn find_users_not_in_role(conn: &Connection, role_id: i64) -> rusqlite::Result<Vec<RoleMember>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name AS username, e.label AS display_name \
         FROM entities e \
         WHERE e.entity_type = 'user' \
           AND e.id NOT IN ( \
               SELECT r.source_id FROM relations r \
               WHERE r.target_id = ?1 \
                 AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
           ) \
         ORDER BY e.label, e.name"
    )?;
    let members = stmt.query_map(params![role_id], |row| {
        Ok(RoleMember {
            user_id: row.get("id")?,
            username: row.get("username")?,
            display_name: row.get("display_name")?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(members)
}
```

**Step 3: Add `UserWithRoles` type and `find_all_with_roles()`**

In `src/models/user/types.rs`, add:

```rust
/// User with all assigned roles — for the "By User" tab.
pub struct UserWithRoles {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub roles: Vec<(i64, String, String)>, // (role_id, role_name, role_label)
}
```

In `src/models/user/queries.rs`, add:

```rust
use super::types::UserWithRoles;

/// Find all users with their assigned roles (for assignment page "By User" tab).
pub fn find_all_with_roles(conn: &Connection) -> rusqlite::Result<Vec<UserWithRoles>> {
    // First get all users
    let mut users_stmt = conn.prepare(
        "SELECT id, name AS username, label AS display_name FROM entities WHERE entity_type = 'user' ORDER BY label, name"
    )?;
    let users: Vec<(i64, String, String)> = users_stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })?.collect::<Result<Vec<_>, _>>()?;

    // Then get all user-role assignments
    let mut roles_stmt = conn.prepare(
        "SELECT r.source_id AS user_id, role_e.id AS role_id, role_e.name, role_e.label \
         FROM relations r \
         JOIN entities role_e ON r.target_id = role_e.id \
         WHERE r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
         ORDER BY role_e.label"
    )?;
    let assignments: Vec<(i64, i64, String, String)> = roles_stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    })?.collect::<Result<Vec<_>, _>>()?;

    // Group assignments by user
    let mut result: Vec<UserWithRoles> = users.into_iter().map(|(id, username, display_name)| {
        let roles: Vec<(i64, String, String)> = assignments.iter()
            .filter(|(uid, _, _, _)| *uid == id)
            .map(|(_, rid, name, label)| (*rid, name.clone(), label.clone()))
            .collect();
        UserWithRoles { id, username, display_name, roles }
    }).collect();

    Ok(result)
}
```

**Step 4: Write tests**

Add to `tests/permission_test.rs`:

```rust
#[test]
fn test_find_users_by_role() {
    let (_dir, conn) = setup_test_db();
    let role_id = entity::create(&conn, "role", "editor", "Editor").unwrap();
    let user1 = entity::create(&conn, "user", "alice", "Alice").unwrap();
    let user2 = entity::create(&conn, "user", "bob", "Bob").unwrap();
    relation::create(&conn, "has_role", user1, role_id).unwrap();
    relation::create(&conn, "has_role", user2, role_id).unwrap();

    let members = ahlt::models::role::find_users_by_role(&conn, role_id).unwrap();
    assert_eq!(members.len(), 2);
}
```

**Step 5: Run tests**

Run: `cargo test`
Expected: All tests pass.

**Step 6: Commit**

```bash
git add src/models/role/queries.rs src/models/role/types.rs src/models/user/queries.rs src/models/user/types.rs tests/permission_test.rs
git commit -m "feat: add query functions for role assignment page (by-role, by-user)"
```

---

### Task 10: Role Assignment Page — Menu Preview API

**GOAL:** A GET endpoint at `/api/roles/preview?user_id=N` returns JSON showing the effective menu items for a user based on ALL their assigned roles.
Success = API returns the combined accessible nav items for a user considering all role permissions.

**CONSTRAINTS:**
- Use existing `find_accessible_nav_items()` or `find_navigation()` logic from `nav_item.rs`
- Permission check: `roles.assign`
- Returns JSON array of `{label, url, icon}` objects
- Must work with the multi-role permission aggregation from Task 3

**FORMAT:**
- Modify: `src/handlers/role_handlers/assignment.rs` — add `menu_preview` handler
- Modify: `src/main.rs` — add route

**FAILURE CONDITIONS:**
- Only considers first role's permissions
- Returns full nav tree instead of flat accessible items
- No permission check on the endpoint

**Step 1: Add `menu_preview` handler**

In `src/handlers/role_handlers/assignment.rs`, add:

```rust
#[derive(Deserialize)]
pub struct PreviewQuery {
    pub user_id: i64,
}

pub async fn menu_preview(
    pool: web::Data<DbPool>,
    session: Session,
    query: web::Query<PreviewQuery>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.assign")?;

    let conn = pool.get()?;

    // Get all permissions for this user across all roles
    let perms = crate::models::permission::find_codes_by_user_id(&conn, query.user_id)?;
    let permissions = crate::auth::session::Permissions(perms);

    // Get accessible nav items using existing logic
    let (modules, sidebar_items) = crate::models::nav_item::find_navigation(&conn, &permissions, "");

    // Build flat list of accessible items
    let mut items: Vec<serde_json::Value> = vec![];
    for module in &modules {
        items.push(serde_json::json!({
            "label": module.label,
            "url": module.url,
            "icon": module.icon,
            "type": "module"
        }));
    }
    for item in &sidebar_items {
        items.push(serde_json::json!({
            "label": item.label,
            "url": item.url,
            "icon": item.icon,
            "type": "sidebar",
            "parent": item.parent_label
        }));
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "user_id": query.user_id,
        "permission_count": permissions.0.len(),
        "menu_items": items
    })))
}
```

**Step 2: Add route**

In `src/main.rs`, add (BEFORE `/roles/{id}/delete` to avoid path conflict):
```rust
.route("/api/roles/preview", web::get().to(handlers::role_handlers::assignment::menu_preview))
```

**Step 3: Run tests**

Run: `cargo check && cargo test`
Expected: Compiles and all tests pass.

**Step 4: Commit**

```bash
git add src/handlers/role_handlers/assignment.rs src/main.rs
git commit -m "feat: add /api/roles/preview endpoint for menu preview"
```

---

### Task 11: Role Assignment Page — List Handler Rewrite

**GOAL:** The `/roles` GET handler renders a new assignment page with data for both "By Role" and "By User" tabs.
Success = `/roles` shows the assignment page with role selector, member lists, and user-role table.

**CONSTRAINTS:**
- Permission check changes from `roles.manage` to `roles.assign`
- Must load: all roles (for tabs), users-by-role for first/selected role, all users-with-roles for By User tab
- Reuse `PageContext` pattern
- Create new template struct

**FORMAT:**
- Modify: `src/handlers/role_handlers/list.rs` — rewrite `list` handler
- Modify: `src/templates_structs.rs` — replace `RoleListTemplate` with `RoleAssignmentTemplate`
- Create: `templates/roles/assignment.html` — new template
- Delete or replace: `templates/roles/list.html` — no longer used

**FAILURE CONDITIONS:**
- Still requires `roles.manage` permission (should be `roles.assign`)
- Template compilation errors from missing struct fields
- Broken navigation highlighting

**Step 1: Create `RoleAssignmentTemplate` struct**

In `src/templates_structs.rs`, replace `RoleListTemplate` with:

```rust
#[derive(Template)]
#[template(path = "roles/assignment.html")]
pub struct RoleAssignmentTemplate {
    pub ctx: PageContext,
    pub roles: Vec<RoleListItem>,
    pub selected_role_id: i64,
    pub members: Vec<crate::models::role::RoleMember>,
    pub available_users: Vec<crate::models::role::RoleMember>,
    pub users_with_roles: Vec<crate::models::user::UserWithRoles>,
    pub active_tab: String,
}
```

**Step 2: Rewrite list handler**

In `src/handlers/role_handlers/list.rs`:

```rust
use crate::templates_structs::{PageContext, RoleAssignmentTemplate};

#[derive(serde::Deserialize)]
pub struct AssignmentQuery {
    pub role_id: Option<i64>,
    pub tab: Option<String>,
}

pub async fn list(
    pool: web::Data<DbPool>,
    session: Session,
    query: web::Query<AssignmentQuery>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.assign")?;

    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/roles")?;
    let roles = role::find_all_list_items(&conn)?;

    let active_tab = query.tab.clone().unwrap_or_else(|| "by-role".to_string());

    // Select first role by default
    let selected_role_id = query.role_id.unwrap_or_else(|| {
        roles.first().map(|r| r.id).unwrap_or(0)
    });

    let members = if selected_role_id > 0 {
        role::find_users_by_role(&conn, selected_role_id)?
    } else {
        vec![]
    };

    let available_users = if selected_role_id > 0 {
        role::find_users_not_in_role(&conn, selected_role_id)?
    } else {
        vec![]
    };

    let users_with_roles = crate::models::user::find_all_with_roles(&conn)?;

    let tmpl = RoleAssignmentTemplate {
        ctx,
        roles,
        selected_role_id,
        members,
        available_users,
        users_with_roles,
        active_tab,
    };
    render(tmpl)
}
```

**Step 3: Create assignment template**

Create `templates/roles/assignment.html` — a full Askama template with:
- Tab toggle (By Role / By User)
- By Role tab: horizontal role selector pills, member list with Remove buttons, Add User dropdown
- By User tab: table with user name, role badges with × remove, Add Role dropdown
- Menu preview panel (collapsible, fetches from `/api/roles/preview?user_id=N`)
- Footer per role: "N permissions · Edit in Role Builder" link

This template is large (100-150 lines). Use BEM CSS classes. Use safe DOM construction (no innerHTML).

**Step 4: Delete old template**

```bash
rm templates/roles/list.html
```

**Step 5: Run tests**

Run: `cargo check && cargo test`
Expected: Compiles and all tests pass.

**Step 6: Manual verification**

```bash
APP_ENV=staging cargo run
```
Login as admin. Navigate to `/roles`. Verify both tabs render. Click a role — see members. Click "By User" — see user-role table.

**Step 7: Commit**

```bash
git add -A
git commit -m "feat: new role assignment page with By Role and By User tabs"
```

---

### Task 12: Role Assignment Page — Interactive JavaScript

**GOAL:** The assignment page has working interactive features: tab switching, Add User dropdown, Remove button, Add Role dropdown, and menu preview panel.
Success = all CRUD operations on the assignment page work via form submissions. Menu preview panel loads and displays via AJAX.

**CONSTRAINTS:**
- No innerHTML — use createElement/textContent/appendChild
- Use `fetchWithTimeout()` pattern for AJAX calls
- All form submissions are standard POST forms (not AJAX) for reliability
- Menu preview is the only AJAX feature

**FORMAT:**
- Modify: `templates/roles/assignment.html` — add inline `<script>` block
- Forms use standard POST submissions to `/roles/assign` and `/roles/unassign`

**FAILURE CONDITIONS:**
- Uses innerHTML
- AJAX calls without timeout
- Tab switching breaks browser history
- Forms missing CSRF token

**Step 1: Add tab switching logic**

The tabs should use URL query params (`?tab=by-role` / `?tab=by-user`) so they work with standard page loads. No JavaScript needed for basic tab switching — it's server-rendered.

**Step 2: Add menu preview JavaScript**

Add inline script to assignment template:

```javascript
// Menu preview panel
function loadMenuPreview(userId) {
    const panel = document.getElementById('menu-preview');
    const content = document.getElementById('menu-preview-content');
    if (!panel || !content) return;

    panel.removeAttribute('hidden');
    content.textContent = 'Loading...';

    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 10000);

    fetch('/api/roles/preview?user_id=' + userId, { signal: controller.signal })
        .then(r => r.json())
        .then(data => {
            clearTimeout(timeout);
            content.textContent = '';
            // Build preview list using safe DOM methods
            const heading = document.createElement('h4');
            heading.textContent = 'Effective Menu (' + data.permission_count + ' permissions)';
            content.appendChild(heading);

            const ul = document.createElement('ul');
            ul.className = 'menu-preview__list';
            data.menu_items.forEach(item => {
                const li = document.createElement('li');
                li.textContent = item.label;
                if (item.parent) {
                    const small = document.createElement('small');
                    small.textContent = ' (' + item.parent + ')';
                    li.appendChild(small);
                }
                ul.appendChild(li);
            });
            content.appendChild(ul);
        })
        .catch(e => {
            clearTimeout(timeout);
            content.textContent = e.name === 'AbortError' ? 'Request timed out' : 'Error loading preview';
        });
}
```

**Step 3: Wire up click handlers**

Add event listeners for "Preview" buttons in the By User tab rows.

**Step 4: Manual verification**

Test all interactions: switch tabs, add user to role, remove user from role, view menu preview.

**Step 5: Commit**

```bash
git add templates/roles/assignment.html
git commit -m "feat: add interactive features to role assignment page"
```

---

### Task 13: Update Roles List Link and Role Builder Delete

**GOAL:** The roles list page's "New Role" button points to `/roles/builder`. Role Builder edit form has a delete button (if no users assigned).
Success = clicking "New Role" goes to Role Builder. Delete button appears on builder edit form for roles with 0 users.

**CONSTRAINTS:**
- "New Role" button only visible to users with `roles.manage` permission
- Delete button on builder sends POST to `/roles/builder/{id}/delete`
- Add the delete route to main.rs

**FORMAT:**
- Modify: `templates/roles/assignment.html` — add "New Role" button (conditional on `roles.manage`)
- Modify: `templates/roles/builder.html` — add delete button on edit mode
- Modify: `src/main.rs` — add delete route for builder
- Modify: `src/handlers/role_builder_handlers.rs` — add delete handler (or reuse existing crud delete)

**FAILURE CONDITIONS:**
- "New Role" visible without `roles.manage` permission
- Delete button shown for roles with assigned users
- Delete doesn't cascade properly

**Step 1: Add "New Role" button to assignment page**

In the page header of `templates/roles/assignment.html`:

```html
{% if ctx.permissions.has("roles.manage") %}
<a href="/roles/builder" class="btn btn-primary">New Role</a>
{% endif %}
```

**Step 2: Add delete route**

In `src/main.rs`, add:
```rust
.route("/roles/builder/{id}/delete", web::post().to(handlers::role_handlers::delete))
```

This reuses the existing `role_handlers::crud::delete` handler which already has user count protection.

**Step 3: Add delete button to builder template**

In `templates/roles/builder.html`, when editing (role is Some), add at the bottom of the form:

```html
{% if let Some(r) = role %}
<div class="form-actions form-actions--danger">
    <form method="post" action="/roles/builder/{{ r.id }}/delete"
          onsubmit="return confirm('Delete this role? This cannot be undone.')">
        <input type="hidden" name="csrf_token" value="{{ csrf_token }}">
        <button type="submit" class="btn btn-danger">Delete Role</button>
    </form>
</div>
{% endif %}
```

**Step 4: Run tests and verify**

Run: `cargo check && cargo test`

**Step 5: Commit**

```bash
git add templates/roles/assignment.html templates/roles/builder.html src/main.rs
git commit -m "feat: add New Role button and delete from Role Builder"
```

---

### Task 14: Update Last-Admin Protection for Multi-Role

**GOAL:** Last-admin protections work correctly with multi-role: protect against removing the last user who has the admin role, protect against deleting a user who is the sole admin.
Success = cannot remove admin role from the last admin via unassign. Cannot delete the last admin user.

**CONSTRAINTS:**
- Check `count_by_role_id()` for the admin role across all users
- Delete handler's last-admin check must query DB for role (not rely on single `role_name`)
- Must handle the case where a user has admin role among multiple roles

**FORMAT:**
- Modify: `src/handlers/user_handlers/crud.rs:285-362` — update delete handler's admin check
- Already handled in Task 8's unassign handler

**FAILURE CONDITIONS:**
- Admin check uses session data instead of DB query
- Allows deleting user who is sole holder of admin role
- False positive: blocks delete of non-admin user

**Step 1: Update user delete handler**

In `src/handlers/user_handlers/crud.rs`, the delete handler currently checks `target.role_name == "admin"`. With multi-role, `UserDisplay` may show only the first role. Change to query directly:

```rust
// Check if this user has the admin role
let has_admin_role: bool = conn.query_row(
    "SELECT COUNT(*) > 0 FROM relations r \
     JOIN entities role_e ON r.target_id = role_e.id \
     WHERE r.source_id = ?1 \
       AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
       AND role_e.name = 'admin'",
    rusqlite::params![id],
    |row| row.get::<_, bool>(0),
).unwrap_or(false);

if has_admin_role {
    // Count total users with admin role
    let admin_role_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type = 'role' AND name = 'admin'",
        [],
        |row| row.get(0),
    ).unwrap_or(0);
    let admin_count = crate::models::user::count_by_role_id(&conn, admin_role_id).unwrap_or(0);
    if admin_count <= 1 {
        let _ = session.insert("flash", "Cannot delete the last administrator");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", "/users"))
            .finish());
    }
}
```

Apply same pattern to bulk_delete handler.

**Step 2: Run tests**

Run: `cargo test`
Expected: All tests pass.

**Step 3: Commit**

```bash
git add src/handlers/user_handlers/crud.rs
git commit -m "fix: update last-admin protection for multi-role model"
```

---

### Task 15: Update UserDisplay for Multi-Role

**GOAL:** `UserDisplay` and the users list table handle multi-role correctly. The list shows all roles (or "N roles") instead of a single role.
Success = users list page shows accurate role information for users with multiple roles.

**CONSTRAINTS:**
- `UserDisplay` query currently LEFT JOINs one role — with multi-role this returns duplicate rows
- Options: (a) change to GROUP_CONCAT, (b) remove role from UserDisplay, (c) separate query
- Simplest: use GROUP_CONCAT for role labels, store as comma-separated string
- Keep `role_id` / `role_name` / `role_label` fields but repurpose for multi-role

**FORMAT:**
- Modify: `src/models/user/queries.rs` — update `SELECT_USER_DISPLAY` constant
- Modify: `src/models/user/types.rs` — adjust `UserDisplay` fields
- Modify: `templates/users/list.html` — update role column display

**FAILURE CONDITIONS:**
- Duplicate rows in user list (one per role)
- Breaks CSV export
- Breaks filter/sort on role column

**Step 1: Update SELECT_USER_DISPLAY to use GROUP_CONCAT**

In `src/models/user/queries.rs`, change the constant:

```rust
const SELECT_USER_DISPLAY: &str = "\
    SELECT e.id, e.name AS username, e.label AS display_name, \
           COALESCE(p_email.value, '') AS email, \
           COALESCE(GROUP_CONCAT(DISTINCT role_e.id), '') AS role_ids, \
           COALESCE(GROUP_CONCAT(DISTINCT role_e.name), '') AS role_names, \
           COALESCE(GROUP_CONCAT(DISTINCT role_e.label), '') AS role_labels, \
           e.created_at, e.updated_at \
    FROM entities e \
    LEFT JOIN entity_properties p_email \
        ON e.id = p_email.entity_id AND p_email.key = 'email' \
    LEFT JOIN relations r_role \
        ON r_role.source_id = e.id \
        AND r_role.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
    LEFT JOIN entities role_e ON r_role.target_id = role_e.id \
    WHERE e.entity_type = 'user'";
```

Add `GROUP BY e.id` to all queries using this constant.

**Step 2: Update `UserDisplay` struct**

Replace single role fields with multi-role:

```rust
pub struct UserDisplay {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub display_name: String,
    pub role_ids: String,    // comma-separated
    pub role_names: String,  // comma-separated
    pub role_labels: String, // comma-separated
    pub created_at: String,
    pub updated_at: String,
}
```

Update `row_to_user_display` to match.

**Step 3: Update templates**

In `templates/users/list.html`, change role column to show badges:

```html
<td>
  {% for label in user.role_labels.split(',') %}
    {% if !label.is_empty() %}
    <span class="badge badge-user">{{ label }}</span>
    {% endif %}
  {% endfor %}
</td>
```

**Step 4: Update CSV export**

In `src/handlers/user_handlers/crud.rs`, update `export_csv()` to use `role_labels` instead of `role_label`.

**Step 5: Update callers**

Search for all uses of `role_id`, `role_name`, `role_label` on `UserDisplay`. Update:
- Dashboard template if it shows role info
- API response types (`ApiUserResponse`)
- Any tests referencing these fields

**Step 6: Run tests**

Run: `cargo test`
Expected: All tests pass.

**Step 7: Commit**

```bash
git add src/models/user/types.rs src/models/user/queries.rs templates/users/list.html src/handlers/user_handlers/crud.rs src/templates_structs.rs
git commit -m "feat: UserDisplay supports multi-role with GROUP_CONCAT"
```

---

### Task 16: Update Route Table in main.rs

**GOAL:** Clean up the route table: remove dead routes, ensure all new routes are registered in correct order.
Success = all routes match the design doc. No 404s for new pages. No dead routes.

**CONSTRAINTS:**
- Route order matters in Actix: specific routes before parameterized
- `/roles/assign` and `/roles/unassign` must come BEFORE `/roles/{id}/delete`
- `/api/roles/preview` must come BEFORE any parameterized role routes

**FORMAT:**
- Modify: `src/main.rs` — final route cleanup

**FAILURE CONDITIONS:**
- Route ordering causes path parameter to swallow specific paths
- Dead routes pointing to removed handlers

**Step 1: Verify final route block**

The roles section should look like:

```rust
// Role assignment
.route("/roles", web::get().to(handlers::role_handlers::list))
.route("/roles/assign", web::post().to(handlers::role_handlers::assignment::assign))
.route("/roles/unassign", web::post().to(handlers::role_handlers::assignment::unassign))
.route("/api/roles/preview", web::get().to(handlers::role_handlers::assignment::menu_preview))
// Role Builder
.route("/roles/builder", web::get().to(handlers::role_builder_handlers::wizard_form))
.route("/roles/builder/preview", web::post().to(handlers::role_builder_handlers::preview_menu))
.route("/roles/builder/create", web::post().to(handlers::role_builder_handlers::create_role))
.route("/roles/builder/update", web::post().to(handlers::role_builder_handlers::update_role))
.route("/roles/builder/{id}/edit", web::get().to(handlers::role_builder_handlers::edit_form))
.route("/roles/builder/{id}/delete", web::post().to(handlers::role_handlers::delete))
// Role delete (legacy path still works)
.route("/roles/{id}/delete", web::post().to(handlers::role_handlers::delete))
```

**Step 2: Remove dead routes**

Confirm `/roles/new` GET and `/roles` POST are removed (done in Task 7).

**Step 3: Run tests**

Run: `cargo check && cargo test`
Expected: Clean compilation, all tests pass.

**Step 4: Commit**

```bash
git add src/main.rs
git commit -m "chore: finalize route table for users/roles separation"
```

---

### Task 17: End-to-End Verification and Cleanup

**GOAL:** Full manual verification that all three pages work independently. Clean up dead code. Update documentation.
Success = Users page has no role references. Roles page assignment works with multi-role. Role Builder is sole path for role CRUD. All 3 pages are independently operable with their own permissions.

**CONSTRAINTS:**
- Run full test suite
- Manual E2E walkthrough
- No dead imports or unused code

**FORMAT:**
- Run: `cargo clippy` — fix all warnings
- Run: `cargo test` — all pass
- Manual walkthrough of all three pages
- Update design doc status to "Implemented"

**FAILURE CONDITIONS:**
- Any test failure
- Clippy warnings in modified files
- Dead code left behind

**Step 1: Run full test suite**

```bash
cargo test 2>&1 | tail -5
```
Expected: All tests pass.

**Step 2: Run clippy**

```bash
cargo clippy 2>&1 | grep warning
```
Fix any warnings in modified files.

**Step 3: Manual E2E walkthrough**

```bash
rm -f data/staging/app.db && APP_ENV=staging cargo run
```

1. Login as admin
2. **Users page**: Create user → verify no role dropdown, user gets viewer role in DB
3. **Users page**: Edit user → verify no role dropdown
4. **Roles page**: Click "By Role" tab → select a role → see members → add a user → remove a user
5. **Roles page**: Click "By User" tab → see all users with role badges → add role → remove role
6. **Roles page**: Click a user → see menu preview panel
7. **Role Builder**: Create new role → assign permissions → save
8. **Role Builder**: Edit existing role → verify delete button
9. **Permissions**: Create a user with `roles.assign` but not `roles.manage` → verify they see Roles page but not Role Builder

**Step 4: Update design doc**

Change status in `docs/plans/2026-02-20-users-roles-separation-design.md` from "Approved" to "Implemented".

**Step 5: Final commit**

```bash
git add -A
git commit -m "feat: complete users/roles/role builder separation (multi-role support)"
```
