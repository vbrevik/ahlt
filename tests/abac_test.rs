//! ABAC (Attribute-Based Access Control) tests.
//!
//! Tests the abac module's query functions:
//! - has_resource_capability: checks if a user has a capability in a resource
//! - load_tor_capabilities: loads all true capability flags for a user in a ToR
//!
//! TDD: these tests are written BEFORE the implementation exists.
//! They will fail to compile until src/auth/abac.rs is created and
//! pub mod abac; is added to src/auth/mod.rs.

mod common;

use ahlt::auth::abac;
use ahlt::auth::session::Permissions;
use common::setup_test_db;
use rusqlite::{params, Connection};

// --- Helpers ---

/// Create a tor_function entity with a single entity_property.
/// Returns the new entity's ID.
fn create_function(conn: &Connection, name: &str, capability: &str, value: &str) -> i64 {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('tor_function', ?1, ?1)",
        params![name],
    )
    .unwrap();
    let entity_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
        params![entity_id, capability, value],
    )
    .unwrap();
    entity_id
}

/// Create a user entity. Returns the new entity's ID.
fn create_user(conn: &Connection, name: &str) -> i64 {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('user', ?1, ?1)",
        params![name],
    )
    .unwrap();
    conn.last_insert_rowid()
}

/// Create a tor entity. Returns the new entity's ID.
fn create_tor(conn: &Connection, name: &str) -> i64 {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('tor', ?1, ?1)",
        params![name],
    )
    .unwrap();
    conn.last_insert_rowid()
}

/// Look up a relation type entity ID by name.
/// Relies on the relation types seeded by setup_test_db().
fn rel_type(conn: &Connection, name: &str) -> i64 {
    conn.query_row(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = ?1",
        params![name],
        |row| row.get(0),
    )
    .unwrap()
}

/// Create a fills_position relation between a user and a tor_function.
fn fills_position(conn: &Connection, user_id: i64, func_id: i64) {
    let rt = rel_type(conn, "fills_position");
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
        params![rt, user_id, func_id],
    )
    .unwrap();
}

/// Create a belongs_to_tor relation between a tor_function and a tor.
fn belongs_to_tor(conn: &Connection, func_id: i64, tor_id: i64) {
    let rt = rel_type(conn, "belongs_to_tor");
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
        params![rt, func_id, tor_id],
    )
    .unwrap();
}

// --- Test 1 ---

#[test]
fn test_has_capability_true() {
    let (_dir, conn) = setup_test_db();
    let user_id = create_user(&conn, "alice");
    let tor_id = create_tor(&conn, "tor-1");
    let func_id = create_function(&conn, "chair-1", "can_call_meetings", "true");
    fills_position(&conn, user_id, func_id);
    belongs_to_tor(&conn, func_id, tor_id);
    let result =
        abac::has_resource_capability(&conn, user_id, tor_id, "belongs_to_tor", "can_call_meetings");
    assert!(result.unwrap());
}

// --- Test 2 ---

#[test]
fn test_has_capability_false_when_flag_is_false() {
    let (_dir, conn) = setup_test_db();
    let user_id = create_user(&conn, "bob");
    let tor_id = create_tor(&conn, "tor-2");
    let func_id = create_function(&conn, "member-1", "can_call_meetings", "false");
    fills_position(&conn, user_id, func_id);
    belongs_to_tor(&conn, func_id, tor_id);
    let result =
        abac::has_resource_capability(&conn, user_id, tor_id, "belongs_to_tor", "can_call_meetings");
    assert!(!result.unwrap());
}

// --- Test 3 ---

#[test]
fn test_has_capability_false_when_not_member() {
    let (_dir, conn) = setup_test_db();
    let user_id = create_user(&conn, "carol");
    let tor_id = create_tor(&conn, "tor-3");
    // No relations wired
    let result =
        abac::has_resource_capability(&conn, user_id, tor_id, "belongs_to_tor", "can_call_meetings");
    assert!(!result.unwrap());
}

// --- Test 4 ---

#[test]
fn test_boundary_isolation_different_tor() {
    let (_dir, conn) = setup_test_db();
    let user_id = create_user(&conn, "dave");
    let tor_a_id = create_tor(&conn, "tor-a");
    let tor_b_id = create_tor(&conn, "tor-b");
    let func_id = create_function(&conn, "chair-a", "can_call_meetings", "true");
    fills_position(&conn, user_id, func_id);
    belongs_to_tor(&conn, func_id, tor_a_id);
    // Check against tor_b — user has no capability here
    let result =
        abac::has_resource_capability(&conn, user_id, tor_b_id, "belongs_to_tor", "can_call_meetings");
    assert!(!result.unwrap());
}

// --- Test 5 ---

#[test]
fn test_missing_capability_key_returns_false() {
    let (_dir, conn) = setup_test_db();
    let user_id = create_user(&conn, "eve");
    let tor_id = create_tor(&conn, "tor-5");
    let func_id = create_function(&conn, "member-5", "can_manage_agenda", "true");
    fills_position(&conn, user_id, func_id);
    belongs_to_tor(&conn, func_id, tor_id);
    // Checking can_call_meetings, but function only has can_manage_agenda
    let result =
        abac::has_resource_capability(&conn, user_id, tor_id, "belongs_to_tor", "can_call_meetings");
    assert!(!result.unwrap());
}

// --- Test 6 ---

#[test]
fn test_load_tor_capabilities_returns_all_true_flags() {
    let (_dir, conn) = setup_test_db();
    let user_id = create_user(&conn, "frank");
    let tor_id = create_tor(&conn, "tor-6");
    let func_id = create_function(&conn, "chair-6", "can_call_meetings", "true");
    // Add remaining two properties via direct SQL (create_function only inserts one)
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
        params![func_id, "can_manage_agenda", "true"],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
        params![func_id, "can_record_decisions", "false"],
    )
    .unwrap();
    fills_position(&conn, user_id, func_id);
    belongs_to_tor(&conn, func_id, tor_id);
    let caps: Permissions = abac::load_tor_capabilities(&conn, user_id, tor_id).unwrap();
    assert!(caps.has("can_call_meetings"));
    assert!(caps.has("can_manage_agenda"));
    assert!(!caps.has("can_record_decisions"));
}

// --- Test 7 ---

#[test]
fn test_load_tor_capabilities_empty_for_non_member() {
    let (_dir, conn) = setup_test_db();
    let user_id = create_user(&conn, "grace");
    let tor_id = create_tor(&conn, "tor-7");
    // No relations wired
    let caps: Permissions = abac::load_tor_capabilities(&conn, user_id, tor_id).unwrap();
    assert!(!caps.has("can_call_meetings"));
}
