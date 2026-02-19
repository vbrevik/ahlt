/// For the meeting list page.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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
}
