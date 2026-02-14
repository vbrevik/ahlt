use serde::{Deserialize, Serialize};

/// Opinion as shown in the list view for an agenda point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpinionListItem {
    pub id: i64,
    pub recorded_by: i64,
    pub recorded_by_name: String,
    pub preferred_coa_id: i64,
    pub commentary: String,
    pub created_date: String,
}

/// Full opinion detail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpinionDetail {
    pub id: i64,
    pub agenda_point_id: i64,
    pub recorded_by: i64,
    pub recorded_by_name: String,
    pub preferred_coa_id: i64,
    pub coa_title: String,
    pub commentary: String,
    pub created_date: String,
}

/// Form input for recording an opinion on an agenda item.
#[derive(Debug, Clone, Deserialize)]
pub struct OpinionForm {
    pub preferred_coa_id: i64,
    pub commentary: String,
    pub csrf_token: String,
}

/// Summary of opinions grouped by COA preference for an agenda point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpinionSummary {
    pub coa_id: i64,
    pub coa_title: String,
    pub preference_count: i32,
    pub opinions: Vec<OpinionListItem>,
}

/// Decision record - the final decision made by decision authority on an agenda item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    pub id: i64,
    pub agenda_point_id: i64,
    pub decided_by: i64,
    pub decided_by_name: String,
    pub selected_coa_id: i64,
    pub decision_rationale: String,
    pub decided_date: String,
    pub opinion_count: i32,
    pub opinions_summary: String,  // e.g., "3 preferred COA#1, 2 preferred COA#2"
}
