use sqlx::PgPool;
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
pub async fn find_schema_graph_data(pool: &PgPool) -> Result<SchemaGraphData, sqlx::Error> {
    // Nodes: one per entity_type with count
    let type_counts: Vec<(String, i64)> = sqlx::query_as(
        "SELECT entity_type, COUNT(*) FROM entities GROUP BY entity_type ORDER BY entity_type"
    )
    .fetch_all(pool)
    .await?;

    // Property keys per type
    let type_keys: Vec<(String, String)> = sqlx::query_as(
        "SELECT DISTINCT e.entity_type, ep.key \
         FROM entity_properties ep \
         JOIN entities e ON ep.entity_id = e.id \
         ORDER BY e.entity_type, ep.key"
    )
    .fetch_all(pool)
    .await?;

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
    let edges: Vec<SchemaEdge> = sqlx::query_as(
        "SELECT src.entity_type, tgt.entity_type, rt.name, rt.label, COUNT(*) \
         FROM relations r \
         JOIN entities src ON r.source_id = src.id \
         JOIN entities tgt ON r.target_id = tgt.id \
         JOIN entities rt ON r.relation_type_id = rt.id \
         GROUP BY src.entity_type, tgt.entity_type, rt.name, rt.label"
    )
    .fetch_all(pool)
    .await?;

    Ok(SchemaGraphData { nodes, edges })
}

/// Get summaries of all entity types: counts, property keys, and sample entities.
pub async fn find_entity_type_summaries(pool: &PgPool) -> Result<Vec<EntityTypeSummary>, sqlx::Error> {
    // Counts per type
    let type_counts: Vec<(String, i64)> = sqlx::query_as(
        "SELECT entity_type, COUNT(*) FROM entities GROUP BY entity_type ORDER BY entity_type"
    )
    .fetch_all(pool)
    .await?;

    // Property keys per type
    let type_keys: Vec<(String, String)> = sqlx::query_as(
        "SELECT DISTINCT e.entity_type, ep.key \
         FROM entity_properties ep \
         JOIN entities e ON ep.entity_id = e.id \
         ORDER BY e.entity_type, ep.key"
    )
    .fetch_all(pool)
    .await?;

    // Sample entities (up to 5 per type)
    #[derive(sqlx::FromRow)]
    struct SampleRow {
        id: i64,
        entity_type: String,
        name: String,
        label: String,
    }
    let all_sample_rows: Vec<SampleRow> = sqlx::query_as(
        "SELECT id, entity_type, name, label FROM entities ORDER BY entity_type, sort_order, id"
    )
    .fetch_all(pool)
    .await?;

    let all_samples: Vec<(String, EntitySample)> = all_sample_rows.into_iter().map(|r| {
        (r.entity_type, EntitySample { id: r.id, name: r.name, label: r.label })
    }).collect();

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

/// Get summaries of relation types: usage counts and source->target patterns.
pub async fn find_relation_type_summaries(pool: &PgPool) -> Result<Vec<RelationTypeSummary>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct RelTypeSummaryRow {
        name: String,
        label: String,
        usage_count: i64,
    }
    let rows: Vec<RelTypeSummaryRow> = sqlx::query_as(
        "SELECT rt.name, rt.label, COUNT(r.id) AS usage_count \
         FROM entities rt \
         LEFT JOIN relations r ON r.relation_type_id = rt.id \
         WHERE rt.entity_type = 'relation_type' \
         GROUP BY rt.id, rt.name, rt.label \
         ORDER BY rt.name"
    )
    .fetch_all(pool)
    .await?;

    let mut summaries: Vec<RelationTypeSummary> = rows.into_iter().map(|r| {
        RelationTypeSummary {
            name: r.name,
            label: r.label,
            usage_count: r.usage_count,
            patterns: vec![],
        }
    }).collect();

    #[derive(sqlx::FromRow)]
    struct PatternRow {
        name: String,
        source_type: String,
        target_type: String,
        count: i64,
    }
    let pattern_rows: Vec<PatternRow> = sqlx::query_as(
        "SELECT rt.name, src.entity_type AS source_type, tgt.entity_type AS target_type, COUNT(*) AS count \
         FROM relations r \
         JOIN entities rt ON r.relation_type_id = rt.id \
         JOIN entities src ON r.source_id = src.id \
         JOIN entities tgt ON r.target_id = tgt.id \
         GROUP BY rt.name, src.entity_type, tgt.entity_type \
         ORDER BY rt.name"
    )
    .fetch_all(pool)
    .await?;

    let patterns: Vec<(String, RelationPattern)> = pattern_rows.into_iter().map(|r| {
        (r.name, RelationPattern {
            source_type: r.source_type,
            target_type: r.target_type,
            count: r.count,
        })
    }).collect();

    for summary in &mut summaries {
        summary.patterns = patterns.iter()
            .filter(|(name, _)| name == &summary.name)
            .map(|(_, p)| p.clone())
            .collect();
    }

    Ok(summaries)
}
