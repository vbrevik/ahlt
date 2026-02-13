# Code Cleanup & Refactoring - Design Document

**Date:** 2026-02-13
**Status:** Approved
**Approach:** Comprehensive phased cleanup with foundation-first strategy

---

## Overview

Comprehensive code quality improvement addressing technical debt, dead code, and architectural consistency across the codebase.

**Goals:**
- Eliminate all Clippy warnings and dead code
- Establish consistent error handling pattern using AppError + `?` operator
- Reduce handler boilerplate by ~50%
- Split long files (300+ lines) into focused modules
- Improve maintainability and readability

**Non-Goals:**
- Automated testing (separate initiative)
- Performance optimization
- Database query refactoring

---

## Current State Analysis

### Issues Identified

**Dead Code:**
- `find_all_display()` in `src/models/user.rs:73` - never used
- `AppError` enum marked with `#[allow(dead_code)]` - defined but barely used

**Clippy Warnings (10 total):**
- Redundant import in `main.rs:1`
- Enum naming: `AuditError` variants end in "Error" (FileError, DbError, JsonError)
- 8 collapsible `if` statements across handlers and models

**Long Files (300+ lines):**
- `src/models/ontology.rs` - 471 lines (7 functions, 10 structs)
- `src/handlers/user_handlers.rs` - 384 lines (7 handlers)
- `src/handlers/role_handlers.rs` - 370 lines (7 handlers)
- `src/models/user.rs` - 323 lines (11 functions)
- `src/models/role.rs` - 236 lines (9 functions)

**Inconsistent Error Handling:**
- `AppError` enum exists with `ResponseError` impl but handlers do manual error handling
- All 24 handlers repeat: permission check → get conn → build PageContext → query → render
- No `render()` helper despite BACKLOG.md recommending it

**Technical Debt:**
- TODO comment: `warning_count` placeholder in `templates_structs.rs:37`

---

## Architecture

### Four-Phase Approach

**Phase 1: Error Handling Foundation** (Build it right)
**Phase 2: Quick Wins** (Clean up warnings)
**Phase 3: Proof-of-Concept** (Validate pattern)
**Phase 4: Expansion** (Apply everywhere)

**Rationale for foundation-first:**
- Validates AppError design before committing to large refactor
- Quick wins build momentum
- POC de-risks handler expansion
- File splitting happens after handlers are already smaller

---

## Phase 1: Error Handling Foundation

### Goal
Create robust error handling foundation enabling handlers to use `?` operator.

### Changes

**1. Enhance AppError enum** (`src/errors.rs`)

Current:
```rust
#[derive(Debug)]
#[allow(dead_code)]  // ← Remove this
pub enum AppError {
    Db(rusqlite::Error),
    Pool(r2d2::Error),
    Template(askama::Error),
    Hash(String),
    NotFound,
}
```

Enhanced:
```rust
#[derive(Debug)]
pub enum AppError {
    Db(rusqlite::Error),
    Pool(r2d2::Error),
    Template(askama::Error),
    Hash(String),
    NotFound,
    PermissionDenied(String),  // NEW: for require_permission
    Session(String),           // NEW: for session errors
}

// Add Display cases for new variants
impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // ... existing cases ...
            AppError::PermissionDenied(perm) => write!(f, "Permission denied: {}", perm),
            AppError::Session(msg) => write!(f, "Session error: {}", msg),
        }
    }
}

// Add ResponseError case for PermissionDenied
impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        match self {
            AppError::PermissionDenied(_) => {
                HttpResponse::Forbidden()
                    .content_type("text/html; charset=utf-8")
                    .body("<h1>403 Forbidden</h1><p>You don't have permission to access this resource.</p>")
            }
            AppError::NotFound => { /* existing */ }
            _ => { /* existing 500 handler */ }
        }
    }
}
```

**2. Create render helper** (`src/errors.rs` or `src/handlers/mod.rs`)

```rust
use askama::Template;
use actix_web::HttpResponse;

pub fn render<T: Template>(tmpl: T) -> Result<HttpResponse, AppError> {
    let body = tmpl.render()?;  // Uses From<askama::Error> for AppError
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(body))
}
```

**3. Update require_permission** (`src/auth/session.rs`)

Current:
```rust
pub fn require_permission(session: &Session, code: &str) -> Result<(), HttpResponse> {
    let permissions = match get_permissions(session) {
        Ok(p) => p,
        Err(_) => return Err(HttpResponse::Forbidden().body("Permission denied")),
    };
    if permissions.has(code) {
        Ok(())
    } else {
        Err(HttpResponse::Forbidden().body("Permission denied"))
    }
}
```

Enhanced:
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

// Update get_permissions to return Result<Permissions, String>
pub fn get_permissions(session: &Session) -> Result<Permissions, String> {
    match session.get::<String>("permissions") {
        Ok(Some(csv)) => Ok(Permissions::from_csv(&csv)),
        Ok(None) => Err("No permissions in session".to_string()),
        Err(e) => Err(format!("Session error: {}", e)),
    }
}
```

**4. Update PageContext::build** (`src/templates_structs.rs`)

Change signature to return `Result<PageContext, AppError>`:
```rust
impl PageContext {
    pub fn build(session: &Session, conn: &Connection, current_url: &str) -> Result<Self, AppError> {
        let username = get_username(session)
            .map_err(|e| AppError::Session(format!("Failed to get username: {}", e)))?;
        let permissions = get_permissions(session)
            .map_err(|e| AppError::Session(format!("Failed to get permissions: {}", e)))?;
        let flash = take_flash(session);
        let (nav_modules, sidebar_items) = nav_item::build_navigation(conn, &permissions, current_url)?;
        let warning_count = 0;  // TODO removed - just a placeholder

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

### Files Modified
- `src/errors.rs` - AppError variants + render helper
- `src/auth/session.rs` - require_permission + get_permissions
- `src/templates_structs.rs` - PageContext::build signature

### Impact
- All session/permission/template errors flow through AppError
- Handlers can use `?` operator consistently
- Foundation ready for POC

---

## Phase 2: Quick Wins (Clippy & Dead Code)

### Goal
Eliminate build noise and unused code.

### Changes

**1. Fix Clippy Warnings**

- **Redundant import** (`src/main.rs:1`) - Remove
- **Enum naming** (`src/audit/mod.rs:7`) - Rename variants:
  ```rust
  pub enum AuditError {
      File(std::io::Error),      // was FileError
      Db(rusqlite::Error),        // was DbError
      Json(serde_json::Error),    // was JsonError
  }
  ```
- **Collapsible if statements** (8 occurrences) - Collapse where appropriate

**2. Remove Dead Code**

- Delete `find_all_display()` in `src/models/user.rs:73`
- Remove `#[allow(dead_code)]` from `AppError` enum (now actively used)

**3. Address TODO**

- Remove TODO comment from `templates_structs.rs:37` (warning_count is just a placeholder)

### Files Modified
- `src/main.rs` - Remove redundant import
- `src/audit/mod.rs` - Rename enum variants
- Various files - Collapse if statements
- `src/models/user.rs` - Delete unused function
- `src/errors.rs` - Remove allow(dead_code)
- `src/templates_structs.rs` - Remove TODO comment

### Success Criteria
- `cargo clippy` produces zero warnings
- `cargo build` clean
- No functional changes

---

## Phase 3: Proof-of-Concept (user_handlers.rs)

### Goal
Validate new error handling pattern with one complete handler file.

### Target
`src/handlers/user_handlers.rs` (384 lines, 7 handlers)

### Transformation Pattern

**Before** (~20 lines per handler):
```rust
pub async fn list(
    pool: web::Data<DbPool>,
    session: Session,
    query: web::Query<PaginationQuery>,
) -> impl Responder {
    if let Err(resp) = require_permission(&session, "users.list") {
        return resp;
    }

    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };

    let ctx = PageContext::build(&session, &conn, "/users");
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(25);
    let search = query.q.as_deref();

    let user_page = user::find_paginated(&conn, page, per_page, search)
        .unwrap_or_else(|_| user::UserPage {
            users: vec![],
            page: 1,
            per_page: 25,
            total_count: 0,
            total_pages: 0,
        });

    let tmpl = UserListTemplate { ctx, user_page, search_query: search };
    match tmpl.render() {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(_) => HttpResponse::InternalServerError().body("Template error"),
    }
}
```

**After** (~8-10 lines per handler):
```rust
pub async fn list(
    pool: web::Data<DbPool>,
    session: Session,
    query: web::Query<PaginationQuery>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.list")?;
    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/users")?;

    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(25);
    let search = query.q.as_deref();
    let user_page = user::find_paginated(&conn, page, per_page, search)?;

    let tmpl = UserListTemplate { ctx, user_page, search_query: search };
    render(tmpl)
}
```

### All Handlers to Convert
1. `list` - GET /users
2. `new` - GET /users/new
3. `create` - POST /users
4. `edit` - GET /users/{id}/edit
5. `update` - POST /users/{id}/update
6. `delete` - POST /users/{id}/delete
7. `show` - GET /users/{id}

### Validation Process
1. Refactor all 7 handlers
2. `cargo build` - must compile cleanly
3. `cargo clippy` - zero warnings
4. Manual browser testing:
   - List users (with pagination & search)
   - Create new user
   - Edit existing user
   - Delete user
   - View user detail
5. Document results:
   - Line count: before vs after
   - Readability assessment
   - Any issues encountered

### Success Criteria
- All handlers compile and work correctly
- ~50% line reduction (384 → ~200 lines)
- Code is more readable and idiomatic
- Zero regressions in functionality

### Decision Point
**If successful** → Proceed to Phase 4
**If issues found** → Document learnings, revise approach

---

## Phase 4: Expansion & File Splitting

### Part A: Handler Expansion

Apply the validated pattern to all remaining handlers.

**Priority 1: Similar CRUD handlers** (~6-8 hours)
- `handlers/role_handlers.rs` (370 lines → ~200 lines, 7 handlers)
- `handlers/audit_handlers.rs` (68 lines → ~40 lines, 1 handler)

**Priority 2: Remaining handlers** (~4-6 hours)
- `handlers/account_handlers.rs` (123 lines → ~70 lines, 2 handlers)
- `handlers/settings_handlers.rs` (104 lines → ~60 lines, 2 handlers)
- `handlers/ontology_handlers.rs` (143 lines → ~80 lines, 3 handlers)
- `handlers/auth_handlers.rs` (121 lines → ~70 lines, 2 handlers)
- `handlers/dashboard.rs` (minimal, may not need refactoring)

**Expected Results:**
- All 24 handlers use consistent error handling
- ~40-50% overall line reduction
- Codebase adheres to BACKLOG.md architectural decision

### Part B: File Splitting

Split large files into focused modules for better maintainability.

#### 1. Models to Split

**`models/ontology.rs` (471 lines) → `models/ontology/`**
```
models/ontology/
├── mod.rs              (~30 lines) - Public API re-exports
├── schema.rs           (~150 lines) - Schema graph types & queries
│   └── EntityTypeSummary, RelationTypeSummary, SchemaNode, SchemaEdge
│   └── find_schema_graph_data(), find_entity_type_summaries(), find_relation_type_summaries()
├── instance.rs         (~150 lines) - Instance graph types & queries
│   └── GraphNode, GraphEdge, GraphData
│   └── (future: graph data query function)
└── entities.rs         (~150 lines) - Entity CRUD types & queries
    └── EntityListItem, EntityDetail, EntityProperty, RelatedEntity
    └── find_entity_list(), find_entity_detail(), find_entity_types()
```

**`models/user.rs` (323 lines) → `models/user/`**
```
models/user/
├── mod.rs              (~20 lines) - Public API re-exports
├── types.rs            (~80 lines) - UserDisplay, UserForm, UserPage, row_to_user_display
└── queries.rs          (~230 lines) - All DB query functions
    └── find_paginated(), find_display_by_id(), create(), update(), delete(), etc.
```

**`models/role.rs` (236 lines) → `models/role/`**
```
models/role/
├── mod.rs              (~20 lines) - Public API re-exports
├── types.rs            (~60 lines) - RoleDisplay, RoleForm, RoleDetail, row_to_role_display
└── queries.rs          (~160 lines) - All DB query functions
    └── find_all_display(), find_detail_by_id(), create(), update(), delete(), etc.
```

#### 2. Handlers to Split (After AppError Refactor)

**`handlers/user_handlers.rs` (~200 lines post-refactor) → `handlers/users/`**
```
handlers/users/
├── mod.rs              (~20 lines) - Public route registration
├── list.rs             (~60 lines) - list handler + PaginationQuery
└── crud.rs             (~120 lines) - new, create, edit, update, delete, show handlers
```

**`handlers/role_handlers.rs` (~200 lines post-refactor) → `handlers/roles/`**
```
handlers/roles/
├── mod.rs              (~20 lines) - Public route registration
├── list.rs             (~60 lines) - list handler
└── crud.rs             (~120 lines) - new, create, edit, update, delete handlers
```

### File Splitting Guidelines

**When to split:**
- File exceeds 300 lines
- Clear logical boundaries (types vs queries, list vs CRUD)
- Multiple distinct concerns (schema, instance, entities)

**When NOT to split:**
- File is under 200 lines after refactoring
- No clear module boundaries
- Would create excessive indirection

**Module structure:**
- `mod.rs` contains only public re-exports
- Submodules are `pub(super)` by default, exposed via `mod.rs`
- Each submodule focuses on one concern

### Impact Summary

**Before:**
- 5 files totaling ~2,000 lines
- Mixed concerns within files
- Difficult to navigate

**After:**
- 15 focused modules averaging ~130 lines each
- Clear separation of concerns
- Easier to find and modify code

---

## Testing Strategy

### Manual Testing (No Automated Tests)

After each phase:
1. `cargo build` - must compile cleanly
2. `cargo clippy` - zero warnings
3. Browser testing:
   - Login
   - Navigate all pages
   - Create/edit/delete users and roles
   - Verify audit logs
   - Check settings
   - Test permission errors (logout, login as non-admin)

### Critical Paths to Test
- User CRUD (list, create, edit, delete)
- Role CRUD (list, create, edit, delete, permissions)
- Permission checks (access denied for unprivileged users)
- Audit trail (entries created for CRUD operations)
- Settings (view, modify)
- Ontology explorer (all three tabs)
- Error pages (404, 403, 500)

---

## Risk Assessment

### Low Risk
- Phase 1 (Foundation) - adds new code, doesn't break existing
- Phase 2 (Quick Wins) - pure cleanup, no logic changes

### Medium Risk
- Phase 3 (POC) - refactors working code, but limited scope (1 file)
- Phase 4A (Handler Expansion) - large changeset but mechanical transformation

### Low-Medium Risk
- Phase 4B (File Splitting) - moves code around but no logic changes

### Mitigation
- Phased approach allows early detection of issues
- POC validates pattern before large-scale application
- Manual testing after each phase
- Git commits after each phase for easy rollback

---

## Success Criteria

### Phase 1
- ✅ AppError enhanced with new variants
- ✅ render() helper created and working
- ✅ require_permission returns Result<(), AppError>
- ✅ PageContext::build returns Result<PageContext, AppError>

### Phase 2
- ✅ Zero Clippy warnings
- ✅ Zero dead code
- ✅ Clean `cargo build`

### Phase 3
- ✅ All 7 handlers in user_handlers.rs refactored
- ✅ ~50% line reduction achieved
- ✅ All functionality working via browser testing
- ✅ Code more readable and idiomatic

### Phase 4A
- ✅ All 24 handlers use AppError + render()
- ✅ Consistent error handling across codebase
- ✅ ~40-50% overall line reduction in handlers

### Phase 4B
- ✅ 5 large files split into 15 focused modules
- ✅ Average module size ~130 lines
- ✅ Clear separation of concerns
- ✅ Zero regressions

---

## Implementation Notes

### Import Changes

After file splitting, imports will change:
```rust
// Before
use crate::models::user;

// After (example for ontology)
use crate::models::ontology::schema;
use crate::models::ontology::entities;

// With re-exports in mod.rs, can still use:
use crate::models::ontology::{EntityTypeSummary, GraphNode};
```

### Backwards Compatibility

Public API maintained via `mod.rs` re-exports:
```rust
// models/user/mod.rs
mod types;
mod queries;

pub use types::*;
pub use queries::*;

// Callers don't need to change
use crate::models::user::{UserDisplay, find_paginated};
```

### Commit Strategy

One commit per phase:
1. Phase 1: "refactor: enhance AppError and add render helper"
2. Phase 2: "fix: clippy warnings and remove dead code"
3. Phase 3: "refactor: user handlers to use AppError pattern (POC)"
4. Phase 4A: "refactor: all handlers to use AppError pattern"
5. Phase 4B: "refactor: split large files into focused modules"

---

## Future Enhancements

(Out of scope for this cleanup)

- Automated testing suite (integration + unit tests)
- Performance profiling and optimization
- Database query refactoring (N+1 queries, indexing)
- Error messages localization
- Structured logging (replace eprintln! with proper logger)
- API documentation (rustdoc)

---

## Decision Rationale

**Why foundation-first over quick-wins-first?**
- Validates AppError design before committing to large refactor
- Ensures POC uses production-ready foundation
- Prevents rework if foundation needs adjustment

**Why POC before full expansion?**
- De-risks large-scale refactoring
- Validates line reduction and readability claims
- Allows course correction if issues discovered

**Why split 5 files but not all long files?**
- Focused on files with clear module boundaries
- Other long files (db.rs 289 lines, user.rs post-refactor ~200 lines) are manageable
- Avoids over-engineering

**Why handlers AND models in file splitting?**
- Both have clear logical separations
- Splitting models improves navigation for ontology complexity
- Splitting handlers creates consistency in project structure

---

## Appendix: File Structure Comparison

### Before
```
src/
├── models/
│   ├── ontology.rs      (471 lines - mixed schema/instance/entities)
│   ├── user.rs          (323 lines - mixed types/queries)
│   └── role.rs          (236 lines - mixed types/queries)
└── handlers/
    ├── user_handlers.rs (384 lines - mixed list/CRUD)
    └── role_handlers.rs (370 lines - mixed list/CRUD)
```

### After
```
src/
├── models/
│   ├── ontology/
│   │   ├── mod.rs       (re-exports)
│   │   ├── schema.rs    (schema types/queries)
│   │   ├── instance.rs  (graph types/queries)
│   │   └── entities.rs  (entity types/queries)
│   ├── user/
│   │   ├── mod.rs       (re-exports)
│   │   ├── types.rs     (UserDisplay, UserForm, etc.)
│   │   └── queries.rs   (DB functions)
│   └── role/
│       ├── mod.rs       (re-exports)
│       ├── types.rs     (RoleDisplay, RoleForm, etc.)
│       └── queries.rs   (DB functions)
└── handlers/
    ├── users/
    │   ├── mod.rs       (route registration)
    │   ├── list.rs      (list handler)
    │   └── crud.rs      (create/edit/update/delete)
    └── roles/
        ├── mod.rs       (route registration)
        ├── list.rs      (list handler)
        └── crud.rs      (create/edit/update/delete)
```
