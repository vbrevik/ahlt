// Template context structures for Askama templates, organized by domain.
// All types are re-exported for backward compatibility: `use ahlt::templates_structs::*`

use actix_session::Session;
use sqlx::PgPool;

use crate::errors::AppError;
use crate::auth::csrf;
use crate::auth::session::{Permissions, get_username, get_permissions, take_flash};
use crate::models::nav_item;
use crate::models::setting;

/// Common context shared by all authenticated pages.
/// Templates access these as `ctx.username`, `ctx.nav_modules`, etc.
pub struct PageContext {
    pub username: String,
    pub avatar_initial: String,
    pub permissions: Permissions,
    pub flash: Option<String>,
    pub nav_modules: Vec<crate::models::nav_item::NavModule>,
    pub sidebar_items: Vec<crate::models::nav_item::NavSidebarItem>,
    pub app_name: String,
    pub csrf_token: String,
    pub warning_count: i64,
    pub tor_context: Option<TorContext>,
    pub theme: String,
}

pub struct TorContext {
    pub tor_id: i64,
    pub tor_name: String,
    pub active_section: String,
}

impl PageContext {
    pub async fn build(session: &Session, pool: &PgPool, current_path: &str) -> Result<Self, AppError> {
        let username = get_username(session)
            .map_err(|e| AppError::Session(format!("Failed to get username: {}", e)))?;
        let permissions = get_permissions(session)
            .map_err(|e| AppError::Session(format!("Failed to get permissions: {}", e)))?;
        let flash = take_flash(session);
        let (nav_modules, sidebar_items) = nav_item::find_navigation(pool, &permissions, current_path).await;
        let app_name = setting::get_value(pool, "app.name", "Ahlt").await;
        let csrf_token = csrf::get_or_create_token(session);
        let avatar_initial = username.chars().next().unwrap_or('?').to_uppercase().to_string();
        let user_id = crate::auth::session::get_user_id(session).unwrap_or(0);
        let theme = crate::models::user::get_user_theme(pool, user_id).await
            .unwrap_or_else(|_| "auto".to_string());
        let warning_count = crate::warnings::queries::count_unread(pool, user_id).await;
        Ok(Self { username, avatar_initial, permissions, flash, nav_modules, sidebar_items, app_name, csrf_token, warning_count, tor_context: None, theme })
    }

    /// Attach ToR context for pages nested under /tor/{id}/...
    pub fn with_tor(mut self, tor_id: i64, name: &str, section: &str) -> Self {
        self.tor_context = Some(TorContext {
            tor_id,
            tor_name: name.to_string(),
            active_section: section.to_string(),
        });
        self
    }
}

mod common;
mod user;
mod role;
mod dashboard;
mod audit;
mod ontology;
mod tor;
mod workflow;
mod suggestion;
mod proposal;
mod agenda;
mod coa;
mod opinion;
mod meeting;
mod warning;
mod document;
mod api;

// Re-export all types for seamless imports
pub use self::common::{LoginTemplate, AccountTemplate, SettingsTemplate, DataManagerTemplate, UserOption};
pub use self::user::{UserListTemplate, UserFormTemplate};
pub use self::role::{
    RoleAssignmentTemplate, MatrixCell, PermissionRow, PageGroup, RoleColumn, MenuBuilderTemplate,
    PermissionGroup, RoleBuilderTemplate, PreviewRequest, PreviewResponse, RoleBuilderForm,
};
pub use self::dashboard::DashboardTemplate;
pub use self::audit::AuditListTemplate;
pub use self::ontology::{OntologyConceptsTemplate, OntologyGraphTemplate, OntologyDataTemplate, OntologyDetailTemplate};
pub use self::tor::{
    TorListTemplate, TorFormTemplate, TorDetailTemplate, GovernanceMapTemplate,
    TorOutlookTemplate, PresentationTemplatesTemplate,
};
pub use self::workflow::{
    WorkflowTemplate, WorkflowIndexTemplate, WorkflowBuilderListTemplate,
    WorkflowBuilderDetailTemplate, QueueTemplate,
};
pub use self::suggestion::SuggestionFormTemplate;
pub use self::proposal::{ProposalFormTemplate, ProposalDetailTemplate};
pub use self::agenda::{AgendaPointFormTemplate, AgendaPointDetailTemplate};
pub use self::coa::CoaFormTemplate;
pub use self::opinion::{OpinionFormTemplate, DecisionFormTemplate};
pub use self::meeting::{
    MeetingsListTemplate, TorMeetingsListTemplate, MeetingDetailTemplate, MinutesViewTemplate,
};
pub use self::warning::{WarningListTemplate, WarningDetailTemplate};
pub use self::document::{DocumentListTemplate, DocumentFormTemplate, DocumentDetailTemplate};
pub use self::api::{
    PaginatedResponse, ApiUserResponse, ApiUserRequest, ApiEntityProperty, ApiEntityResponse, ApiEntityRequest, ApiErrorResponse,
};
