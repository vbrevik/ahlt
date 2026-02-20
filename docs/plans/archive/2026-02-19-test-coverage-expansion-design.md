# H.4 Test Coverage Expansion — Design

**Date:** 2026-02-19
**Status:** Approved
**Priority:** Medium | Effort: Large

## Context

Current coverage: 49 tests across 6 files. All real tests are model-layer (direct `rusqlite::Connection` calls). Zero HTTP-layer tests exist. Large handler modules (user CRUD 362 lines, workflow builder 262 lines, CoA 423 lines, proposal 450 lines) have no safety net.

Scope explicitly covered by existing tests:
- Suggestion → proposal workflow (phase2a)
- Agenda / COA / opinion pipeline (phase2b)
- Warning lifecycle (warnings_test)
- Role builder model + role model (role_builder_test, role_builder_model_test)

Out of scope for this task (already covered): suggestion→proposal, agenda/COA/opinion, warning lifecycle.

## Approach

**Risk-first, file-per-domain.** Each test file covers one domain completely — both model-layer and HTTP-layer tests in the same file. Processed in risk order: auth and user CRUD first (highest blast radius), workflow builder second (new code, no tests), ToR and minutes last.

## Architecture

### Two Test Layers

**Model-layer tests** (existing pattern):
```rust
fn setup_test_db() -> (TempDir, rusqlite::Connection) { ... }
#[test] fn test_create_user() { let (_, conn) = setup_test_db(); ... }
```
Direct model function calls. No HTTP stack. Millisecond execution.

**HTTP-layer tests** (new pattern via `actix_web::test`):
```rust
let app = test::init_service(build_test_app(pool)).await;
let req = test::TestRequest::get().uri("/users").to_request();
let resp = test::call_service(&app, req).await;
assert_eq!(resp.status(), StatusCode::FOUND); // → /login
```

Three HTTP test sub-patterns:
1. **Unauthenticated gate** — no cookie, expect 302 to `/login`
2. **Login flow** — POST `/login` with form body, assert redirect + session cookie
3. **Happy-path CRUD** — login → extract CSRF → POST mutation → assert DB state or redirect

### Shared Infrastructure (`tests/common/mod.rs`)

Created before any domain test files. Provides:

| Helper | Purpose |
|---|---|
| `setup_test_db_seeded()` | Temp DB with schema + seed: all relation types, admin user, admin role, all permissions |
| `build_test_app(pool)` | `actix_web::App` with session middleware + all route registrations |
| `login_as_admin(app)` | POSTs to `/login`, returns session cookie string |
| `get_csrf_token(app, cookie, path)` | GETs a form page, extracts hidden CSRF input value |

The seeded DB uses hardcoded test data (no JSON fixtures) for isolation and speed.

## Test Files

### H.4.1 — `tests/common/mod.rs`
Shared infrastructure only. No `#[test]` functions. All HTTP helpers live here.

**Effort:** Medium (one-time cost for all subsequent files)

### H.4.2 — `tests/auth_test.rs`
**Risk:** High — auth is the security boundary for everything.

Model tests:
- `find_by_username` returns `None` for unknown user
- `find_by_username` returns `Some(user)` for known user
- Password hash verify succeeds with correct password
- Password hash verify fails with wrong password

HTTP tests:
- `GET /dashboard` (unauthed) → 302 to `/login`
- `GET /users` (unauthed) → 302 to `/login`
- `POST /login` with valid credentials → 302 to `/dashboard`
- `POST /login` with invalid credentials → 200 with error message

**Target:** ~8 tests

### H.4.3 — `tests/user_test.rs`
**Risk:** High — user CRUD is 362 lines with the most complex form handling in the codebase.

Model tests:
- Create user → verify in DB
- Find paginated (page 1, page 2)
- Search filter reduces results
- Update user (username, email, role)
- Delete user removes entity + properties
- `count_by_role_id` returns correct count
- `update_password` + verify new hash works

HTTP tests:
- `GET /users` (unauthed) → 302 to `/login`
- `GET /users` (authed, no `users.list` permission) → 302 to `/login`
- `POST /users/new` (authed, with CSRF) → creates user, redirects
- `POST /users/{id}/delete` (authed, CSRF) → deletes user, redirects

**Target:** ~12 tests

### H.4.4 — `tests/workflow_builder_test.rs`
**Risk:** High — F.1 shipped with zero tests. New code, entirely untested.

Model tests:
- `list_workflow_scopes` returns seeded scopes
- `list_statuses_for_scope` returns correct statuses
- `list_transitions_for_scope` returns correct transitions
- `create_status` → verify via list
- `update_status` → verify change
- `delete_status` → verify removed
- `create_transition` → verify via list
- `update_transition` → verify change
- `delete_transition` → verify removed
- Transition validates `from_status` belongs to same scope

HTTP tests:
- `GET /workflow/builder` (unauthed) → 302 to `/login`
- `GET /workflow/builder/{scope}` (unauthed) → 302 to `/login`

**Target:** ~12 tests

### H.4.5 — `tests/tor_test.rs`
**Risk:** Medium — large domain, complex calendar logic is pure computation.

Model tests:
- Create ToR → find by ID → verify properties
- List ToRs returns all active
- Add member (fills_position) → verify relation
- Remove member → verify relation removed
- `mandatory` position with no fill appears in vacancy query
- Protocol step create + ordering
- Calendar: weekly cadence generates correct dates in range
- Calendar: monthly cadence generates correct date in range
- Calendar: ad-hoc cadence generates zero events
- Dependency create (`feeds_into`) → verify relation
- Dependency with `is_blocking=true` → verify property

HTTP tests:
- `GET /tor` (unauthed) → 302 to `/login`

**Target:** ~12 tests

### H.4.6 — `tests/minutes_test.rs`
**Risk:** Medium — scaffold generation is complex business logic, approved minutes are immutable.

Model tests:
- Scaffold creates exactly 5 sections
- Scaffold section names match expected (Attendance, Agenda, etc.)
- Status transition: draft → pending_approval
- Status transition: pending_approval → approved
- Approved minutes cannot be edited (returns error)
- Add section to draft minutes → verify
- Reorder sections → verify new order

HTTP tests:
- `GET /tor/{id}/minutes` (unauthed) → 302 to `/login`

**Target:** ~8 tests

## Summary

| File | New Tests | Priority |
|---|---|---|
| `tests/common/mod.rs` | 0 (infra) | 1st |
| `tests/auth_test.rs` | ~8 | 2nd |
| `tests/user_test.rs` | ~12 | 3rd |
| `tests/workflow_builder_test.rs` | ~12 | 4th |
| `tests/tor_test.rs` | ~12 | 5th |
| `tests/minutes_test.rs` | ~8 | 6th |
| **Total new** | **~52** | |
| **Grand total** | **~101** | |

## Backlog Sub-tasks

Each file is tracked as a backlog sub-task H.4.1–H.4.6 in BACKLOG.md.

## Prompt Contracts

Each sub-task (H.4.2–H.4.6, with H.4.1 as prerequisite) gets its own `/prompt-contracts` invocation at implementation time. This ensures each test file has a clear scope contract before code is written.
