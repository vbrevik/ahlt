use std::collections::HashMap;
use rusqlite::Connection;
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
pub fn find_graph_data(conn: &Connection) -> rusqlite::Result<GraphData> {
    let mut type_stmt = conn.prepare(
        "SELECT DISTINCT entity_type FROM entities ORDER BY entity_type"
    )?;
    let entity_types: Vec<String> = type_stmt.query_map([], |row| {
        row.get(0)
    })?.collect::<Result<_, _>>()?;

    let mut node_stmt = conn.prepare(
        "SELECT id, entity_type, name, label FROM entities ORDER BY entity_type, id"
    )?;
    let mut nodes: Vec<GraphNode> = node_stmt.query_map([], |row| {
        Ok(GraphNode {
            id: row.get(0)?,
            entity_type: row.get(1)?,
            name: row.get(2)?,
            label: row.get(3)?,
            properties: HashMap::new(),
        })
    })?.collect::<Result<_, _>>()?;

    // Fetch all properties and attach to nodes
    let mut prop_stmt = conn.prepare(
        "SELECT entity_id, key, value FROM entity_properties ORDER BY entity_id, key"
    )?;
    let props: Vec<(i64, String, String)> = prop_stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })?.collect::<Result<_, _>>()?;
    let mut prop_map: HashMap<i64, HashMap<String, String>> = HashMap::new();
    for (eid, key, val) in props {
        prop_map.entry(eid).or_default().insert(key, val);
    }
    for node in &mut nodes {
        if let Some(p) = prop_map.remove(&node.id) {
            node.properties = p;
        }
    }

    let mut edge_stmt = conn.prepare(
        "SELECT r.source_id, r.target_id, rt.name, rt.label \
         FROM relations r \
         JOIN entities rt ON r.relation_type_id = rt.id"
    )?;
    let edges: Vec<GraphEdge> = edge_stmt.query_map([], |row| {
        Ok(GraphEdge {
            source: row.get(0)?,
            target: row.get(1)?,
            relation_type: row.get(2)?,
            relation_label: row.get(3)?,
        })
    })?.collect::<Result<_, _>>()?;

    Ok(GraphData { nodes, edges, entity_types })
}
