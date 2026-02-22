use askama::Template;

use crate::warnings::queries::{WarningPage, WarningDetail, WarningRecipient, WarningTimelineEvent};
use super::PageContext;
use super::tor::UserOption;

#[derive(Template)]
#[template(path = "warnings/list.html")]
pub struct WarningListTemplate {
    pub ctx: PageContext,
    pub warning_page: WarningPage,
    pub category_filter: Option<String>,
    pub severity_filter: Option<String>,
    pub show_read: bool,
    pub show_deleted: bool,
}

#[derive(Template)]
#[template(path = "warnings/detail.html")]
pub struct WarningDetailTemplate {
    pub ctx: PageContext,
    pub warning: WarningDetail,
    pub recipients: Vec<WarningRecipient>,
    pub timeline: Vec<WarningTimelineEvent>,
    pub user_receipt_id: i64,
    pub users: Vec<UserOption>,
}
