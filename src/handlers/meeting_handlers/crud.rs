use actix_session::Session;
use actix_web::{web, HttpResponse};
use std::collections::HashMap;
use sqlx::PgPool;

use crate::auth::abac;
use crate::auth::csrf;
use crate::auth::session::{get_permissions, get_user_id, require_permission};
use crate::errors::{render, AppError};
use crate::models::meeting;
use crate::models::minutes;
use crate::models::protocol;
use crate::models::workflow;
use crate::templates_structs::{MeetingDetailTemplate, PageContext};

// ---------------------------------------------------------------------------
// Form structs
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
pub struct ConfirmForm {
    pub csrf_token: String,
    pub meeting_date: String,
    pub tor_name: String,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub meeting_number: Option<String>,
    pub classification: Option<String>,
    pub vtc_details: Option<String>,
    pub chair_user_id: Option<String>,
    pub secretary_user_id: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct CalendarConfirmForm {
    pub csrf_token: String,
    pub meeting_date: String,
    pub tor_name: String,
    pub meeting_id: Option<i64>,
}

#[derive(serde::Deserialize)]
pub struct TransitionForm {
    pub csrf_token: String,
    pub new_status: String,
}

#[derive(serde::Deserialize)]
pub struct AgendaForm {
    pub csrf_token: String,
    pub agenda_point_id: i64,
}

#[derive(serde::Deserialize)]
pub struct CsrfOnly {
    pub csrf_token: String,
}

// ---------------------------------------------------------------------------
// GET — meeting detail
// ---------------------------------------------------------------------------

/// GET /tor/{id}/meetings/{mid} — meeting detail page.
pub async fn detail(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "meetings.view")?;
    let (tor_id, mid) = path.into_inner();
    let ctx = PageContext::build(&session, &pool, "/meetings").await?;

    let meeting = meeting::find_by_id(&pool, mid).await?
        .ok_or(AppError::NotFound)?;

    // Verify the meeting belongs to the requested ToR
    if meeting.tor_id != tor_id {
        return Err(AppError::NotFound);
    }

    let agenda_points = meeting::find_agenda_points(&pool, mid).await?;
    let unassigned_points = meeting::find_unassigned_agenda_points(&pool, tor_id).await?;
    let protocol_steps = protocol::find_steps_for_tor(&pool, tor_id).await?;
    let permissions = get_permissions(&session)
        .map_err(|e| AppError::Session(format!("Failed to get permissions: {}", e)))?;
    let transitions = workflow::find_available_transitions(
        &pool,
        "meeting",
        &meeting.status,
        &permissions,
        &HashMap::new(),
    ).await?;
    let existing_minutes = minutes::find_by_meeting(&pool, mid).await?;
    let user_id = get_user_id(&session).unwrap_or(0);
    let tor_capabilities = abac::load_tor_capabilities(&pool, user_id, tor_id)
        .await
        .unwrap_or_default();

    let tmpl = MeetingDetailTemplate {
        ctx,
        meeting,
        agenda_points,
        unassigned_points,
        protocol_steps,
        transitions,
        minutes: existing_minutes,
        tor_id,
        tor_capabilities,
    };
    render(tmpl)
}

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
    let has_access = require_permission(&session, "tor.edit").is_ok() || has_abac;
    if !has_access {
        return Ok(HttpResponse::Forbidden()
            .content_type("application/json")
            .body(serde_json::json!({"ok": false, "error": "Permission denied"}).to_string()));
    }

    // Validate date format
    if chrono::NaiveDate::parse_from_str(&form.meeting_date, "%Y-%m-%d").is_err() {
        return Ok(HttpResponse::BadRequest()
            .content_type("application/json")
            .body(serde_json::json!({"ok": false, "error": "Invalid meeting_date format, expected YYYY-MM-DD"}).to_string()));
    }

    // Validate date is in the future
    let parsed_date = match chrono::NaiveDate::parse_from_str(&form.meeting_date, "%Y-%m-%d") {
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

// ---------------------------------------------------------------------------
// POST — transition meeting lifecycle state
// ---------------------------------------------------------------------------

/// POST /tor/{id}/meetings/{mid}/transition — advance meeting lifecycle state.
pub async fn transition(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<TransitionForm>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, mid) = path.into_inner();
    abac::require_tor_capability(&pool, &session, tor_id, "can_call_meetings").await?;

    let meeting_detail = meeting::find_by_id(&pool, mid).await?
        .ok_or(AppError::NotFound)?;

    if meeting_detail.tor_id != tor_id {
        return Err(AppError::NotFound);
    }

    let permissions = get_permissions(&session)
        .map_err(|e| AppError::Session(format!("Failed to get permissions: {}", e)))?;

    // Validate the transition via the workflow engine (returns error if invalid).
    workflow::validate_transition(
        &pool,
        "meeting",
        &meeting_detail.status,
        &form.new_status,
        &permissions,
        &HashMap::new(),
    ).await?;

    meeting::update_status(&pool, mid, &form.new_status).await?;

    // Audit
    let current_user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "meeting_id": mid,
        "tor_id": tor_id,
        "from_status": &meeting_detail.status,
        "to_status": &form.new_status,
        "summary": format!("Meeting transitioned from {} to {}", &meeting_detail.status, &form.new_status),
    });
    let _ = crate::audit::log(
        &pool,
        current_user_id,
        "meeting.transition",
        "meeting",
        mid,
        details,
    ).await;

    let _ = session.insert(
        "flash",
        format!("Meeting status changed to {}", &form.new_status),
    );
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{}/meetings/{}", tor_id, mid)))
        .finish())
}

// ---------------------------------------------------------------------------
// POST — assign agenda point to meeting
// ---------------------------------------------------------------------------

/// POST /tor/{id}/meetings/{mid}/agenda/assign — assign an agenda point to a meeting.
pub async fn assign_agenda(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<AgendaForm>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, mid) = path.into_inner();
    abac::require_tor_capability(&pool, &session, tor_id, "can_manage_agenda").await?;

    meeting::assign_agenda(&pool, mid, form.agenda_point_id).await?;

    // Audit
    let current_user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "meeting_id": mid,
        "tor_id": tor_id,
        "agenda_point_id": form.agenda_point_id,
        "summary": format!("Agenda point {} assigned to meeting {}", form.agenda_point_id, mid),
    });
    let _ = crate::audit::log(
        &pool,
        current_user_id,
        "meeting.agenda_assigned",
        "meeting",
        mid,
        details,
    ).await;

    let _ = session.insert("flash", "Agenda point assigned to meeting");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{}/meetings/{}", tor_id, mid)))
        .finish())
}

// ---------------------------------------------------------------------------
// POST — remove agenda point from meeting
// ---------------------------------------------------------------------------

/// POST /tor/{id}/meetings/{mid}/agenda/remove — remove an agenda point from a meeting.
pub async fn remove_agenda(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<AgendaForm>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, mid) = path.into_inner();
    abac::require_tor_capability(&pool, &session, tor_id, "can_manage_agenda").await?;

    meeting::remove_agenda(&pool, mid, form.agenda_point_id).await?;

    // Audit
    let current_user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "meeting_id": mid,
        "tor_id": tor_id,
        "agenda_point_id": form.agenda_point_id,
        "summary": format!("Agenda point {} removed from meeting {}", form.agenda_point_id, mid),
    });
    let _ = crate::audit::log(
        &pool,
        current_user_id,
        "meeting.agenda_removed",
        "meeting",
        mid,
        details,
    ).await;

    let _ = session.insert("flash", "Agenda point removed from meeting");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{}/meetings/{}", tor_id, mid)))
        .finish())
}

// ---------------------------------------------------------------------------
// POST — generate minutes scaffold
// ---------------------------------------------------------------------------

/// POST /tor/{id}/meetings/{mid}/minutes/generate — generate minutes for a meeting.
pub async fn generate_minutes(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<CsrfOnly>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, mid) = path.into_inner();
    abac::require_tor_capability(&pool, &session, tor_id, "can_record_decisions").await?;

    let meeting_detail = meeting::find_by_id(&pool, mid).await?
        .ok_or(AppError::NotFound)?;

    if meeting_detail.tor_id != tor_id {
        return Err(AppError::NotFound);
    }

    // Only completed meetings can have minutes generated.
    if meeting_detail.status != "completed" {
        return Err(AppError::PermissionDenied(
            "Minutes can only be generated for completed meetings".to_string(),
        ));
    }

    // Prevent duplicate generation.
    if minutes::find_by_meeting(&pool, mid).await?.is_some() {
        return Err(AppError::PermissionDenied(
            "Minutes already exist for this meeting".to_string(),
        ));
    }

    let minutes_id =
        minutes::generate_scaffold(&pool, mid, tor_id, &meeting_detail.label).await?;

    // Audit
    let current_user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "meeting_id": mid,
        "minutes_id": minutes_id,
        "tor_id": tor_id,
        "summary": format!("Minutes generated for meeting {}", mid),
    });
    let _ = crate::audit::log(
        &pool,
        current_user_id,
        "meeting.minutes_generated",
        "meeting",
        mid,
        details,
    ).await;

    let _ = session.insert("flash", "Minutes generated successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/minutes/{}", minutes_id)))
        .finish())
}

// ---------------------------------------------------------------------------
// POST — save roll call data
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
pub struct RollCallForm {
    pub csrf_token: String,
    pub roll_call_data: String, // raw JSON string from hidden input
}

/// POST /tor/{id}/meetings/{mid}/roll-call — upsert roll call JSON data for a meeting.
pub async fn save_roll_call(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<RollCallForm>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;
    let (tor_id, meeting_id) = path.into_inner();
    abac::require_tor_capability(&pool, &session, tor_id, "can_record_decisions").await?;

    let meeting = meeting::find_by_id(&pool, meeting_id).await?
        .ok_or(AppError::NotFound)?;
    if meeting.tor_id != tor_id {
        return Err(AppError::NotFound);
    }

    meeting::update_roll_call(&pool, meeting_id, &form.roll_call_data).await?;

    let user_id = get_user_id(&session).unwrap_or(0);
    let _ = crate::audit::log(
        &pool,
        user_id,
        "meeting.roll_call_saved",
        "meeting",
        meeting_id,
        serde_json::json!({"meeting_id": meeting_id, "tor_id": tor_id, "summary": "Roll call updated"}),
    ).await;

    let _ = session.insert("flash", "Roll call saved");
    Ok(HttpResponse::SeeOther()
        .insert_header((
            "Location",
            format!("/tor/{}/meetings/{}", tor_id, meeting_id),
        ))
        .finish())
}
