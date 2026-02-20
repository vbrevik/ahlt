use actix_session::Session;
use actix_web::{web, HttpResponse};
use serde::Deserialize;

use crate::db::DbPool;
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
    pool: web::Data<DbPool>,
    session: Session,
    form: web::Form<AssignForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.assign")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let conn = pool.get()?;

    // relation::create uses INSERT OR IGNORE — safe against duplicates
    relation::create(&conn, "has_role", form.user_id, form.role_id)?;

    // Audit
    let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "user_id": form.user_id,
        "role_id": form.role_id,
        "summary": "Assigned role to user"
    });
    let _ = crate::audit::log(&conn, current_user_id, "role.assigned", "role", form.role_id, details);

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
    pool: web::Data<DbPool>,
    session: Session,
    form: web::Form<UnassignForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.assign")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let conn = pool.get()?;

    // Last-admin protection: don't allow removing admin role if this is the last admin
    let is_admin_role: bool = conn.query_row(
        "SELECT name = 'admin' FROM entities WHERE id = ?1 AND entity_type = 'role'",
        rusqlite::params![form.role_id],
        |row| row.get(0),
    ).unwrap_or(false);

    if is_admin_role {
        let admin_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM relations \
             WHERE relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
             AND target_id = ?1",
            rusqlite::params![form.role_id],
            |row| row.get(0),
        ).unwrap_or(0);

        if admin_count <= 1 {
            let _ = session.insert("flash", "Cannot remove role: this is the last administrator");
            return Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/roles"))
                .finish());
        }
    }

    relation::delete(&conn, "has_role", form.user_id, form.role_id)?;

    // Audit
    let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "user_id": form.user_id,
        "role_id": form.role_id,
        "summary": "Unassigned role from user"
    });
    let _ = crate::audit::log(&conn, current_user_id, "role.unassigned", "role", form.role_id, details);

    let _ = session.insert("flash", "Role unassigned");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/roles"))
        .finish())
}
