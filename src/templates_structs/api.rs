use serde::{Serialize, Deserialize};

use crate::models::user::UserDisplay;

/// Generic paginated response wrapper for API endpoints.
#[derive(Serialize, Debug, Clone)]
pub struct PaginatedResponse<T: Serialize> {
    pub items: Vec<T>,
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
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

/// Entity property in API responses.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ApiEntityProperty {
    pub key: String,
    pub value: String,
}

/// Entity response for API.
#[derive(Serialize, Debug, Clone)]
pub struct ApiEntityResponse {
    pub id: i64,
    pub entity_type: String,
    pub name: String,
    pub label: Option<String>,
    pub properties: Vec<ApiEntityProperty>,
}

/// Create entity request for API.
#[derive(Deserialize, Debug)]
pub struct ApiEntityRequest {
    pub entity_type: String,
    pub name: String,
    pub label: Option<String>,
    pub properties: Option<Vec<ApiEntityProperty>>,
}

/// API error response.
#[derive(Serialize, Debug)]
pub struct ApiErrorResponse {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}
