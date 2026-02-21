use serde::{Deserialize, Serialize};

/// Document as shown in the list view.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DocumentListItem {
    pub id: i64,
    pub title: String,
    pub doc_type: String,
    pub created_by_id: i64,
    pub created_by_name: String,
    pub created_date: String,
    pub tor_id: Option<i64>,
    pub tor_name: Option<String>,
}

/// Full document detail.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DocumentDetail {
    pub id: i64,
    pub title: String,
    pub doc_type: String,
    pub body: String,
    pub created_by_id: i64,
    pub created_by_name: String,
    pub created_date: String,
    pub updated_date: String,  // Empty string if not updated
    pub tor_id: i64,  // 0 if not scoped to a ToR
    pub tor_name: String,  // Empty string if not scoped to a ToR
}

/// Form input for creating/editing a document.
#[derive(Debug, Clone, Deserialize)]
pub struct DocumentForm {
    pub title: String,
    pub doc_type: String,
    pub body: String,
    #[serde(default)]
    pub tor_id: Option<String>,
    pub csrf_token: String,
}
