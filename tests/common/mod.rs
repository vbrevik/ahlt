//! Shared test infrastructure for model layer tests.
//!
//! This module provides common utilities for setting up test databases.
//!
//! # Test Database Setup
//! - `setup_test_db()` - Schema + basic entities (relation types, roles)
//! - `setup_test_db_seeded()` - Schema + full staging seed data (in progress)

use rusqlite::Connection;
use tempfile::TempDir;

use ahlt::db::MIGRATIONS;

// ============================================================================
// TEST CONSTANTS
// ============================================================================

pub const ADMIN_USER: &str = "admin";
pub const ADMIN_PASS: &str = "admin123";
pub const TEST_USER_EMAIL: &str = "test@example.com";

// ============================================================================
// DATABASE SETUP
// ============================================================================

/// Setup a test database with schema and basic entities.
///
/// Creates a temporary SQLite database, runs migrations, and seeds with
/// essential entities like relation types and a default role.
/// This is the standard setup for all model-layer tests.
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

    // Seed essential entities
    seed_base_entities(&conn).expect("Failed to seed base entities");

    (dir, conn)
}

/// Seed basic entities needed for most tests.
///
/// Creates:
/// - relation_type entities (has_role, requires_permission, belongs_to_tor, etc.)
/// - workflow relation types (transition_from, transition_to)
/// - A default role with ID 0
fn seed_base_entities(conn: &Connection) -> rusqlite::Result<()> {
    // Create relation types
    let relation_types = vec![
        "has_role",
        "has_permission",
        "requires_permission",
        "belongs_to_tor",
        "fills_position",
        "participates_in",
        "is_blocking",
        "transition_from",
        "transition_to",
        "minutes_of",
        "section_of",
    ];

    for rt in relation_types {
        conn.execute(
            "INSERT INTO entities (entity_type, name, label) VALUES ('relation_type', ?1, ?2)",
            [rt, &rt.replace('_', " ")],
        )?;
    }

    // Create a default role with ID 0 for tests
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (0, 'role', 'default', 'Default Role')",
        [],
    )?;

    Ok(())
}

/// Setup a test database with schema and staging seed data.
///
/// Creates a temporary SQLite database, runs migrations, seeds with
/// basic entities, and prepares for full staging data.
/// This provides a consistent foundation for all domain-specific tests.
///
/// Returns a tuple of (TempDir, Connection) where TempDir must be kept
/// alive for the Connection to remain valid.
pub fn setup_test_db_seeded() -> (TempDir, Connection) {
    // For now, setup_test_db_seeded() is the same as setup_test_db()
    // since seed_base_entities() sets up relation types and roles
    setup_test_db()
}
