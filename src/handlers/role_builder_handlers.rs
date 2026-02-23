use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

use crate::auth::session::{require_permission, get_user_id};
use crate::auth::csrf;
use crate::errors::{AppError, render};
use crate::models::{permission, role};
use crate::templates_structs::{
    PageContext, RoleBuilderTemplate, PreviewRequest, PreviewResponse, RoleBuilderForm,
    PermissionGroup,
};
use crate::audit;
use crate::handlers::warning_handlers::ws::ConnectionMap;

pub async fn wizard_form(
    pool: web::Data<PgPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.manage")?;

    let ctx = PageContext::build(&session, &pool, "/roles/builder").await?;

    // Get all permissions grouped by group_name
    let all_permissions = permission::find_all_with_groups(&pool).await?;

    // Group permissions by group_name
    let mut groups_map: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
    for perm in all_permissions {
        groups_map.entry(perm.group_name.clone()).or_insert_with(Vec::new).push(
            crate::models::role::PermissionCheckbox {
                id: perm.id,
                code: perm.code,
                label: perm.label,
                group_name: perm.group_name,
                description: perm.description,
                checked: false,
            }
        );
    }

    let mut permission_groups: Vec<PermissionGroup> = groups_map.into_iter()
        .map(|(group_name, permissions)| PermissionGroup { group_name, permissions })
        .collect();
    permission_groups.sort_by(|a, b| a.group_name.cmp(&b.group_name));

    let csrf_token = ctx.csrf_token.clone();

    let tmpl = RoleBuilderTemplate {
        ctx,
        permission_groups,
        csrf_token,
        role: None,
    };

    render(tmpl)
}

pub async fn edit_form(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.manage")?;

    let id = path.into_inner();
    let ctx = PageContext::build(&session, &pool, "/roles").await?;

    let role_detail = role::find_detail_by_id(&pool, id).await?
        .ok_or(AppError::NotFound)?;

    // Get all permissions with checked state for this role
    let all_checkboxes = role::find_permission_checkboxes(&pool, id).await?;

    let mut groups_map: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
    for perm in all_checkboxes {
        groups_map.entry(perm.group_name.clone()).or_default().push(perm);
    }
    let mut permission_groups: Vec<PermissionGroup> = groups_map.into_iter()
        .map(|(group_name, permissions)| PermissionGroup { group_name, permissions })
        .collect();
    permission_groups.sort_by(|a, b| a.group_name.cmp(&b.group_name));

    let csrf_token = ctx.csrf_token.clone();

    let tmpl = RoleBuilderTemplate {
        ctx,
        permission_groups,
        csrf_token,
        role: Some(role_detail),
    };

    render(tmpl)
}

pub async fn preview_menu(
    pool: web::Data<PgPool>,
    session: Session,
    body: web::Json<PreviewRequest>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.manage")?;

    let items = role::builder::find_accessible_nav_items(&pool, &body.permission_ids).await?;
    let count = items.len();

    Ok(HttpResponse::Ok().json(PreviewResponse { items, count }))
}

pub async fn create_role(
    pool: web::Data<PgPool>,
    session: Session,
    form: web::Form<RoleBuilderForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.manage")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    validate_role_name(&form.name)?;
    validate_role_label(&form.label)?;
    ensure_unique_role_name(&pool, &form.name).await?;

    let permission_ids: Vec<i64> = serde_json::from_str(&form.permission_ids)
        .map_err(|_| AppError::Session("Invalid permission data".into()))?;

    if permission_ids.is_empty() {
        return Err(AppError::Session("Please select at least one permission".into()));
    }

    let role_id: i64 = sqlx::query_scalar(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', $1, $2) RETURNING id",
    )
    .bind(form.name.trim())
    .bind(form.label.trim())
    .fetch_one(pool.get_ref())
    .await?;

    if !form.description.trim().is_empty() {
        sqlx::query(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'description', $2)",
        )
        .bind(role_id)
        .bind(form.description.trim())
        .execute(pool.get_ref())
        .await?;
    }

    let rt_id: i64 = sqlx::query_scalar(
        "SELECT id FROM entities WHERE entity_type='relation_type' AND name='has_permission'",
    )
    .fetch_one(pool.get_ref())
    .await?;

    for perm_id in &permission_ids {
        sqlx::query(
            "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES ($1, $2, $3)",
        )
        .bind(rt_id)
        .bind(role_id)
        .bind(perm_id)
        .execute(pool.get_ref())
        .await?;
    }

    let user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "name": form.name,
        "label": form.label,
        "permission_count": permission_ids.len(),
    });
    let _ = audit::log(&pool, user_id, "role.created_via_builder", "role", role_id, details).await;

    let _ = session.insert("flash", "Role created successfully");
    Ok(HttpResponse::SeeOther()
        .append_header(("Location", "/roles"))
        .finish())
}

pub async fn update_role(
    pool: web::Data<PgPool>,
    session: Session,
    form: web::Form<RoleBuilderForm>,
    conn_map: web::Data<ConnectionMap>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.manage")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let role_id: i64 = form.role_id.as_deref()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| AppError::Session("Missing role ID".into()))?;

    validate_role_name(&form.name)?;
    validate_role_label(&form.label)?;
    ensure_unique_role_name_excluding(&pool, &form.name, role_id).await?;

    let permission_ids: Vec<i64> = serde_json::from_str(&form.permission_ids)
        .map_err(|_| AppError::Session("Invalid permission data".into()))?;

    if permission_ids.is_empty() {
        return Err(AppError::Session("Please select at least one permission".into()));
    }

    role::update(&pool, role_id, form.name.trim(), form.label.trim(),
                 form.description.trim(), &permission_ids).await?;

    // Audit log
    let user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "role_name": form.name.trim(),
        "new_permission_count": permission_ids.len(),
        "summary": format!("Updated permissions for role '{}'", form.label.trim())
    });
    let _ = audit::log(&pool, user_id, "role.permissions_changed", "role", role_id, details).await;

    // Warning for admins
    let msg = format!("Permissions updated for role '{}'", form.label.trim());
    if let Ok(wid) = crate::warnings::create_warning(
        &pool, "info", "security", "event.role.permissions_changed", &msg, "", "system"
    ).await {
        let admins = crate::warnings::get_users_with_permission(&pool, "admin.settings")
            .await
            .unwrap_or_default();
        if !admins.is_empty() {
            let _ = crate::warnings::create_receipts(&pool, wid, &admins).await;
            crate::handlers::warning_handlers::ws::notify_users(
                &conn_map, &pool, &admins, wid, "info", &msg,
            ).await;
        }
    }

    let _ = session.insert("flash", "Role updated successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/roles"))
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

async fn ensure_unique_role_name(pool: &PgPool, name: &str) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM entities WHERE entity_type='role' AND name=$1)",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if exists {
        Err(AppError::Session(format!("Role '{}' already exists", name)))
    } else {
        Ok(())
    }
}

async fn ensure_unique_role_name_excluding(pool: &PgPool, name: &str, exclude_id: i64) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM entities WHERE entity_type='role' AND name=$1 AND id != $2)",
    )
    .bind(name)
    .bind(exclude_id)
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if exists {
        Err(AppError::Session(format!("Role '{}' already exists", name)))
    } else {
        Ok(())
    }
}
