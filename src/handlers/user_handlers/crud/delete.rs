use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use serde::Deserialize;

use crate::models::user;
use crate::auth::session::{get_user_id, require_permission};
use crate::errors::AppError;
use crate::handlers::auth_handlers::CsrfOnly;
use crate::handlers::warning_handlers::ws::ConnectionMap;
use super::helpers::{is_last_admin, is_last_admin_bulk, fetch_user_for_audit};

pub async fn delete(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<CsrfOnly>,
    conn_map: web::Data<ConnectionMap>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.delete")?;
    crate::auth::csrf::validate_csrf(&session, &form.csrf_token)?;

    let id = path.into_inner();
    let current_user_id = get_user_id(&session).unwrap_or(0);

    // Self-deletion protection
    if id == current_user_id {
        let _ = session.insert("flash", "You cannot delete your own account");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", "/users"))
            .finish());
    }

    // Last-admin protection
    if is_last_admin(&pool, id).await? {
        let _ = session.insert("flash", "Cannot delete the last administrator");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", "/users"))
            .finish());
    }

    // Fetch user details before deletion for audit log
    let user_details = fetch_user_for_audit(&pool, id).await;

    match user::delete(&pool, id).await {
        Ok(_) => {
            // Audit log
            if let Some(deleted_user) = user_details {
                let details = serde_json::json!({
                    "username": deleted_user.username,
                    "summary": format!("Deleted user '{}'", deleted_user.username)
                });
                let _ = crate::audit::log(&pool, current_user_id, "user.deleted",
                                          "user", id, details).await;

                // Generate warning for admins
                let msg = format!("User '{}' was deleted", deleted_user.username);
                if let Ok(wid) = crate::warnings::create_warning(
                    &pool, "medium", "governance", "event.user.deleted", &msg, "", "system"
                ).await {
                    let admins = crate::warnings::get_users_with_permission(&pool, "admin.settings").await.unwrap_or_default();
                    if !admins.is_empty() {
                        let _ = crate::warnings::create_receipts(&pool, wid, &admins).await;
                        crate::handlers::warning_handlers::ws::notify_users(
                            &conn_map, &pool, &admins, wid, "medium", &msg,
                        ).await;
                    }
                }
            }

            let _ = session.insert("flash", "User deleted");
            Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/users"))
                .finish())
        }
        Err(_) => {
            let _ = session.insert("flash", "Error deleting user");
            Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/users"))
                .finish())
        }
    }
}

#[derive(Deserialize)]
pub struct BulkDeleteForm {
    pub csrf_token: String,
    pub user_ids: String, // JSON array string: "[1, 2, 3]"
}

pub async fn bulk_delete(
    pool: web::Data<PgPool>,
    session: Session,
    form: web::Form<BulkDeleteForm>,
    conn_map: web::Data<ConnectionMap>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.delete")?;
    crate::auth::csrf::validate_csrf(&session, &form.csrf_token)?;

    let current_user_id = get_user_id(&session).unwrap_or(0);

    // Parse the JSON array of user IDs
    let user_ids: Vec<i64> = match serde_json::from_str(&form.user_ids) {
        Ok(ids) => ids,
        Err(_) => {
            let _ = session.insert("flash", "Invalid user IDs");
            return Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/users"))
                .finish());
        }
    };

    if user_ids.is_empty() {
        let _ = session.insert("flash", "No users selected");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", "/users"))
            .finish());
    }

    let mut deleted_count = 0;
    let mut error_count = 0;

    for id in user_ids {
        // Self-deletion protection
        if id == current_user_id {
            error_count += 1;
            continue;
        }

        // Last-admin protection (bulk version with safe defaults)
        if is_last_admin_bulk(&pool, id).await {
            error_count += 1;
            continue;
        }

        // Fetch user details before deletion for audit log
        let user_details = fetch_user_for_audit(&pool, id).await;

        if user::delete(&pool, id).await.is_ok() {
            deleted_count += 1;

            // Audit log
            if let Some(deleted_user) = user_details {
                let details = serde_json::json!({
                    "username": deleted_user.username,
                    "summary": format!("Deleted user '{}' (bulk delete)", deleted_user.username)
                });
                let _ = crate::audit::log(&pool, current_user_id, "user.deleted",
                                          "user", id, details).await;

                // Generate warning for admins
                let msg = format!("User '{}' was deleted", deleted_user.username);
                if let Ok(wid) = crate::warnings::create_warning(
                    &pool, "medium", "governance", "event.user.deleted", &msg, "", "system"
                ).await {
                    let admins = crate::warnings::get_users_with_permission(&pool, "admin.settings").await.unwrap_or_default();
                    if !admins.is_empty() {
                        let _ = crate::warnings::create_receipts(&pool, wid, &admins).await;
                        crate::handlers::warning_handlers::ws::notify_users(
                            &conn_map, &pool, &admins, wid, "medium", &msg,
                        ).await;
                    }
                }
            }
        } else {
            error_count += 1;
        }
    }

    // Build summary message
    let msg = if error_count == 0 {
        format!("Deleted {} user{}", deleted_count, if deleted_count == 1 { "" } else { "s" })
    } else if deleted_count == 0 {
        "Could not delete selected users".to_string()
    } else {
        format!("Deleted {} user{} ({} failed)", deleted_count, if deleted_count == 1 { "" } else { "s" }, error_count)
    };

    let _ = session.insert("flash", &msg);
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/users"))
        .finish())
}
