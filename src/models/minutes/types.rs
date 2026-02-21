#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Minutes {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub status: String,         // "draft", "pending_approval", "approved"
    pub generated_date: String, // ISO-8601
    pub meeting_id: i64,
    pub meeting_name: String,
    pub approved_by: String,
    pub approved_date: String,
    pub distribution_list: String,       // JSON: ["name/email"]
    pub structured_attendance: String,   // JSON: [{user_id, name, status, delegation_to}]
    pub structured_action_items: String, // JSON: [{description, responsible, due_date, status}]
}

#[derive(Debug, Clone)]
pub struct AttendanceEntry {
    pub name: String,
    pub status: String,        // "present" | "absent" | "excused"
    pub delegation_to: String,
}

#[derive(Debug, Clone)]
pub struct ActionItem {
    pub description: String,
    pub responsible: String,
    pub due_date: String,
    pub status: String,        // "open" | "in_progress" | "done"
}

impl Minutes {
    pub fn distribution_items(&self) -> Vec<String> {
        serde_json::from_str(&self.distribution_list).unwrap_or_default()
    }

    pub fn attendance_list(&self) -> Vec<AttendanceEntry> {
        let raw: Vec<serde_json::Value> = serde_json::from_str(&self.structured_attendance)
            .unwrap_or_default();
        raw.into_iter().filter_map(|v| {
            Some(AttendanceEntry {
                name: v.get("name")?.as_str()?.to_string(),
                status: v.get("status")?.as_str()?.to_string(),
                delegation_to: v.get("delegation_to")
                    .and_then(|s| s.as_str()).unwrap_or("").to_string(),
            })
        }).collect()
    }

    pub fn action_items_list(&self) -> Vec<ActionItem> {
        let raw: Vec<serde_json::Value> = serde_json::from_str(&self.structured_action_items)
            .unwrap_or_default();
        raw.into_iter().filter_map(|v| {
            Some(ActionItem {
                description: v.get("description")?.as_str()?.to_string(),
                responsible: v.get("responsible")?.as_str()?.to_string(),
                due_date: v.get("due_date").and_then(|s| s.as_str()).unwrap_or("").to_string(),
                status: v.get("status")?.as_str()?.to_string(),
            })
        }).collect()
    }
}

#[derive(Debug, Clone)]
pub struct MinutesSection {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub section_type: String, // "attendance", "protocol", "agenda_items", "decisions", "action_items"
    pub sequence_order: i64,
    pub content: String,
    pub is_auto_generated: bool,
}
