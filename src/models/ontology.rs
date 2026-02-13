use std::collections::HashMap;
use rusqlite::Connection;
use serde::Serialize;

/// Summary of an entity type for the concepts view.
#[derive(Debug, Clone)]
pub struct EntityTypeSummary {
    pub entity_type: String,
    pub count: i64,
    pub property_keys: Vec<String>,
    pub sample_entities: Vec<EntitySample>,
}

#[derive(Debug, Clone)]
pub struct EntitySample {
    pub id: i64,
    pub name: String,
    pub label: String,
}

/// Relation type summary showing connection patterns.
#[derive(Debug, Clone)]
pub struct RelationTypeSummary {
    pub name: String,
    pub label: String,
    pub usage_count: i64,
    pub patterns: Vec<RelationPattern>,
}

#[derive(Debug, Clone)]
pub struct RelationPattern {
    pub source_type: String,
    pub target_type: String,
    pub count: i64,
}

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

/// Schema-level node: one per entity_type.
#[derive(Debug, Clone, Serialize)]
pub struct SchemaNode {
    pub id: String,
    pub label: String,
    pub count: i64,
    pub property_keys: Vec<String>,
}

/// Schema-level edge: one per (source_type, target_type, relation_type) pattern.
#[derive(Debug, Clone, Serialize)]
pub struct SchemaEdge {
    pub source: String,
    pub target: String,
    pub relation_type: String,
    pub relation_label: String,
    pub count: i64,
}

/// Schema graph data for the concepts view.
#[derive(Debug, Clone, Serialize)]
pub struct SchemaGraphData {
    pub nodes: Vec<SchemaNode>,
    pub edges: Vec<SchemaEdge>,
}

/// Get schema-level graph: entity types as nodes, relation patterns as edges.
pub fn find_schema_graph_data(conn: &Connection) -> rusqlite::Result<SchemaGraphData> {
    // Nodes: one per entity_type with count
    let mut type_stmt = conn.prepare(
        "SELECT entity_type, COUNT(*) FROM entities GROUP BY entity_type ORDER BY entity_type"
    )?;
    let type_counts: Vec<(String, i64)> = type_stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?.collect::<Result<_, _>>()?;

    // Property keys per type
    let mut key_stmt = conn.prepare(
        "SELECT DISTINCT e.entity_type, ep.key \
         FROM entity_properties ep \
         JOIN entities e ON ep.entity_id = e.id \
         ORDER BY e.entity_type, ep.key"
    )?;
    let type_keys: Vec<(String, String)> = key_stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?.collect::<Result<_, _>>()?;

    let mut nodes: Vec<SchemaNode> = type_counts.iter().map(|(t, c)| {
        let keys: Vec<String> = type_keys.iter()
            .filter(|(tt, _)| tt == t)
            .map(|(_, k)| k.clone())
            .collect();
        SchemaNode {
            id: t.clone(),
            label: t.replace('_', " "),
            count: *c,
            property_keys: keys,
        }
    }).collect();

    // Capitalize first letter of label
    for node in &mut nodes {
        if let Some(first) = node.label.get(0..1) {
            node.label = first.to_uppercase() + &node.label[1..];
        }
    }

    // Edges: relation patterns between entity types
    let mut edge_stmt = conn.prepare(
        "SELECT src.entity_type, tgt.entity_type, rt.name, rt.label, COUNT(*) \
         FROM relations r \
         JOIN entities src ON r.source_id = src.id \
         JOIN entities tgt ON r.target_id = tgt.id \
         JOIN entities rt ON r.relation_type_id = rt.id \
         GROUP BY src.entity_type, tgt.entity_type, rt.name, rt.label"
    )?;
    let edges: Vec<SchemaEdge> = edge_stmt.query_map([], |row| {
        Ok(SchemaEdge {
            source: row.get(0)?,
            target: row.get(1)?,
            relation_type: row.get(2)?,
            relation_label: row.get(3)?,
            count: row.get(4)?,
        })
    })?.collect::<Result<_, _>>()?;

    Ok(SchemaGraphData { nodes, edges })
}

/// Get summaries of all entity types: counts, property keys, and sample entities.
pub fn find_entity_type_summaries(conn: &Connection) -> rusqlite::Result<Vec<EntityTypeSummary>> {
    // Counts per type
    let mut count_stmt = conn.prepare(
        "SELECT entity_type, COUNT(*) FROM entities GROUP BY entity_type ORDER BY entity_type"
    )?;
    let type_counts: Vec<(String, i64)> = count_stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?.collect::<Result<_, _>>()?;

    // Property keys per type
    let mut key_stmt = conn.prepare(
        "SELECT DISTINCT e.entity_type, ep.key \
         FROM entity_properties ep \
         JOIN entities e ON ep.entity_id = e.id \
         ORDER BY e.entity_type, ep.key"
    )?;
    let type_keys: Vec<(String, String)> = key_stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?.collect::<Result<_, _>>()?;

    // Sample entities (up to 5 per type)
    let mut sample_stmt = conn.prepare(
        "SELECT id, entity_type, name, label FROM entities ORDER BY entity_type, sort_order, id"
    )?;
    let all_samples: Vec<(String, EntitySample)> = sample_stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(1)?,
            EntitySample {
                id: row.get(0)?,
                name: row.get(2)?,
                label: row.get(3)?,
            },
        ))
    })?.collect::<Result<_, _>>()?;

    let summaries = type_counts.into_iter().map(|(et, count)| {
        let property_keys: Vec<String> = type_keys.iter()
            .filter(|(t, _)| t == &et)
            .map(|(_, k)| k.clone())
            .collect();
        let sample_entities: Vec<EntitySample> = all_samples.iter()
            .filter(|(t, _)| t == &et)
            .take(5)
            .map(|(_, s)| s.clone())
            .collect();
        EntityTypeSummary { entity_type: et, count, property_keys, sample_entities }
    }).collect();

    Ok(summaries)
}

/// Get summaries of relation types: usage counts and source→target patterns.
pub fn find_relation_type_summaries(conn: &Connection) -> rusqlite::Result<Vec<RelationTypeSummary>> {
    let mut type_stmt = conn.prepare(
        "SELECT rt.name, rt.label, COUNT(r.id) \
         FROM entities rt \
         LEFT JOIN relations r ON r.relation_type_id = rt.id \
         WHERE rt.entity_type = 'relation_type' \
         GROUP BY rt.id, rt.name, rt.label \
         ORDER BY rt.name"
    )?;
    let mut summaries: Vec<RelationTypeSummary> = type_stmt.query_map([], |row| {
        Ok(RelationTypeSummary {
            name: row.get(0)?,
            label: row.get(1)?,
            usage_count: row.get(2)?,
            patterns: vec![],
        })
    })?.collect::<Result<_, _>>()?;

    let mut pattern_stmt = conn.prepare(
        "SELECT rt.name, src.entity_type, tgt.entity_type, COUNT(*) \
         FROM relations r \
         JOIN entities rt ON r.relation_type_id = rt.id \
         JOIN entities src ON r.source_id = src.id \
         JOIN entities tgt ON r.target_id = tgt.id \
         GROUP BY rt.name, src.entity_type, tgt.entity_type \
         ORDER BY rt.name"
    )?;
    let patterns: Vec<(String, RelationPattern)> = pattern_stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            RelationPattern {
                source_type: row.get(1)?,
                target_type: row.get(2)?,
                count: row.get(3)?,
            },
        ))
    })?.collect::<Result<_, _>>()?;

    for summary in &mut summaries {
        summary.patterns = patterns.iter()
            .filter(|(name, _)| name == &summary.name)
            .map(|(_, p)| p.clone())
            .collect();
    }

    Ok(summaries)
}

/// Entity row for the data browser list view.
#[derive(Debug, Clone)]
pub struct EntityListItem {
    pub id: i64,
    pub entity_type: String,
    pub name: String,
    pub label: String,
}

/// A single property key-value pair.
#[derive(Debug, Clone)]
pub struct EntityProperty {
    pub key: String,
    pub value: String,
}

/// A related entity (used for both incoming and outgoing relations).
#[derive(Debug, Clone)]
pub struct RelatedEntity {
    pub id: i64,
    pub entity_type: String,
    pub name: String,
    pub label: String,
    pub relation_type: String,
    pub relation_label: String,
}

/// Full detail of a single entity: base fields + properties + relations.
#[derive(Debug, Clone)]
pub struct EntityDetail {
    pub id: i64,
    pub entity_type: String,
    pub name: String,
    pub label: String,
    pub sort_order: i64,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
    pub properties: Vec<EntityProperty>,
    pub outgoing: Vec<RelatedEntity>,
    pub incoming: Vec<RelatedEntity>,
}

/// List all entities, optionally filtered by entity_type.
pub fn find_entity_list(conn: &Connection, type_filter: Option<&str>) -> rusqlite::Result<Vec<EntityListItem>> {
    let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match type_filter {
        Some(t) if !t.is_empty() => (
            "SELECT id, entity_type, name, label FROM entities WHERE entity_type = ?1 ORDER BY entity_type, sort_order, id".to_string(),
            vec![Box::new(t.to_string()) as Box<dyn rusqlite::types::ToSql>],
        ),
        _ => (
            "SELECT id, entity_type, name, label FROM entities ORDER BY entity_type, sort_order, id".to_string(),
            vec![],
        ),
    };
    let mut stmt = conn.prepare(&sql)?;
    let params_ref: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let items = stmt.query_map(params_ref.as_slice(), |row| {
        Ok(EntityListItem {
            id: row.get(0)?,
            entity_type: row.get(1)?,
            name: row.get(2)?,
            label: row.get(3)?,
        })
    })?.collect::<Result<_, _>>()?;
    Ok(items)
}

/// Get full detail for a single entity by id.
pub fn find_entity_detail(conn: &Connection, id: i64) -> rusqlite::Result<Option<EntityDetail>> {
    let mut stmt = conn.prepare(
        "SELECT id, entity_type, name, label, sort_order, is_active, created_at, updated_at \
         FROM entities WHERE id = ?1"
    )?;
    let mut rows = stmt.query_map([id], |row| {
        Ok(EntityDetail {
            id: row.get(0)?,
            entity_type: row.get(1)?,
            name: row.get(2)?,
            label: row.get(3)?,
            sort_order: row.get(4)?,
            is_active: row.get::<_, i64>(5)? != 0,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
            properties: vec![],
            outgoing: vec![],
            incoming: vec![],
        })
    })?;

    let entity = match rows.next() {
        Some(Ok(e)) => e,
        _ => return Ok(None),
    };
    let mut entity = entity;

    // Properties
    let mut prop_stmt = conn.prepare(
        "SELECT key, value FROM entity_properties WHERE entity_id = ?1 ORDER BY key"
    )?;
    entity.properties = prop_stmt.query_map([id], |row| {
        Ok(EntityProperty {
            key: row.get(0)?,
            value: row.get(1)?,
        })
    })?.collect::<Result<_, _>>()?;

    // Outgoing relations (this entity is source)
    let mut out_stmt = conn.prepare(
        "SELECT tgt.id, tgt.entity_type, tgt.name, tgt.label, rt.name, rt.label \
         FROM relations r \
         JOIN entities tgt ON r.target_id = tgt.id \
         JOIN entities rt ON r.relation_type_id = rt.id \
         WHERE r.source_id = ?1 \
         ORDER BY rt.name, tgt.name"
    )?;
    entity.outgoing = out_stmt.query_map([id], |row| {
        Ok(RelatedEntity {
            id: row.get(0)?,
            entity_type: row.get(1)?,
            name: row.get(2)?,
            label: row.get(3)?,
            relation_type: row.get(4)?,
            relation_label: row.get(5)?,
        })
    })?.collect::<Result<_, _>>()?;

    // Incoming relations (this entity is target)
    let mut in_stmt = conn.prepare(
        "SELECT src.id, src.entity_type, src.name, src.label, rt.name, rt.label \
         FROM relations r \
         JOIN entities src ON r.source_id = src.id \
         JOIN entities rt ON r.relation_type_id = rt.id \
         WHERE r.target_id = ?1 \
         ORDER BY rt.name, src.name"
    )?;
    entity.incoming = in_stmt.query_map([id], |row| {
        Ok(RelatedEntity {
            id: row.get(0)?,
            entity_type: row.get(1)?,
            name: row.get(2)?,
            label: row.get(3)?,
            relation_type: row.get(4)?,
            relation_label: row.get(5)?,
        })
    })?.collect::<Result<_, _>>()?;

    Ok(Some(entity))
}

/// Get distinct entity types for filter UI.
pub fn find_entity_types(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT entity_type FROM entities ORDER BY entity_type"
    )?;
    let types = stmt.query_map([], |row| row.get(0))?.collect::<Result<_, _>>()?;
    Ok(types)
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
