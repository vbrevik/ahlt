use std::collections::HashMap;
use sqlx::PgPool;
use serde::Serialize;

/// Graph node for the force-directed visualization.
#[derive(Debug, Clone, Serialize)]
pub struct GraphNode {
    pub id: i64,
    pub entity_type: String,
    pub name: String,
    pub label: String,
    pub properties: HashMap<String, String>,
}

/// Graph edge for the force-directed visualization.
#[derive(Debug, Clone, Serialize)]
pub struct GraphEdge {
    pub source: i64,
    pub target: i64,
    pub relation_type: String,
    pub relation_label: String,
}

/// Complete graph data returned as JSON.
#[derive(Debug, Clone, Serialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub entity_types: Vec<String>,
}

/// Get all graph data: nodes, edges, and available entity types.
pub async fn find_graph_data(pool: &PgPool) -> Result<GraphData, sqlx::Error> {
    let entity_types: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT entity_type FROM entities ORDER BY entity_type"
    )
    .fetch_all(pool)
    .await?;
    let entity_types: Vec<String> = entity_types.into_iter().map(|(t,)| t).collect();

    #[derive(sqlx::FromRow)]
    struct NodeRow {
        id: i64,
        entity_type: String,
        name: String,
        label: String,
    }
    let node_rows: Vec<NodeRow> = sqlx::query_as(
        "SELECT id, entity_type, name, label FROM entities ORDER BY entity_type, id"
    )
    .fetch_all(pool)
    .await?;

    let mut nodes: Vec<GraphNode> = node_rows.into_iter().map(|r| {
        GraphNode {
            id: r.id,
            entity_type: r.entity_type,
            name: r.name,
            label: r.label,
            properties: HashMap::new(),
        }
    }).collect();

    // Fetch all properties and attach to nodes
    let props: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT entity_id, key, value FROM entity_properties ORDER BY entity_id, key"
    )
    .fetch_all(pool)
    .await?;
    let mut prop_map: HashMap<i64, HashMap<String, String>> = HashMap::new();
    for (eid, key, val) in props {
        prop_map.entry(eid).or_default().insert(key, val);
    }
    for node in &mut nodes {
        if let Some(p) = prop_map.remove(&node.id) {
            node.properties = p;
        }
    }

    #[derive(sqlx::FromRow)]
    struct EdgeRow {
        source_id: i64,
        target_id: i64,
        name: String,
        label: String,
    }
    let edge_rows: Vec<EdgeRow> = sqlx::query_as(
        "SELECT r.source_id, r.target_id, rt.name, rt.label \
         FROM relations r \
         JOIN entities rt ON r.relation_type_id = rt.id"
    )
    .fetch_all(pool)
    .await?;

    let edges: Vec<GraphEdge> = edge_rows.into_iter().map(|r| {
        GraphEdge {
            source: r.source_id,
            target: r.target_id,
            relation_type: r.name,
            relation_label: r.label,
        }
    }).collect();

    Ok(GraphData { nodes, edges, entity_types })
}
