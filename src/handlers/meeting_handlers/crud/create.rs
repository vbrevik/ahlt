/// Meeting creation operations.
///
/// Handles POST requests to create new meetings or confirm projected meetings.
/// Includes both form-based confirmation and calendar-based confirmation.

use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

use crate::auth::abac;
use crate::auth::csrf;
use crate::auth::session::get_user_id;
use crate::errors::AppError;
use crate::models::meeting;

use super::forms::{ConfirmForm, CalendarConfirmForm};
use super::helpers::parse_and_validate_date;

// ---------------------------------------------------------------------------
// POST — confirm a projected meeting
// ---------------------------------------------------------------------------

/// POST /tor/{id}/meetings/confirm — confirm a scheduled meeting.
///
/// Creates the meeting entity (which starts as "projected" internally) then
/// immediately transitions it to "confirmed".
pub async fn confirm(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<ConfirmForm>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let tor_id = path.into_inner();
    abac::require_tor_capability(&pool, &session, tor_id, "can_call_meetings").await?;

    let location = form.location.as_deref().unwrap_or("");
    let notes = form.notes.as_deref().unwrap_or("");
    let meeting_number = form.meeting_number.as_deref().unwrap_or("");
    let classification = form.classification.as_deref().unwrap_or("");
    let vtc_details = form.vtc_details.as_deref().unwrap_or("");
    let chair_user_id = form.chair_user_id.as_deref().unwrap_or("");
    let secretary_user_id = form.secretary_user_id.as_deref().unwrap_or("");

    let meeting_id = meeting::create(
        &pool,
        tor_id,
        &form.meeting_date,
        &form.tor_name,
        location,
        notes,
        meeting_number,
        classification,
        vtc_details,
        chair_user_id,
        secretary_user_id,
    ).await?;

    // Immediately transition to "confirmed" status.
    meeting::update_status(&pool, meeting_id, "confirmed").await?;

    // Audit
    let current_user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "meeting_id": meeting_id,
        "tor_id": tor_id,
        "meeting_date": &form.meeting_date,
        "summary": format!("Meeting confirmed for {} on {}", &form.tor_name, &form.meeting_date),
    });
    let _ = crate::audit::log(
        &pool,
        current_user_id,
        "meeting.confirmed",
        "meeting",
        meeting_id,
        details,
    ).await;

    let _ = session.insert("flash", "Meeting confirmed successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header((
            "Location",
            format!("/tor/{}/meetings/{}", tor_id, meeting_id),
        ))
        .finish())
}

// ---------------------------------------------------------------------------
// POST — confirm from calendar (returns JSON)
// ---------------------------------------------------------------------------

/// POST /api/tor/{id}/meetings/confirm-calendar — confirm a meeting from the calendar view.
///
/// Returns JSON `{"ok":true,"meeting_id":N}` on success.
/// Handles two cases:
///   - meeting_id present  -> meeting already exists as "projected", just update status
///   - meeting_id absent   -> cadence slot, create the meeting entity then confirm it
pub async fn confirm_calendar(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<CalendarConfirmForm>,
) -> Result<HttpResponse, AppError> {
    if csrf::validate_csrf(&session, &form.csrf_token).is_err() {
        return Ok(HttpResponse::Forbidden()
            .content_type("application/json")
            .body(serde_json::json!({"ok": false, "error": "CSRF token invalid"}).to_string()));
    }

    let tor_id = path.into_inner();
    let current_user_id = get_user_id(&session).unwrap_or(0);

    // Two-phase access check: global tor.edit bypass OR resource-scoped ABAC capability.
    let has_abac = abac::has_resource_capability(
        &pool,
        current_user_id,
        tor_id,
        "belongs_to_tor",
        "can_call_meetings",
    )
    .await
    .unwrap_or(false);

    let has_global_permission = crate::auth::session::require_permission(&session, "tor.edit").is_ok();
    if !has_global_permission && !has_abac {
        return Ok(HttpResponse::Forbidden()
            .content_type("application/json")
            .body(serde_json::json!({"ok": false, "error": "Permission denied"}).to_string()));
    }

    // Validate date format
    if parse_and_validate_date(&form.meeting_date).is_err() {
        return Ok(HttpResponse::BadRequest()
            .content_type("application/json")
            .body(serde_json::json!({"ok": false, "error": "Invalid meeting_date format, expected YYYY-MM-DD"}).to_string()));
    }

    // Validate date is in the future
    let parsed_date = match parse_and_validate_date(&form.meeting_date) {
        Ok(d) => d,
        Err(_) => {
            return Ok(HttpResponse::BadRequest()
                .content_type("application/json")
                .body(serde_json::json!({"ok": false, "error": "Invalid date"}).to_string()));
        }
    };
    let today = chrono::Local::now().naive_local().date();
    if parsed_date <= today {
        return Ok(HttpResponse::BadRequest()
            .content_type("application/json")
            .body(serde_json::json!({"ok": false, "error": "Cannot confirm meetings in the past"}).to_string()));
    }

    let meeting_id = if let Some(mid) = form.meeting_id {
        // Meeting already exists — verify ownership then update status
        let existing = match meeting::find_by_id(&pool, mid).await {
            Ok(Some(m)) => m,
            Ok(None) => {
                return Ok(HttpResponse::NotFound()
                    .content_type("application/json")
                    .body(serde_json::json!({"ok": false, "error": "Meeting not found"}).to_string()));
            }
            Err(_) => {
                return Ok(HttpResponse::InternalServerError()
                    .content_type("application/json")
                    .body(serde_json::json!({"ok": false, "error": "Database error"}).to_string()));
            }
        };

        if existing.tor_id != tor_id {
            return Ok(HttpResponse::NotFound()
                .content_type("application/json")
                .body(serde_json::json!({"ok": false, "error": "Meeting not found"}).to_string()));
        }
        if existing.status != "projected" {
            return Ok(HttpResponse::BadRequest()
                .content_type("application/json")
                .body(serde_json::json!({"ok": false, "error": format!("Meeting is already '{}' and cannot be confirmed", existing.status)}).to_string()));
        }
        match meeting::update_status(&pool, mid, "confirmed").await {
            Ok(_) => mid,
            Err(_) => {
                return Ok(HttpResponse::InternalServerError()
                    .content_type("application/json")
                    .body(serde_json::json!({"ok": false, "error": "Failed to update meeting"}).to_string()));
            }
        }
    } else {
        // No persisted meeting yet — create it and confirm in one step
        let mid = match meeting::create(&pool, tor_id, &form.meeting_date, &form.tor_name, "", "", "", "", "", "", "").await {
            Ok(id) => id,
            Err(_) => {
                return Ok(HttpResponse::InternalServerError()
                    .content_type("application/json")
                    .body(serde_json::json!({"ok": false, "error": "Failed to create meeting"}).to_string()));
            }
        };
        match meeting::update_status(&pool, mid, "confirmed").await {
            Ok(_) => mid,
            Err(_) => {
                return Ok(HttpResponse::InternalServerError()
                    .content_type("application/json")
                    .body(serde_json::json!({"ok": false, "error": "Failed to confirm meeting"}).to_string()));
            }
        }
    };

    let details = serde_json::json!({
        "meeting_id": meeting_id,
        "tor_id": tor_id,
        "meeting_date": &form.meeting_date,
        "summary": format!("Meeting confirmed for {} on {}", &form.tor_name, &form.meeting_date),
    });
    let _ = crate::audit::log(
        &pool,
        current_user_id,
        "meeting.confirmed",
        "meeting",
        meeting_id,
        details,
    ).await;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::json!({"ok": true, "meeting_id": meeting_id}).to_string()))
}
