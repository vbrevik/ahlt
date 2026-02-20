use serde::{Deserialize, Serialize};

/// Agenda point as shown in the workflow list view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgendaPointListItem {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub scheduled_date: String,
    pub item_type: String,  // "informative" or "decision"
    pub tor_id: i64,
}

/// Agenda point as shown in the cross-ToR workflow index view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossTorAgendaItem {
    pub tor_id: i64,
    pub tor_name: String,
    pub id: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub scheduled_date: String,
    pub item_type: String,
}

/// Full agenda point detail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgendaPointDetail {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub item_type: String,
    pub tor_id: i64,
    pub created_by: i64,
    pub created_date: String,
    pub scheduled_date: String,
    pub time_allocation_minutes: i32,
    pub coa_ids: Vec<i64>,  // Related COAs for decision items
    pub presenter: String,
    pub priority: String,   // "normal", "high", "urgent"
    pub pre_read_url: String,
}

/// Form input for creating/editing an agenda point.
#[derive(Debug, Clone, Deserialize)]
pub struct AgendaPointForm {
    pub title: String,
    pub description: String,
    pub item_type: String,
    pub scheduled_date: String,
    pub time_allocation_minutes: String,
    pub csrf_token: String,
    pub presenter: Option<String>,
    pub priority: Option<String>,
    pub pre_read_url: Option<String>,
}
