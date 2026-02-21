/// For the meeting list page.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MeetingListItem {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub meeting_date: String,
    pub status: String,
    pub tor_id: i64,
    pub tor_name: String,
    pub tor_label: String,
    pub agenda_count: i64,
    pub has_minutes: bool,
}

/// For meeting detail/edit.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MeetingDetail {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub meeting_date: String,
    pub status: String,
    pub location: String,
    pub notes: String,
    pub tor_id: i64,
    pub tor_name: String,
    pub tor_label: String,
    pub meeting_number: String,
    pub classification: String,
    pub vtc_details: String,
    pub chair_user_id: String,
    pub secretary_user_id: String,
    pub roll_call_data: String,    // JSON: [{username, status}]
}

/// A single roll call entry parsed from roll_call_data JSON.
#[derive(Debug, Clone)]
pub struct RollCallEntry {
    pub username: String,
    pub status: String,      // "present" | "absent" | "excused"
}

impl MeetingDetail {
    fn parse_roll_call(json: &str) -> Vec<RollCallEntry> {
        let raw: Vec<serde_json::Value> = serde_json::from_str(json).unwrap_or_default();
        raw.into_iter().filter_map(|v| {
            Some(RollCallEntry {
                username: v.get("username")?.as_str()?.to_string(),
                status: v.get("status")?.as_str()?.to_string(),
            })
        }).collect()
    }

    pub fn roll_call_list(&self) -> Vec<RollCallEntry> {
        Self::parse_roll_call(&self.roll_call_data)
    }
}
