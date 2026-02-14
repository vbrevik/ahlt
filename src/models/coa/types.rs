use serde::{Deserialize, Serialize};

/// Course of Action as shown in the list view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoaListItem {
    pub id: i64,
    pub title: String,
    pub coa_type: String,  // "simple" or "complex"
    pub created_by: i64,
    pub created_date: String,
}

/// Full COA detail with nested sections (for complex COAs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoaDetail {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub coa_type: String,  // "simple" or "complex"
    pub created_by: i64,
    pub created_date: String,
    pub sections: Vec<CoaSection>,  // Nested sections for complex COAs
}

/// Section within a COA (for complex COAs with nesting).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoaSection {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub order: i32,
    pub subsections: Vec<CoaSubsection>,
}

/// Subsection nested within a section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoaSubsection {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub order: i32,
}

/// Form input for creating/editing a COA.
#[derive(Debug, Clone, Deserialize)]
pub struct CoaForm {
    pub title: String,
    pub description: String,
    pub coa_type: String,  // "simple" or "complex"
    pub csrf_token: String,
}
