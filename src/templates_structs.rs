use askama::Template;
use crate::models::user::UserDisplay;
use crate::models::role::RoleDisplay;
use crate::auth::session::Permissions;

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub username: String,
    pub role_label: String,
    pub user_count: i64,
    pub flash: Option<String>,
    pub permissions: Permissions,
}

#[derive(Template)]
#[template(path = "users/list.html")]
pub struct UserListTemplate {
    pub users: Vec<UserDisplay>,
    pub username: String,
    pub flash: Option<String>,
    pub permissions: Permissions,
}

#[derive(Template)]
#[template(path = "users/form.html")]
pub struct UserFormTemplate {
    pub form_action: String,
    pub form_title: String,
    pub user: Option<UserDisplay>,
    pub roles: Vec<RoleDisplay>,
    pub errors: Vec<String>,
    pub username: String,
    pub permissions: Permissions,
}
