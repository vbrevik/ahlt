use actix_session::Session;
use actix_web::{web, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;

use crate::models::relation;
use crate::auth::csrf;
use crate::auth::session::require_permission;
use crate::errors::AppError;

#[derive(Deserialize)]
pub struct AssignForm {
    pub user_id: i64,
    pub role_id: i64,
    pub csrf_token: String,
}

/// POST /roles/assign — create a has_role relation between user and role.
/// Uses INSERT OR IGNORE (via relation::create) to prevent duplicates.
pub async fn assign(
    pool: web::Data<PgPool>,
    session: Session,
    form: web::Form<AssignForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.assign")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    // relation::create uses INSERT OR IGNORE — safe against duplicates
    relation::create(&pool, "has_role", form.user_id, form.role_id).await?;

    // Audit
    let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "user_id": form.user_id,
        "role_id": form.role_id,
        "summary": "Assigned role to user"
    });
    let _ = crate::audit::log(&pool, current_user_id, "role.assigned", "role", form.role_id, details).await;

    let _ = session.insert("flash", "Role assigned");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/roles"))
        .finish())
}

#[derive(Deserialize)]
pub struct UnassignForm {
    pub user_id: i64,
    pub role_id: i64,
    pub csrf_token: String,
}

/// POST /roles/unassign — remove a has_role relation between user and role.
/// Prevents removing the admin role if this is the last admin user.
pub async fn unassign(
    pool: web::Data<PgPool>,
    session: Session,
    form: web::Form<UnassignForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.assign")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    // Last-admin protection: don't allow removing admin role if this is the last admin
    let is_admin_role: bool = sqlx::query_scalar(
        "SELECT name = 'admin' FROM entities WHERE id = $1 AND entity_type = 'role'",
    )
    .bind(form.role_id)
    .fetch_one(pool.get_ref())
    .await?;

    if is_admin_role {
        let admin_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM relations \
             WHERE relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
             AND target_id = $1",
        )
        .bind(form.role_id)
        .fetch_one(pool.get_ref())
        .await?;

        if admin_count <= 1 {
            let _ = session.insert("flash", "Cannot remove role: this is the last administrator");
            return Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/roles"))
                .finish());
        }
    }

    relation::delete(&pool, "has_role", form.user_id, form.role_id).await?;

    // Audit
    let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "user_id": form.user_id,
        "role_id": form.role_id,
        "summary": "Unassigned role from user"
    });
    let _ = crate::audit::log(&pool, current_user_id, "role.unassigned", "role", form.role_id, details).await;

    let _ = session.insert("flash", "Role unassigned");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/roles"))
        .finish())
}

#[derive(Deserialize)]
pub struct PreviewQuery {
    pub user_id: i64,
}

/// GET /api/roles/preview?user_id=N — returns JSON showing the effective menu
/// items for a user based on ALL their assigned roles.
pub async fn menu_preview(
    pool: web::Data<PgPool>,
    session: Session,
    query: web::Query<PreviewQuery>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.assign")?;

    // Get all permissions for this user across all roles
    let perms = crate::models::permission::find_codes_by_user_id(&pool, query.user_id).await?;
    let permissions = crate::auth::session::Permissions(perms);

    // Get accessible nav items using existing logic
    let (modules, sidebar_items) = crate::models::nav_item::find_navigation(&pool, &permissions, "").await;

    // Build flat list of accessible items
    let mut items: Vec<serde_json::Value> = vec![];
    for module in &modules {
        items.push(serde_json::json!({
            "label": module.label,
            "url": module.url,
            "type": "module"
        }));
    }
    for item in &sidebar_items {
        items.push(serde_json::json!({
            "label": item.label,
            "url": item.url,
            "type": "sidebar"
        }));
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "user_id": query.user_id,
        "permission_count": permissions.0.len(),
        "menu_items": items
    })))
}
