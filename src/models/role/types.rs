/// For use in templates (dropdowns, display).
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RoleDisplay {
    pub id: i64,
    pub name: String,
    pub label: String,
}

/// Extended display for the roles list page — includes counts and description.
#[derive(Debug, Clone)]
pub struct RoleListItem {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub description: String,
    pub user_count: i64,
    pub permission_count: i64,
}

/// For role edit form — role info + list of all permissions with checked state.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RoleDetail {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub description: String,
}

/// A permission with its checked state for the role form.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PermissionCheckbox {
    pub id: i64,
    pub code: String,
    pub label: String,
    pub group_name: String,
    pub checked: bool,
}

/// A user assigned to a role — for the assignment page member list.
#[derive(Debug, Clone)]
pub struct RoleMember {
    pub user_id: i64,
    pub username: String,
    pub display_name: String,
}
