use sqlx::PgPool;
use std::collections::HashMap;

use super::types::{EntityExport, ExportPayload, RelationExport};

/// Build a lookup map of entity ID -> "entity_type:name" for resolving relations.
async fn build_entity_ref_map(pool: &PgPool) -> Result<HashMap<i64, String>, sqlx::Error> {
    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT id, entity_type, name FROM entities",
    )
    .fetch_all(pool)
    .await?;

    let mut map = HashMap::new();
    for (id, entity_type, name) in rows {
        map.insert(id, format!("{}:{}", entity_type, name));
    }
    Ok(map)
}

/// Query entities into a vec, with optional entity_type filter.
async fn query_entities(pool: &PgPool, types: Option<&[String]>) -> Result<Vec<EntityExport>, sqlx::Error> {
    match types {
        Some(ts) if !ts.is_empty() => {
            // Use ANY($1) with a slice instead of individual placeholders
            let rows: Vec<(i64, String, String, String, i64)> = sqlx::query_as(
                "SELECT id, entity_type, name, label, sort_order FROM entities WHERE entity_type = ANY($1) ORDER BY id",
            )
                .bind(ts)
                .fetch_all(pool)
                .await?;

            Ok(rows
                .into_iter()
                .map(|(id, entity_type, name, label, sort_order)| EntityExport {
                    id,
                    entity_type,
                    name,
                    label,
                    sort_order,
                    properties: HashMap::new(),
                })
                .collect())
        }
        _ => {
            let rows: Vec<(i64, String, String, String, i64)> = sqlx::query_as(
                "SELECT id, entity_type, name, label, sort_order FROM entities ORDER BY id",
            )
            .fetch_all(pool)
            .await?;

            Ok(rows
                .into_iter()
                .map(|(id, entity_type, name, label, sort_order)| EntityExport {
                    id,
                    entity_type,
                    name,
                    label,
                    sort_order,
                    properties: HashMap::new(),
                })
                .collect())
        }
    }
}

/// Export entities with their properties, optionally filtered by entity type.
pub async fn export_entities(
    pool: &PgPool,
    types: Option<&[String]>,
) -> Result<ExportPayload, sqlx::Error> {
    let ref_map = build_entity_ref_map(pool).await?;

    let mut entities = query_entities(pool, types).await?;
    let entity_ids: Vec<i64> = entities.iter().map(|e| e.id).collect();

    // Batch-load all properties for matched entities (avoid N+1)
    if !entity_ids.is_empty() {
        let prop_rows: Vec<(i64, String, String)> = sqlx::query_as(
            "SELECT entity_id, key, value FROM entity_properties ORDER BY entity_id",
        )
        .fetch_all(pool)
        .await?;

        let mut props_map: HashMap<i64, HashMap<String, String>> = HashMap::new();
        for (entity_id, key, value) in prop_rows {
            props_map.entry(entity_id).or_default().insert(key, value);
        }

        for entity in &mut entities {
            if let Some(props) = props_map.remove(&entity.id) {
                entity.properties = props;
            }
        }
    }

    // Query relations, resolving IDs to type:name via ref_map
    let mut relations: Vec<RelationExport> = Vec::new();
    let rel_rows: Vec<(i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT id, relation_type_id, source_id, target_id FROM relations ORDER BY id",
    )
    .fetch_all(pool)
    .await?;

    // Build a set of entity IDs for filtering relations when type filter is active
    let entity_id_set: std::collections::HashSet<i64> = entity_ids.iter().cloned().collect();

    // Batch-load all relation_properties
    let rp_rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT relation_id, key, value FROM relation_properties ORDER BY relation_id",
    )
    .fetch_all(pool)
    .await?;
    let mut rel_props_map: HashMap<i64, HashMap<String, String>> = HashMap::new();
    for (rel_id, key, value) in rp_rows {
        rel_props_map.entry(rel_id).or_default().insert(key, value);
    }

    for (id, rel_type_id, source_id, target_id) in rel_rows {
        // When filtering by type, only include relations where both source and target are in the set
        if types.is_some() && !types.unwrap().is_empty() {
            if !entity_id_set.contains(&source_id) || !entity_id_set.contains(&target_id) {
                continue;
            }
        }

        let relation_type = ref_map
            .get(&rel_type_id)
            .map(|r| {
                // Strip "relation_type:" prefix to get just the name
                r.strip_prefix("relation_type:").unwrap_or(r).to_string()
            })
            .unwrap_or_else(|| format!("unknown:{}", rel_type_id));
        let source = ref_map
            .get(&source_id)
            .cloned()
            .unwrap_or_else(|| format!("unknown:{}", source_id));
        let target = ref_map
            .get(&target_id)
            .cloned()
            .unwrap_or_else(|| format!("unknown:{}", target_id));

        let properties = rel_props_map.remove(&id).unwrap_or_default();

        relations.push(RelationExport {
            id,
            relation_type,
            source,
            target,
            properties,
        });
    }

    Ok(ExportPayload {
        entities,
        relations,
    })
}

/// Export the entity graph as SQL INSERT statements.
pub async fn export_sql(
    pool: &PgPool,
    types: Option<&[String]>,
) -> Result<String, sqlx::Error> {
    let payload = export_entities(pool, types).await?;
    let mut sql = String::new();

    sql.push_str("-- Data Manager export\n");
    sql.push_str("-- Generated by ahlt\n\n");

    // Entities
    sql.push_str("-- Entities\n");
    for e in &payload.entities {
        sql.push_str(&format!(
            "INSERT INTO entities (entity_type, name, label, sort_order) VALUES ('{}', '{}', '{}', {}) ON CONFLICT DO NOTHING;\n",
            escape_sql(&e.entity_type),
            escape_sql(&e.name),
            escape_sql(&e.label),
            e.sort_order
        ));
    }

    // Properties
    sql.push_str("\n-- Entity properties\n");
    for e in &payload.entities {
        for (key, value) in &e.properties {
            sql.push_str(&format!(
                "INSERT INTO entity_properties (entity_id, key, value) VALUES ((SELECT id FROM entities WHERE entity_type='{}' AND name='{}'), '{}', '{}') ON CONFLICT DO NOTHING;\n",
                escape_sql(&e.entity_type),
                escape_sql(&e.name),
                escape_sql(key),
                escape_sql(value)
            ));
        }
    }

    // Relations
    sql.push_str("\n-- Relations\n");
    for r in &payload.relations {
        let (src_type, src_name) = split_ref(&r.source);
        let (tgt_type, tgt_name) = split_ref(&r.target);
        sql.push_str(&format!(
            "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (\
            (SELECT id FROM entities WHERE entity_type='relation_type' AND name='{}'), \
            (SELECT id FROM entities WHERE entity_type='{}' AND name='{}'), \
            (SELECT id FROM entities WHERE entity_type='{}' AND name='{}')) ON CONFLICT DO NOTHING;\n",
            escape_sql(&r.relation_type),
            escape_sql(src_type),
            escape_sql(src_name),
            escape_sql(tgt_type),
            escape_sql(tgt_name)
        ));
    }

    // Relation properties
    let has_rel_props = payload.relations.iter().any(|r| !r.properties.is_empty());
    if has_rel_props {
        sql.push_str("\n-- Relation properties\n");
        for r in &payload.relations {
            if r.properties.is_empty() {
                continue;
            }
            let (src_type, src_name) = split_ref(&r.source);
            let (tgt_type, tgt_name) = split_ref(&r.target);
            for (key, value) in &r.properties {
                sql.push_str(&format!(
                    "INSERT INTO relation_properties (relation_id, key, value) VALUES (\
                    (SELECT id FROM relations WHERE relation_type_id=\
                    (SELECT id FROM entities WHERE entity_type='relation_type' AND name='{}') \
                    AND source_id=(SELECT id FROM entities WHERE entity_type='{}' AND name='{}') \
                    AND target_id=(SELECT id FROM entities WHERE entity_type='{}' AND name='{}')), \
                    '{}', '{}') ON CONFLICT DO NOTHING;\n",
                    escape_sql(&r.relation_type),
                    escape_sql(src_type), escape_sql(src_name),
                    escape_sql(tgt_type), escape_sql(tgt_name),
                    escape_sql(key), escape_sql(value)
                ));
            }
        }
    }

    Ok(sql)
}

/// Escape single quotes for SQL string literals.
fn escape_sql(s: &str) -> String {
    s.replace('\'', "''")
}

/// Split a "type:name" reference into (type, name).
fn split_ref(ref_str: &str) -> (&str, &str) {
    match ref_str.split_once(':') {
        Some((t, n)) => (t, n),
        None => ("unknown", ref_str),
    }
}
