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
