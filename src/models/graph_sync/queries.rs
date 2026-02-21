use neo4rs::Graph;

/// Check if a user has a specific capability on a ToR via Neo4j graph traversal.
/// Returns None if the query fails (caller should fall back to Postgres).
pub async fn has_tor_capability(
    graph: &Graph,
    user_id: i64,
    tor_id: i64,
    capability_key: &str,
) -> Option<bool> {
    let q = neo4rs::query(
        "MATCH (u:user {id: $uid})-[:fills_position]->(f:tor_function)-[:belongs_to_tor]->(t:tor {id: $tid}) \
         WHERE f[$cap] = 'true' \
         RETURN COUNT(f) > 0 AS has_cap",
    )
    .param("uid", user_id)
    .param("tid", tor_id)
    .param("cap", capability_key.to_string());

    match graph.execute(q).await {
        Ok(mut result) => {
            if let Ok(Some(row)) = result.next().await {
                row.get::<bool>("has_cap").ok()
            } else {
                Some(false)
            }
        }
        Err(e) => {
            log::warn!("Neo4j ABAC query failed, falling back to Postgres: {}", e);
            None
        }
    }
}

/// Get governance graph data from Neo4j for the governance map visualization.
/// Returns nodes and edges as JSON-serializable structures.
pub async fn governance_graph(
    graph: &Graph,
) -> Option<(Vec<GraphNode>, Vec<GraphEdge>)> {
    // Get all governance-relevant nodes
    let node_q = neo4rs::query(
        "MATCH (n) WHERE n.entity_type IN ['tor', 'tor_function', 'user'] \
         RETURN n.id AS id, n.entity_type AS entity_type, n.name AS name, n.label AS label",
    );

    let edge_q = neo4rs::query(
        "MATCH (s)-[r]->(t) \
         WHERE s.entity_type IN ['tor', 'tor_function', 'user'] \
           AND t.entity_type IN ['tor', 'tor_function', 'user'] \
         RETURN s.id AS source, t.id AS target, TYPE(r) AS rel_type",
    );

    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    match graph.execute(node_q).await {
        Ok(mut result) => {
            while let Ok(Some(row)) = result.next().await {
                if let (Ok(id), Ok(entity_type), Ok(name)) = (
                    row.get::<i64>("id"),
                    row.get::<String>("entity_type"),
                    row.get::<String>("name"),
                ) {
                    let label = row.get::<String>("label").unwrap_or_default();
                    nodes.push(GraphNode {
                        id,
                        entity_type,
                        name,
                        label,
                    });
                }
            }
        }
        Err(e) => {
            log::warn!("Neo4j governance node query failed: {}", e);
            return None;
        }
    }

    match graph.execute(edge_q).await {
        Ok(mut result) => {
            while let Ok(Some(row)) = result.next().await {
                if let (Ok(source), Ok(target), Ok(rel_type)) = (
                    row.get::<i64>("source"),
                    row.get::<i64>("target"),
                    row.get::<String>("rel_type"),
                ) {
                    edges.push(GraphEdge {
                        source,
                        target,
                        rel_type,
                    });
                }
            }
        }
        Err(e) => {
            log::warn!("Neo4j governance edge query failed: {}", e);
            return None;
        }
    }

    Some((nodes, edges))
}

#[derive(Debug, serde::Serialize)]
pub struct GraphNode {
    pub id: i64,
    pub entity_type: String,
    pub name: String,
    pub label: String,
}

#[derive(Debug, serde::Serialize)]
pub struct GraphEdge {
    pub source: i64,
    pub target: i64,
    pub rel_type: String,
}
