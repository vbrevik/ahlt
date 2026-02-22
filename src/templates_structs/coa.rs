use askama::Template;

use crate::models::coa::CoaDetail;
use super::PageContext;

#[derive(Template)]
#[template(path = "coa/form.html")]
pub struct CoaFormTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub form_action: String,
    pub form_title: String,
    pub coa: Option<CoaDetail>,
    pub errors: Vec<String>,
}
