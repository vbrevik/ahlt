use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::models::tor;
use crate::auth::csrf;
use crate::auth::session::require_permission;
use crate::errors::AppError;

pub async fn manage_members(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.manage_members")?;
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    let tor_id = path.into_inner();
    let conn = pool.get()?;

    let action = form.get("action").map(|s| s.as_str()).unwrap_or("");
    let user_id: i64 = form.get("user_id")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if user_id == 0 {
        let _ = session.insert("flash", "Please select a user");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", format!("/tor/{tor_id}")))
            .finish());
    }

    let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);

    match action {
        "add" => {
            tor::add_member(&conn, user_id, tor_id)?;
            let details = serde_json::json!({
                "user_id": user_id,
                "summary": format!("Added member to ToR")
            });
            let _ = crate::audit::log(&conn, current_user_id, "tor.member_added", "tor", tor_id, details);
            let _ = session.insert("flash", "Member added successfully");
        }
        "remove" => {
            tor::remove_member(&conn, user_id, tor_id)?;
            let details = serde_json::json!({
                "user_id": user_id,
                "summary": format!("Removed member from ToR")
            });
            let _ = crate::audit::log(&conn, current_user_id, "tor.member_removed", "tor", tor_id, details);
            let _ = session.insert("flash", "Member removed");
        }
        _ => {
            let _ = session.insert("flash", "Unknown action");
        }
    }

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}")))
        .finish())
}
