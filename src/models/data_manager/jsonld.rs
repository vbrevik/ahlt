use sqlx::PgPool;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

use super::export::export_entities;
use super::types::{ConflictMode, EntityImport, ImportPayload, RelationImport};

const AHLT_NS: &str = "http://ahlt.local/ontology/";

/// Build the JSON-LD @context dynamically from property keys and relation types in the DB.
pub async fn build_context(pool: &PgPool) -> Result<Value, sqlx::Error> {
    let mut ctx = Map::new();
    ctx.insert("ahlt".to_string(), json!(AHLT_NS));
    ctx.insert("rdf".to_string(), json!("http://www.w3.org/1999/02/22-rdf-syntax-ns#"));

    // Collect all unique property keys
    let keys: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT key FROM entity_properties ORDER BY key",
    )
    .fetch_all(pool)
    .await?;
    for (k,) in keys {
        ctx.insert(k.clone(), json!(format!("ahlt:{}", k)));
    }

    // Collect all relation type names
    let rels: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM entities WHERE entity_type = 'relation_type' ORDER BY name",
    )
    .fetch_all(pool)
    .await?;
    for (r,) in rels {
        ctx.entry(r.clone()).or_insert_with(|| json!(format!("ahlt:{}", r)));
    }

    Ok(Value::Object(ctx))
}

/// Export the entity graph as JSON-LD with @context and @graph.
pub async fn export_jsonld(
    pool: &PgPool,
    types: Option<&[String]>,
) -> Result<Value, sqlx::Error> {
    let payload = export_entities(pool, types).await?;
    let context = build_context(pool).await?;

    // Build a lookup from "type:name" -> list of relation predicates with targets
    let mut relations_by_source: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for r in &payload.relations {
        let target_iri = ref_to_iri(&r.target);
        relations_by_source
            .entry(r.source.clone())
            .or_default()
            .push((r.relation_type.clone(), target_iri));
    }

    let mut graph: Vec<Value> = Vec::new();

    for entity in &payload.entities {
        let mut node = Map::new();
        let ref_str = format!("{}:{}", entity.entity_type, entity.name);
        let iri = format!("ahlt:{}/{}", entity.entity_type, entity.name);

        node.insert("@id".to_string(), json!(iri));
        node.insert(
            "@type".to_string(),
            json!(format!("ahlt:{}", entity_type_to_class(&entity.entity_type))),
        );
        node.insert("ahlt:label".to_string(), json!(entity.label));

        if entity.sort_order != 0 {
            node.insert("ahlt:sort_order".to_string(), json!(entity.sort_order));
        }

        // Properties as literal predicates
        for (key, value) in &entity.properties {
            node.insert(format!("ahlt:{}", key), json!(value));
        }

        // Relations as IRI-valued predicates
        if let Some(rels) = relations_by_source.get(&ref_str) {
            // Group by predicate since a node may have multiple relations of the same type
            let mut rel_groups: HashMap<&str, Vec<&str>> = HashMap::new();
            for (pred, target_iri) in rels {
                rel_groups.entry(pred).or_default().push(target_iri);
            }
            for (pred, targets) in rel_groups {
                let key = format!("ahlt:{}", pred);
                if targets.len() == 1 {
                    node.insert(key, json!({"@id": targets[0]}));
                } else {
                    let arr: Vec<Value> = targets.iter().map(|t| json!({"@id": t})).collect();
                    node.insert(key, Value::Array(arr));
                }
            }
        }

        graph.push(Value::Object(node));
    }

    Ok(json!({
        "@context": context,
        "@graph": graph
    }))
}

/// Parse a JSON-LD document back into an ImportPayload.
pub fn parse_jsonld(value: &Value) -> Result<ImportPayload, String> {
    let obj = value.as_object().ok_or("JSON-LD must be an object")?;

    // Extract conflict_mode if present
    let conflict_mode = obj
        .get("ahlt:conflict_mode")
        .and_then(|v| v.as_str())
        .map(|s| match s {
            "upsert" => ConflictMode::Upsert,
            "fail" => ConflictMode::Fail,
            _ => ConflictMode::Skip,
        })
        .unwrap_or_default();

    let graph = obj
        .get("@graph")
        .and_then(|v| v.as_array())
        .ok_or("JSON-LD must contain @graph array")?;

    // Build the context map for resolving compact IRIs
    let context = obj.get("@context").and_then(|v| v.as_object());

    let mut entities: Vec<EntityImport> = Vec::new();
    let mut relations: Vec<RelationImport> = Vec::new();

    // First pass: collect all entity @ids for distinguishing relations from properties
    let mut known_ids: HashSet<String> = HashSet::new();
    for node in graph {
        if let Some(id) = node.get("@id").and_then(|v| v.as_str()) {
            known_ids.insert(id.to_string());
        }
    }

    // Collect all relation type names for distinguishing relation predicates
    let mut relation_type_names: HashSet<String> = HashSet::new();
    if let Some(ctx) = context {
        // We can't fully distinguish here without the DB, so we rely on the value shape:
        // - Relations have {"@id": "..."} values (IRI references)
        // - Properties have string/number literal values
        let _ = ctx; // Context is available but we use value shape for detection
    }

    for node in graph {
        let node_obj = node.as_object().ok_or("Each @graph item must be an object")?;

        let id_iri = node_obj
            .get("@id")
            .and_then(|v| v.as_str())
            .ok_or("Each node must have @id")?;

        let (entity_type, name) = iri_to_type_name(id_iri)?;

        let label = node_obj
            .get("ahlt:label")
            .and_then(|v| v.as_str())
            .unwrap_or(&name)
            .to_string();

        let sort_order = node_obj
            .get("ahlt:sort_order")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let mut properties: HashMap<String, String> = HashMap::new();

        for (key, val) in node_obj {
            // Skip JSON-LD keywords and metadata fields
            if key.starts_with('@') || key == "ahlt:label" || key == "ahlt:sort_order" {
                continue;
            }

            let prop_name = strip_ahlt_prefix(key);

            // Check if value is an IRI reference (relation) or literal (property)
            if let Some(ref_obj) = val.as_object() {
                // Single relation: {"@id": "ahlt:type/name"}
                if let Some(target_iri) = ref_obj.get("@id").and_then(|v| v.as_str()) {
                    let (target_type, target_name) = iri_to_type_name(target_iri)?;
                    relation_type_names.insert(prop_name.clone());
                    relations.push(RelationImport {
                        relation_type: prop_name,
                        source: format!("{}:{}", entity_type, name),
                        target: format!("{}:{}", target_type, target_name),
                        properties: HashMap::new(),
                    });
                    continue;
                }
            } else if let Some(arr) = val.as_array() {
                // Multiple relations of the same type: [{"@id": "..."}, {"@id": "..."}]
                let mut all_refs = true;
                for item in arr {
                    if item.get("@id").is_none() {
                        all_refs = false;
                        break;
                    }
                }
                if all_refs && !arr.is_empty() {
                    for item in arr {
                        if let Some(target_iri) = item.get("@id").and_then(|v| v.as_str()) {
                            let (target_type, target_name) = iri_to_type_name(target_iri)?;
                            relation_type_names.insert(prop_name.clone());
                            relations.push(RelationImport {
                                relation_type: prop_name.clone(),
                                source: format!("{}:{}", entity_type, name),
                                target: format!("{}:{}", target_type, target_name),
                                properties: HashMap::new(),
                            });
                        }
                    }
                    continue;
                }
            }

            // It's a literal property
            let str_val = match val {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                other => other.to_string(),
            };
            properties.insert(prop_name, str_val);
        }

        entities.push(EntityImport {
            entity_type,
            name,
            label,
            sort_order,
            properties,
        });
    }

    Ok(ImportPayload {
        conflict_mode,
        entities,
        relations,
    })
}

/// Convert entity_type to PascalCase class name.
/// e.g. "tor" -> "Tor", "tor_function" -> "TorFunction", "workflow_status" -> "WorkflowStatus"
fn entity_type_to_class(entity_type: &str) -> String {
    entity_type
        .split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    format!("{}{}", upper, chars.as_str())
                }
                None => String::new(),
            }
        })
        .collect()
}

/// Parse an IRI like "ahlt:tor/budget_committee" into ("tor", "budget_committee").
fn iri_to_type_name(iri: &str) -> Result<(String, String), String> {
    let path = iri
        .strip_prefix("ahlt:")
        .or_else(|| iri.strip_prefix(AHLT_NS))
        .ok_or_else(|| format!("IRI does not start with ahlt: or full namespace: {}", iri))?;

    path.split_once('/')
        .map(|(t, n)| (t.to_string(), n.to_string()))
        .ok_or_else(|| format!("IRI missing type/name separator: {}", iri))
}

/// Convert an "entity_type:name" ref to an IRI.
fn ref_to_iri(ref_str: &str) -> String {
    match ref_str.split_once(':') {
        Some((t, n)) => format!("ahlt:{}/{}", t, n),
        None => format!("ahlt:unknown/{}", ref_str),
    }
}

/// Strip "ahlt:" prefix from a property key.
fn strip_ahlt_prefix(key: &str) -> String {
    key.strip_prefix("ahlt:")
        .unwrap_or(key)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_type_to_class() {
        assert_eq!(entity_type_to_class("tor"), "Tor");
        assert_eq!(entity_type_to_class("tor_function"), "TorFunction");
        assert_eq!(entity_type_to_class("workflow_status"), "WorkflowStatus");
        assert_eq!(entity_type_to_class("user"), "User");
    }

    #[test]
    fn test_iri_to_type_name() {
        assert_eq!(
            iri_to_type_name("ahlt:tor/budget_committee").unwrap(),
            ("tor".to_string(), "budget_committee".to_string())
        );
        assert_eq!(
            iri_to_type_name("ahlt:user/alice").unwrap(),
            ("user".to_string(), "alice".to_string())
        );
        assert!(iri_to_type_name("invalid:thing").is_err());
    }

    #[test]
    fn test_strip_ahlt_prefix() {
        assert_eq!(strip_ahlt_prefix("ahlt:status"), "status");
        assert_eq!(strip_ahlt_prefix("plain_key"), "plain_key");
    }
}
