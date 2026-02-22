use actix_session::Session;
use askama::Template;
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

    /// Attach ToR context for pages nested under /tor/{id}/..
    pub fn with_tor(mut self, tor_id: i64, name: &str, section: &str) -> Self {
        self.tor_context = Some(TorContext {
            tor_id,
            tor_name: name.to_string(),
            active_section: section.to_string(),
        });
        self
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
#[template(path = "account.html")]
pub struct AccountTemplate {
    pub ctx: PageContext,
    pub errors: Vec<String>,
}

#[derive(Template)]
#[template(path = "settings.html")]
pub struct SettingsTemplate {
    pub ctx: PageContext,
    pub settings: Vec<crate::models::setting::SettingDisplay>,
}

#[derive(Template)]
#[template(path = "admin/data_manager.html")]
pub struct DataManagerTemplate {
    pub ctx: PageContext,
    pub entity_types: Vec<String>,
}
