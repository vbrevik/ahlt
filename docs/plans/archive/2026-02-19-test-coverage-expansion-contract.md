# Prompt Contract: H.4 — Test Coverage Expansion

## GOAL

Expand test coverage from ~52 tests to ~101 tests (49 new tests) by implementing:
1. Shared HTTP test infrastructure in `tests/common/mod.rs` with reusable helpers
2. Five domain-specific test files covering auth, user CRUD, workflow builder, ToR, and minutes

**Success metric**: `cargo test` shows ~101 passing tests across 13 test files, all model-level and HTTP integration tests execute in <5 seconds total, no test depends on external services.

**Test breakdown:**
- Current: 52 tests (8 files: auth, warnings, role builder, phase2a, phase2b, meeting, infrastructure)
- Adding: 49 new tests (5 files: auth_test, user_test, workflow_builder_test, tor_test, minutes_test)
- Result: ~101 tests (13 files)

---

## CONSTRAINTS

### Stack & Patterns
- **Language**: Rust/Actix-web integration tests, no new dependencies
- **Test framework**: Use `#[test]` and `TempDir` (already imported in existing tests)
- **Database**: SQLite with `schema.sql` migrations embedded via `include_str!`
- **Session/Auth**: Actix-session with CSRF tokens; no real HTTP middleware in unit tests
- **HTTP testing**: Use `test::TestServer` or `actix-rt::test` for full app integration

### Code Quality
- **Max 150 lines per test function** (larger tests = unclear what's being tested)
- **DRY**: Reuse helpers from `tests/common/mod.rs` — don't repeat setup code
- **Naming**: Test names follow `test_<domain>_<scenario>` (e.g., `test_user_update_rejects_invalid_email`)
- **Coverage priority**: Auth (permission gates), CRUD (create/read/update/delete), error cases (invalid input, missing resources)
- **No hardcoded values**: Use constants for test user IDs, dates, email addresses
- **Documentation**: Each test file has a module comment explaining what it covers

### What NOT to Do
- ❌ Don't test every permutation of every parameter
- ❌ Don't create new test infrastructure outside `tests/common/mod.rs`
- ❌ Don't duplicate setup code from helpers
- ❌ Don't add external test data fixtures (seed data comes from schema.sql only)
- ❌ Don't test 3rd-party libraries (e.g., actix-web routing details)

---

## FORMAT

### 1. Shared Infrastructure — `tests/common/mod.rs`

**Exports:**
```rust
pub fn setup_test_db() -> (TempDir, Connection)
pub fn setup_test_db_seeded() -> (TempDir, Connection)
pub fn build_test_app(conn: Arc<Mutex<Connection>>) -> TestServer
pub fn login_as_admin(server: &TestServer) -> String  // Returns session cookie
pub fn login_as(server: &TestServer, username: &str, password: &str) -> Option<String>
pub fn get_csrf_token(server: &TestServer, path: &str, cookie: &str) -> String
pub fn extract_json<T>(response: ServiceResponse) -> T where T: DeserializeOwned
pub const ADMIN_USER: &str = "admin"
pub const ADMIN_PASS: &str = "admin123"
pub const TEST_USER_EMAIL: &str = "test@example.com"
```

**Functions:**
- `setup_test_db()` — Create temp DB with schema only (no seed data)
- `setup_test_db_seeded()` — Create temp DB with schema + staging seed data (roles, permissions, ToRs)
- `build_test_app(conn)` — Construct a full Actix-web TestServer with all routes mounted
- `login_as_admin()` — POST /login with admin credentials, return session cookie
- `login_as(username, password)` — Generic login, return session cookie if successful
- `get_csrf_token(server, path, cookie)` — Fetch a page, extract CSRF token from form/hidden input
- `extract_json<T>()` — Parse JSON response body into Rust struct

**Behavior:**
- `setup_test_db_seeded()` calls `ahlt::db::seed_staging()` after schema load
- Helpers should NOT panic; return `Option<T>` for optional operations (login_as returns None on failure)
- No file I/O beyond TempDir; all data in-memory SQLite

### 2. Auth Tests — `tests/auth_test.rs`

**File structure:**
```rust
use tempfile::TempDir;
use ahlt::models::user;
use ahlt::auth::session;
use crate::common::*;

#[test]
fn test_find_user_by_username() { ... }

#[test]
fn test_verify_password_correct() { ... }

#[test]
fn test_verify_password_incorrect() { ... }

#[test]
fn test_login_success() { ... }

#[test]
fn test_login_invalid_credentials() { ... }

#[test]
fn test_login_redirects_to_dashboard() { ... }

#[test]
fn test_unauthorized_redirect_to_login() { ... }

#[test]
fn test_csrf_validation_blocks_invalid_token() { ... }
```

**Scope:**
- Model layer: `user::find_by_username()`, `user::verify_password()`
- HTTP layer: GET /login (form), POST /login (submission), unauthorized access redirect
- Error cases: missing user, wrong password, invalid CSRF token

**Test count:** ~8 tests

### 3. User CRUD Tests — `tests/user_test.rs`

**File structure:**
```rust
#[test]
fn test_user_list_requires_permission() { ... }

#[test]
fn test_user_list_pagination() { ... }

#[test]
fn test_user_list_search_by_email() { ... }

#[test]
fn test_create_user_success() { ... }

#[test]
fn test_create_user_invalid_email() { ... }

#[test]
fn test_create_user_duplicate_username() { ... }

#[test]
fn test_update_user_success() { ... }

#[test]
fn test_update_user_permissions_gate() { ... }

#[test]
fn test_delete_user_success() { ... }

#[test]
fn test_delete_user_last_admin_protection() { ... }

#[test]
fn test_change_password_requires_current() { ... }

#[test]
fn test_change_password_success() { ... }
```

**Scope:**
- HTTP: GET /users, POST /users, GET /users/{id}/edit, POST /users/{id}, POST /users/{id}/delete
- Model: user create, update, delete, password change
- Error cases: validation failures, permission denials, constraint violations
- Edge cases: pagination boundaries, last admin, duplicate username

**Test count:** ~12 tests

### 4. Workflow Builder Tests — `tests/workflow_builder_test.rs`

**File structure:**
```rust
#[test]
fn test_workflow_status_crud() { ... }

#[test]
fn test_workflow_transition_crud() { ... }

#[test]
fn test_find_available_transitions() { ... }

#[test]
fn test_workflow_builder_requires_permission() { ... }

#[test]
fn test_workflow_builder_page_renders() { ... }

// More tests...
```

**Scope:**
- Model: create/list/update/delete workflow_status, workflow_transition entities
- HTTP: GET /workflow/builder (list), GET /workflow/builder/{scope} (detail), POST mutations
- Error cases: invalid scope, permission denials, malformed transitions
- Edge cases: orphaned statuses, circular transitions

**Test count:** ~12 tests

### 5. ToR Tests — `tests/tor_test.rs`

**File structure:**
```rust
#[test]
fn test_tor_create() { ... }

#[test]
fn test_tor_update() { ... }

#[test]
fn test_tor_delete() { ... }

#[test]
fn test_tor_members_add() { ... }

#[test]
fn test_tor_members_remove() { ... }

#[test]
fn test_tor_protocol_steps() { ... }

#[test]
fn test_tor_calendar_computation() { ... }

// More tests...
```

**Scope:**
- Model: ToR CRUD, member management, protocol steps, calendar event generation
- HTTP: GET /tor (list), GET /tor/{id}, POST /tor, POST /tor/{id}/members, etc.
- Error cases: missing permissions, invalid member IDs, malformed calendar data
- Edge cases: cadence computation (weekly, monthly, ad-hoc), vacant positions, blocking relations

**Test count:** ~12 tests

### 6. Minutes Tests — `tests/minutes_test.rs`

**File structure:**
```rust
#[test]
fn test_minutes_scaffold_generation() { ... }

#[test]
fn test_minutes_default_sections() { ... }

#[test]
fn test_minutes_update_status() { ... }

#[test]
fn test_minutes_approve_prevents_edits() { ... }

#[test]
fn test_minutes_section_crud() { ... }

#[test]
fn test_minutes_requires_generate_permission() { ... }

// More tests...
```

**Scope:**
- Model: minutes scaffold creation, section defaults, lifecycle (draft → pending → approved)
- HTTP: GET /minutes/{id}, POST /minutes/{id}/status, POST /minutes/{id}/sections
- Error cases: invalid meeting state, permission denials
- Edge cases: approved minutes immutability, vacant position marking

**Test count:** ~8 tests

---

## FAILURE CONDITIONS

Tests fail (and should be fixed) if any of these are true:

### Critical Failures
- ❌ Any test panics or unwraps (use `?` and `assert!` only)
- ❌ Test depends on external network/service (use in-memory SQLite only)
- ❌ Test modifies shared state that affects other tests (use TempDir isolation)
- ❌ Permission gate test passes without actually checking the permission
- ❌ Duplicate username test succeeds (unique constraint not enforced)

### Test Quality Failures
- ❌ Test function exceeds 150 lines
- ❌ Test doesn't validate the error case (just checks for non-success)
- ❌ Test uses magic numbers instead of named constants
- ❌ Test name doesn't match what it tests
- ❌ Helper function called more than 3 times in a test without extracting to setup

### Infrastructure Failures
- ❌ `build_test_app()` doesn't mount all routes
- ❌ `setup_test_db_seeded()` doesn't load seed data
- ❌ `login_as_admin()` doesn't return a usable session cookie
- ❌ CSRF token extraction fails on forms/hidden inputs
- ❌ Test assumes hardcoded role IDs instead of querying from seeded data

### Coverage Failures
- ❌ Auth tests don't cover both successful and failed login paths
- ❌ CRUD tests only test happy path (missing create/read/update/delete)
- ❌ Permission gate tests don't verify both allowed and denied cases
- ❌ Error validation tests don't verify the actual error message/code
- ❌ Fewer than 8 auth tests, 12 user tests, 12 workflow tests, 12 ToR tests, 8 minutes tests

### Performance Failures
- ❌ Any single test takes >1 second (likely waiting on DB lock or missing timeout)
- ❌ Total test suite takes >5 seconds (parallel execution should be fast)
- ❌ Memory leaks from TempDir not being cleaned up

---

## Acceptance Checklist

**Before marking complete:**
- [ ] `cargo test` shows ~101 passing tests
- [ ] New test files created: auth_test.rs, user_test.rs, workflow_builder_test.rs, tor_test.rs, minutes_test.rs
- [ ] `tests/common/mod.rs` has all required helpers (8 functions)
- [ ] All tests use `tests/common::*` helpers (no duplicate setup code)
- [ ] No test exceeds 150 lines
- [ ] Permission gate tests verify both allowed + denied cases
- [ ] CRUD tests cover C+R+U+D (not just one operation)
- [ ] Error cases tested: invalid input, missing permissions, missing resources
- [ ] Test names follow `test_<domain>_<scenario>` pattern
- [ ] No hardcoded IDs (use constants from common.rs)
- [ ] All tests pass with `cargo test` (no intermittent failures)
- [ ] Commit message follows: `feat(tests): expand coverage to 101 tests with shared HTTP infrastructure`

---

## Reference

**Related files:**
- Design doc: `docs/plans/2026-02-19-test-coverage-expansion-design.md`
- Existing tests: `tests/meeting_test.rs` (model tests), `tests/role_builder_test.rs` (HTTP integration)
- CLAUDE.md: Testing patterns, AppError handling, auth/session helpers
