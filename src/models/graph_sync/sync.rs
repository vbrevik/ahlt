use neo4rs::Graph;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;

/// Optional Neo4j connection. None means Neo4j is disabled.
pub type GraphPool = Option<Arc<Graph>>;

/// Initialize Neo4j connection. Returns None if connection fails (best-effort).
pub async fn init(uri: &str, user: &str, password: &str) -> GraphPool {
    match Graph::new(uri, user, password).await {
        Ok(graph) => {
            log::info!("Connected to Neo4j at {}", uri);
            Some(Arc::new(graph))
        }
        Err(e) => {
            log::warn!("Neo4j unavailable ({}), running without graph projection", e);
            None
        }
    }
}

/// Sync an entity to Neo4j. Creates/updates a node with the entity's type as label.
pub async fn sync_entity(
    graph: &Graph,
    entity_id: i64,
    entity_type: &str,
    name: &str,
    label: &str,
    properties: &HashMap<String, String>,
) {
    let mut props_cypher = String::from("n.name = $name, n.label = $label");
    let mut q = neo4rs::query(
        &format!(
            "MERGE (n:{} {{id: $id}}) SET {}",
            sanitize_label(entity_type),
            {
                for key in properties.keys() {
                    props_cypher.push_str(&format!(", n.`{}` = $prop_{}", key, key));
                }
                &props_cypher
            }
        ),
    )
    .param("id", entity_id)
    .param("name", name.to_string())
    .param("label", label.to_string());

    for (key, value) in properties {
        q = q.param(&format!("prop_{}", key), value.clone());
    }

    if let Err(e) = graph.run(q).await {
        log::error!("Neo4j sync_entity failed for entity {}: {}", entity_id, e);
    }
}

/// Sync a relation to Neo4j. Creates a relationship between two nodes.
pub async fn sync_relation(
    graph: &Graph,
    relation_type: &str,
    source_id: i64,
    target_id: i64,
) {
    let rel_type = sanitize_label(relation_type);
    let q = neo4rs::query(&format!(
        "MATCH (s {{id: $src}}), (t {{id: $tgt}}) \
         MERGE (s)-[r:{}]->(t)",
        rel_type
    ))
    .param("src", source_id)
    .param("tgt", target_id);

    if let Err(e) = graph.run(q).await {
        log::error!(
            "Neo4j sync_relation failed for {} -> {}: {}",
            source_id, target_id, e
        );
    }
}

/// Delete an entity node and all its relationships from Neo4j.
pub async fn delete_entity(graph: &Graph, entity_id: i64) {
    let q = neo4rs::query("MATCH (n {id: $id}) DETACH DELETE n")
        .param("id", entity_id);

    if let Err(e) = graph.run(q).await {
        log::error!("Neo4j delete_entity failed for {}: {}", entity_id, e);
    }
}

/// Delete a specific relation from Neo4j.
pub async fn delete_relation(
    graph: &Graph,
    relation_type: &str,
    source_id: i64,
    target_id: i64,
) {
    let rel_type = sanitize_label(relation_type);
    let q = neo4rs::query(&format!(
        "MATCH (s {{id: $src}})-[r:{}]->(t {{id: $tgt}}) DELETE r",
        rel_type
    ))
    .param("src", source_id)
    .param("tgt", target_id);

    if let Err(e) = graph.run(q).await {
        log::error!(
            "Neo4j delete_relation failed for {} -> {}: {}",
            source_id, target_id, e
        );
    }
}

/// Full resync: read all entities and relations from Postgres, write to Neo4j.
pub async fn full_resync(graph: &Graph, pool: &PgPool) -> Result<(), String> {
    log::info!("Starting full Neo4j resync from Postgres...");

    // Clear all data in Neo4j
    graph
        .run(neo4rs::query("MATCH (n) DETACH DELETE n"))
        .await
        .map_err(|e| format!("Failed to clear Neo4j: {}", e))?;

    // Sync all entities with their properties
    let entities: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT id, entity_type, name, label FROM entities ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to read entities: {}", e))?;

    let total = entities.len();
    for (i, (id, entity_type, name, label)) in entities.iter().enumerate() {
        let props = crate::models::entity::get_properties(pool, *id)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect::<HashMap<String, String>>();

        sync_entity(graph, *id, entity_type, name, label, &props).await;

        if (i + 1) % 500 == 0 {
            log::info!("Synced {}/{} entities to Neo4j", i + 1, total);
        }
    }
    log::info!("Synced {} entities to Neo4j", total);

    // Sync all relations
    let relations: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT rt.name, r.source_id, r.target_id
         FROM relations r
         JOIN entities rt ON rt.id = r.relation_type_id
         ORDER BY r.id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to read relations: {}", e))?;

    let total = relations.len();
    for (i, (rel_type, source_id, target_id)) in relations.iter().enumerate() {
        sync_relation(graph, rel_type, *source_id, *target_id).await;

        if (i + 1) % 500 == 0 {
            log::info!("Synced {}/{} relations to Neo4j", i + 1, total);
        }
    }
    log::info!("Synced {} relations to Neo4j", total);
    log::info!("Full Neo4j resync complete");

    Ok(())
}

/// Fire-and-forget sync: spawns a background task to sync an entity.
pub fn spawn_sync_entity(
    graph: &GraphPool,
    entity_id: i64,
    entity_type: String,
    name: String,
    label: String,
    properties: HashMap<String, String>,
) {
    if let Some(g) = graph {
        let g = g.clone();
        tokio::spawn(async move {
            sync_entity(&g, entity_id, &entity_type, &name, &label, &properties).await;
        });
    }
}

/// Fire-and-forget sync: spawns a background task to sync a relation.
pub fn spawn_sync_relation(
    graph: &GraphPool,
    relation_type: String,
    source_id: i64,
    target_id: i64,
) {
    if let Some(g) = graph {
        let g = g.clone();
        tokio::spawn(async move {
            sync_relation(&g, &relation_type, source_id, target_id).await;
        });
    }
}

/// Fire-and-forget delete: spawns a background task to delete an entity from Neo4j.
pub fn spawn_delete_entity(graph: &GraphPool, entity_id: i64) {
    if let Some(g) = graph {
        let g = g.clone();
        tokio::spawn(async move {
            delete_entity(&g, entity_id).await;
        });
    }
}

/// Sanitize an entity type or relation name for use as a Neo4j label.
/// Neo4j labels can't contain spaces or special chars — replace with underscores.
fn sanitize_label(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}
