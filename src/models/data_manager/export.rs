use rusqlite::Connection;
use std::collections::HashMap;

use super::types::{EntityExport, ExportPayload, RelationExport};

/// Build a lookup map of entity ID -> "entity_type:name" for resolving relations.
fn build_entity_ref_map(conn: &Connection) -> Result<HashMap<i64, String>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT id, entity_type, name FROM entities")?;
    let rows = stmt.query_map([], |row| {
        let id: i64 = row.get(0)?;
        let entity_type: String = row.get(1)?;
        let name: String = row.get(2)?;
        Ok((id, format!("{}:{}", entity_type, name)))
    })?;

    let mut map = HashMap::new();
    for row in rows {
        let (id, ref_str) = row?;
        map.insert(id, ref_str);
    }
    Ok(map)
}

/// Query entities into a vec, with optional entity_type filter.
fn query_entities(conn: &Connection, types: Option<&[String]>) -> Result<Vec<EntityExport>, rusqlite::Error> {
    let (sql, params) = match types {
        Some(ts) if !ts.is_empty() => {
            let placeholders: Vec<String> = (1..=ts.len()).map(|i| format!("?{}", i)).collect();
            (
                format!(
                    "SELECT id, entity_type, name, label, sort_order FROM entities WHERE entity_type IN ({}) ORDER BY id",
                    placeholders.join(", ")
                ),
                ts.to_vec(),
            )
        }
        _ => (
            "SELECT id, entity_type, name, label, sort_order FROM entities ORDER BY id".to_string(),
            vec![],
        ),
    };

    let mut stmt = conn.prepare(&sql)?;
    let params_ref: Vec<&dyn rusqlite::types::ToSql> =
        params.iter().map(|s| s as &dyn rusqlite::types::ToSql).collect();

    let rows = stmt.query_map(params_ref.as_slice(), |row| {
        Ok(EntityExport {
            id: row.get(0)?,
            entity_type: row.get(1)?,
            name: row.get(2)?,
            label: row.get(3)?,
            sort_order: row.get(4)?,
            properties: HashMap::new(),
        })
    })?;

    rows.collect()
}

/// Export entities with their properties, optionally filtered by entity type.
pub fn export_entities(
    conn: &Connection,
    types: Option<&[String]>,
) -> Result<ExportPayload, rusqlite::Error> {
    let ref_map = build_entity_ref_map(conn)?;

    let mut entities = query_entities(conn, types)?;
    let entity_ids: Vec<i64> = entities.iter().map(|e| e.id).collect();

    // Batch-load all properties for matched entities (avoid N+1)
    if !entity_ids.is_empty() {
        let mut prop_stmt =
            conn.prepare("SELECT entity_id, key, value FROM entity_properties ORDER BY entity_id")?;
        let prop_rows = prop_stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut props_map: HashMap<i64, HashMap<String, String>> = HashMap::new();
        for row in prop_rows {
            let (entity_id, key, value) = row?;
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
    let mut rel_stmt = conn.prepare(
        "SELECT id, relation_type_id, source_id, target_id FROM relations ORDER BY id",
    )?;
    let rel_rows = rel_stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })?;

    // Build a set of entity IDs for filtering relations when type filter is active
    let entity_id_set: std::collections::HashSet<i64> = entity_ids.iter().cloned().collect();

    // Batch-load all relation_properties
    let mut rp_stmt = conn.prepare(
        "SELECT relation_id, key, value FROM relation_properties ORDER BY relation_id",
    )?;
    let rp_rows = rp_stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut rel_props_map: HashMap<i64, HashMap<String, String>> = HashMap::new();
    for row in rp_rows {
        let (rel_id, key, value) = row?;
        rel_props_map.entry(rel_id).or_default().insert(key, value);
    }

    for row in rel_rows {
        let (id, rel_type_id, source_id, target_id) = row?;

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
pub fn export_sql(
    conn: &Connection,
    types: Option<&[String]>,
) -> Result<String, rusqlite::Error> {
    let payload = export_entities(conn, types)?;
    let mut sql = String::new();

    sql.push_str("-- Data Manager export\n");
    sql.push_str("-- Generated by ahlt\n\n");

    // Entities
    sql.push_str("-- Entities\n");
    for e in &payload.entities {
        sql.push_str(&format!(
            "INSERT OR IGNORE INTO entities (entity_type, name, label, sort_order) VALUES ('{}', '{}', '{}', {});\n",
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
                "INSERT OR IGNORE INTO entity_properties (entity_id, key, value) VALUES ((SELECT id FROM entities WHERE entity_type='{}' AND name='{}'), '{}', '{}');\n",
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
            "INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id) VALUES (\
            (SELECT id FROM entities WHERE entity_type='relation_type' AND name='{}'), \
            (SELECT id FROM entities WHERE entity_type='{}' AND name='{}'), \
            (SELECT id FROM entities WHERE entity_type='{}' AND name='{}'));\n",
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
                    "INSERT OR IGNORE INTO relation_properties (relation_id, key, value) VALUES (\
                    (SELECT id FROM relations WHERE relation_type_id=\
                    (SELECT id FROM entities WHERE entity_type='relation_type' AND name='{}') \
                    AND source_id=(SELECT id FROM entities WHERE entity_type='{}' AND name='{}') \
                    AND target_id=(SELECT id FROM entities WHERE entity_type='{}' AND name='{}')), \
                    '{}', '{}');\n",
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
