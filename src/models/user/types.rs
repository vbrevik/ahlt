use serde::Deserialize;

/// Internal user struct for authentication — includes password hash.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub password: String,
    pub email: String,
    pub display_name: String,
    pub role_id: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// Safe version for templates — no password hash, includes role info from relations.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct UserDisplay {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub display_name: String,
    pub role_id: i64,
    pub role_name: String,
    pub role_label: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Pagination metadata for user list.
pub struct UserPage {
    pub users: Vec<UserDisplay>,
    pub page: i64,
    pub per_page: i64,
    pub total_count: i64,
    pub total_pages: i64,
}

/// New user data for creation.
pub struct NewUser {
    pub username: String,
    pub password: String,
    pub email: String,
    pub display_name: String,
}

/// Form data from create/edit user forms.
#[derive(Debug, Deserialize)]
pub struct UserForm {
    pub username: String,
    pub password: String,
    pub email: String,
    pub display_name: String,
    pub csrf_token: String,
}

/// User with all assigned roles — for the "By User" tab on assignment page.
#[derive(Debug, Clone)]
pub struct UserWithRoles {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub roles: Vec<(i64, String, String)>, // (role_id, role_name, role_label)
}
