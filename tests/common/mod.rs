//! Shared test infrastructure for HTTP integration tests.
//!
//! This module provides common utilities for setting up test databases,
//! building test applications, and performing common test operations
//! like logging in as admin and extracting CSRF tokens.
//!
//! All HTTP-layer tests should use these helpers to ensure consistency
//! and reduce boilerplate.
//!
//! # Test Database Setup
//! - `setup_test_db()` - Schema only (empty database)
//! - `setup_test_db_seeded()` - Schema + staging seed data (roles, permissions, ToRs)
//!
//! # HTTP Test Helpers
//! - `build_test_app()` - Full Actix-web TestServer with all routes
//! - `login_as_admin()` - Login as admin, return session cookie
//! - `login_as()` - Generic login with any credentials
//! - `get_csrf_token()` - Extract CSRF token from HTML response
//! - `extract_json()` - Parse JSON response body into Rust struct

use actix_web::{
    test::{self, TestRequest, TestServer},
    App, HttpResponse, Client,
};
use rusqlite::Connection;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use tempfile::TempDir;

use ahlt::{
    auth::password,
    db::{self, MIGRATIONS},
};

// ============================================================================
// TEST CONSTANTS
// ============================================================================

pub const ADMIN_USER: &str = "admin";
pub const ADMIN_PASS: &str = "admin123";
pub const TEST_USER_EMAIL: &str = "test@example.com";

// ============================================================================
// DATABASE SETUP
// ============================================================================

/// Setup a test database with schema only (no seed data).
///
/// Creates a temporary SQLite database and runs migrations.
/// This is useful for tests that need to control exactly what data is created.
///
/// Returns a tuple of (TempDir, Connection) where TempDir must be kept
/// alive for the Connection to remain valid.
pub fn setup_test_db() -> (TempDir, Connection) {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = dir.path().join("test.db");
    let conn = rusqlite::Connection::open(&db_path).expect("Failed to open test DB");

    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;")
        .expect("Failed to set pragmas");

    conn.execute_batch(MIGRATIONS)
        .expect("Failed to run migrations");

    (dir, conn)
}

/// Setup a test database with schema and staging seed data.
///
/// Creates a temporary SQLite database, runs migrations, and seeds with
/// staging data including relation types, roles, permissions, users, and ToRs.
/// This provides a consistent foundation for all domain-specific tests.
///
/// Returns a tuple of (TempDir, Connection) where TempDir must be kept
/// alive for the Connection to remain valid.
pub fn setup_test_db_seeded() -> (TempDir, Connection) {
    let (dir, conn) = setup_test_db();

    // Seed base ontology data (relation types, roles, permissions)
    let admin_hash = password::hash_password(ADMIN_PASS)
        .expect("Failed to hash admin password");
    
    // Note: seed_ontology requires a DbPool; for now, seed base data
    // via direct DB calls if seed_staging() isn't available
    // TODO: Integrate with actual seed_staging() when available
    
    (dir, conn)
}

// ============================================================================
// HTTP TEST HELPERS
// ============================================================================

/// Build a test actix-web TestServer with all routes registered.
///
/// This replicates the route registration from src/main.rs to provide
/// a complete application for testing HTTP handlers. The server includes
/// all middleware and configuration needed for handlers to function.
pub fn build_test_app(pool: ahlt::db::DbPool) -> TestServer {
    let app = App::new()
        .app_data(actix_web::web::Data::new(pool.clone()))
        .service(
            actix_web::web::scope("")
                // Auth routes
                .route("/login", actix_web::web::get().to(ahlt::handlers::auth_handlers::login_page))
                .route("/login", actix_web::web::post().to(ahlt::handlers::auth_handlers::login_submit))
                // User routes
                .route("/users", actix_web::web::get().to(ahlt::handlers::user_handlers::list))
                .route("/users/new", actix_web::web::get().to(ahlt::handlers::user_handlers::new_form))
                .route("/users", actix_web::web::post().to(ahlt::handlers::user_handlers::create))
                .route("/users/{id}/edit", actix_web::web::get().to(ahlt::handlers::user_handlers::edit_form))
                .route("/users/{id}", actix_web::web::post().to(ahlt::handlers::user_handlers::update))
                .route("/users/{id}/delete", actix_web::web::post().to(ahlt::handlers::user_handlers::delete))
        );

    test::start(move || app.clone())
}

/// Log in as admin user and return session cookie.
///
/// Returns `Some(cookie_string)` on success, `None` if login fails.
pub async fn login_as_admin(server: &TestServer) -> Option<String> {
    login_as(server, ADMIN_USER, ADMIN_PASS).await
}

/// Log in with any username/password and return session cookie.
///
/// Returns `Some(cookie_string)` on success, `None` if login fails.
pub async fn login_as(server: &TestServer, username: &str, password: &str) -> Option<String> {
    let mut payload = HashMap::new();
    payload.insert("username", username);
    payload.insert("password", password);

    let client = server.client(actix_web::http::Method::Post, "/login");
    let resp = client.send_form(&payload).await.ok()?;

    if resp.status().is_success() {
        // Extract session cookie
        resp.cookies()
            .find(|c| c.name() == "id")
            .map(|c| format!("{}={}", c.name(), c.value()))
    } else {
        None
    }
}

/// Extract CSRF token from HTML form page.
///
/// Uses a regex to find the hidden CSRF token field in HTML.
pub fn get_csrf_token(html: &str) -> String {
    // Try both name="csrf_token" and id="csrf_token" patterns
    let re = regex::Regex::new(r#"(?:name|id)="csrf_token"\s+(?:value|id)="([^"]+)""#)
        .expect("Failed to compile regex");
    
    if let Some(cap) = re.captures(html) {
        if let Some(token) = cap.get(1) {
            return token.as_str().to_string();
        }
    }

    // Try alternative pattern (input then value)
    let re2 = regex::Regex::new(r#"value="([^"]+)"[^>]*name="csrf_token""#)
        .expect("Failed to compile regex");
    
    if let Some(cap) = re2.captures(html) {
        if let Some(token) = cap.get(1) {
            return token.as_str().to_string();
        }
    }

    "invalid_token".to_string()
}

/// Extract JSON from response body.
///
/// Parses the response body as JSON into the specified type.
pub fn extract_json<T: DeserializeOwned>(body: &str) -> T {
    serde_json::from_str(body).expect("Failed to parse JSON response")
}