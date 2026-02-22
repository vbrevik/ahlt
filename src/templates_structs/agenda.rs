use askama::Template;

use crate::models::agenda_point::{AgendaPointDetail};
use crate::models::coa::CoaDetail;
use crate::models::opinion::OpinionSummary;
use crate::models::workflow::AvailableTransition;
use super::PageContext;

#[derive(Template)]
#[template(path = "agenda/form.html")]
pub struct AgendaPointFormTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub form_action: String,
    pub form_title: String,
    pub agenda_point: Option<AgendaPointDetail>,
    pub errors: Vec<String>,
}

#[derive(Template)]
#[template(path = "agenda/detail.html")]
pub struct AgendaPointDetailTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub agenda_point: AgendaPointDetail,
    pub coas: Vec<CoaDetail>,
    pub opinions: Vec<OpinionSummary>,
    pub available_transitions: Vec<AvailableTransition>,
}
