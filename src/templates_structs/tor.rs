use askama::Template;
use sqlx::FromRow;

use crate::models::tor::{TorListItem, TorDetail, TorMember, TorFunctionListItem, TorDependency, GovernanceMapEntry};
use crate::models::meeting::MeetingListItem;
use crate::models::protocol::ProtocolStep;
use crate::models::presentation_template::{PresentationTemplate, TemplateSlide};
use super::PageContext;

#[derive(Template)]
#[template(path = "tor/list.html")]
pub struct TorListTemplate {
    pub ctx: PageContext,
    pub tors: Vec<TorListItem>,
}

#[derive(Template)]
#[template(path = "tor/form.html")]
pub struct TorFormTemplate {
    pub ctx: PageContext,
    pub form_action: String,
    pub form_title: String,
    pub tor: Option<TorDetail>,
    pub errors: Vec<String>,
}

/// UserOption for the "add member" dropdown.
#[derive(FromRow)]
pub struct UserOption {
    pub id: i64,
    pub name: String,
    pub label: String,
}

#[derive(Template)]
#[template(path = "tor/detail.html")]
pub struct TorDetailTemplate {
    pub ctx: PageContext,
    pub tor: TorDetail,
    pub members: Vec<TorMember>,
    pub functions: Vec<TorFunctionListItem>,
    pub protocol_steps: Vec<ProtocolStep>,
    pub available_users: Vec<UserOption>,
    pub upstream_deps: Vec<TorDependency>,
    pub downstream_deps: Vec<TorDependency>,
    pub other_tors: Vec<(i64, String, String)>,
    pub meetings: Vec<MeetingListItem>,
}

#[derive(Template)]
#[template(path = "governance/map.html")]
pub struct GovernanceMapTemplate {
    pub ctx: PageContext,
    pub tors: Vec<(i64, String, String)>,
    pub dependencies: Vec<GovernanceMapEntry>,
}

#[derive(Template)]
#[template(path = "tor/outlook.html")]
pub struct TorOutlookTemplate {
    pub ctx: PageContext,
    pub events_json: String,  // JSON-serialized Vec<CalendarEvent> for initial week
    pub today: String,        // YYYY-MM-DD
    pub week_start: String,   // YYYY-MM-DD (Monday of initial week)
}

#[derive(Template)]
#[template(path = "tor/presentation_templates.html")]
pub struct PresentationTemplatesTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub tor_label: String,
    pub templates: Vec<PresentationTemplate>,
    pub selected_template: Option<PresentationTemplate>,
    pub slides: Vec<TemplateSlide>,
}
