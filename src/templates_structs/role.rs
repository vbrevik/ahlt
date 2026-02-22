use askama::Template;

use crate::models::role::{RoleListItem, RoleDetail, PermissionCheckbox};
use crate::models::role::builder::NavItemPreview;
use super::PageContext;

#[derive(Template)]
#[template(path = "roles/assignment.html")]
pub struct RoleAssignmentTemplate {
    pub ctx: PageContext,
    pub roles: Vec<RoleListItem>,
    pub selected_role_id: i64,
    pub members: Vec<crate::models::role::RoleMember>,
    pub available_users: Vec<crate::models::role::RoleMember>,
    pub users_with_roles: Vec<crate::models::user::UserWithRoles>,
    pub active_tab: String,
}

/// A single cell in the permission matrix (one role x one permission).
pub struct MatrixCell {
    pub role_id: i64,
    pub permission_id: i64,
    pub checked: bool,
}

/// One row in the matrix (one permission, with cells for each role).
pub struct PermissionRow {
    pub code: String,
    pub label: String,
    pub cells: Vec<MatrixCell>,
}

/// A group of permission rows under a page section header.
pub struct PageGroup {
    pub group_name: String,
    pub permissions: Vec<PermissionRow>,
}

/// Column header data for a role.
pub struct RoleColumn {
    pub id: i64,
    pub label: String,
    pub name: String,
}

#[derive(Template)]
#[template(path = "menu_builder.html")]
pub struct MenuBuilderTemplate {
    pub ctx: PageContext,
    pub roles: Vec<RoleColumn>,
    pub page_groups: Vec<PageGroup>,
    pub col_count: usize,
}

/// A group of permissions for the role builder.
pub struct PermissionGroup {
    pub group_name: String,
    pub permissions: Vec<PermissionCheckbox>,
}

#[derive(Template)]
#[template(path = "roles/builder.html")]
pub struct RoleBuilderTemplate {
    pub ctx: PageContext,
    pub permission_groups: Vec<PermissionGroup>,
    pub csrf_token: String,
    pub role: Option<RoleDetail>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PreviewRequest {
    pub permission_ids: Vec<i64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PreviewResponse {
    pub items: Vec<NavItemPreview>,
    pub count: usize,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RoleBuilderForm {
    pub name: String,
    pub label: String,
    pub description: String,
    pub permission_ids: String, // JSON array
    pub csrf_token: String,
    pub role_id: Option<String>,
}
