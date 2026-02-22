use serde::{Serialize, Deserialize};

/// Generic paginated response wrapper for API endpoints.
#[derive(Serialize, Debug, Clone)]
pub struct PaginatedResponse<T: Serialize> {
    pub items: Vec<T>,
    pub page: i64,
    pub per_page: i64,
    pub total: i64,
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
