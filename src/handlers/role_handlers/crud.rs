use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::models::role;
use crate::auth::csrf;
use crate::auth::session::require_permission;
use crate::errors::AppError;
use crate::handlers::auth_handlers::CsrfOnly;

pub async fn delete(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<CsrfOnly>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.manage")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let id = path.into_inner();

    let conn = pool.get()?;

    let user_count = role::count_users(&conn, id)?;
    if user_count > 0 {
        let _ = session.insert("flash", format!("Cannot delete role: {user_count} user(s) still assigned"));
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", "/roles"))
            .finish());
    }

    let role_details = role::find_detail_by_id(&conn, id).ok().flatten();

    match role::delete(&conn, id) {
        Ok(_) => {
            let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
            if let Some(deleted_role) = role_details {
                let details = serde_json::json!({
                    "role_name": deleted_role.name,
                    "summary": format!("Deleted role '{}'", deleted_role.label)
                });
                let _ = crate::audit::log(&conn, current_user_id, "role.deleted",
                                          "role", id, details);
            }

            let _ = session.insert("flash", "Role deleted");
            Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/roles"))
                .finish())
        }
        Err(_) => {
            let _ = session.insert("flash", "Error deleting role");
            Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/roles"))
                .finish())
        }
    }
}
