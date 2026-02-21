use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

use crate::models::tor;
use crate::auth::csrf;
use crate::auth::session::require_permission;
use crate::errors::AppError;

pub async fn handle_add_dependency(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    let tor_id = path.into_inner();

    let target_tor_id: i64 = form.get("target_tor_id")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let relation_type = form.get("relation_type").map(|s| s.as_str()).unwrap_or("feeds_into");
    let output_types = form.get("output_types").map(|s| s.as_str()).unwrap_or("");
    let description = form.get("description").map(|s| s.as_str()).unwrap_or("");
    let is_blocking = form.get("is_blocking").map(|s| s.as_str()) == Some("true");

    // Prevent self-referencing
    if target_tor_id == tor_id || target_tor_id == 0 {
        let _ = session.insert("flash", "Please select a different ToR");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", format!("/tor/{tor_id}")))
            .finish());
    }

    tor::add_dependency(&pool, tor_id, target_tor_id, relation_type, output_types, description, is_blocking).await?;

    let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "target_tor_id": target_tor_id,
        "relation_type": relation_type,
        "summary": format!("Added {} dependency", relation_type)
    });
    let _ = crate::audit::log(&pool, current_user_id, "tor.dependency_added", "tor", tor_id, details).await;

    let _ = session.insert("flash", "Dependency added");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}")))
        .finish())
}

pub async fn handle_remove_dependency(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    let (tor_id, relation_id) = path.into_inner();

    tor::remove_dependency(&pool, relation_id).await?;

    let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "relation_id": relation_id,
        "summary": "Removed dependency"
    });
    let _ = crate::audit::log(&pool, current_user_id, "tor.dependency_removed", "tor", tor_id, details).await;

    let _ = session.insert("flash", "Dependency removed");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}")))
        .finish())
}
