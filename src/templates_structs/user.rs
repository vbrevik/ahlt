use askama::Template;
use serde::{Serialize, Deserialize};

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

/// User response for API (no password hash, includes role info).
#[derive(Serialize, Debug, Clone)]
pub struct ApiUserResponse {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub display_name: String,
    pub role_ids: String,
    pub role_names: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<UserDisplay> for ApiUserResponse {
    fn from(u: UserDisplay) -> Self {
        ApiUserResponse {
            id: u.id,
            username: u.username,
            email: u.email,
            display_name: u.display_name,
            role_ids: u.role_ids,
            role_names: u.role_names,
            created_at: u.created_at,
            updated_at: u.updated_at,
        }
    }
}

/// Create/update user request for API.
#[derive(Deserialize, Debug)]
pub struct ApiUserRequest {
    pub username: String,
    pub email: String,
    pub display_name: String,
    pub password: Option<String>, // required for create, optional for update
    #[serde(default)]
    pub role_id: Option<i64>, // deprecated — role assignment is handled separately
}
