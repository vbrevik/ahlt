use askama::Template;

use crate::models::suggestion::SuggestionListItem;
use crate::models::proposal::ProposalListItem;
use crate::models::agenda_point::AgendaPointListItem;
use super::PageContext;

#[derive(Template)]
#[template(path = "workflow/view.html")]
pub struct WorkflowTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub tor_name: String,
    pub active_tab: String,  // "suggestions", "proposals", or "agenda"
    pub suggestions: Vec<SuggestionListItem>,
    pub proposals: Vec<ProposalListItem>,
    pub agenda_points: Vec<AgendaPointListItem>,
}

#[derive(Template)]
#[template(path = "workflow/index.html")]
pub struct WorkflowIndexTemplate {
    pub ctx: PageContext,
    pub active_tab: String,
    pub suggestions: Vec<crate::models::suggestion::CrossTorSuggestionItem>,
    pub proposals: Vec<crate::models::proposal::CrossTorProposalItem>,
    pub agenda_points: Vec<crate::models::agenda_point::CrossTorAgendaItem>,
}

#[derive(Template)]
#[template(path = "workflow/builder_list.html")]
pub struct WorkflowBuilderListTemplate {
    pub ctx: PageContext,
    pub scopes: Vec<crate::models::workflow::WorkflowScope>,
}

#[derive(Template)]
#[template(path = "workflow/builder_detail.html")]
pub struct WorkflowBuilderDetailTemplate {
    pub ctx: PageContext,
    pub scope: String,
    pub statuses: Vec<crate::models::workflow::WorkflowStatus>,
    pub transitions: Vec<crate::models::workflow::WorkflowTransition>,
    pub permissions: Vec<crate::models::entity::Entity>,
}

#[derive(Template)]
#[template(path = "workflow/queue.html")]
#[allow(dead_code)]
pub struct QueueTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub tor_name: String,
    pub queued_proposals: Vec<ProposalListItem>,
}
