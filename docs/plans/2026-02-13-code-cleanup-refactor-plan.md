# Code Cleanup & Refactoring Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Comprehensive code quality improvement: eliminate technical debt, establish consistent error handling, reduce boilerplate by ~50%, and split long files into focused modules.

**Architecture:** Four-phase approach: (1) Build error handling foundation with AppError + render helper, (2) Fix clippy warnings and dead code, (3) Validate pattern with user_handlers.rs POC, (4) Expand to all handlers and split 5 large files into 15 focused modules.

**Tech Stack:** Rust, Actix-web 4, rusqlite, Askama 0.14

---

## Phase 1: Error Handling Foundation

### Task 1: Enhance AppError enum

**Files:**
- Modify: `src/errors.rs`

**Step 1: Add new error variants**

Add `PermissionDenied` and `Session` variants to AppError enum:

```rust
#[derive(Debug)]
pub enum AppError {
    Db(rusqlite::Error),
    Pool(r2d2::Error),
    Template(askama::Error),
    Hash(String),
    NotFound,
    PermissionDenied(String),  // NEW
    Session(String),           // NEW
}
```

**Step 2: Update Display impl**

Add Display cases for new variants:

```rust
impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Db(e) => write!(f, "Database error: {e}"),
            AppError::Pool(e) => write!(f, "Pool error: {e}"),
            AppError::Template(e) => write!(f, "Template error: {e}"),
            AppError::Hash(e) => write!(f, "Hash error: {e}"),
            AppError::NotFound => write!(f, "Not found"),
            AppError::PermissionDenied(perm) => write!(f, "Permission denied: {}", perm),
            AppError::Session(msg) => write!(f, "Session error: {}", msg),
        }
    }
}
```

**Step 3: Update ResponseError impl**

Add PermissionDenied case to error_response:

```rust
impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        match self {
            AppError::PermissionDenied(_) => {
                HttpResponse::Forbidden()
                    .content_type("text/html; charset=utf-8")
                    .body("<h1>403 Forbidden</h1><p>You don't have permission to access this resource.</p>")
            }
            AppError::NotFound => {
                let html = include_str!("../templates/errors/404.html");
                HttpResponse::NotFound()
                    .content_type("text/html; charset=utf-8")
                    .body(html)
            }
            _ => {
                log::error!("{self}");
                let html = include_str!("../templates/errors/500.html");
                HttpResponse::InternalServerError()
                    .content_type("text/html; charset=utf-8")
                    .body(html)
            }
        }
    }
}
```

**Step 4: Remove #[allow(dead_code)] attribute**

Remove the `#[allow(dead_code)]` line from above the AppError enum.

**Step 5: Build to verify**

Run: `cargo build`
Expected: Clean build (may have warnings from other files)

**Step 6: Commit**

```bash
git add src/errors.rs
git commit -m "refactor: enhance AppError with PermissionDenied and Session variants

- Add PermissionDenied(String) for authorization failures
- Add Session(String) for session-related errors
- Update Display and ResponseError impls
- Remove allow(dead_code) attribute

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 2: Create render helper function

**Files:**
- Modify: `src/errors.rs`

**Step 1: Add imports**

Add askama and actix-web imports at top of file:

```rust
use actix_web::{HttpResponse, ResponseError};
use askama::Template;
use std::fmt;
```

**Step 2: Implement render helper**

Add this function after the ResponseError impl:

```rust
/// Helper to render Askama templates with automatic error conversion.
pub fn render<T: Template>(tmpl: T) -> Result<HttpResponse, AppError> {
    let body = tmpl.render()?;  // Uses From<askama::Error> for AppError
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(body))
}
```

**Step 3: Build to verify**

Run: `cargo build`
Expected: Clean build

**Step 4: Commit**

```bash
git add src/errors.rs
git commit -m "feat: add render() helper for template rendering

Enables idiomatic error handling with ? operator for templates.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 3: Update require_permission to return Result<(), AppError>

**Files:**
- Modify: `src/auth/session.rs`

**Step 1: Import AppError**

Add import at top of file:

```rust
use crate::errors::AppError;
```

**Step 2: Update get_permissions signature**

Change `get_permissions` to return `Result<Permissions, String>`:

```rust
pub fn get_permissions(session: &Session) -> Result<Permissions, String> {
    match session.get::<String>("permissions") {
        Ok(Some(csv)) => Ok(Permissions::from_csv(&csv)),
        Ok(None) => Err("No permissions in session".to_string()),
        Err(e) => Err(format!("Session error: {}", e)),
    }
}
```

**Step 3: Update require_permission signature**

Change return type from `Result<(), HttpResponse>` to `Result<(), AppError>`:

```rust
pub fn require_permission(session: &Session, code: &str) -> Result<(), AppError> {
    let permissions = get_permissions(session)
        .map_err(|e| AppError::Session(format!("Failed to get permissions: {}", e)))?;

    if permissions.has(code) {
        Ok(())
    } else {
        Err(AppError::PermissionDenied(code.to_string()))
    }
}
```

**Step 4: Update get_username to return Result**

Change `get_username` for consistency:

```rust
pub fn get_username(session: &Session) -> Result<String, String> {
    match session.get::<String>("username") {
        Ok(Some(username)) => Ok(username),
        Ok(None) => Err("No username in session".to_string()),
        Err(e) => Err(format!("Session error: {}", e)),
    }
}
```

**Step 5: Build to verify**

Run: `cargo build`
Expected: Will fail - handlers still expect old signature

**Step 6: Commit**

```bash
git add src/auth/session.rs
git commit -m "refactor: update session helpers to return Result with AppError

- require_permission now returns Result<(), AppError>
- get_permissions returns Result<Permissions, String>
- get_username returns Result<String, String>

This enables ? operator usage in handlers.
Note: Breaks compilation until handlers are updated.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 4: Update PageContext::build to return Result

**Files:**
- Modify: `src/templates_structs.rs`

**Step 1: Import AppError**

Add import:

```rust
use crate::errors::AppError;
```

**Step 2: Update PageContext::build signature**

Change return type to `Result<PageContext, AppError>`:

```rust
impl PageContext {
    pub fn build(session: &Session, conn: &Connection, current_url: &str) -> Result<Self, AppError> {
        let username = get_username(session)
            .map_err(|e| AppError::Session(format!("Failed to get username: {}", e)))?;
        let permissions = get_permissions(session)
            .map_err(|e| AppError::Session(format!("Failed to get permissions: {}", e)))?;
        let flash = take_flash(session);
        let (nav_modules, sidebar_items) = nav_item::build_navigation(conn, &permissions, current_url)?;
        let warning_count = 0;

        Ok(PageContext {
            username,
            permissions,
            flash,
            nav_modules,
            sidebar_items,
            warning_count,
        })
    }
}
```

**Step 3: Remove TODO comment**

Find and remove the TODO comment about warning_count (around line 37):

```rust
// Before:
let warning_count = 0; // TODO: wire up when warnings feature is built

// After:
let warning_count = 0;
```

**Step 4: Build to verify**

Run: `cargo build`
Expected: Still fails - handlers need updating

**Step 5: Commit**

```bash
git add src/templates_structs.rs
git commit -m "refactor: PageContext::build returns Result for error propagation

- Changed signature to return Result<PageContext, AppError>
- Session and navigation errors properly propagated
- Removed TODO comment for warning_count

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Phase 2: Quick Wins (Clippy & Dead Code)

### Task 5: Fix redundant import in main.rs

**Files:**
- Modify: `src/main.rs`

**Step 1: Find and remove redundant import**

Run: `cargo clippy 2>&1 | grep "redundant import" -A 3`

Identify the redundant import and remove it from `src/main.rs:1`.

**Step 2: Build to verify**

Run: `cargo clippy 2>&1 | grep "redundant import"`
Expected: No output (warning fixed)

**Step 3: Commit**

```bash
git add src/main.rs
git commit -m "fix: remove redundant import flagged by clippy

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 6: Fix AuditError enum naming

**Files:**
- Modify: `src/audit/mod.rs`

**Step 1: Rename enum variants**

Change AuditError variants to remove redundant "Error" suffix:

```rust
// Before:
#[derive(Debug)]
pub enum AuditError {
    FileError(std::io::Error),
    DbError(rusqlite::Error),
    JsonError(serde_json::Error),
}

// After:
#[derive(Debug)]
pub enum AuditError {
    File(std::io::Error),
    Db(rusqlite::Error),
    Json(serde_json::Error),
}
```

**Step 2: Update Display impl**

Update match arms to use new variant names:

```rust
impl std::fmt::Display for AuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditError::File(e) => write!(f, "File error: {}", e),
            AuditError::Db(e) => write!(f, "Database error: {}", e),
            AuditError::Json(e) => write!(f, "JSON error: {}", e),
        }
    }
}
```

**Step 3: Update usage in functions**

Find all uses of the old variant names and update them:

```rust
// In write_to_file:
.map_err(|e| AuditError::File(e))?;
.map_err(|e| AuditError::Json(e))?;

// Similar for other functions
```

**Step 4: Build to verify**

Run: `cargo build`
Expected: Clean build

Run: `cargo clippy 2>&1 | grep "same postfix"`
Expected: No output (warning fixed)

**Step 5: Commit**

```bash
git add src/audit/mod.rs
git commit -m "fix: remove redundant 'Error' suffix from AuditError variants

FileError -> File, DbError -> Db, JsonError -> Json

Fixes clippy warning about enum naming convention.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 7: Fix collapsible if statements

**Files:**
- Modify: `src/handlers/role_handlers.rs`
- Modify: `src/handlers/settings_handlers.rs`
- Modify: `src/handlers/user_handlers.rs`
- Modify: `src/models/audit.rs`

**Step 1: Find collapsible if statements**

Run: `cargo clippy 2>&1 | grep "can be collapsed" -B 2`

**Step 2: Fix first occurrence**

Example pattern to fix:

```rust
// Before:
if let Some(filter) = filter {
    if !filter.is_empty() {
        // use filter
    }
}

// After:
if let Some(filter) = filter {
    if !filter.is_empty() {
        // use filter
    }
}
// OR combine the conditions if appropriate:
if let Some(filter) = filter.filter(|f| !f.is_empty()) {
    // use filter
}
```

Note: Only collapse if it improves readability. Some nested ifs are clearer separate.

**Step 3: Apply similar fixes to other files**

Check all 8 occurrences and fix appropriately.

**Step 4: Build to verify**

Run: `cargo clippy 2>&1 | grep "can be collapsed"`
Expected: No output (all warnings fixed)

**Step 5: Commit**

```bash
git add src/handlers/role_handlers.rs src/handlers/settings_handlers.rs src/handlers/user_handlers.rs src/models/audit.rs
git commit -m "fix: collapse nested if statements flagged by clippy

Improves code clarity by combining conditions where appropriate.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 8: Remove dead code

**Files:**
- Modify: `src/models/user.rs`

**Step 1: Delete find_all_display function**

Remove the entire `find_all_display` function around line 73:

```rust
// DELETE THIS ENTIRE FUNCTION:
pub fn find_all_display(conn: &Connection) -> rusqlite::Result<Vec<UserDisplay>> {
    let sql = format!("{SELECT_USER_DISPLAY} ORDER BY e.id");
    let mut stmt = conn.prepare(&sql)?;
    let users = stmt.query_map([], row_to_user_display)?.collect::<Result<Vec<_>, _>>()?;
    Ok(users)
}
```

**Step 2: Build to verify**

Run: `cargo build`
Expected: Clean build

Run: `cargo clippy 2>&1 | grep "never used"`
Expected: No output (dead code removed)

**Step 3: Commit**

```bash
git add src/models/user.rs
git commit -m "refactor: remove unused find_all_display function

Replaced by find_paginated in earlier work. No longer needed.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 9: Verify Phase 2 complete

**Step 1: Run final clippy check**

Run: `cargo clippy`
Expected: Zero warnings

**Step 2: Build**

Run: `cargo build`
Expected: Will still fail due to Phase 1 changes breaking handlers

**Step 3: Document Phase 2 completion**

Phase 2 complete. All clippy warnings fixed, dead code removed.
Next: Phase 3 will fix compilation by refactoring user_handlers.rs.

---

## Phase 3: Proof-of-Concept (user_handlers.rs)

### Task 10: Refactor user list handler

**Files:**
- Modify: `src/handlers/user_handlers.rs`

**Step 1: Update function signature**

Change return type from `impl Responder` to `Result<HttpResponse, AppError>`:

```rust
pub async fn list(
    pool: web::Data<DbPool>,
    session: Session,
    query: web::Query<PaginationQuery>,
) -> Result<HttpResponse, AppError> {
```

**Step 2: Replace permission check**

Replace the if-let pattern with `?`:

```rust
// Before:
if let Err(resp) = require_permission(&session, "users.list") {
    return resp;
}

// After:
require_permission(&session, "users.list")?;
```

**Step 3: Replace pool.get() handling**

Replace match with `?`:

```rust
// Before:
let conn = match pool.get() {
    Ok(c) => c,
    Err(_) => return HttpResponse::InternalServerError().body("Database error"),
};

// After:
let conn = pool.get()?;
```

**Step 4: Replace PageContext::build handling**

Add `?` to PageContext::build:

```rust
// Before:
let ctx = PageContext::build(&session, &conn, "/users");

// After:
let ctx = PageContext::build(&session, &conn, "/users")?;
```

**Step 5: Replace user::find_paginated handling**

Replace unwrap_or_else with `?`:

```rust
// Before:
let user_page = user::find_paginated(&conn, page, per_page, search)
    .unwrap_or_else(|_| user::UserPage {
        users: vec![],
        page: 1,
        per_page: 25,
        total_count: 0,
        total_pages: 0,
    });

// After:
let user_page = user::find_paginated(&conn, page, per_page, search)?;
```

**Step 6: Replace template rendering**

Replace match with render() helper:

```rust
// Before:
let tmpl = UserListTemplate { ctx, user_page, search_query: search };
match tmpl.render() {
    Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
    Err(_) => HttpResponse::InternalServerError().body("Template error"),
}

// After:
use crate::errors::render;

let tmpl = UserListTemplate { ctx, user_page, search_query: search };
render(tmpl)
```

**Step 7: Add render import at top of file**

Add to imports:

```rust
use crate::errors::{AppError, render};
```

**Step 8: Build to verify**

Run: `cargo build`
Expected: May still have errors from other handlers

**Step 9: Commit**

```bash
git add src/handlers/user_handlers.rs
git commit -m "refactor: convert user list handler to AppError pattern

- Changed return type to Result<HttpResponse, AppError>
- Use ? operator for error propagation
- Reduced from ~20 lines to ~10 lines
- Improved readability and consistency

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 11: Refactor user new handler

**Files:**
- Modify: `src/handlers/user_handlers.rs`

**Step 1: Update signature and refactor**

Apply same pattern as list handler:

```rust
pub async fn new(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.create")?;
    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/users/new")?;
    let roles = role::find_all_display(&conn)?;

    let tmpl = UserFormTemplate {
        ctx,
        user: None,
        roles,
        errors: vec![],
    };
    render(tmpl)
}
```

**Step 2: Build**

Run: `cargo build`

**Step 3: Commit**

```bash
git add src/handlers/user_handlers.rs
git commit -m "refactor: convert user new handler to AppError pattern

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 12: Refactor user create handler

**Files:**
- Modify: `src/handlers/user_handlers.rs`

**Step 1: Update signature**

```rust
pub async fn create(
    pool: web::Data<DbPool>,
    session: Session,
    body: String,
) -> Result<HttpResponse, AppError> {
```

**Step 2: Refactor permission and connection**

```rust
    require_permission(&session, "users.create")?;
    let conn = pool.get()?;
```

**Step 3: Keep validation logic but update error responses**

The validation logic should remain, but when re-rendering the form on error, use `?`:

```rust
    if !errors.is_empty() {
        let ctx = PageContext::build(&session, &conn, "/users/new")?;
        let roles = role::find_all_display(&conn)?;
        let tmpl = UserFormTemplate { ctx, user: Some(new), roles, errors };
        return render(tmpl);
    }
```

**Step 4: Update successful creation path**

```rust
    match user::create(&conn, &new) {
        Ok(user_id) => {
            // ... audit logging ...
            crate::auth::session::set_flash(&session, "User created successfully");
            Ok(HttpResponse::SeeOther()
                .append_header(("Location", format!("/users/{}", user_id)))
                .finish())
        }
        Err(_) => {
            let ctx = PageContext::build(&session, &conn, "/users/new")?;
            let roles = role::find_all_display(&conn)?;
            let mut errs = errors;
            errs.push("Database error creating user".to_string());
            let tmpl = UserFormTemplate { ctx, user: Some(new), roles, errors: errs };
            render(tmpl)
        }
    }
```

**Step 5: Build**

Run: `cargo build`

**Step 6: Commit**

```bash
git add src/handlers/user_handlers.rs
git commit -m "refactor: convert user create handler to AppError pattern

Maintains validation logic while using ? for error propagation.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 13: Refactor remaining user handlers (edit, update, delete, show)

**Files:**
- Modify: `src/handlers/user_handlers.rs`

**Step 1: Refactor edit handler**

Apply AppError pattern to `edit` function following same pattern.

**Step 2: Refactor update handler**

Apply AppError pattern to `update` function.

**Step 3: Refactor delete handler**

Apply AppError pattern to `delete` function.

**Step 4: Refactor show handler**

Apply AppError pattern to `show` function (if it exists).

**Step 5: Build**

Run: `cargo build`
Expected: user_handlers.rs should compile cleanly now

**Step 6: Commit**

```bash
git add src/handlers/user_handlers.rs
git commit -m "refactor: convert all remaining user handlers to AppError pattern

- edit, update, delete, show handlers refactored
- Consistent error handling across all user CRUD operations
- ~50% line reduction (384 → ~200 lines)

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 14: Manual testing of user handlers

**Step 1: Start server**

Run: `cargo run`

**Step 2: Test user list**

- Open browser to http://localhost:8080/users
- Verify list displays
- Test pagination
- Test search

**Step 3: Test user create**

- Click "New User"
- Fill form
- Submit
- Verify user created
- Check flash message

**Step 4: Test user edit**

- Click edit on a user
- Modify fields
- Submit
- Verify changes saved

**Step 5: Test user delete**

- Delete a user
- Verify deletion
- Check audit log

**Step 6: Test permission denied**

- Logout
- Login as non-admin user
- Try to access /users
- Verify 403 Forbidden page displays

**Step 7: Document results**

Create test report:
- All CRUD operations working: ✅
- Error pages displaying correctly: ✅
- Line count reduction: 384 → ~200 lines (✅ ~48%)
- Readability improved: ✅

**Step 8: Stop server**

Ctrl+C

**Step 9: Decision: Proceed to Phase 4**

POC successful. Pattern validated. Ready to expand.

---

## Phase 4A: Handler Expansion

### Task 15: Refactor role handlers

**Files:**
- Modify: `src/handlers/role_handlers.rs`

**Step 1: Apply AppError pattern to all handlers**

Convert all 7 role handlers using same pattern as user handlers:
- list
- new
- create
- edit
- update
- delete
- (show if exists)

**Step 2: Build**

Run: `cargo build`

**Step 3: Manual test**

- Test role list, create, edit, delete
- Verify permissions assignment works
- Check audit logging

**Step 4: Commit**

```bash
git add src/handlers/role_handlers.rs
git commit -m "refactor: convert role handlers to AppError pattern

All 7 handlers refactored with ~50% line reduction.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 16: Refactor audit handlers

**Files:**
- Modify: `src/handlers/audit_handlers.rs`

**Step 1: Refactor list handler**

Apply AppError pattern to audit list handler.

**Step 2: Build and test**

Run: `cargo build`
Test: Browse to /audit, verify search/filter works

**Step 3: Commit**

```bash
git add src/handlers/audit_handlers.rs
git commit -m "refactor: convert audit handlers to AppError pattern

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 17: Refactor account handlers

**Files:**
- Modify: `src/handlers/account_handlers.rs`

**Step 1: Refactor account handlers**

Apply AppError pattern to both account handlers (view + change_password).

**Step 2: Build and test**

**Step 3: Commit**

```bash
git add src/handlers/account_handlers.rs
git commit -m "refactor: convert account handlers to AppError pattern

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 18: Refactor settings handlers

**Files:**
- Modify: `src/handlers/settings_handlers.rs`

**Step 1: Refactor settings handlers**

Apply AppError pattern.

**Step 2: Build and test**

**Step 3: Commit**

```bash
git add src/handlers/settings_handlers.rs
git commit -m "refactor: convert settings handlers to AppError pattern

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 19: Refactor ontology handlers

**Files:**
- Modify: `src/handlers/ontology_handlers.rs`

**Step 1: Refactor ontology handlers**

Apply AppError pattern to all 3 ontology handlers.

**Step 2: Build and test**

Test all three tabs of ontology explorer.

**Step 3: Commit**

```bash
git add src/handlers/ontology_handlers.rs
git commit -m "refactor: convert ontology handlers to AppError pattern

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 20: Refactor auth handlers

**Files:**
- Modify: `src/handlers/auth_handlers.rs`

**Step 1: Refactor login/logout handlers**

Apply AppError pattern to auth handlers.

Note: Be careful with login handler - error handling might be intentionally different for security.

**Step 2: Build and test**

Test login and logout flows.

**Step 3: Commit**

```bash
git add src/handlers/auth_handlers.rs
git commit -m "refactor: convert auth handlers to AppError pattern

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 21: Check dashboard handler

**Files:**
- Check: `src/handlers/dashboard.rs`

**Step 1: Review dashboard handler**

Check if dashboard.rs needs refactoring or if it's simple enough to leave as-is.

**Step 2: Refactor if needed**

If it follows the same pattern, apply AppError.
If it's already simple (5-10 lines), consider leaving it.

**Step 3: Commit if changed**

```bash
git add src/handlers/dashboard.rs
git commit -m "refactor: convert dashboard handler to AppError pattern

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 22: Full application test

**Step 1: Build final check**

Run: `cargo build`
Expected: Clean build, zero warnings

Run: `cargo clippy`
Expected: Zero warnings

**Step 2: Comprehensive manual testing**

- Login
- Dashboard
- User CRUD (all operations)
- Role CRUD (all operations)
- Settings (view and modify)
- Account (change password)
- Audit log (view, search, filter)
- Ontology explorer (all three tabs)
- Permission denied scenarios
- Error pages (404, 403, 500)
- Logout

**Step 3: Document Phase 4A completion**

All handlers refactored. Error handling consistent across entire application.

---

## Phase 4B: File Splitting

### Task 23: Split models/ontology.rs

**Files:**
- Create: `src/models/ontology/mod.rs`
- Create: `src/models/ontology/schema.rs`
- Create: `src/models/ontology/instance.rs`
- Create: `src/models/ontology/entities.rs`
- Delete: `src/models/ontology.rs`
- Modify: `src/models/mod.rs`

**Step 1: Create ontology directory**

Run: `mkdir -p src/models/ontology`

**Step 2: Create schema.rs**

Extract schema-related code from ontology.rs:

```rust
// src/models/ontology/schema.rs
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct EntityTypeSummary {
    pub entity_type: String,
    pub count: i64,
    pub property_keys: Vec<String>,
    pub sample_entities: Vec<EntitySample>,
}

#[derive(Debug, Clone)]
pub struct EntitySample {
    pub id: i64,
    pub name: String,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct RelationTypeSummary {
    pub name: String,
    pub label: String,
    pub usage_count: i64,
    pub patterns: Vec<RelationPattern>,
}

#[derive(Debug, Clone)]
pub struct RelationPattern {
    pub source_type: String,
    pub target_type: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SchemaNode {
    pub entity_type: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SchemaEdge {
    pub source: String,
    pub target: String,
    pub relation_type: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SchemaGraphData {
    pub nodes: Vec<SchemaNode>,
    pub edges: Vec<SchemaEdge>,
}

pub fn find_schema_graph_data(conn: &Connection) -> rusqlite::Result<SchemaGraphData> {
    // ... implementation from ontology.rs ...
}

pub fn find_entity_type_summaries(conn: &Connection) -> rusqlite::Result<Vec<EntityTypeSummary>> {
    // ... implementation from ontology.rs ...
}

pub fn find_relation_type_summaries(conn: &Connection) -> rusqlite::Result<Vec<RelationTypeSummary>> {
    // ... implementation from ontology.rs ...
}
```

**Step 3: Create instance.rs**

Extract instance graph code:

```rust
// src/models/ontology/instance.rs
use std::collections::HashMap;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct GraphNode {
    pub id: i64,
    pub entity_type: String,
    pub name: String,
    pub label: String,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphEdge {
    pub source: i64,
    pub target: i64,
    pub relation_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}
```

**Step 4: Create entities.rs**

Extract entity CRUD code:

```rust
// src/models/ontology/entities.rs
use rusqlite::Connection;

#[derive(Debug, Clone)]
pub struct EntityListItem {
    pub id: i64,
    pub entity_type: String,
    pub name: String,
    pub label: String,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct EntityProperty {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct RelatedEntity {
    pub relation_id: i64,
    pub relation_type: String,
    pub entity_id: i64,
    pub entity_name: String,
    pub entity_label: String,
    pub direction: String,
}

#[derive(Debug, Clone)]
pub struct EntityDetail {
    pub id: i64,
    pub entity_type: String,
    pub name: String,
    pub label: String,
    pub is_active: bool,
    pub properties: Vec<EntityProperty>,
    pub outgoing_relations: Vec<RelatedEntity>,
    pub incoming_relations: Vec<RelatedEntity>,
    pub created_at: String,
    pub updated_at: String,
}

pub fn find_entity_list(conn: &Connection, type_filter: Option<&str>) -> rusqlite::Result<Vec<EntityListItem>> {
    // ... implementation ...
}

pub fn find_entity_detail(conn: &Connection, id: i64) -> rusqlite::Result<Option<EntityDetail>> {
    // ... implementation ...
}

pub fn find_entity_types(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    // ... implementation ...
}
```

**Step 5: Create mod.rs**

```rust
// src/models/ontology/mod.rs
mod schema;
mod instance;
mod entities;

// Re-export all public types and functions
pub use schema::*;
pub use instance::*;
pub use entities::*;
```

**Step 6: Update models/mod.rs**

Remove `pub mod ontology;` if it was a file, keep it as is if it was already `pub mod ontology;` (directory).

**Step 7: Delete old ontology.rs**

Run: `rm src/models/ontology.rs`

**Step 8: Build**

Run: `cargo build`
Expected: Clean build with no changes to public API

**Step 9: Commit**

```bash
git add src/models/ontology/
git rm src/models/ontology.rs
git commit -m "refactor: split ontology.rs into focused modules

- schema.rs (150 lines) - Schema graph types and queries
- instance.rs (150 lines) - Instance graph types
- entities.rs (150 lines) - Entity CRUD operations
- mod.rs (30 lines) - Public API re-exports

Reduces single 471-line file into four focused modules.
Public API unchanged via re-exports.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 24: Split models/user.rs

**Files:**
- Create: `src/models/user/mod.rs`
- Create: `src/models/user/types.rs`
- Create: `src/models/user/queries.rs`
- Delete: `src/models/user.rs`

**Step 1: Create user directory**

Run: `mkdir -p src/models/user`

**Step 2: Create types.rs**

Extract type definitions:

```rust
// src/models/user/types.rs
use rusqlite::Row;

#[derive(Debug, Clone)]
pub struct UserDisplay {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub email: String,
    pub role_id: i64,
    pub role_name: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct UserForm {
    pub username: String,
    pub password: String,
    pub display_name: String,
    pub email: String,
    pub role_id: i64,
}

#[derive(Debug, Clone)]
pub struct UserPage {
    pub users: Vec<UserDisplay>,
    pub page: i64,
    pub per_page: i64,
    pub total_count: i64,
    pub total_pages: i64,
}

pub(super) fn row_to_user_display(row: &Row) -> rusqlite::Result<UserDisplay> {
    // ... implementation ...
}

pub(super) const SELECT_USER_DISPLAY: &str = "...";
```

**Step 3: Create queries.rs**

Extract all query functions:

```rust
// src/models/user/queries.rs
use rusqlite::Connection;
use super::types::*;

pub fn find_paginated(
    conn: &Connection,
    page: i64,
    per_page: i64,
    search: Option<&str>
) -> rusqlite::Result<UserPage> {
    // ... implementation ...
}

pub fn find_display_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<UserDisplay>> {
    // ... implementation ...
}

pub fn create(conn: &Connection, new: &UserForm) -> rusqlite::Result<i64> {
    // ... implementation ...
}

pub fn update(conn: &Connection, id: i64, form: &UserForm) -> rusqlite::Result<()> {
    // ... implementation ...
}

pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    // ... implementation ...
}

// ... other query functions ...
```

**Step 4: Create mod.rs**

```rust
// src/models/user/mod.rs
mod types;
mod queries;

pub use types::{UserDisplay, UserForm, UserPage};
pub use queries::*;
```

**Step 5: Delete old user.rs and rebuild**

Run: `rm src/models/user.rs`
Run: `cargo build`

**Step 6: Commit**

```bash
git add src/models/user/
git rm src/models/user.rs
git commit -m "refactor: split user.rs into types and queries modules

- types.rs (80 lines) - UserDisplay, UserForm, UserPage
- queries.rs (230 lines) - All database query functions
- mod.rs (20 lines) - Public API re-exports

Reduces single 323-line file into three focused modules.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 25: Split models/role.rs

**Files:**
- Create: `src/models/role/mod.rs`
- Create: `src/models/role/types.rs`
- Create: `src/models/role/queries.rs`
- Delete: `src/models/role.rs`

**Step 1: Apply same pattern as user.rs**

Create role directory and split into types.rs and queries.rs following same structure.

**Step 2: Build and commit**

```bash
git add src/models/role/
git rm src/models/role.rs
git commit -m "refactor: split role.rs into types and queries modules

- types.rs (60 lines) - RoleDisplay, RoleForm, RoleDetail
- queries.rs (160 lines) - All database query functions
- mod.rs (20 lines) - Public API re-exports

Reduces single 236-line file into three focused modules.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 26: Split handlers/user_handlers.rs

**Files:**
- Create: `src/handlers/users/mod.rs`
- Create: `src/handlers/users/list.rs`
- Create: `src/handlers/users/crud.rs`
- Delete: `src/handlers/user_handlers.rs`
- Modify: `src/handlers/mod.rs`

**Step 1: Create users directory**

Run: `mkdir -p src/handlers/users`

**Step 2: Create list.rs**

Extract list handler and PaginationQuery:

```rust
// src/handlers/users/list.rs
use actix_session::Session;
use actix_web::{web, HttpResponse};
use serde::Deserialize;

use crate::db::DbPool;
use crate::models::user;
use crate::auth::session::require_permission;
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, UserListTemplate};

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub q: Option<String>,
}

pub async fn list(
    pool: web::Data<DbPool>,
    session: Session,
    query: web::Query<PaginationQuery>,
) -> Result<HttpResponse, AppError> {
    // ... implementation ...
}
```

**Step 3: Create crud.rs**

Extract all other handlers:

```rust
// src/handlers/users/crud.rs
use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::models::{user, role};
use crate::auth::{session::require_permission, csrf, password};
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, UserFormTemplate};

pub async fn new(...) -> Result<HttpResponse, AppError> {
    // ...
}

pub async fn create(...) -> Result<HttpResponse, AppError> {
    // ...
}

pub async fn edit(...) -> Result<HttpResponse, AppError> {
    // ...
}

pub async fn update(...) -> Result<HttpResponse, AppError> {
    // ...
}

pub async fn delete(...) -> Result<HttpResponse, AppError> {
    // ...
}

pub async fn show(...) -> Result<HttpResponse, AppError> {
    // ...
}
```

**Step 4: Create mod.rs**

```rust
// src/handlers/users/mod.rs
mod list;
mod crud;

pub use list::*;
pub use crud::*;
```

**Step 5: Update handlers/mod.rs**

Change `pub mod user_handlers;` to `pub mod users;` (or update the use statements).

**Step 6: Update main.rs route registration**

Update imports and route registration:

```rust
// Before:
use handlers::user_handlers;

// After:
use handlers::users;
```

**Step 7: Delete old file, build, and commit**

```bash
git add src/handlers/users/
git rm src/handlers/user_handlers.rs
git add src/handlers/mod.rs src/main.rs
git commit -m "refactor: split user_handlers.rs into list and crud modules

- list.rs (60 lines) - List handler with pagination
- crud.rs (120 lines) - New, create, edit, update, delete handlers
- mod.rs (20 lines) - Public API

Reduces ~200-line file into three focused modules.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 27: Split handlers/role_handlers.rs

**Files:**
- Create: `src/handlers/roles/mod.rs`
- Create: `src/handlers/roles/list.rs`
- Create: `src/handlers/roles/crud.rs`
- Delete: `src/handlers/role_handlers.rs`

**Step 1: Apply same pattern**

Follow same structure as user handlers split.

**Step 2: Build and commit**

```bash
git add src/handlers/roles/
git rm src/handlers/role_handlers.rs
git add src/handlers/mod.rs src/main.rs
git commit -m "refactor: split role_handlers.rs into list and crud modules

- list.rs (60 lines) - List handler
- crud.rs (120 lines) - CRUD handlers
- mod.rs (20 lines) - Public API

Reduces ~200-line file into three focused modules.

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 28: Final validation

**Step 1: Clean build**

Run: `cargo clean`
Run: `cargo build --release`
Expected: Clean build, zero warnings

Run: `cargo clippy`
Expected: Zero warnings

**Step 2: Comprehensive testing**

Run: `cargo run --release`

Test all functionality:
- ✅ User CRUD
- ✅ Role CRUD
- ✅ Settings
- ✅ Audit log
- ✅ Ontology explorer
- ✅ Account management
- ✅ Login/logout
- ✅ Permission denied
- ✅ Error pages

**Step 3: Code metrics**

Count lines:

Run: `find src/handlers -name "*.rs" -exec wc -l {} + | tail -1`
Run: `find src/models -name "*.rs" -exec wc -l {} + | tail -1`

Compare to original:
- Handlers: ~1800 lines → ~900 lines (50% reduction)
- Models: ~1300 lines → ~1300 lines (same total, better organized)
- Files: 8 large files → 23 focused modules

**Step 4: Update BACKLOG.md**

Add to "Architecture Decisions" section:

```markdown
### Code Quality Improvements (2026-02-13)
**Handler Pattern Evolution:** All 24 handlers refactored to use AppError with `?` operator, reducing boilerplate by ~50%. The `render()` helper and consistent error handling realize the architecture decision in BACKLOG.md to "adopt a `render()` helper (Approach B) and a proper `AppError` type with `ResponseError` impl."

**File Organization:** Split 5 large files (471, 384, 370, 323, 236 lines) into 15 focused modules averaging ~130 lines each. Clear separation: schema vs instance vs entities (ontology), types vs queries (models), list vs CRUD (handlers).
```

**Step 5: Final commit**

```bash
git add docs/BACKLOG.md
git commit -m "docs: update BACKLOG with code quality improvements

Documenting completion of comprehensive refactoring:
- All handlers use AppError pattern (~50% line reduction)
- 5 large files split into 15 focused modules
- Zero clippy warnings
- Zero dead code

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Completion Checklist

### Phase 1: Error Handling Foundation
- ✅ AppError enhanced with PermissionDenied + Session variants
- ✅ render() helper created
- ✅ require_permission returns Result<(), AppError>
- ✅ PageContext::build returns Result

### Phase 2: Quick Wins
- ✅ Zero Clippy warnings
- ✅ Dead code removed
- ✅ TODO comment removed

### Phase 3: Proof-of-Concept
- ✅ user_handlers.rs refactored (7 handlers)
- ✅ ~50% line reduction validated
- ✅ Manual testing passed
- ✅ Pattern approved for expansion

### Phase 4A: Handler Expansion
- ✅ role_handlers.rs (7 handlers)
- ✅ audit_handlers.rs (1 handler)
- ✅ account_handlers.rs (2 handlers)
- ✅ settings_handlers.rs (2 handlers)
- ✅ ontology_handlers.rs (3 handlers)
- ✅ auth_handlers.rs (2 handlers)
- ✅ dashboard.rs (1 handler)

### Phase 4B: File Splitting
- ✅ models/ontology.rs → 4 modules
- ✅ models/user.rs → 3 modules
- ✅ models/role.rs → 3 modules
- ✅ handlers/user_handlers.rs → 3 modules
- ✅ handlers/role_handlers.rs → 3 modules

### Final Validation
- ✅ cargo build --release (clean)
- ✅ cargo clippy (zero warnings)
- ✅ All functionality tested
- ✅ Documentation updated

---

## Success Metrics

**Before:**
- 10 Clippy warnings
- Dead code in 2 places
- Manual error handling in 24 handlers
- 8 files over 200 lines (5 over 300)
- Inconsistent error patterns

**After:**
- 0 Clippy warnings
- 0 Dead code
- AppError + ? operator in all 24 handlers
- 23 focused modules averaging ~130 lines
- Consistent error handling across entire app
- ~50% handler line reduction
- Cleaner, more maintainable codebase

---

## Notes

**No Automated Tests:**
This project does not have automated tests. Manual testing via browser is required after each phase. Focus testing on:
- Happy paths (CRUD operations work)
- Error cases (permission denied, validation errors)
- Edge cases (empty lists, missing data)

**Commit Frequency:**
Commit after each task completion. Each commit should compile (after Phase 1 is complete). Use descriptive commit messages.

**Rollback Strategy:**
Each phase is independently committable. If issues arise, can roll back to previous phase with `git reset --hard <commit>`.

**Time Estimates:**
- Phase 1: 2-3 hours
- Phase 2: 1-2 hours
- Phase 3: 3-4 hours
- Phase 4A: 8-10 hours
- Phase 4B: 6-8 hours
- Total: ~20-27 hours

**Dependencies:**
No new crate dependencies needed. All work uses existing libraries.
