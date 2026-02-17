use actix_session::Session;
use actix_web::{web, HttpResponse};
use rusqlite::params;

use crate::auth::session::{require_permission, get_user_id};
use crate::auth::csrf;
use crate::db::DbPool;
use crate::errors::{AppError, render};
use crate::models::{permission, role};
use crate::templates_structs::{
    PageContext, RoleBuilderTemplate, PreviewRequest, PreviewResponse, RoleBuilderForm,
    PermissionGroup,
};
use crate::audit;

pub async fn wizard_form(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.manage")?;

    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/roles/builder")?;

    // Get all permissions grouped by group_name
    let all_permissions = permission::find_all_with_groups(&conn)?;

    // Group permissions by group_name
    let mut groups_map: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
    for perm in all_permissions {
        groups_map.entry(perm.group_name.clone()).or_insert_with(Vec::new).push(
            crate::models::role::PermissionCheckbox {
                id: perm.id,
                code: perm.code,
                label: perm.label,
                group_name: perm.group_name,
                checked: false, // New role has no permissions yet
            }
        );
    }

    // Convert to Vec<PermissionGroup>
    let mut permission_groups: Vec<PermissionGroup> = groups_map.into_iter()
        .map(|(group_name, permissions)| PermissionGroup { group_name, permissions })
        .collect();
    permission_groups.sort_by(|a, b| a.group_name.cmp(&b.group_name));

    let csrf_token = ctx.csrf_token.clone();

    let tmpl = RoleBuilderTemplate {
        ctx,
        permission_groups,
        csrf_token,
    };

    render(tmpl)
}

pub async fn preview_menu(
    pool: web::Data<DbPool>,
    session: Session,
    body: web::Json<PreviewRequest>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.manage")?;

    let conn = pool.get()?;
    let items = role::builder::find_accessible_nav_items(&conn, &body.permission_ids)?;
    let count = items.len();

    Ok(HttpResponse::Ok().json(PreviewResponse { items, count }))
}

pub async fn create_role(
    pool: web::Data<DbPool>,
    session: Session,
    form: web::Form<RoleBuilderForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.manage")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let conn = pool.get()?;

    // Validate role name
    validate_role_name(&form.name)?;
    validate_role_label(&form.label)?;
    ensure_unique_role_name(&conn, &form.name)?;

    // Parse permission IDs
    let permission_ids: Vec<i64> = serde_json::from_str(&form.permission_ids)
        .map_err(|_| AppError::Session("Invalid permission data".into()))?;

    if permission_ids.is_empty() {
        return Err(AppError::Session("Please select at least one permission".into()));
    }

    // Transaction: create role entity + properties + relations
    let role_id = conn.query_row(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', ?1, ?2) RETURNING id",
        params![&form.name, &form.label],
        |row| row.get::<_, i64>(0),
    )?;

    // Add description property if provided
    if !form.description.trim().is_empty() {
        conn.execute(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'description', ?2)",
            params![role_id, &form.description],
        )?;
    }

    // Add permission relations
    let rt_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='relation_type' AND name='has_permission'",
        [],
        |row| row.get(0),
    )?;

    for perm_id in &permission_ids {
        conn.execute(
            "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
            params![rt_id, role_id, perm_id],
        )?;
    }

    // Audit log
    let user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "name": form.name,
        "label": form.label,
        "permission_count": permission_ids.len(),
    });
    let _ = audit::log(&conn, user_id, "role.created_via_builder", "role", role_id, details);

    Ok(HttpResponse::SeeOther()
        .append_header(("Location", format!("/roles/{}", role_id)))
        .finish())
}

fn validate_role_name(name: &str) -> Result<(), AppError> {
    if name.is_empty() {
        return Err(AppError::Session("Role name required".into()));
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(AppError::Session("Role name must be alphanumeric + underscore".into()));
    }
    if name.len() > 50 {
        return Err(AppError::Session("Role name too long (max 50)".into()));
    }
    Ok(())
}

fn validate_role_label(label: &str) -> Result<(), AppError> {
    if label.trim().is_empty() {
        return Err(AppError::Session("Role label required".into()));
    }
    if label.len() > 100 {
        return Err(AppError::Session("Role label too long (max 100)".into()));
    }
    Ok(())
}

fn ensure_unique_role_name(conn: &rusqlite::Connection, name: &str) -> Result<(), AppError> {
    let exists = conn.query_row(
        "SELECT 1 FROM entities WHERE entity_type='role' AND name=?1",
        params![name],
        |_| Ok(true),
    ).unwrap_or(false);

    if exists {
        Err(AppError::Session(format!("Role '{}' already exists", name)))
    } else {
        Ok(())
    }
}
