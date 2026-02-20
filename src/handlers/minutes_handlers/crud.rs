use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::models::minutes;
use crate::auth::csrf;
use crate::auth::session::{require_permission, get_user_id};
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, MinutesViewTemplate};

/// Generate minutes scaffold for a meeting.
pub async fn generate_minutes(
    pool: web::Data<DbPool>,
    session: Session,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "minutes.generate")?;
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    let conn = pool.get()?;

    let meeting_id: i64 = form.get("meeting_id")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let tor_id: i64 = form.get("tor_id")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let meeting_name = form.get("meeting_name").map(|s| s.as_str()).unwrap_or("Meeting");

    if meeting_id == 0 || tor_id == 0 {
        let _ = session.insert("flash", "Invalid meeting or ToR");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", "/tor"))
            .finish());
    }

    // Check if minutes already exist for this meeting
    if minutes::find_by_meeting(&conn, meeting_id)?.is_some() {
        let _ = session.insert("flash", "Minutes already exist for this meeting");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", "/tor"))
            .finish());
    }

    let minutes_id = minutes::generate_scaffold(&conn, meeting_id, tor_id, meeting_name)?;

    let current_user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "meeting_id": meeting_id,
        "tor_id": tor_id,
        "minutes_id": minutes_id,
        "summary": "Generated minutes scaffold"
    });
    let _ = crate::audit::log(&conn, current_user_id, "minutes.generated", "minutes", minutes_id, details);

    let _ = session.insert("flash", "Minutes generated");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/minutes/{minutes_id}")))
        .finish())
}

/// View minutes with all sections.
pub async fn view_minutes(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "minutes.edit")?;

    let minutes_id = path.into_inner();
    let conn = pool.get()?;

    match minutes::find_by_id(&conn, minutes_id)? {
        Some(mins) => {
            let ctx = PageContext::build(&session, &conn, "/minutes")?;
            let sections = minutes::find_sections(&conn, minutes_id)?;
            let tmpl = MinutesViewTemplate {
                ctx,
                minutes: mins,
                sections,
            };
            render(tmpl)
        }
        None => Err(AppError::NotFound),
    }
}

/// Update a section's content.
pub async fn update_section(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "minutes.edit")?;
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    let (minutes_id, section_id) = path.into_inner();
    let conn = pool.get()?;

    // Check if minutes are approved (read-only)
    if let Some(mins) = minutes::find_by_id(&conn, minutes_id)?
        && mins.status == "approved"
    {
        let _ = session.insert("flash", "Cannot edit approved minutes");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", format!("/minutes/{minutes_id}")))
            .finish());
    }

    let content = form.get("content").map(|s| s.as_str()).unwrap_or("");
    minutes::update_section_content(&conn, section_id, content)?;

    let current_user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "section_id": section_id,
        "summary": "Updated minutes section"
    });
    let _ = crate::audit::log(&conn, current_user_id, "minutes.section_edited", "minutes", minutes_id, details);

    let _ = session.insert("flash", "Section updated");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/minutes/{minutes_id}")))
        .finish())
}

/// Update minutes status (draft -> pending_approval -> approved).
pub async fn update_minutes_status(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    let minutes_id = path.into_inner();
    let conn = pool.get()?;

    let new_status = form.get("status").map(|s| s.as_str()).unwrap_or("");
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    // Permission check based on target status
    match new_status {
        "pending_approval" => require_permission(&session, "minutes.edit")?,
        "approved" => require_permission(&session, "minutes.approve")?,
        _ => {
            let _ = session.insert("flash", "Invalid status");
            return Ok(HttpResponse::SeeOther()
                .insert_header(("Location", format!("/minutes/{minutes_id}")))
                .finish());
        }
    }

    // Validate transition
    if let Some(mins) = minutes::find_by_id(&conn, minutes_id)? {
        let valid_transition = matches!(
            (mins.status.as_str(), new_status),
            ("draft", "pending_approval") | ("pending_approval", "approved")
        );
        if !valid_transition {
            let _ = session.insert("flash", format!("Cannot transition from {} to {}", mins.status, new_status));
            return Ok(HttpResponse::SeeOther()
                .insert_header(("Location", format!("/minutes/{minutes_id}")))
                .finish());
        }
    }

    minutes::update_status(&conn, minutes_id, new_status)?;

    let current_user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "new_status": new_status,
        "summary": format!("Minutes status changed to {}", new_status)
    });
    let _ = crate::audit::log(&conn, current_user_id, "minutes.status_changed", "minutes", minutes_id, details);

    let _ = session.insert("flash", format!("Minutes status updated to {}", new_status));
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/minutes/{minutes_id}")))
        .finish())
}

/// Convert newline-separated textarea text into a JSON array string.
/// Filters empty lines. Returns "[]" if no items.
fn lines_to_json(text: &str) -> String {
    let items: Vec<&str> = text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string())
}

#[derive(serde::Deserialize)]
pub struct DistributionForm {
    pub csrf_token: String,
    pub distribution_list: String,
}

#[derive(serde::Deserialize)]
pub struct AttendanceForm {
    pub csrf_token: String,
    pub structured_attendance: String,
}

#[derive(serde::Deserialize)]
pub struct ActionItemsForm {
    pub csrf_token: String,
    pub structured_action_items: String,
}

/// Save the distribution list for a minutes document.
pub async fn save_distribution(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<DistributionForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "minutes.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;
    let minutes_id = path.into_inner();
    let conn = pool.get()?;

    if let Some(m) = minutes::find_by_id(&conn, minutes_id)?
        && m.status == "approved"
    {
        let _ = session.insert("flash", "Cannot edit approved minutes");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", format!("/minutes/{}", minutes_id)))
            .finish());
    }

    let json = lines_to_json(&form.distribution_list);
    minutes::update_distribution_list(&conn, minutes_id, &json)?;

    let user_id = get_user_id(&session).unwrap_or(0);
    let _ = crate::audit::log(&conn, user_id, "minutes.distribution_saved", "minutes", minutes_id,
        serde_json::json!({"minutes_id": minutes_id, "summary": "Distribution list updated"}));

    let _ = session.insert("flash", "Distribution list saved");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/minutes/{}", minutes_id)))
        .finish())
}

/// Save structured attendance for a minutes document.
pub async fn save_attendance(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<AttendanceForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "minutes.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;
    let minutes_id = path.into_inner();
    let conn = pool.get()?;

    if let Some(m) = minutes::find_by_id(&conn, minutes_id)?
        && m.status == "approved"
    {
        let _ = session.insert("flash", "Cannot edit approved minutes");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", format!("/minutes/{}", minutes_id)))
            .finish());
    }

    minutes::update_structured_attendance(&conn, minutes_id, &form.structured_attendance)?;

    let user_id = get_user_id(&session).unwrap_or(0);
    let _ = crate::audit::log(&conn, user_id, "minutes.attendance_saved", "minutes", minutes_id,
        serde_json::json!({"minutes_id": minutes_id, "summary": "Attendance updated"}));

    let _ = session.insert("flash", "Attendance saved");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/minutes/{}", minutes_id)))
        .finish())
}

/// Save structured action items for a minutes document.
pub async fn save_action_items(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<ActionItemsForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "minutes.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;
    let minutes_id = path.into_inner();
    let conn = pool.get()?;

    if let Some(m) = minutes::find_by_id(&conn, minutes_id)?
        && m.status == "approved"
    {
        let _ = session.insert("flash", "Cannot edit approved minutes");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", format!("/minutes/{}", minutes_id)))
            .finish());
    }

    minutes::update_structured_action_items(&conn, minutes_id, &form.structured_action_items)?;

    let user_id = get_user_id(&session).unwrap_or(0);
    let _ = crate::audit::log(&conn, user_id, "minutes.action_items_saved", "minutes", minutes_id,
        serde_json::json!({"minutes_id": minutes_id, "summary": "Action items updated"}));

    let _ = session.insert("flash", "Action items saved");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/minutes/{}", minutes_id)))
        .finish())
}
