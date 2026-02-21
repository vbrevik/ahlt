use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

use crate::models::tor;
use crate::auth::csrf;
use crate::auth::session::require_permission;
use crate::errors::AppError;

pub async fn manage_members(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.manage_members")?;
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    let tor_id = path.into_inner();

    let action = form.get("action").map(|s| s.as_str()).unwrap_or("");
    let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);

    match action {
        "assign" => {
            let user_id: i64 = form.get("user_id")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let position_id: i64 = form.get("position_id")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let membership_type = form.get("membership_type")
                .map(|s| s.as_str())
                .unwrap_or("optional");

            if user_id == 0 || position_id == 0 {
                let _ = session.insert("flash", "Please select a user and position");
                return Ok(HttpResponse::SeeOther()
                    .insert_header(("Location", format!("/tor/{tor_id}")))
                    .finish());
            }

            tor::assign_to_position(&pool, user_id, position_id, membership_type).await?;
            let details = serde_json::json!({
                "user_id": user_id,
                "position_id": position_id,
                "membership_type": membership_type,
                "summary": "Assigned user to position"
            });
            let _ = crate::audit::log(&pool, current_user_id, "tor.position_assigned", "tor", tor_id, details).await;
            let _ = session.insert("flash", "User assigned to position");
        }
        "vacate" => {
            let position_id: i64 = form.get("position_id")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            if position_id == 0 {
                let _ = session.insert("flash", "Invalid position");
                return Ok(HttpResponse::SeeOther()
                    .insert_header(("Location", format!("/tor/{tor_id}")))
                    .finish());
            }

            tor::vacate_position(&pool, position_id).await?;
            let details = serde_json::json!({
                "position_id": position_id,
                "summary": "Vacated position"
            });
            let _ = crate::audit::log(&pool, current_user_id, "tor.position_vacated", "tor", tor_id, details).await;
            let _ = session.insert("flash", "Position vacated");
        }
        _ => {
            let _ = session.insert("flash", "Unknown action");
        }
    }

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}")))
        .finish())
}
