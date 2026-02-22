use askama::Template;

use crate::models::proposal::{ProposalDetail};
use super::PageContext;

#[derive(Template)]
#[template(path = "proposals/form.html")]
pub struct ProposalFormTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub tor_name: String,
    pub form_action: String,
    pub form_title: String,
    pub proposal: Option<ProposalDetail>,
    pub errors: Vec<String>,
}

#[derive(Template)]
#[template(path = "proposals/detail.html")]
pub struct ProposalDetailTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub proposal: ProposalDetail,
}
