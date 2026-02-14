use serde::{Deserialize, Serialize};

/// Proposal as shown in the pipeline list view.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Full proposal detail.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub related_suggestion_id: Option<String>,
    pub csrf_token: String,
}
