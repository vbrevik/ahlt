use askama::Template;

use crate::models::user::UserDisplay;
use super::PageContext;

#[derive(Template)]
#[template(path = "users/list.html")]
pub struct UserListTemplate {
    pub ctx: PageContext,
    pub user_page: crate::models::user::types::UserPage,
    pub filter_json: String,
    pub filter_active: bool,
    pub sort_column: String,
    pub sort_dir: String,
    pub columns: Vec<crate::models::table_filter::ColumnDef>,
    pub available_roles: Vec<(String, String)>,
    pub fields_json: String,
}

#[derive(Template)]
#[template(path = "users/form.html")]
pub struct UserFormTemplate {
    pub ctx: PageContext,
    pub form_action: String,
    pub form_title: String,
    pub user: Option<UserDisplay>,
    pub errors: Vec<String>,
}
