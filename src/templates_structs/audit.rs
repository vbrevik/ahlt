use askama::Template;

use crate::models::audit::AuditEntryPage;
use super::PageContext;

#[derive(Template)]
#[template(path = "audit/list.html")]
pub struct AuditListTemplate {
    pub ctx: PageContext,
    pub audit_page: AuditEntryPage,
    pub search_query: Option<String>,
    pub action_filter: Option<String>,
    pub target_type_filter: Option<String>,
}
