use askama::Template;

use super::PageContext;

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub ctx: PageContext,
    pub role_label: String,
    pub greeting: String,
    // System stats (secondary)
    pub user_count: i64,
    pub role_count: i64,
    pub proposal_count: i64,
    pub tor_position_count: i64,
    pub audit_entry_count: i64,
    pub recent_activity: Vec<crate::models::audit::AuditEntry>,
    // Personalized data
    pub user_tors: Vec<crate::models::dashboard::UserTorMembership>,
    pub upcoming_meetings: Vec<crate::models::dashboard::UpcomingMeeting>,
    pub pending_items: crate::models::dashboard::PendingItems,
}
