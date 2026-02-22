use askama::Template;

use crate::models::meeting::{MeetingListItem, MeetingDetail, MeetingAgendaPoint};
use crate::models::minutes::Minutes;
use crate::models::protocol::ProtocolStep;
use crate::models::workflow::AvailableTransition;
use crate::auth::session::Permissions;
use super::PageContext;

#[derive(Template)]
#[template(path = "meetings/list.html")]
pub struct MeetingsListTemplate {
    pub ctx: PageContext,
    pub upcoming: Vec<MeetingListItem>,
    pub past: Vec<MeetingListItem>,
}

#[derive(Template)]
#[template(path = "meetings/tor_list.html")]
pub struct TorMeetingsListTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub tor_name: String,
    pub meetings: Vec<MeetingListItem>,
}

#[derive(Template)]
#[template(path = "meetings/detail.html")]
pub struct MeetingDetailTemplate {
    pub ctx: PageContext,
    pub meeting: MeetingDetail,
    pub agenda_points: Vec<MeetingAgendaPoint>,
    pub unassigned_points: Vec<MeetingAgendaPoint>,
    pub protocol_steps: Vec<ProtocolStep>,
    pub transitions: Vec<AvailableTransition>,
    pub minutes: Option<Minutes>,
    pub tor_id: i64,
    pub tor_capabilities: Permissions,
}

#[derive(Template)]
#[template(path = "minutes/view.html")]
pub struct MinutesViewTemplate {
    pub ctx: PageContext,
    pub minutes: Minutes,
    pub sections: Vec<crate::models::minutes::MinutesSection>,
}
