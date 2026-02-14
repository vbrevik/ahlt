/// For the ToR list page.
#[derive(Debug, Clone)]
pub struct TorListItem {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub description: String,
    pub status: String,
    pub meeting_cadence: String,
    pub member_count: i64,
    pub function_count: i64,
}

/// For ToR detail/edit.
#[derive(Debug, Clone)]
pub struct TorDetail {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub description: String,
    pub status: String,
    pub meeting_cadence: String,
    pub cadence_day: String,
    pub cadence_time: String,
    pub cadence_duration_minutes: String,
    pub default_location: String,
    pub remote_url: String,
    pub background_repo_url: String,
}

/// A member of a ToR with their function(s).
#[derive(Debug, Clone)]
pub struct TorMember {
    pub user_id: i64,
    pub user_name: String,
    pub user_label: String,
    pub functions: Vec<TorFunctionRef>,
}

/// A function assigned to a member (lightweight reference).
#[derive(Debug, Clone)]
pub struct TorFunctionRef {
    pub id: i64,
    pub name: String,
    pub label: String,
}

/// A tor_function entity with its authority properties.
#[derive(Debug, Clone)]
pub struct TorFunctionDetail {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub description: String,
    pub category: String,
    pub can_review_suggestions: bool,
    pub can_create_proposals: bool,
    pub can_approve_proposals: bool,
    pub can_manage_agenda: bool,
    pub can_record_decisions: bool,
    pub can_call_meetings: bool,
}

/// For the function list on the ToR detail page.
#[derive(Debug, Clone)]
pub struct TorFunctionListItem {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub category: String,
    pub assigned_to: Vec<String>,
}
