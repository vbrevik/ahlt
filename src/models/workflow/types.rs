use serde::{Deserialize, Serialize};

/// Summary of a workflow scope (e.g. "suggestion", "proposal") for the builder list page.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkflowScope {
    pub scope: String,
    pub status_count: i64,
    pub transition_count: i64,
}

/// A workflow status (state) for a given entity type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStatus {
    pub id: i64,
    pub name: String,
    pub entity_type_scope: String,
    pub status_code: String,
    pub label: String,
    pub order: i64,
    pub is_initial: bool,
    pub is_terminal: bool,
}

/// A valid transition between two statuses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTransition {
    pub id: i64,
    pub name: String,
    pub entity_type_scope: String,
    pub from_status_code: String,
    pub to_status_code: String,
    pub from_status_id: i64,
    pub to_status_id: i64,
    pub required_permission: String,
    pub condition: Option<String>,
    pub requires_outcome: bool,
    pub transition_label: String,
}

/// Information about an available transition for UI rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableTransition {
    pub to_status_code: String,
    pub transition_label: String,
    pub requires_outcome: bool,
}
