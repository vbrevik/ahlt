use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// How to handle conflicts when an entity with the same type+name already exists.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ConflictMode {
    Skip,
    Upsert,
    Fail,
}

impl Default for ConflictMode {
    fn default() -> Self {
        ConflictMode::Skip
    }
}

// ── Import types ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPayload {
    #[serde(default)]
    pub conflict_mode: ConflictMode,
    #[serde(default)]
    pub entities: Vec<EntityImport>,
    #[serde(default)]
    pub relations: Vec<RelationImport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityImport {
    pub entity_type: String,
    pub name: String,
    pub label: String,
    #[serde(default)]
    pub sort_order: i64,
    #[serde(default)]
    pub properties: HashMap<String, String>,
}

/// Relations reference entities by "type:name" strings, not numeric IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationImport {
    pub relation_type: String,
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub created: usize,
    pub updated: usize,
    pub skipped: usize,
    pub errors: Vec<ImportError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportError {
    pub item: serde_json::Value,
    pub reason: String,
}

// ── Export types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportPayload {
    pub entities: Vec<EntityExport>,
    pub relations: Vec<RelationExport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityExport {
    pub id: i64,
    pub entity_type: String,
    pub name: String,
    pub label: String,
    pub sort_order: i64,
    pub properties: HashMap<String, String>,
}

/// Exported relations use "type:name" strings for all references.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationExport {
    pub id: i64,
    pub relation_type: String,
    pub source: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub properties: HashMap<String, String>,
}
