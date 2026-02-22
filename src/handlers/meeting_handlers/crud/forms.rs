/// Form structures for meeting CRUD operations.
///
/// Centralizes all form deserialize structs to keep handler files focused
/// on business logic rather than form definitions.

#[derive(serde::Deserialize)]
pub struct ConfirmForm {
    pub csrf_token: String,
    pub meeting_date: String,
    pub tor_name: String,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub meeting_number: Option<String>,
    pub classification: Option<String>,
    pub vtc_details: Option<String>,
    pub chair_user_id: Option<String>,
    pub secretary_user_id: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct CalendarConfirmForm {
    pub csrf_token: String,
    pub meeting_date: String,
    pub tor_name: String,
    pub meeting_id: Option<i64>,
}

#[derive(serde::Deserialize)]
pub struct TransitionForm {
    pub csrf_token: String,
    pub new_status: String,
}

#[derive(serde::Deserialize)]
pub struct AgendaForm {
    pub csrf_token: String,
    pub agenda_point_id: i64,
}

#[derive(serde::Deserialize)]
pub struct CsrfOnly {
    pub csrf_token: String,
}

#[derive(serde::Deserialize)]
pub struct RollCallForm {
    pub csrf_token: String,
    pub roll_call_data: String, // raw JSON string from hidden input
}
