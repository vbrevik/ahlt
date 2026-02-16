use actix_session::Session;
use askama::Template;
use rusqlite::Connection;

use crate::errors::AppError;
use crate::models::user::{UserDisplay, UserPage};
use crate::models::role::{RoleDisplay, RoleListItem, RoleDetail, PermissionCheckbox};
use crate::models::ontology::{EntityTypeSummary, RelationTypeSummary, EntityDetail};
use crate::models::setting::{self, SettingDisplay};
use crate::models::nav_item::{self, NavModule, NavSidebarItem};
use crate::models::audit::AuditEntryPage;
use crate::models::tor::{TorListItem, TorDetail, TorMember, TorFunctionListItem};
use crate::models::suggestion::SuggestionListItem;
use crate::models::proposal::{ProposalListItem, ProposalDetail};
use crate::models::agenda_point::{AgendaPointListItem, AgendaPointDetail};
use crate::models::coa::{CoaListItem, CoaDetail};
use crate::models::opinion::{OpinionDetail, OpinionSummary};
use crate::models::workflow::AvailableTransition;
use crate::auth::csrf;
use crate::auth::session::{Permissions, get_username, get_permissions, take_flash};

/// Common context shared by all authenticated pages.
/// Templates access these as `ctx.username`, `ctx.nav_modules`, etc.
pub struct PageContext {
    pub username: String,
    pub avatar_initial: String,
    pub permissions: Permissions,
    pub flash: Option<String>,
    pub nav_modules: Vec<NavModule>,
    pub sidebar_items: Vec<NavSidebarItem>,
    pub app_name: String,
    pub csrf_token: String,
    pub warning_count: i64,
}

impl PageContext {
    pub fn build(session: &Session, conn: &Connection, current_path: &str) -> Result<Self, AppError> {
        let username = get_username(session)
            .map_err(|e| AppError::Session(format!("Failed to get username: {}", e)))?;
        let permissions = get_permissions(session)
            .map_err(|e| AppError::Session(format!("Failed to get permissions: {}", e)))?;
        let flash = take_flash(session);
        let (nav_modules, sidebar_items) = nav_item::find_navigation(conn, &permissions, current_path);
        let app_name = setting::get_value(conn, "app.name", "Ahlt");
        let csrf_token = csrf::get_or_create_token(session);
        let avatar_initial = username.chars().next().unwrap_or('?').to_uppercase().to_string();
        let user_id = crate::auth::session::get_user_id(session).unwrap_or(0);
        let warning_count = crate::warnings::queries::count_unread(conn, user_id);
        Ok(Self { username, avatar_initial, permissions, flash, nav_modules, sidebar_items, app_name, csrf_token, warning_count })
    }
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub error: Option<String>,
    pub app_name: String,
    pub csrf_token: String,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub ctx: PageContext,
    pub role_label: String,
    pub user_count: i64,
}

#[derive(Template)]
#[template(path = "users/list.html")]
pub struct UserListTemplate {
    pub ctx: PageContext,
    pub user_page: UserPage,
    pub search_query: Option<String>,
}

#[derive(Template)]
#[template(path = "users/form.html")]
pub struct UserFormTemplate {
    pub ctx: PageContext,
    pub form_action: String,
    pub form_title: String,
    pub user: Option<UserDisplay>,
    pub roles: Vec<RoleDisplay>,
    pub errors: Vec<String>,
}

#[derive(Template)]
#[template(path = "roles/list.html")]
pub struct RoleListTemplate {
    pub ctx: PageContext,
    pub roles: Vec<RoleListItem>,
}

#[derive(Template)]
#[template(path = "roles/form.html")]
pub struct RoleFormTemplate {
    pub ctx: PageContext,
    pub form_action: String,
    pub form_title: String,
    pub role: Option<RoleDetail>,
    pub permissions: Vec<PermissionCheckbox>,
    pub errors: Vec<String>,
}

#[derive(Template)]
#[template(path = "ontology/concepts.html")]
pub struct OntologyConceptsTemplate {
    pub ctx: PageContext,
    pub entity_types: Vec<EntityTypeSummary>,
    pub relation_types: Vec<RelationTypeSummary>,
}

#[derive(Template)]
#[template(path = "ontology/graph.html")]
pub struct OntologyGraphTemplate {
    pub ctx: PageContext,
}

#[derive(Template)]
#[template(path = "ontology/data.html")]
pub struct OntologyDataTemplate {
    pub ctx: PageContext,
}

#[derive(Template)]
#[template(path = "ontology/detail.html")]
pub struct OntologyDetailTemplate {
    pub ctx: PageContext,
    pub entity: EntityDetail,
}

#[derive(Template)]
#[template(path = "settings.html")]
pub struct SettingsTemplate {
    pub ctx: PageContext,
    pub settings: Vec<SettingDisplay>,
}

#[derive(Template)]
#[template(path = "account.html")]
pub struct AccountTemplate {
    pub ctx: PageContext,
    pub errors: Vec<String>,
}

#[derive(Template)]
#[template(path = "audit/list.html")]
pub struct AuditListTemplate {
    pub ctx: PageContext,
    pub audit_page: AuditEntryPage,
    pub search_query: Option<String>,
    pub action_filter: Option<String>,
    pub target_type_filter: Option<String>,
}

// --- ToR (Terms of Reference) templates ---

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
    pub available_users: Vec<UserOption>,
}

// --- Workflow templates ---

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
#[template(path = "suggestions/form.html")]
pub struct SuggestionFormTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub tor_name: String,
    pub errors: Vec<String>,
}

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

// --- Workflow queue view ---

#[derive(Template)]
#[template(path = "workflow/queue.html")]
#[allow(dead_code)]
pub struct QueueTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub tor_name: String,
    pub queued_proposals: Vec<ProposalListItem>,
}

// --- Agenda point templates ---

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

// --- COA templates ---

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

// --- Opinion templates ---

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
    pub agenda_point: AgendaPointDetail,
    pub coas: Vec<CoaDetail>,
    pub opinions: Vec<OpinionSummary>,
    pub errors: Vec<String>,
}

// --- Menu Builder types ---

/// A single cell in the permission matrix (one role x one permission).
pub struct MatrixCell {
    pub role_id: i64,
    pub permission_id: i64,
    pub checked: bool,
}

/// One row in the matrix (one permission, with cells for each role).
pub struct PermissionRow {
    pub code: String,
    pub label: String,
    pub cells: Vec<MatrixCell>,
}

/// A group of permission rows under a page section header.
pub struct PageGroup {
    pub group_name: String,
    pub permissions: Vec<PermissionRow>,
}

/// Column header data for a role.
pub struct RoleColumn {
    pub id: i64,
    pub label: String,
    pub name: String,
}

#[derive(Template)]
#[template(path = "menu_builder.html")]
pub struct MenuBuilderTemplate {
    pub ctx: PageContext,
    pub roles: Vec<RoleColumn>,
    pub page_groups: Vec<PageGroup>,
    pub col_count: usize,
}
