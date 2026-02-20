use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::models::protocol;
use crate::auth::csrf;
use crate::auth::session::require_permission;
use crate::errors::AppError;

pub async fn add_step(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    let tor_id = path.into_inner();
    let conn = pool.get()?;

    let name = form.get("name").map(|s| s.as_str()).unwrap_or("");
    let label = form.get("label").map(|s| s.as_str()).unwrap_or("");
    let step_type = form.get("step_type").map(|s| s.as_str()).unwrap_or("procedural");
    let sequence_order: i64 = form.get("sequence_order")
        .and_then(|s| s.parse().ok())
        .unwrap_or(99);
    let duration: Option<i64> = form.get("default_duration_minutes")
        .and_then(|s| if s.is_empty() { None } else { s.parse().ok() });
    let description = form.get("description").map(|s| s.as_str()).unwrap_or("");
    let is_required = form.get("is_required").map(|s| s.as_str()) == Some("true");
    let responsible = form.get("responsible").map(|s| s.as_str()).unwrap_or("");

    if name.trim().is_empty() || label.trim().is_empty() {
        let _ = session.insert("flash", "Name and label are required");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", format!("/tor/{tor_id}")))
            .finish());
    }

    protocol::create_step(&conn, tor_id, name.trim(), label.trim(), step_type,
                          sequence_order, duration, description, is_required, responsible.trim())?;

    let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "name": name.trim(),
        "label": label.trim(),
        "step_type": step_type,
        "summary": "Added protocol step"
    });
    let _ = crate::audit::log(&conn, current_user_id, "tor.protocol_step_added", "tor", tor_id, details);

    let _ = session.insert("flash", "Protocol step added");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}")))
        .finish())
}

pub async fn delete_step(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    let (tor_id, step_id) = path.into_inner();
    let conn = pool.get()?;

    protocol::delete_step(&conn, step_id)?;

    let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "step_id": step_id,
        "summary": "Removed protocol step"
    });
    let _ = crate::audit::log(&conn, current_user_id, "tor.protocol_step_removed", "tor", tor_id, details);

    let _ = session.insert("flash", "Protocol step removed");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}")))
        .finish())
}

pub async fn move_step(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    let (tor_id, step_id) = path.into_inner();
    let conn = pool.get()?;

    let direction = form.get("direction").map(|s| s.as_str()).unwrap_or("");

    // Load all steps for this ToR to find the neighbor
    let steps = protocol::find_steps_for_tor(&conn, tor_id)?;
    let current_idx = steps.iter().position(|s| s.id == step_id);

    if let Some(idx) = current_idx {
        let swap_idx = match direction {
            "up" if idx > 0 => Some(idx - 1),
            "down" if idx + 1 < steps.len() => Some(idx + 1),
            _ => None,
        };

        if let Some(other_idx) = swap_idx {
            protocol::reorder_steps(&conn, steps[idx].id, steps[other_idx].id)?;
        }
    }

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}")))
        .finish())
}
