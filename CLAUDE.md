# Alt - Rust Web Application

Ontology-based entity management system built with Actix-web, Askama templates, and SQLite.

## Quick Start

```bash
# Build and run
cargo run

# Development with auto-reload
cargo watch -x run

# Run tests
cargo test

# Check for compilation errors
cargo check

# Run linter
cargo clippy
```

**Access**: http://localhost:8080
**Default login**: admin / (password set during initial setup)

## Why This File Matters

**Time saved with upfront context**: Having this CLAUDE.md from day one would have prevented these common time sinks:

### 1. Template Debugging (Save: ~2-3 hours per issue)
Without knowing Askama's quirks upfront:
- Spent time debugging `ref` in `if let` patterns before discovering they're not supported
- Hit template scope issues with included partials multiple times
- Discovered `Vec<String>::contains()` limitation the hard way through compilation errors
- Each issue required 30-60 minutes to diagnose and fix

**With CLAUDE.md**: Check gotchas section first, avoid entire debugging cycles.

### 2. Error Pattern Design (Save: ~4-6 hours)
Without documented patterns:
- Designed AppError enum from scratch through trial and error
- Discovered which error variants were needed incrementally
- Had to refactor session helpers multiple times to get error types right
- Learned `?` operator compatibility through compilation failures

**With CLAUDE.md**: See established AppError pattern, copy proven approach immediately.

### 3. Route Registration (Save: ~1-2 hours)
Without knowing route order matters:
- Debugged why `/users/new` was being caught by `/users/{id}`
- Tried multiple workarounds before discovering registration order matters
- Similar issue with other parameterized routes

**With CLAUDE.md**: Register specific routes before parameterized ones from the start.

### 4. Form Handling with Checkboxes (Save: ~3-4 hours)
Without knowing `serde_urlencoded` limitation:
- Spent time figuring out why role permissions checkboxes weren't working
- Tried various `web::Form` struct patterns
- Eventually wrote custom `parse_form_body()` function
- Had to refactor role creation/update handlers

**With CLAUDE.md**: Use custom parser for duplicate keys from the start.

### 5. Database Connection Setup (Save: ~1-2 hours)
Without knowing SQLite pragma requirements:
- Debugged foreign key CASCADE not working (forgot `PRAGMA foreign_keys=ON`)
- Hit concurrency issues before discovering WAL mode
- Discovered pragmas must be set per-connection, not once

**With CLAUDE.md**: Set up connection manager correctly on first try.

### 6. EAV Pattern Understanding (Save: ~2-3 hours)
Without documented architecture:
- Had to reverse-engineer entity/property/relation model from queries
- Misunderstood entity type discriminator initially
- Confused about why entity IDs aren't sequential per type

**With CLAUDE.md**: Understand EAV design immediately, write queries correctly.

### 7. Session Helper Patterns (Save: ~2-3 hours)
Without knowing centralized helpers exist:
- Duplicated `get_user_id()` logic across multiple handlers
- Inconsistent permission check patterns
- Each handler implemented session access differently

**With CLAUDE.md**: Use established session helpers from `auth/session.rs` immediately.

### 8. Template Rendering Approach (Save: ~1-2 hours during refactoring)
Without `render()` helper documented:
- Wrote manual `tmpl.render()` + `HttpResponse::Ok()` in every handler
- Inconsistent error handling for template errors
- Had to refactor all handlers to use helper during cleanup

**With CLAUDE.md**: Use `render()` helper from the start, avoid refactoring work.

**Total time saved**: ~18-28 hours across these common issues
**Additional benefit**: Faster onboarding for new contributors/sessions

## Architecture

### Stack
- **Web framework**: Actix-web 4
- **Templates**: Askama 0.14
- **Database**: SQLite (rusqlite 0.32 + r2d2_sqlite 0.25)
- **Auth**: argon2 0.5, actix-session 0.10
- **Serialization**: serde + serde_json

### Directory Structure

```
src/
├── main.rs           # App configuration, routing, middleware
├── db.rs            # Database pool initialization
├── errors.rs        # AppError enum, ResponseError trait, render() helper
├── auth/            # Authentication & authorization
│   ├── mod.rs       # Password hashing, login
│   ├── session.rs   # Session helpers (get_user_id, require_permission, etc.)
│   └── csrf.rs      # CSRF token validation
├── models/          # Database models & queries
│   ├── ontology.rs  # Entity, property, relation queries (EAV pattern)
│   ├── user.rs      # User CRUD & display types
│   ├── role.rs      # Role & permission management
│   ├── audit.rs     # Audit log queries
│   ├── nav_item.rs  # Navigation menu building
│   └── setting.rs   # Application settings
├── handlers/        # HTTP request handlers (all use AppError pattern)
│   ├── user_handlers.rs
│   ├── role_handlers.rs
│   ├── account_handlers.rs
│   ├── settings_handlers.rs
│   ├── ontology_handlers.rs
│   ├── audit_handlers.rs
│   ├── auth_handlers.rs
│   └── dashboard.rs
├── audit/           # Audit logging subsystem
│   └── mod.rs       # Log CRUD actions, retention cleanup
└── templates_structs.rs  # Template context types

templates/          # Askama HTML templates
static/            # CSS, fonts, client-side JS
data/              # SQLite database file (app.db)
docs/plans/        # Design & implementation documentation
```

### Key Patterns

**1. AppError Pattern** (established in Tasks 1-21 refactoring)

All handlers return `Result<HttpResponse, AppError>`:

```rust
pub async fn handler(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "permission.code")?;
    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/path")?;
    // ... business logic ...
    render(tmpl)
}
```

**AppError variants**:
- `Db(rusqlite::Error)` - Database errors
- `Pool(r2d2::Error)` - Connection pool errors
- `Template(askama::Error)` - Template rendering errors
- `Hash(String)` - Password hashing errors
- `NotFound` - 404 errors
- `PermissionDenied(String)` - 403 errors
- `Session(String)` - Session errors
- `Csrf(String)` - CSRF validation errors

**2. Session Helpers** (src/auth/session.rs)

```rust
require_permission(&session, "code")?;  // Returns AppError::PermissionDenied
let user_id = get_user_id(&session)?;    // Returns AppError::Session
let username = get_username(&session)?;
let permissions = get_permissions(&session)?;
```

**3. Template Rendering**

Use the `render()` helper instead of manual rendering:

```rust
let tmpl = MyTemplate { ctx, data };
render(tmpl)  // Automatically converts askama::Error to AppError
```

**4. EAV Ontology Pattern**

Everything is an entity with properties and relations:

```sql
entities (id, entity_type, name, created_at)
entity_properties (entity_id, key, value)  -- EAV for flexible schema
relations (id, relation_type_id, from_entity_id, to_entity_id)
```

Entity types: `user`, `role`, `permission`, `nav_item`, `nav_module`, `relation_type`

### Database

**Location**: `data/app.db`

**Initialization**: Automatic on first run (creates schema + admin user)

**Pragmas** (set per-connection via r2d2 init):
```sql
PRAGMA foreign_keys = ON;  -- Required for CASCADE deletes
PRAGMA journal_mode = WAL; -- Write-Ahead Logging for concurrency
```

**Important constraints**:
- Foreign keys CASCADE delete properties and relations when entity deleted
- UNIQUE constraints on usernames, role names
- Autoincrement IDs shared across all entity types (non-sequential per type)

## Gotchas & Quirks

### Askama 0.14

❌ **No `ref` in `if let`**:
```rust
// Wrong
{% if let Some(ref x) = val %}

// Correct
{% if let Some(x) = val %}
```

❌ **Included templates share parent scope**: Every template struct must carry fields used by included partials (e.g. `username`, `permissions` for nav)

❌ **String equality in loops**: Use `.as_str()` on both sides
```rust
{% if field.as_str() == item.as_str() %}
```

❌ **Can't call `Vec<String>::contains()` with `&str`**: Create wrapper types with template-friendly methods (e.g. `Permissions::has(&str)`)

### Actix-web 4

⚠️ **Route order matters**: `/users/new` must be registered BEFORE `/users/{id}` or path param swallows "new"

⚠️ **Session cookie key**: `Key::generate()` invalidates all sessions on restart - load from env in production

⚠️ **`serde_urlencoded` doesn't support duplicate keys**: HTML checkboxes with same `name` fail with `web::Form`. Use custom `parse_form_body()` for repeated fields (see role permissions checkboxes)

⚠️ **Middleware `from_fn` needs `'static`**: `Next<impl MessageBody + 'static>`, not without lifetime

### SQLite + r2d2

⚠️ **Create parent dirs first**: `fs::create_dir_all("data")` before pool init

⚠️ **WAL pragma is per-connection**: Set via `SqliteConnectionManager::file(path).with_init(...)`

⚠️ **`COALESCE(col, '')` required in LEFT JOINs**: rusqlite `row.get()` fails on NULL for non-Option types

⚠️ **Dynamic SQL table aliases must be consistent**: Search clause `e.name LIKE ?1` requires count query to use `FROM entities e` not just `FROM entities`

## Development Workflow

### Refactoring Workflow

**For large-scale changes** (e.g., migrating error patterns):

1. **Phase 1**: Build infrastructure (AppError enum, helpers) - expect breaking changes
2. **Phase 2**: Quick wins (clippy fixes, dead code removal)
3. **Phase 3**: Migrate incrementally (file by file, handler by handler)
   - Implement changes
   - Spec compliance review (did it meet requirements?)
   - Code quality review (validation, audit logging, edge cases)
   - Fix issues, re-review
4. **Phase 4**: File splitting and polish

**Avoid**: Trying to refactor everything at once. Incremental with reviews prevents issues.

### Code Review Checklist

When adding/modifying handlers, verify:

**CRUD Consistency:**
- ✅ Create and Update handlers have **matching validation** (name, email, etc.)
- ✅ Create, Update, Delete handlers have **audit logging** (if meaningful data change)
- ✅ Update handler validates required fields (don't trust form data)
- ✅ Delete handler captures entity details **before** deletion for audit log

**Error Handling:**
- ✅ All database queries use `?` operator (no `.unwrap_or_default()` on critical data)
- ✅ Template rendering uses `render()` helper
- ✅ Permission checks happen first (before any business logic)
- ✅ CSRF validation on all mutations (POST/PUT/DELETE)

### Adding a New Handler

1. Create handler function in appropriate file (e.g. `src/handlers/user_handlers.rs`)
2. Use `Result<HttpResponse, AppError>` return type
3. Check permissions first: `require_permission(&session, "code")?`
4. Validate CSRF for mutations: `csrf::validate_csrf(&session, &token)?`
5. Get DB connection: `let conn = pool.get()?`
6. Build page context: `let ctx = PageContext::build(&session, &conn, "/path")?`
7. Business logic with `?` for error propagation
8. Return `render(tmpl)` for HTML or `Ok(HttpResponse::SeeOther()...` for redirects

### Adding Audit Logging

```rust
let current_user_id = get_user_id(&session).unwrap_or(0);
let details = serde_json::json!({
    "field": value,
    "summary": "Short description"
});
let _ = audit::log(&conn, current_user_id, "action.name", "target_type", target_id, details);
```

### Database Migrations

During rapid development: delete `data/app.db` and re-seed. For production, write migration scripts in `src/db.rs`.

## Testing

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture
```

## Recent Refactoring (Tasks 1-21)

**Completed**: All handlers migrated to AppError pattern
- ✅ User handlers (6)
- ✅ Role handlers (6)
- ✅ Audit handlers (1)
- ✅ Account handlers (2)
- ✅ Settings handlers (2)
- ✅ Ontology handlers (6)
- ✅ Auth handlers (3)
- ✅ Dashboard (1)

**Impact**: ~280 lines of boilerplate eliminated, consistent error handling with `?` operator

See `docs/plans/` for detailed refactoring documentation.

## Troubleshooting

**Build errors after git pull**: Run `cargo clean && cargo build`

**Database locked errors**: Check for zombie connections in debugger. WAL mode helps but doesn't eliminate all locking.

**Template not found**: Askama requires templates at compile time. Run `cargo clean` after adding new templates.

**Session cookie issues**: Check browser dev tools → Application → Cookies. Clear cookies if testing login flow changes.

## Verification Commands

```bash
# Check build status concisely
cargo check 2>&1 | tail -10

# Find specific compilation errors
cargo build 2>&1 | grep -E "file_name|pattern"

# Verify zero errors (should output "Finished")
cargo build 2>&1 | tail -1

# Recent commit history
git log --oneline -20
```
