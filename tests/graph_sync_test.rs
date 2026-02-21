mod common;

use ahlt::models::graph_sync;
use std::collections::HashMap;

/// Test Neo4j connection and basic CRUD operations.
/// Requires NEO4J_URI=bolt://localhost:7687 with NEO4J_USER=neo4j NEO4J_PASSWORD=secretpass
///
/// Run with --test-threads=1 because tests share a single Neo4j instance
/// and full_resync clears all data: cargo test --test graph_sync_test -- --test-threads=1
#[tokio::test]
#[ignore] // Requires running Neo4j: cargo test --test graph_sync_test -- --ignored --test-threads=1
async fn test_neo4j_connect() {
    let uri = std::env::var("NEO4J_URI").unwrap_or_else(|_| "bolt://localhost:7687".to_string());
    let user = std::env::var("NEO4J_USER").unwrap_or_else(|_| "neo4j".to_string());
    let password = std::env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "secretpass".to_string());

    let graph = graph_sync::init(&uri, &user, &password).await;
    assert!(graph.is_some(), "Should connect to Neo4j");
}

#[tokio::test]
#[ignore]
async fn test_sync_entity_and_delete() {
    let uri = std::env::var("NEO4J_URI").unwrap_or_else(|_| "bolt://localhost:7687".to_string());
    let user = std::env::var("NEO4J_USER").unwrap_or_else(|_| "neo4j".to_string());
    let password = std::env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "secretpass".to_string());

    let graph_pool = graph_sync::init(&uri, &user, &password).await;
    let graph = graph_pool.as_ref().expect("Neo4j must be running");

    let mut props = HashMap::new();
    props.insert("email".to_string(), "test@example.com".to_string());

    // Sync entity
    graph_sync::sync::sync_entity(graph, 9999, "user", "test_user", "Test User", &props).await;

    // Verify node exists
    let q = neo4rs::query("MATCH (n:user {id: $id}) RETURN n.name AS name")
        .param("id", 9999_i64);
    let mut result = graph.execute(q).await.expect("query should work");
    let row = result.next().await.expect("should have result").expect("should have row");
    let name: String = row.get("name").expect("should have name");
    assert_eq!(name, "test_user");

    // Delete entity
    graph_sync::sync::delete_entity(graph, 9999).await;

    // Verify node is gone
    let q = neo4rs::query("MATCH (n:user {id: $id}) RETURN COUNT(n) AS cnt")
        .param("id", 9999_i64);
    let mut result = graph.execute(q).await.expect("query should work");
    let row = result.next().await.expect("should have result").expect("should have row");
    let cnt: i64 = row.get("cnt").expect("should have count");
    assert_eq!(cnt, 0);
}

#[tokio::test]
#[ignore]
async fn test_sync_relation_and_delete() {
    let uri = std::env::var("NEO4J_URI").unwrap_or_else(|_| "bolt://localhost:7687".to_string());
    let user = std::env::var("NEO4J_USER").unwrap_or_else(|_| "neo4j".to_string());
    let password = std::env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "secretpass".to_string());

    let graph_pool = graph_sync::init(&uri, &user, &password).await;
    let graph = graph_pool.as_ref().expect("Neo4j must be running");

    // Use unique IDs to avoid interference from parallel tests (e.g. full_resync clears all)
    let src_id: i64 = 70001;
    let tgt_id: i64 = 70002;

    // Create two nodes
    graph_sync::sync::sync_entity(graph, src_id, "user", "alice_rel", "Alice Rel", &HashMap::new()).await;
    graph_sync::sync::sync_entity(graph, tgt_id, "role", "admin_rel", "Admin Rel", &HashMap::new()).await;

    // Verify nodes exist before creating relation
    let q = neo4rs::query("MATCH (n {id: $id}) RETURN COUNT(n) AS cnt").param("id", src_id);
    let mut result = graph.execute(q).await.expect("query should work");
    let row = result.next().await.expect("should have result").expect("should have row");
    let cnt: i64 = row.get("cnt").expect("count");
    assert_eq!(cnt, 1, "Source node should exist");

    // Create relation
    graph_sync::sync::sync_relation(graph, "has_role", src_id, tgt_id).await;

    // Verify relation exists
    let q = neo4rs::query(
        "MATCH (s {id: $src})-[:has_role]->(t {id: $tgt}) RETURN COUNT(*) AS cnt",
    )
    .param("src", src_id)
    .param("tgt", tgt_id);
    let mut result = graph.execute(q).await.expect("query should work");
    let row = result.next().await.expect("should have result").expect("should have row");
    let cnt: i64 = row.get("cnt").expect("should have count");
    assert_eq!(cnt, 1, "Relation should exist after sync");

    // Delete relation
    graph_sync::sync::delete_relation(graph, "has_role", src_id, tgt_id).await;

    // Verify relation gone
    let q = neo4rs::query(
        "MATCH (s {id: $src})-[:has_role]->(t {id: $tgt}) RETURN COUNT(*) AS cnt",
    )
    .param("src", src_id)
    .param("tgt", tgt_id);
    let mut result = graph.execute(q).await.expect("query should work");
    let row = result.next().await.expect("should have result").expect("should have row");
    let cnt: i64 = row.get("cnt").expect("should have count");
    assert_eq!(cnt, 0, "Relation should be gone after delete");

    // Cleanup
    graph_sync::sync::delete_entity(graph, src_id).await;
    graph_sync::sync::delete_entity(graph, tgt_id).await;
}

#[tokio::test]
#[ignore]
async fn test_full_resync() {
    let db = common::setup_test_db().await;
    let pool = db.pool();

    let uri = std::env::var("NEO4J_URI").unwrap_or_else(|_| "bolt://localhost:7687".to_string());
    let neo_user = std::env::var("NEO4J_USER").unwrap_or_else(|_| "neo4j".to_string());
    let password = std::env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "secretpass".to_string());

    let graph_pool = graph_sync::init(&uri, &neo_user, &password).await;
    let graph = graph_pool.as_ref().expect("Neo4j must be running");

    // Run full resync from test Postgres
    let result = graph_sync::sync::full_resync(graph, pool).await;
    assert!(result.is_ok(), "Full resync should succeed: {:?}", result.err());

    // Verify nodes were created (seed_base_entities creates 10 relation types + default role)
    let q = neo4rs::query("MATCH (n) RETURN COUNT(n) AS cnt");
    let mut result = graph.execute(q).await.expect("count query");
    let row = result.next().await.expect("should have result").expect("should have row");
    let cnt: i64 = row.get("cnt").expect("count");
    assert!(cnt > 0, "Should have synced some nodes, got {}", cnt);

    // Clean up
    graph.run(neo4rs::query("MATCH (n) DETACH DELETE n")).await.expect("cleanup");
}
