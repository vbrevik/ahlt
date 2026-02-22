use askama::Template;
use sqlx::FromRow;

use super::PageContext;

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

/// UserOption for the "add member" dropdown.
#[derive(FromRow)]
pub struct UserOption {
    pub id: i64,
    pub name: String,
    pub label: String,
}
