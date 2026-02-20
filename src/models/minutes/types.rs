#[derive(Debug, Clone)]
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
