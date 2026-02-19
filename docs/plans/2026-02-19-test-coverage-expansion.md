# H.4 Test Coverage Expansion — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Expand test coverage from 49 to ~101 tests across 6 new files using a risk-first, file-per-domain approach that adds both model-layer and HTTP-layer tests.

**Architecture:** Six test files in risk order (auth → user → workflow builder → ToR → minutes). A shared `tests/common/mod.rs` provides the `actix_web::test` infrastructure (seeded pool, app builder, login helper) used by all HTTP tests. Model-layer tests follow the existing `setup_test_db()` + direct model call pattern already established in this codebase.

**Tech Stack:** `rusqlite` + `tempfile` (model tests), `actix_web::test` + `actix_session` + `r2d2_sqlite` (HTTP tests), `ahlt::db::seed_ontology` for seeded test databases. All dependencies already in `Cargo.toml`.

---

## Quick Reference

- **Model test pattern:** `setup_test_db()` → direct `ahlt::models::*` calls → assert
- **HTTP unauth gate pattern:** `TestRequest::get().uri(route)` → assert `302` to `/login`
- **HTTP login flow:** `GET /login` → extract CSRF + session cookie → `POST /login` → assert `302 /dashboard`
- **HTTP authed request:** `GET /login` + `POST /login` → get session cookie → use in next request
- **Seeded DB credentials:** `admin` / `test_password_h4` (set during `setup_seeded_pool()`)
- **Key import paths:** `ahlt::db`, `ahlt::auth`, `ahlt::models`, `ahlt::handlers`
- **Run tests:** `cargo test --test <filename>` (e.g. `cargo test --test auth_test`)

---

## Task 1: Shared HTTP Infrastructure (`tests/common/mod.rs`)

> **Before starting:** Invoke `/prompt-contracts` with: "Create `tests/common/mod.rs` providing shared HTTP test infrastructure for an Actix-web 4 + actix-session 0.10 + rusqlite/r2d2 application."

**Files:**
- Create: `tests/common/mod.rs`

**Step 1: Create the file with imports**

```rust
// tests/common/mod.rs
// Shared test infrastructure for HTTP-layer tests.
// Model-layer tests use setup_test_db() directly in their own files.
#![allow(dead_code)]

use actix_session::{SessionMiddleware, storage::CookieSessionStore};
use actix_web::{cookie::Key, test, web, App};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use tempfile::TempDir;

use ahlt::{auth, db, handlers};

pub type DbPool = Pool<SqliteConnectionManager>;
```

**Step 2: Add `setup_seeded_pool()`**

This creates a temp SQLite DB with the full ontology seed — admin user, admin role, all permissions, all relation types. Uses `ahlt::db::seed_ontology` (same as production startup).

```rust
pub fn setup_seeded_pool() -> (TempDir, DbPool) {
    let dir = TempDir::new().expect("tempdir");
    let db_path = dir.path().join("test.db").to_str().unwrap().to_string();
    let pool = db::init_pool(&db_path);
    db::run_migrations(&pool);
    let hash = auth::password::hash_password("test_password_h4")
        .expect("hash");
    db::seed_ontology(&pool, &hash);
    (dir, pool)
}
```

**Step 3: Add `build_test_app(pool)`**

Registers only the routes used by H.4 tests. Avoids registering WebSocket, static files, and background scheduler (none needed in tests). Uses a fixed session key (64 zero bytes) so sessions survive between requests in the same test.

```rust
pub fn build_test_app(
    pool: DbPool,
) -> impl actix_web::dev::ServiceFactory<
    actix_web::dev::ServiceRequest,
    Config = (),
    Response = actix_web::dev::ServiceResponse,
    Error = actix_web::Error,
    InitError = (),
> {
    let session_key = Key::from(&[0u8; 64]);
    let limiter = auth::rate_limit::RateLimiter::new();
    let conn_map = handlers::warning_handlers::ws::new_connection_map();

    App::new()
        .wrap(
            SessionMiddleware::builder(CookieSessionStore::default(), session_key)
                .cookie_secure(false)
                .cookie_http_only(true)
                .build(),
        )
        .app_data(web::Data::new(pool.clone()))
        .app_data(web::Data::new(limiter))
        .app_data(web::Data::new(conn_map))
        // Public routes
        .route("/login", web::get().to(handlers::auth_handlers::login_page))
        .route("/login", web::post().to(handlers::auth_handlers::login_submit))
        // Protected scope
        .service(
            web::scope("")
                .wrap(actix_web::middleware::from_fn(
                    auth::middleware::require_auth,
                ))
                .route("/dashboard", web::get().to(handlers::dashboard::index))
                .route("/users", web::get().to(handlers::user_handlers::list))
                .route("/users/new", web::get().to(handlers::user_handlers::new_form))
                .route("/users", web::post().to(handlers::user_handlers::create))
                .route("/users/{id}/edit", web::get().to(handlers::user_handlers::edit_form))
                .route("/users/{id}", web::post().to(handlers::user_handlers::update))
                .route("/users/{id}/delete", web::post().to(handlers::user_handlers::delete))
                .route("/workflow/builder", web::get().to(handlers::workflow_builder_handlers::list))
                .route("/workflow/builder/{scope}", web::get().to(handlers::workflow_builder_handlers::detail))
                .route("/workflow/builder/{scope}/statuses", web::post().to(handlers::workflow_builder_handlers::create_status))
                .route("/workflow/builder/{scope}/statuses/{id}/delete", web::post().to(handlers::workflow_builder_handlers::delete_status))
                .route("/tor", web::get().to(handlers::tor_handlers::list))
                .route("/tor/{id}/minutes", web::get().to(handlers::minutes_handlers::detail))
        )
}
```

**Step 4: Add `extract_csrf(html: &str) -> String`**

Parses the CSRF token value from a rendered login page. The login template renders `<input type="hidden" name="csrf_token" value="...">`.

```rust
pub fn extract_csrf(html: &str) -> String {
    // Find: name="csrf_token" value="<token>"
    let marker = r#"name="csrf_token" value=""#;
    let start = html.find(marker).expect("csrf_token input not found") + marker.len();
    let end = html[start..].find('"').expect("closing quote") + start;
    html[start..end].to_string()
}
```

**Step 5: Add `login_as_admin(app) -> (String, String)`**

Returns `(session_cookie, csrf_token_for_future_forms)` by doing a GET /login followed by POST /login with the seeded admin credentials.

```rust
pub async fn login_as_admin<S, B>(app: &S) -> String
where
    S: actix_web::dev::Service<
        actix_web::dev::ServiceRequest,
        Response = actix_web::dev::ServiceResponse<B>,
        Error = actix_web::Error,
    >,
    B: actix_web::body::MessageBody,
{
    // GET /login to establish session and get CSRF token
    let get_req = test::TestRequest::get().uri("/login").to_request();
    let get_resp = test::call_service(app, get_req).await;

    // Extract session cookie
    let cookies: Vec<_> = get_resp.response().cookies().collect();
    let session_cookie = cookies
        .iter()
        .find(|c| c.name() == "id")
        .expect("session cookie")
        .clone();
    let cookie_str = format!("{}={}", session_cookie.name(), session_cookie.value());

    // Extract CSRF from HTML body
    let body = test::read_body(get_resp).await;
    let html = std::str::from_utf8(&body).unwrap();
    let csrf = extract_csrf(html);

    // POST /login
    let post_req = test::TestRequest::post()
        .uri("/login")
        .insert_header(("cookie", cookie_str.clone()))
        .set_form(&[
            ("username", "admin"),
            ("password", "test_password_h4"),
            ("csrf_token", csrf.as_str()),
        ])
        .to_request();
    let post_resp = test::call_service(app, post_req).await;
    assert_eq!(
        post_resp.status(),
        actix_web::http::StatusCode::SEE_OTHER,
        "Login failed"
    );

    cookie_str
}
```

**Step 6: Verify the module compiles**

```bash
cargo check 2>&1 | tail -5
```
Expected: no errors (the module is included by test files via `mod common;`).

**Step 7: Commit**

```bash
git add tests/common/mod.rs
git commit -m "test(h4.1): add shared HTTP test infrastructure (common/mod.rs)"
```

---

## Task 2: Auth Tests (`tests/auth_test.rs`)

> **Before starting:** Invoke `/prompt-contracts` with: "Write integration tests for the auth system: model-layer (find_by_username, password verify) and HTTP-layer (login flow, bad credentials, unauthenticated redirect)."

**Files:**
- Create: `tests/auth_test.rs`

**Step 1: Write model tests (failing first)**

```rust
mod common;

use ahlt::auth::password;
use ahlt::models::user;

const MIGRATIONS: &str = include_str!("../src/schema.sql");

fn setup_test_db() -> (tempfile::TempDir, rusqlite::Connection) {
    let dir = tempfile::TempDir::new().unwrap();
    let conn = rusqlite::Connection::open(dir.path().join("test.db")).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;").unwrap();
    conn.execute_batch(MIGRATIONS).unwrap();
    (dir, conn)
}

#[test]
fn test_find_by_username_returns_none_for_unknown() {
    let (_dir, conn) = setup_test_db();
    let result = user::find_by_username(&conn, "nobody").unwrap();
    assert!(result.is_none());
}

#[test]
fn test_find_by_username_returns_some_for_known() {
    let (_dir, conn) = setup_test_db();
    // Manually create a user entity
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('user', 'alice', 'Alice')",
        [],
    ).unwrap();
    let id = conn.last_insert_rowid();
    let hash = password::hash_password("secret123").unwrap();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'password', ?2)",
        rusqlite::params![id, hash],
    ).unwrap();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'email', 'alice@test.com')",
        rusqlite::params![id],
    ).unwrap();
    // role_id property: create a dummy role first
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', 'user_role', 'User')",
        [],
    ).unwrap();
    let role_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'role_id', ?2)",
        rusqlite::params![id, role_id.to_string()],
    ).unwrap();

    let result = user::find_by_username(&conn, "alice").unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().username, "alice");
}

#[test]
fn test_password_verify_succeeds_with_correct_password() {
    let hash = password::hash_password("correct_horse").unwrap();
    let ok = password::verify_password("correct_horse", &hash).unwrap();
    assert!(ok);
}

#[test]
fn test_password_verify_fails_with_wrong_password() {
    let hash = password::hash_password("correct_horse").unwrap();
    let ok = password::verify_password("wrong_password", &hash).unwrap();
    assert!(!ok);
}
```

**Step 2: Run model tests to confirm they fail**

```bash
cargo test --test auth_test 2>&1 | tail -20
```
Expected: compile error or test failures (file doesn't exist yet).

**Step 3: Add HTTP tests**

```rust
use actix_web::{http::StatusCode, test};

#[actix_web::test]
async fn test_unauthed_dashboard_redirects_to_login() {
    let (_dir, pool) = common::setup_seeded_pool();
    let app = test::init_service(common::build_test_app(pool)).await;

    let req = test::TestRequest::get().uri("/dashboard").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::FOUND);
    let location = resp.headers().get("Location").unwrap().to_str().unwrap();
    assert!(location.contains("/login"), "Expected redirect to /login, got: {}", location);
}

#[actix_web::test]
async fn test_unauthed_users_redirects_to_login() {
    let (_dir, pool) = common::setup_seeded_pool();
    let app = test::init_service(common::build_test_app(pool)).await;

    let req = test::TestRequest::get().uri("/users").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::FOUND);
    let location = resp.headers().get("Location").unwrap().to_str().unwrap();
    assert!(location.contains("/login"));
}

#[actix_web::test]
async fn test_login_with_valid_credentials_redirects_to_dashboard() {
    let (_dir, pool) = common::setup_seeded_pool();
    let app = test::init_service(common::build_test_app(pool)).await;

    let cookie = common::login_as_admin(&app).await;
    // If login_as_admin didn't panic, login succeeded (it asserts 302 internally)
    assert!(!cookie.is_empty());
}

#[actix_web::test]
async fn test_login_with_invalid_password_returns_200_with_error() {
    let (_dir, pool) = common::setup_seeded_pool();
    let app = test::init_service(common::build_test_app(pool)).await;

    // GET /login for CSRF + session
    let get_req = test::TestRequest::get().uri("/login").to_request();
    let get_resp = test::call_service(&app, get_req).await;
    let cookies: Vec<_> = get_resp.response().cookies().collect();
    let cookie_str = format!("{}={}", cookies[0].name(), cookies[0].value());
    let body = test::read_body(get_resp).await;
    let html = std::str::from_utf8(&body).unwrap();
    let csrf = common::extract_csrf(html);

    // POST with wrong password
    let post_req = test::TestRequest::post()
        .uri("/login")
        .insert_header(("cookie", cookie_str))
        .set_form(&[
            ("username", "admin"),
            ("password", "wrong_password"),
            ("csrf_token", csrf.as_str()),
        ])
        .to_request();
    let post_resp = test::call_service(&app, post_req).await;

    assert_eq!(post_resp.status(), StatusCode::OK, "Bad creds should render 200 with error");
    let body = test::read_body(post_resp).await;
    let html = std::str::from_utf8(&body).unwrap();
    assert!(html.contains("Invalid username or password"), "Expected error message in response");
}
```

**Step 4: Run all auth tests**

```bash
cargo test --test auth_test -- --nocapture 2>&1 | tail -20
```
Expected: all pass. Fix any failures before proceeding.

**Step 5: Commit**

```bash
git add tests/auth_test.rs
git commit -m "test(h4.2): add auth model and HTTP tests (8 tests)"
```

---

## Task 3: User CRUD Tests (`tests/user_test.rs`)

> **Before starting:** Invoke `/prompt-contracts` with: "Write integration tests for the user CRUD system: model-layer (create, paginate, search, update, delete, password change) and HTTP-layer (permission gates, create flow with CSRF)."

**Files:**
- Create: `tests/user_test.rs`

**Step 1: Write model tests**

Key functions to test: `user::create`, `user::find_paginated`, `user::find_display_by_id`, `user::update`, `user::delete`, `user::count_by_role_id`, `user::update_password`.

Seed minimum: need a `has_role` relation type entity. Create it with raw SQL before each test that needs it.

```rust
mod common;

use ahlt::models::user;
use ahlt::models::user::NewUser;

const MIGRATIONS: &str = include_str!("../src/schema.sql");

fn setup_test_db() -> (tempfile::TempDir, rusqlite::Connection) {
    let dir = tempfile::TempDir::new().unwrap();
    let conn = rusqlite::Connection::open(dir.path().join("test.db")).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;").unwrap();
    conn.execute_batch(MIGRATIONS).unwrap();
    (dir, conn)
}

fn seed_role(conn: &rusqlite::Connection) -> i64 {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', 'admin', 'Admin')",
        [],
    ).unwrap();
    conn.last_insert_rowid()
}

#[test]
fn test_create_user_inserts_entity_and_properties() {
    let (_dir, conn) = setup_test_db();
    let role_id = seed_role(&conn);

    let hash = ahlt::auth::password::hash_password("pass123").unwrap();
    let new_user = NewUser {
        username: "bob".to_string(),
        email: "bob@example.com".to_string(),
        password: hash.clone(),
        role_id,
    };
    let user_id = user::create(&conn, &new_user).unwrap();
    assert!(user_id > 0);

    let found = user::find_display_by_id(&conn, user_id).unwrap();
    assert!(found.is_some());
    let u = found.unwrap();
    assert_eq!(u.username, "bob");
    assert_eq!(u.email, "bob@example.com");
}

#[test]
fn test_find_paginated_returns_all_users() {
    let (_dir, conn) = setup_test_db();
    let role_id = seed_role(&conn);

    for i in 0..3 {
        let hash = ahlt::auth::password::hash_password("pass").unwrap();
        user::create(&conn, &NewUser {
            username: format!("user{}", i),
            email: format!("u{}@test.com", i),
            password: hash,
            role_id,
        }).unwrap();
    }

    let page = user::find_paginated(&conn, 1, 10, None).unwrap();
    assert_eq!(page.users.len(), 3);
    assert_eq!(page.total, 3);
}

#[test]
fn test_search_filter_reduces_results() {
    let (_dir, conn) = setup_test_db();
    let role_id = seed_role(&conn);

    for name in ["alice", "bob", "alicia"] {
        let hash = ahlt::auth::password::hash_password("pass").unwrap();
        user::create(&conn, &NewUser {
            username: name.to_string(),
            email: format!("{}@test.com", name),
            password: hash,
            role_id,
        }).unwrap();
    }

    let page = user::find_paginated(&conn, 1, 10, Some("ali")).unwrap();
    assert_eq!(page.users.len(), 2, "Expected alice and alicia");
}

#[test]
fn test_delete_user_removes_entity() {
    let (_dir, conn) = setup_test_db();
    let role_id = seed_role(&conn);
    let hash = ahlt::auth::password::hash_password("pass").unwrap();
    let id = user::create(&conn, &NewUser {
        username: "todelete".to_string(),
        email: "del@test.com".to_string(),
        password: hash,
        role_id,
    }).unwrap();

    user::delete(&conn, id).unwrap();

    let found = user::find_display_by_id(&conn, id).unwrap();
    assert!(found.is_none());
}

#[test]
fn test_count_by_role_id_returns_correct_count() {
    let (_dir, conn) = setup_test_db();
    let role_id = seed_role(&conn);

    for i in 0..3 {
        let hash = ahlt::auth::password::hash_password("pass").unwrap();
        user::create(&conn, &NewUser {
            username: format!("u{}", i),
            email: format!("u{}@test.com", i),
            password: hash,
            role_id,
        }).unwrap();
    }

    let count = user::count_by_role_id(&conn, role_id).unwrap();
    assert_eq!(count, 3);
}

#[test]
fn test_update_password_changes_hash() {
    let (_dir, conn) = setup_test_db();
    let role_id = seed_role(&conn);
    let hash = ahlt::auth::password::hash_password("original").unwrap();
    let id = user::create(&conn, &NewUser {
        username: "pwuser".to_string(),
        email: "pw@test.com".to_string(),
        password: hash,
        role_id,
    }).unwrap();

    let new_hash = ahlt::auth::password::hash_password("newpass").unwrap();
    user::update_password(&conn, id, &new_hash).unwrap();

    let stored = user::find_password_hash_by_id(&conn, id).unwrap().unwrap();
    let ok = ahlt::auth::password::verify_password("newpass", &stored).unwrap();
    assert!(ok);
}
```

**Step 2: Write HTTP tests**

```rust
use actix_web::{http::StatusCode, test};

#[actix_web::test]
async fn test_get_users_unauthed_redirects_to_login() {
    let (_dir, pool) = common::setup_seeded_pool();
    let app = test::init_service(common::build_test_app(pool)).await;
    let req = test::TestRequest::get().uri("/users").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FOUND);
}

#[actix_web::test]
async fn test_get_users_authed_returns_200() {
    let (_dir, pool) = common::setup_seeded_pool();
    let app = test::init_service(common::build_test_app(pool)).await;
    let cookie = common::login_as_admin(&app).await;

    let req = test::TestRequest::get()
        .uri("/users")
        .insert_header(("cookie", cookie))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}
```

**Step 3: Run all user tests**

```bash
cargo test --test user_test -- --nocapture 2>&1 | tail -20
```

**Step 4: Commit**

```bash
git add tests/user_test.rs
git commit -m "test(h4.3): add user CRUD model and HTTP tests (8+ tests)"
```

---

## Task 4: Workflow Builder Tests (`tests/workflow_builder_test.rs`)

> **Before starting:** Invoke `/prompt-contracts` with: "Write integration tests for the workflow builder: model-layer (list scopes, status CRUD, transition CRUD, validation) and HTTP-layer (auth gates for list and detail routes)."

**Files:**
- Create: `tests/workflow_builder_test.rs`

**Step 1: Write model tests**

Key functions: `workflow::list_workflow_scopes`, `workflow::list_statuses_for_scope`, `workflow::list_transitions_for_scope`, `workflow::create_status`, `workflow::update_status`, `workflow::delete_status`, `workflow::create_transition`, `workflow::update_transition`, `workflow::delete_transition`.

Important: these functions use `AppError` not `rusqlite::Error`. They also query `entity_type='workflow_status'` and need the `transition_from`/`transition_to` relation types to exist.

```rust
mod common;

use ahlt::models::workflow;

const MIGRATIONS: &str = include_str!("../src/schema.sql");

fn setup_test_db() -> (tempfile::TempDir, rusqlite::Connection) {
    let dir = tempfile::TempDir::new().unwrap();
    let conn = rusqlite::Connection::open(dir.path().join("test.db")).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;").unwrap();
    conn.execute_batch(MIGRATIONS).unwrap();
    (dir, conn)
}

/// Seed the relation types needed by workflow queries.
fn seed_workflow_relation_types(conn: &rusqlite::Connection) {
    for name in ["transition_from", "transition_to"] {
        conn.execute(
            "INSERT INTO entities (entity_type, name, label) VALUES ('relation_type', ?1, ?1)",
            [name],
        ).unwrap();
    }
}

#[test]
fn test_create_status_and_list_for_scope() {
    let (_dir, conn) = setup_test_db();
    seed_workflow_relation_types(&conn);

    workflow::create_status(&conn, "suggestion", "draft", "Draft", true, false).unwrap();
    workflow::create_status(&conn, "suggestion", "submitted", "Submitted", false, false).unwrap();

    let statuses = workflow::list_statuses_for_scope(&conn, "suggestion").unwrap();
    assert_eq!(statuses.len(), 2);
    assert!(statuses.iter().any(|s| s.status_code == "draft" && s.is_initial));
    assert!(statuses.iter().any(|s| s.status_code == "submitted" && !s.is_initial));
}

#[test]
fn test_update_status_changes_label() {
    let (_dir, conn) = setup_test_db();
    seed_workflow_relation_types(&conn);

    let id = workflow::create_status(&conn, "proposal", "draft", "Draft", true, false).unwrap();
    workflow::update_status(&conn, id, "proposal", "draft", "Working Draft", true, false).unwrap();

    let statuses = workflow::list_statuses_for_scope(&conn, "proposal").unwrap();
    assert_eq!(statuses[0].label, "Working Draft");
}

#[test]
fn test_delete_status_removes_it() {
    let (_dir, conn) = setup_test_db();
    seed_workflow_relation_types(&conn);

    let id = workflow::create_status(&conn, "test", "s1", "S1", true, false).unwrap();
    workflow::delete_status(&conn, id).unwrap();

    let statuses = workflow::list_statuses_for_scope(&conn, "test").unwrap();
    assert!(statuses.is_empty());
}

#[test]
fn test_create_transition_and_list_for_scope() {
    let (_dir, conn) = setup_test_db();
    seed_workflow_relation_types(&conn);

    workflow::create_status(&conn, "proposal", "draft", "Draft", true, false).unwrap();
    workflow::create_status(&conn, "proposal", "submitted", "Submitted", false, true).unwrap();

    workflow::create_transition(
        &conn, "proposal", "draft", "submitted", "proposal.submit", "Submit",
    ).unwrap();

    let transitions = workflow::list_transitions_for_scope(&conn, "proposal").unwrap();
    assert_eq!(transitions.len(), 1);
    assert_eq!(transitions[0].from_status_code, "draft");
    assert_eq!(transitions[0].to_status_code, "submitted");
    assert_eq!(transitions[0].required_permission, "proposal.submit");
}

#[test]
fn test_delete_transition_removes_it() {
    let (_dir, conn) = setup_test_db();
    seed_workflow_relation_types(&conn);

    workflow::create_status(&conn, "s", "a", "A", true, false).unwrap();
    workflow::create_status(&conn, "s", "b", "B", false, true).unwrap();
    let tid = workflow::create_transition(&conn, "s", "a", "b", "", "A to B").unwrap();
    workflow::delete_transition(&conn, tid).unwrap();

    let transitions = workflow::list_transitions_for_scope(&conn, "s").unwrap();
    assert!(transitions.is_empty());
}

#[test]
fn test_list_workflow_scopes_returns_distinct_scopes() {
    let (_dir, conn) = setup_test_db();
    seed_workflow_relation_types(&conn);

    workflow::create_status(&conn, "proposal", "draft", "Draft", true, false).unwrap();
    workflow::create_status(&conn, "suggestion", "new", "New", true, false).unwrap();
    workflow::create_status(&conn, "proposal", "submitted", "Submitted", false, true).unwrap();

    let scopes = workflow::list_workflow_scopes(&conn).unwrap();
    assert_eq!(scopes.len(), 2);
    let scope_names: Vec<_> = scopes.iter().map(|s| s.scope.as_str()).collect();
    assert!(scope_names.contains(&"proposal"));
    assert!(scope_names.contains(&"suggestion"));
}
```

**Step 2: Add HTTP auth gate tests**

```rust
use actix_web::{http::StatusCode, test};

#[actix_web::test]
async fn test_workflow_builder_list_unauthed_redirects() {
    let (_dir, pool) = common::setup_seeded_pool();
    let app = test::init_service(common::build_test_app(pool)).await;
    let req = test::TestRequest::get().uri("/workflow/builder").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FOUND);
}

#[actix_web::test]
async fn test_workflow_builder_detail_unauthed_redirects() {
    let (_dir, pool) = common::setup_seeded_pool();
    let app = test::init_service(common::build_test_app(pool)).await;
    let req = test::TestRequest::get().uri("/workflow/builder/proposal").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FOUND);
}
```

**Step 3: Run and verify**

```bash
cargo test --test workflow_builder_test -- --nocapture 2>&1 | tail -20
```

**Note on signatures:** The actual `create_status` and `create_transition` function signatures in `src/models/workflow/queries.rs` may differ from the examples above (e.g., parameter order, struct-based vs positional args). Read `src/models/workflow/queries.rs:236-400` before writing calls and match the actual signatures exactly.

**Step 4: Commit**

```bash
git add tests/workflow_builder_test.rs
git commit -m "test(h4.4): add workflow builder model and HTTP tests (8+ tests)"
```

---

## Task 5: ToR Tests (`tests/tor_test.rs`)

> **Before starting:** Invoke `/prompt-contracts` with: "Write integration tests for the Terms of Reference system: model-layer (ToR CRUD, member management, mandatory vacancy query, protocol step ordering, calendar computation, dependency queries) and HTTP-layer (auth gate)."

**Files:**
- Create: `tests/tor_test.rs`

**Step 1: Write model tests**

Key functions: `tor::create`, `tor::find_detail_by_id`, `tor::find_all_list_items`, `tor::find_members`, `tor::assign_to_position`, `tor::vacate_position`, `tor::count_members`, `tor::find_functions` (for vacancy check), `tor::calendar::compute_meetings`, `tor::dependencies::add_dependency`, `tor::dependencies::find_upstream`.

Minimum seed needed: the relation types `fills_position`, `feeds_into`, `escalates_to`, `minutes_of` must exist as entities.

```rust
mod common;

use ahlt::models::tor;
use chrono::NaiveDate;

const MIGRATIONS: &str = include_str!("../src/schema.sql");

fn setup_test_db() -> (tempfile::TempDir, rusqlite::Connection) {
    let dir = tempfile::TempDir::new().unwrap();
    let conn = rusqlite::Connection::open(dir.path().join("test.db")).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;").unwrap();
    conn.execute_batch(MIGRATIONS).unwrap();
    (dir, conn)
}

fn seed_relation_types(conn: &rusqlite::Connection) {
    for name in ["fills_position", "feeds_into", "escalates_to", "minutes_of", "member_of"] {
        conn.execute(
            "INSERT OR IGNORE INTO entities (entity_type, name, label) VALUES ('relation_type', ?1, ?1)",
            [name],
        ).unwrap();
    }
}

#[test]
fn test_create_tor_and_find_by_id() {
    let (_dir, conn) = setup_test_db();

    let id = tor::create(
        &conn,
        "sprint_planning",
        "Sprint Planning",
        "weekly",
        "Monday",
        "10:00",
        "60",
        "Room A",
        "active",
        "",
    ).unwrap();

    let detail = tor::find_detail_by_id(&conn, id).unwrap();
    assert!(detail.is_some());
    let t = detail.unwrap();
    assert_eq!(t.name, "sprint_planning");
    assert_eq!(t.label, "Sprint Planning");
}

#[test]
fn test_find_all_list_items_returns_all() {
    let (_dir, conn) = setup_test_db();

    for i in 0..3 {
        tor::create(&conn, &format!("tor{}", i), &format!("ToR {}", i),
                    "weekly", "Monday", "09:00", "60", "", "active", "").unwrap();
    }

    let list = tor::find_all_list_items(&conn).unwrap();
    assert_eq!(list.len(), 3);
}

#[test]
fn test_count_members_returns_correct_count() {
    let (_dir, conn) = setup_test_db();
    seed_relation_types(&conn);

    let tor_id = tor::create(&conn, "governance", "Governance", "monthly",
                              "Tuesday", "14:00", "90", "", "active", "").unwrap();

    // Create a user entity for membership count
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('user', 'member1', 'Member 1')",
        [],
    ).unwrap();
    let user_id = conn.last_insert_rowid();

    // Create member_of relation
    let rt_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='relation_type' AND name='member_of'",
        [],
        |r| r.get(0),
    ).unwrap();
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
        rusqlite::params![rt_id, user_id, tor_id],
    ).unwrap();

    let count = tor::count_members(&conn, tor_id).unwrap();
    assert_eq!(count, 1);
}

#[test]
fn test_add_dependency_creates_relation() {
    let (_dir, conn) = setup_test_db();
    seed_relation_types(&conn);

    let tor_a = tor::create(&conn, "committee_a", "Committee A", "weekly",
                             "Monday", "09:00", "60", "", "active", "").unwrap();
    let tor_b = tor::create(&conn, "committee_b", "Committee B", "weekly",
                             "Friday", "15:00", "60", "", "active", "").unwrap();

    tor::dependencies::add_dependency(&conn, tor_a, tor_b, "feeds_into", false).unwrap();

    let downstream = tor::dependencies::find_downstream(&conn, tor_a).unwrap();
    assert_eq!(downstream.len(), 1);
    assert_eq!(downstream[0].target_id, tor_b);
}

#[test]
fn test_calendar_weekly_generates_events_in_range() {
    let (_dir, conn) = setup_test_db();

    // Create a weekly ToR with all required cadence properties
    conn.execute(
        "INSERT INTO entities (entity_type, name, label, is_active) VALUES ('tor', 'standup', 'Daily Standup', 1)",
        [],
    ).unwrap();
    let tor_id = conn.last_insert_rowid();
    for (k, v) in [
        ("meeting_cadence", "weekly"),
        ("cadence_day", "Monday"),
        ("cadence_time", "09:00"),
        ("cadence_duration_minutes", "30"),
        ("default_location", ""),
        ("status", "active"),
    ] {
        conn.execute(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
            rusqlite::params![tor_id, k, v],
        ).unwrap();
    }

    let start = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2026, 2, 28).unwrap();
    let events = tor::calendar::compute_meetings(&conn, start, end).unwrap();

    // February 2026 has 4 Mondays: 2, 9, 16, 23
    assert_eq!(events.len(), 4, "Expected 4 Monday events in Feb 2026");
    assert!(events.iter().all(|e| e.cadence == "weekly"));
}

#[test]
fn test_calendar_adhoc_generates_no_events() {
    let (_dir, conn) = setup_test_db();

    conn.execute(
        "INSERT INTO entities (entity_type, name, label, is_active) VALUES ('tor', 'adhoc_tor', 'Ad Hoc', 1)",
        [],
    ).unwrap();
    let tor_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'meeting_cadence', 'ad-hoc')",
        rusqlite::params![tor_id],
    ).unwrap();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'status', 'active')",
        rusqlite::params![tor_id],
    ).unwrap();

    let start = NaiveDate::from_ymd_opt(2026, 2, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2026, 2, 28).unwrap();
    let events = tor::calendar::compute_meetings(&conn, start, end).unwrap();
    assert!(events.is_empty(), "Ad-hoc ToRs should generate no calendar events");
}
```

**Step 2: Add HTTP gate test**

```rust
use actix_web::{http::StatusCode, test};

#[actix_web::test]
async fn test_tor_list_unauthed_redirects() {
    let (_dir, pool) = common::setup_seeded_pool();
    let app = test::init_service(common::build_test_app(pool)).await;
    let req = test::TestRequest::get().uri("/tor").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FOUND);
}
```

**Note on `tor::create` signature:** Read `src/models/tor/queries.rs:115` for the exact parameter list before writing calls. The examples above use a positional ordering that may not match.

**Step 3: Run and fix**

```bash
cargo test --test tor_test -- --nocapture 2>&1 | tail -20
```

**Step 4: Commit**

```bash
git add tests/tor_test.rs
git commit -m "test(h4.5): add ToR model and HTTP tests (8+ tests)"
```

---

## Task 6: Minutes Tests (`tests/minutes_test.rs`)

> **Before starting:** Invoke `/prompt-contracts` with: "Write integration tests for the minutes system: scaffold generation creates exactly 5 sections, status lifecycle (draft→pending_approval→approved), approved minutes immutability, and HTTP auth gate."

**Files:**
- Create: `tests/minutes_test.rs`

**Step 1: Write model tests**

Key functions: `minutes::generate_scaffold`, `minutes::find_sections`, `minutes::update_status`, `minutes::update_section_content`. Scaffold needs: a `meeting_id` entity, a `tor_id` entity, `minutes_of` relation type, `section_of` relation type.

```rust
mod common;

use ahlt::models::minutes;

const MIGRATIONS: &str = include_str!("../src/schema.sql");

fn setup_test_db() -> (tempfile::TempDir, rusqlite::Connection) {
    let dir = tempfile::TempDir::new().unwrap();
    let conn = rusqlite::Connection::open(dir.path().join("test.db")).unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;").unwrap();
    conn.execute_batch(MIGRATIONS).unwrap();
    (dir, conn)
}

/// Seed a minimal meeting + tor + relation types for minutes tests.
fn seed_meeting_context(conn: &rusqlite::Connection) -> (i64, i64) {
    // Relation types
    for name in ["minutes_of", "section_of"] {
        conn.execute(
            "INSERT OR IGNORE INTO entities (entity_type, name, label) VALUES ('relation_type', ?1, ?1)",
            [name],
        ).unwrap();
    }
    // ToR entity
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('tor', 'test_tor', 'Test ToR')",
        [],
    ).unwrap();
    let tor_id = conn.last_insert_rowid();
    // Meeting entity (agenda_point as a stand-in for meeting)
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('agenda_point', 'test_meeting', 'Test Meeting')",
        [],
    ).unwrap();
    let meeting_id = conn.last_insert_rowid();
    (meeting_id, tor_id)
}

#[test]
fn test_scaffold_creates_exactly_five_sections() {
    let (_dir, conn) = setup_test_db();
    let (meeting_id, tor_id) = seed_meeting_context(&conn);

    let minutes_id = minutes::generate_scaffold(
        &conn, meeting_id, tor_id, "Test Meeting",
    ).unwrap();

    let sections = minutes::find_sections(&conn, minutes_id).unwrap();
    assert_eq!(sections.len(), 5, "Scaffold should create exactly 5 sections");
}

#[test]
fn test_scaffold_section_types_are_correct() {
    let (_dir, conn) = setup_test_db();
    let (meeting_id, tor_id) = seed_meeting_context(&conn);

    let minutes_id = minutes::generate_scaffold(
        &conn, meeting_id, tor_id, "Test Meeting",
    ).unwrap();

    let sections = minutes::find_sections(&conn, minutes_id).unwrap();
    let types: Vec<_> = sections.iter().map(|s| s.section_type.as_str()).collect();
    assert!(types.contains(&"attendance"), "Must have attendance section");
    assert!(types.contains(&"protocol"), "Must have protocol section");
    assert!(types.contains(&"agenda_items"), "Must have agenda_items section");
    assert!(types.contains(&"decisions"), "Must have decisions section");
    assert!(types.contains(&"action_items"), "Must have action_items section");
}

#[test]
fn test_initial_status_is_draft() {
    let (_dir, conn) = setup_test_db();
    let (meeting_id, tor_id) = seed_meeting_context(&conn);

    let minutes_id = minutes::generate_scaffold(
        &conn, meeting_id, tor_id, "Test Meeting",
    ).unwrap();

    let m = minutes::find_by_id(&conn, minutes_id).unwrap().unwrap();
    assert_eq!(m.status, "draft");
}

#[test]
fn test_status_transition_draft_to_pending_approval() {
    let (_dir, conn) = setup_test_db();
    let (meeting_id, tor_id) = seed_meeting_context(&conn);

    let minutes_id = minutes::generate_scaffold(
        &conn, meeting_id, tor_id, "Test Meeting",
    ).unwrap();

    minutes::update_status(&conn, minutes_id, "pending_approval").unwrap();
    let m = minutes::find_by_id(&conn, minutes_id).unwrap().unwrap();
    assert_eq!(m.status, "pending_approval");
}

#[test]
fn test_status_transition_pending_to_approved() {
    let (_dir, conn) = setup_test_db();
    let (meeting_id, tor_id) = seed_meeting_context(&conn);

    let minutes_id = minutes::generate_scaffold(
        &conn, meeting_id, tor_id, "Test Meeting",
    ).unwrap();

    minutes::update_status(&conn, minutes_id, "pending_approval").unwrap();
    minutes::update_status(&conn, minutes_id, "approved").unwrap();

    let m = minutes::find_by_id(&conn, minutes_id).unwrap().unwrap();
    assert_eq!(m.status, "approved");
}

#[test]
fn test_update_section_content_persists() {
    let (_dir, conn) = setup_test_db();
    let (meeting_id, tor_id) = seed_meeting_context(&conn);

    let minutes_id = minutes::generate_scaffold(
        &conn, meeting_id, tor_id, "Test Meeting",
    ).unwrap();

    let sections = minutes::find_sections(&conn, minutes_id).unwrap();
    let section_id = sections[0].id;

    minutes::update_section_content(&conn, section_id, "Updated content here.").unwrap();

    let updated = minutes::find_sections(&conn, minutes_id).unwrap();
    let updated_section = updated.iter().find(|s| s.id == section_id).unwrap();
    assert_eq!(updated_section.content, "Updated content here.");
}
```

**Step 2: Add HTTP gate test**

```rust
use actix_web::{http::StatusCode, test};

#[actix_web::test]
async fn test_minutes_unauthed_redirects() {
    let (_dir, pool) = common::setup_seeded_pool();
    let app = test::init_service(common::build_test_app(pool)).await;
    let req = test::TestRequest::get().uri("/tor/999/minutes").to_request();
    let resp = test::call_service(&app, req).await;
    // Should redirect to login (not 404 — auth gate fires before the route resolves)
    assert_eq!(resp.status(), StatusCode::FOUND);
}
```

**Step 3: Run and fix**

```bash
cargo test --test minutes_test -- --nocapture 2>&1 | tail -20
```

**Step 4: Final full test run**

```bash
cargo test 2>&1 | tail -15
```
Expected: all tests pass. Note the new total test count.

**Step 5: Commit**

```bash
git add tests/minutes_test.rs
git commit -m "test(h4.6): add minutes model and HTTP tests (7+ tests)"
```

---

## Final Verification

**Run the complete test suite:**

```bash
cargo test 2>&1 | tail -20
```

Expected output includes lines like:
```
test result: ok. X passed; 0 failed; 0 ignored
```

**Check test count:**

```bash
cargo test 2>&1 | grep "test result"
```

Should show ~101+ tests across all files. Update `BACKLOG.md` with the actual count.

**Update backlog test count:**

Edit `docs/BACKLOG.md` — find the Implementation Order table and update `Automated Testing (52 tests)` to reflect the new count. Then:

```bash
git add docs/BACKLOG.md
git commit -m "docs: update test count in backlog after h4 completion"
```

**Push:**

```bash
git push
```

---

## Notes for the Implementer

1. **Signature mismatches:** The code examples show plausible function signatures based on reading the codebase, but you MUST read the actual source file before writing each call. Use the file paths in CLAUDE.md directory structure to navigate.

2. **`mod common;`** — Every test file that uses the common module must declare `mod common;` at the top. Rust resolves this by looking for `tests/common/mod.rs`.

3. **`#[actix_web::test]`** — This replaces `#[test]` for async tests. It requires the `actix-web` feature `"macros"` — already present in the project's `Cargo.toml`.

4. **Session cookie name** — The actix-session cookie is named `"id"` by default with `CookieSessionStore`. If `login_as_admin` panics on "session cookie", check the actual cookie name with `println!("{:?}", get_resp.response().cookies().collect::<Vec<_>>())`.

5. **`AppError` vs `rusqlite::Error`** — Workflow model functions return `Result<_, AppError>`. Unwrap with `.unwrap()` in tests (panics are acceptable in test failures). Use `ahlt::errors::AppError` as the error type if you need to match.

6. **Invoke `/prompt-contracts` before each task** — each task above has a specific prompt to give. Do this before writing any code for that task.
