use actix_session::Session;
use askama::Template;
use rusqlite::Connection;

use crate::models::user::UserDisplay;
use crate::models::role::{RoleDisplay, RoleListItem, RoleDetail, PermissionCheckbox};
use crate::models::ontology::{EntityTypeSummary, RelationTypeSummary, EntityDetail};
use crate::models::setting::{self, SettingDisplay};
use crate::models::nav_item::{self, NavModule, NavSidebarItem};
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
    pub fn build(session: &Session, conn: &Connection, current_path: &str) -> Self {
        let username = get_username(session);
        let permissions = get_permissions(session);
        let flash = take_flash(session);
        let (nav_modules, sidebar_items) = nav_item::find_navigation(conn, &permissions, current_path);
        let app_name = setting::get_value(conn, "app.name", "Ahlt");
        let csrf_token = csrf::get_or_create_token(session);
        let avatar_initial = username.chars().next().unwrap_or('?').to_uppercase().to_string();
        let warning_count = 0; // TODO: wire up when warnings feature is built
        Self { username, avatar_initial, permissions, flash, nav_modules, sidebar_items, app_name, csrf_token, warning_count }
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
    pub users: Vec<UserDisplay>,
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
