use askama::Template;

use crate::models::opinion::OpinionDetail;
use crate::models::coa::CoaListItem;
use super::PageContext;
use super::tor::UserOption;

#[derive(Template)]
#[template(path = "opinion/form.html")]
pub struct OpinionFormTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub agenda_point_id: i64,
    pub coas: Vec<CoaListItem>,
    pub existing_opinion: Option<OpinionDetail>,
    pub errors: Vec<String>,
}

#[derive(Template)]
#[template(path = "agenda/decision_form.html")]
pub struct DecisionFormTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub agenda_point: crate::models::agenda_point::AgendaPointDetail,
    pub coas: Vec<crate::models::coa::CoaDetail>,
    pub opinions: Vec<crate::models::opinion::OpinionSummary>,
    pub errors: Vec<String>,
}
