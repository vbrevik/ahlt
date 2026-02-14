use serde::{Deserialize, Serialize};

/// A workflow status (state) for a given entity type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStatus {
    pub id: i64,
    pub entity_type_scope: String,
    pub status_code: String,
    pub label: String,
    pub is_initial: bool,
    pub is_terminal: bool,
}

/// A valid transition between two statuses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowTransition {
    pub id: i64,
    pub from_status_code: String,
    pub to_status_code: String,
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
