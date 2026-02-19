//! Shared test infrastructure for HTTP integration tests.
//!
//! This module provides common utilities for setting up test databases,
//! building test applications, and performing common test operations
//! like logging in as admin and extracting CSRF tokens.
//!
//! All HTTP-layer tests should use these helpers to ensure consistency
//! and reduce boilerplate.

use actix_web::{
    test::{self, TestRequest},
    App, HttpResponse,
};
use rusqlite::Connection;
use std::collections::HashMap;
use tempfile::TempDir;

use ahlt::{
    auth::password,
    db::{self, MIGRATIONS},
    models::{entity, relation},
};

/// Setup a test database with schema and base seed data.
///
/// Creates a temporary SQLite database, runs migrations, and seeds with
/// base ontology data including relation types, roles, permissions,
/// and an admin user. This provides a consistent foundation for all
/// domain-specific tests.
///
/// Returns a tuple of (TempDir, Connection) where TempDir must be kept
/// alive for the Connection to remain valid.
pub fn setup_test_db_seeded() -> (TempDir, Connection) {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = dir.path().join("test.db");
    let conn = rusqlite::Connection::open(&db_path).expect("Failed to open test DB");

    // Set pragmas
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;")
        .expect("Failed to set pragmas");

    // Run migrations
    conn.execute_batch(MIGRATIONS)
        .expect("Failed to run migrations");

    // Seed base data
    let admin_hash = password::hash_password("admin123")
        .expect("Failed to hash default password");
    db::seed_ontology(&crate::get_test_pool(&conn), &admin_hash);

    (dir, conn)
}

/// Get a test database pool from a connection.
///
/// This is a helper to convert a rusqlite::Connection into a pool
/// suitable for use with actix-web test::init_service.
fn get_test_pool(conn: &Connection) -> db::DbPool {
    use r2d2::Pool;
    use r2d2_sqlite::SqliteConnectionManager;

    let manager = SqliteConnectionManager::memory().with_init(|c| {
        // Copy the schema and data from the existing connection
        let backup = rusqlite::backup::Backup::new(conn, c)?;
        backup.step(-1)?;
        Ok::<(), rusqlite::Error>(())
    });
    Pool::builder()
        .max_size(1)
        .build(manager)
        .expect("Failed to create test pool")
}

/// Build a test actix-web App with all routes registered.
///
/// This replicates the route registration from src/main.rs to provide
/// a complete application for testing HTTP handlers. The app includes
/// all middleware and configuration needed for handlers to function.
pub fn build_test_app(pool: db::DbPool) -> App<
    impl actix_web::dev::ServiceFactory<
        actix_web::dev::ServiceRequest,
        Config = (),
        Response = HttpResponse,
        Error = actix_web::Error,
    >,
> {
    use actix_web::middleware;
    use actix_session::{SessionMiddleware, storage::CookieSessionStore};
    use actix_web::cookie::Key;

    // Create a dummy key for sessions
    let secret_key = Key::generate();

    let session_mw = SessionMiddleware::builder(
        CookieSessionStore::default(),
        secret_key,
    )
    .cookie_secure(false)
    .cookie_http_only(true)
    .build();

    App::new()
        .wrap(session_mw)
        .wrap(middleware::Logger::default())
        .app_data(actix_web::web::Data::new(pool))
        // Register all routes (simplified for testing)
        .configure(|cfg| {
            // Public routes
            cfg.route("/login", actix_web::web::get().to(ahlt::handlers::auth_handlers::login_page))
               .route("/login", actix_web::web::post().to(ahlt::handlers::auth_handlers::login_submit));

            // Protected routes - just a few examples for testing
            cfg.route("/users", actix_web::web::get().to(ahlt::handlers::user_handlers::list))
               .route("/users/new", actix_web::web::get().to(ahlt::handlers::user_handlers::new_form))
               .route("/users", actix_web::web::post().to(ahlt::handlers::user_handlers::create));

            // Add more routes as needed for specific tests
        })
}

/// Log in as admin user and return session cookie.
///
/// Performs a POST to /login with admin credentials and extracts
/// the session cookie from the response. This cookie can be used
/// in subsequent requests to authenticate as an admin user.
pub async fn login_as_admin(app: &actix_web::dev::Service<App>) -> String {
    let req = TestRequest::post()
        .uri("/login")
        .set_form(&HashMap::from([
            ("username", "admin"),
            ("password", "admin123"),
        ]))
        .to_request();

    let resp = test::call_service(app, req).await;

    // Extract session cookie from response
    let cookies = resp.response().cookies().collect::<Vec<_>>();
    let session_cookie = cookies
        .iter()
        .find(|cookie| cookie.name() == "id")
        .expect("No session cookie found");

    format!("{}={}", session_cookie.name(), session_cookie.value())
}

/// Extract CSRF token from HTML form page.
///
/// Many forms in the application include a hidden CSRF token field.
/// This function uses a regex to extract that token from the HTML
/// response body so it can be used in subsequent form submissions.
pub fn get_csrf_token(html: &str) -> String {
    let re = regex::Regex::new(r#"name="csrf_token"\s+value="([^"]+)""#)
        .expect("Failed to compile regex");
    re.captures(html)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "invalid_token".to_string())
}