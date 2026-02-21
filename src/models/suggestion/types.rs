use serde::{Deserialize, Serialize};

/// Suggestion as shown in the workflow list view.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SuggestionListItem {
    pub id: i64,
    pub description: String,
    pub description_preview: String,
    pub submitted_by_id: i64,
    pub submitted_by_name: String,
    pub submitted_date: String,
    pub status: String,
    pub rejection_reason: Option<String>,
    pub spawned_proposal_id: Option<i64>,
}

/// Suggestion as shown in the cross-ToR workflow index view.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CrossTorSuggestionItem {
    pub tor_id: i64,
    pub tor_name: String,
    pub id: i64,
    pub description: String,
    pub description_preview: String,
    pub submitted_by_id: i64,
    pub submitted_by_name: String,
    pub submitted_date: String,
    pub status: String,
    pub rejection_reason: Option<String>,
    pub spawned_proposal_id: Option<i64>,
}

/// Full suggestion detail.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SuggestionDetail {
    pub id: i64,
    pub description: String,
    pub submitted_by_id: i64,
    pub submitted_by_name: String,
    pub submitted_date: String,
    pub status: String,
    pub rejection_reason: Option<String>,
    pub spawned_proposal_id: Option<i64>,
}

/// Form input for creating a suggestion.
#[derive(Debug, Clone, Deserialize)]
pub struct SuggestionForm {
    pub description: String,
    pub csrf_token: String,
}
