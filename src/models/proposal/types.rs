use serde::{Deserialize, Serialize};

/// Proposal as shown in the workflow list view.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProposalListItem {
    pub id: i64,
    pub title: String,
    pub submitted_by_id: i64,
    pub submitted_by_name: String,
    pub submitted_date: String,
    pub status: String,
    pub rejection_reason: Option<String>,
    pub related_suggestion_id: Option<i64>,
}

/// Proposal as shown in the cross-ToR workflow index view.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CrossTorProposalItem {
    pub tor_id: i64,
    pub tor_name: String,
    pub id: i64,
    pub title: String,
    pub submitted_by_id: i64,
    pub submitted_by_name: String,
    pub submitted_date: String,
    pub status: String,
    pub rejection_reason: Option<String>,
    pub related_suggestion_id: Option<i64>,
}

/// Full proposal detail.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProposalDetail {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub rationale: String,
    pub submitted_by_id: i64,
    pub submitted_by_name: String,
    pub submitted_date: String,
    pub status: String,
    pub rejection_reason: Option<String>,
    pub related_suggestion_id: Option<i64>,
}

/// Form input for creating/editing a proposal.
#[derive(Debug, Clone, Deserialize)]
pub struct ProposalForm {
    pub title: String,
    pub description: String,
    pub rationale: String,
    #[allow(dead_code)]
    pub related_suggestion_id: Option<String>,
    pub csrf_token: String,
}
