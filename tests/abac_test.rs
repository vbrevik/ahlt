//! ABAC (Attribute-Based Access Control) tests.
//!
//! Tests the abac module's query functions:
//! - has_resource_capability: checks if a user has a capability in a resource
//! - load_tor_capabilities: loads all true capability flags for a user in a ToR

mod common;

use ahlt::auth::abac;
use ahlt::auth::session::Permissions;
use common::*;
use sqlx::PgPool;

// --- Helpers ---

/// Create a tor_function entity with a single entity_property.
/// Returns the new entity's ID.
async fn create_function(pool: &PgPool, name: &str, capability: &str, value: &str) -> i64 {
    let entity_id = insert_entity(pool, "tor_function", name, name).await;
    insert_prop(pool, entity_id, capability, value).await;
    entity_id
}

/// Create a user entity. Returns the new entity's ID.
async fn create_user(pool: &PgPool, name: &str) -> i64 {
    insert_entity(pool, "user", name, name).await
}

/// Create a tor entity. Returns the new entity's ID.
async fn create_tor(pool: &PgPool, name: &str) -> i64 {
    insert_entity(pool, "tor", name, name).await
}

/// Look up a relation type entity ID by name.
/// Relies on the relation types seeded by setup_test_db().
async fn rel_type(pool: &PgPool, name: &str) -> i64 {
    sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = $1",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
    .0
}

/// Create a fills_position relation between a user and a tor_function.
async fn fills_position(pool: &PgPool, user_id: i64, func_id: i64) {
    let rt = rel_type(pool, "fills_position").await;
    insert_relation(pool, rt, user_id, func_id).await;
}

/// Create a belongs_to_tor relation between a tor_function and a tor.
async fn belongs_to_tor(pool: &PgPool, func_id: i64, tor_id: i64) {
    let rt = rel_type(pool, "belongs_to_tor").await;
    insert_relation(pool, rt, func_id, tor_id).await;
}

// --- Test 1 ---

#[tokio::test]
async fn test_has_capability_true() {
    let db = setup_test_db().await;
    let pool = db.pool();
    let user_id = create_user(pool, "alice").await;
    let tor_id = create_tor(pool, "tor-1").await;
    let func_id = create_function(pool, "chair-1", "can_call_meetings", "true").await;
    fills_position(pool, user_id, func_id).await;
    belongs_to_tor(pool, func_id, tor_id).await;
    let result =
        abac::has_resource_capability(pool, user_id, tor_id, "belongs_to_tor", "can_call_meetings").await;
    assert!(result.unwrap());
}

// --- Test 2 ---

#[tokio::test]
async fn test_has_capability_false_when_flag_is_false() {
    let db = setup_test_db().await;
    let pool = db.pool();
    let user_id = create_user(pool, "bob").await;
    let tor_id = create_tor(pool, "tor-2").await;
    let func_id = create_function(pool, "member-1", "can_call_meetings", "false").await;
    fills_position(pool, user_id, func_id).await;
    belongs_to_tor(pool, func_id, tor_id).await;
    let result =
        abac::has_resource_capability(pool, user_id, tor_id, "belongs_to_tor", "can_call_meetings").await;
    assert!(!result.unwrap());
}

// --- Test 3 ---

#[tokio::test]
async fn test_has_capability_false_when_not_member() {
    let db = setup_test_db().await;
    let pool = db.pool();
    let user_id = create_user(pool, "carol").await;
    let tor_id = create_tor(pool, "tor-3").await;
    // No relations wired
    let result =
        abac::has_resource_capability(pool, user_id, tor_id, "belongs_to_tor", "can_call_meetings").await;
    assert!(!result.unwrap());
}

// --- Test 4 ---

#[tokio::test]
async fn test_boundary_isolation_different_tor() {
    let db = setup_test_db().await;
    let pool = db.pool();
    let user_id = create_user(pool, "dave").await;
    let tor_a_id = create_tor(pool, "tor-a").await;
    let tor_b_id = create_tor(pool, "tor-b").await;
    let func_id = create_function(pool, "chair-a", "can_call_meetings", "true").await;
    fills_position(pool, user_id, func_id).await;
    belongs_to_tor(pool, func_id, tor_a_id).await;
    // Check against tor_b -- user has no capability here
    let result =
        abac::has_resource_capability(pool, user_id, tor_b_id, "belongs_to_tor", "can_call_meetings").await;
    assert!(!result.unwrap());
}

// --- Test 5 ---

#[tokio::test]
async fn test_missing_capability_key_returns_false() {
    let db = setup_test_db().await;
    let pool = db.pool();
    let user_id = create_user(pool, "eve").await;
    let tor_id = create_tor(pool, "tor-5").await;
    let func_id = create_function(pool, "member-5", "can_manage_agenda", "true").await;
    fills_position(pool, user_id, func_id).await;
    belongs_to_tor(pool, func_id, tor_id).await;
    // Checking can_call_meetings, but function only has can_manage_agenda
    let result =
        abac::has_resource_capability(pool, user_id, tor_id, "belongs_to_tor", "can_call_meetings").await;
    assert!(!result.unwrap());
}

// --- Test 6 ---

#[tokio::test]
async fn test_load_tor_capabilities_returns_all_true_flags() {
    let db = setup_test_db().await;
    let pool = db.pool();
    let user_id = create_user(pool, "frank").await;
    let tor_id = create_tor(pool, "tor-6").await;
    let func_id = create_function(pool, "chair-6", "can_call_meetings", "true").await;
    // Add remaining two properties via direct SQL (create_function only inserts one)
    insert_prop(pool, func_id, "can_manage_agenda", "true").await;
    insert_prop(pool, func_id, "can_record_decisions", "false").await;
    fills_position(pool, user_id, func_id).await;
    belongs_to_tor(pool, func_id, tor_id).await;
    let caps: Permissions = abac::load_tor_capabilities(pool, user_id, tor_id).await.unwrap();
    assert!(caps.has("can_call_meetings"));
    assert!(caps.has("can_manage_agenda"));
    assert!(!caps.has("can_record_decisions"));
}

// --- Test 7 ---

#[tokio::test]
async fn test_load_tor_capabilities_empty_for_non_member() {
    let db = setup_test_db().await;
    let pool = db.pool();
    let user_id = create_user(pool, "grace").await;
    let tor_id = create_tor(pool, "tor-7").await;
    // No relations wired
    let caps: Permissions = abac::load_tor_capabilities(pool, user_id, tor_id).await.unwrap();
    assert!(!caps.has("can_call_meetings"));
}
