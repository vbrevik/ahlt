/// For the ToR list page.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TorListItem {
    pub id: i64,
    pub name: String,
    pub label: String,
    #[allow(dead_code)]
    pub description: String,
    pub status: String,
    pub meeting_cadence: String,
    pub member_count: i64,
    pub function_count: i64,
}

/// For ToR detail/edit.
#[derive(Debug, Clone, sqlx::FromRow)]
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
    // Identity
    pub tor_number: String,
    pub classification: String,
    pub version: String,
    pub organization: String,
    // Purpose
    pub focus_scope: String,
    pub objectives: String,        // JSON array string
    // Governance
    pub inputs_required: String,   // JSON array string
    pub outputs_expected: String,  // JSON array string
    pub poc_contact: String,
    // Operational
    pub phase_scheduling: String,
    pub info_platform: String,
    pub invite_policy: String,
}

impl TorDetail {
    /// Parse a JSON array property into a Vec for template iteration.
    fn parse_json_list(json: &str) -> Vec<String> {
        serde_json::from_str(json).unwrap_or_default()
    }

    /// Objectives as a list for display.
    pub fn objectives_list(&self) -> Vec<String> {
        Self::parse_json_list(&self.objectives)
    }

    /// Inputs required as a list for display.
    pub fn inputs_required_list(&self) -> Vec<String> {
        Self::parse_json_list(&self.inputs_required)
    }

    /// Outputs expected as a list for display.
    pub fn outputs_expected_list(&self) -> Vec<String> {
        Self::parse_json_list(&self.outputs_expected)
    }

    /// Convert a JSON array back to newline-separated text for textarea repopulation.
    fn json_to_lines(json: &str) -> String {
        let items: Vec<String> = serde_json::from_str(json).unwrap_or_default();
        items.join("\n")
    }

    pub fn objectives_text(&self) -> String { Self::json_to_lines(&self.objectives) }
    pub fn inputs_required_text(&self) -> String { Self::json_to_lines(&self.inputs_required) }
    pub fn outputs_expected_text(&self) -> String { Self::json_to_lines(&self.outputs_expected) }
}

/// A position in a ToR with its current holder (if any).
/// Position-based: authority flows from position, not person.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TorMember {
    pub position_id: i64,
    pub position_name: String,
    pub position_label: String,
    pub membership_type: String, // "mandatory" or "optional"
    pub holder_id: Option<i64>,
    pub holder_name: Option<String>,
    pub holder_label: Option<String>,
}

/// A function assigned to a member (lightweight reference).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TorFunctionRef {
    #[allow(dead_code)]
    pub id: i64,
    #[allow(dead_code)]
    pub name: String,
    pub label: String,
}

/// A tor_function entity with its authority properties.
#[allow(dead_code)]
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
    #[allow(dead_code)]
    pub id: i64,
    pub name: String,
    pub label: String,
    pub category: String,
    pub assigned_to: Vec<String>,
}
