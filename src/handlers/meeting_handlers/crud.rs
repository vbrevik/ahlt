use actix_session::Session;
use actix_web::{web, HttpResponse};
use std::collections::HashMap;

use crate::auth::csrf;
use crate::auth::session::{get_permissions, get_user_id, require_permission};
use crate::db::DbPool;
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
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "meetings.view")?;
    let (tor_id, mid) = path.into_inner();
    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/meetings")?;

    let meeting = meeting::find_by_id(&conn, mid)?
        .ok_or(AppError::NotFound)?;

    // Verify the meeting belongs to the requested ToR
    if meeting.tor_id != tor_id {
        return Err(AppError::NotFound);
    }

    let agenda_points = meeting::find_agenda_points(&conn, mid)?;
    let unassigned_points = meeting::find_unassigned_agenda_points(&conn, tor_id)?;
    let protocol_steps = protocol::find_steps_for_tor(&conn, tor_id)?;
    let permissions = get_permissions(&session)
        .map_err(|e| AppError::Session(format!("Failed to get permissions: {}", e)))?;
    let transitions = workflow::find_available_transitions(
        &conn,
        "meeting",
        &meeting.status,
        &permissions,
        &HashMap::new(),
    )?;
    let existing_minutes = minutes::find_by_meeting(&conn, mid)?;

    let tmpl = MeetingDetailTemplate {
        ctx,
        meeting,
        agenda_points,
        unassigned_points,
        protocol_steps,
        transitions,
        minutes: existing_minutes,
        tor_id,
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
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<ConfirmForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let tor_id = path.into_inner();
    let conn = pool.get()?;

    let location = form.location.as_deref().unwrap_or("");
    let notes = form.notes.as_deref().unwrap_or("");

    let meeting_id = meeting::create(
        &conn,
        tor_id,
        &form.meeting_date,
        &form.tor_name,
        location,
        notes,
    )?;

    // Immediately transition to "confirmed" status.
    meeting::update_status(&conn, meeting_id, "confirmed")?;

    // Audit
    let current_user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "meeting_id": meeting_id,
        "tor_id": tor_id,
        "meeting_date": &form.meeting_date,
        "summary": format!("Meeting confirmed for {} on {}", &form.tor_name, &form.meeting_date),
    });
    let _ = crate::audit::log(
        &conn,
        current_user_id,
        "meeting.confirmed",
        "meeting",
        meeting_id,
        details,
    );

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
///   - meeting_id present  → meeting already exists as "projected", just update status
///   - meeting_id absent   → cadence slot, create the meeting entity then confirm it
pub async fn confirm_calendar(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<CalendarConfirmForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let tor_id = path.into_inner();
    let conn = pool.get()?;
    let current_user_id = get_user_id(&session).unwrap_or(0);

    let meeting_id = if let Some(mid) = form.meeting_id {
        // Meeting already exists — verify ownership then update status
        let existing = meeting::find_by_id(&conn, mid)?.ok_or(AppError::NotFound)?;
        if existing.tor_id != tor_id {
            return Err(AppError::NotFound);
        }
        meeting::update_status(&conn, mid, "confirmed")?;
        mid
    } else {
        // No persisted meeting yet — create it and confirm in one step
        let mid = meeting::create(&conn, tor_id, &form.meeting_date, &form.tor_name, "", "")?;
        meeting::update_status(&conn, mid, "confirmed")?;
        mid
    };

    let details = serde_json::json!({
        "meeting_id": meeting_id,
        "tor_id": tor_id,
        "meeting_date": &form.meeting_date,
        "summary": format!("Meeting confirmed for {} on {}", &form.tor_name, &form.meeting_date),
    });
    let _ = crate::audit::log(
        &conn,
        current_user_id,
        "meeting.confirmed",
        "meeting",
        meeting_id,
        details,
    );

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::json!({"ok": true, "meeting_id": meeting_id}).to_string()))
}

// ---------------------------------------------------------------------------
// POST — transition meeting lifecycle state
// ---------------------------------------------------------------------------

/// POST /tor/{id}/meetings/{mid}/transition — advance meeting lifecycle state.
pub async fn transition(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<TransitionForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, mid) = path.into_inner();
    let conn = pool.get()?;

    let meeting_detail = meeting::find_by_id(&conn, mid)?
        .ok_or(AppError::NotFound)?;

    if meeting_detail.tor_id != tor_id {
        return Err(AppError::NotFound);
    }

    let permissions = get_permissions(&session)
        .map_err(|e| AppError::Session(format!("Failed to get permissions: {}", e)))?;

    // Validate the transition via the workflow engine (returns error if invalid).
    workflow::validate_transition(
        &conn,
        "meeting",
        &meeting_detail.status,
        &form.new_status,
        &permissions,
        &HashMap::new(),
    )?;

    meeting::update_status(&conn, mid, &form.new_status)?;

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
        &conn,
        current_user_id,
        "meeting.transition",
        "meeting",
        mid,
        details,
    );

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
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<AgendaForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, mid) = path.into_inner();
    let conn = pool.get()?;

    meeting::assign_agenda(&conn, mid, form.agenda_point_id)?;

    // Audit
    let current_user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "meeting_id": mid,
        "tor_id": tor_id,
        "agenda_point_id": form.agenda_point_id,
        "summary": format!("Agenda point {} assigned to meeting {}", form.agenda_point_id, mid),
    });
    let _ = crate::audit::log(
        &conn,
        current_user_id,
        "meeting.agenda_assigned",
        "meeting",
        mid,
        details,
    );

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
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<AgendaForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, mid) = path.into_inner();
    let conn = pool.get()?;

    meeting::remove_agenda(&conn, mid, form.agenda_point_id)?;

    // Audit
    let current_user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "meeting_id": mid,
        "tor_id": tor_id,
        "agenda_point_id": form.agenda_point_id,
        "summary": format!("Agenda point {} removed from meeting {}", form.agenda_point_id, mid),
    });
    let _ = crate::audit::log(
        &conn,
        current_user_id,
        "meeting.agenda_removed",
        "meeting",
        mid,
        details,
    );

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
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<CsrfOnly>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "minutes.generate")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, mid) = path.into_inner();
    let conn = pool.get()?;

    let meeting_detail = meeting::find_by_id(&conn, mid)?
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
    if minutes::find_by_meeting(&conn, mid)?.is_some() {
        return Err(AppError::PermissionDenied(
            "Minutes already exist for this meeting".to_string(),
        ));
    }

    let minutes_id =
        minutes::generate_scaffold(&conn, mid, tor_id, &meeting_detail.label)?;

    // Audit
    let current_user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "meeting_id": mid,
        "minutes_id": minutes_id,
        "tor_id": tor_id,
        "summary": format!("Minutes generated for meeting {}", mid),
    });
    let _ = crate::audit::log(
        &conn,
        current_user_id,
        "meeting.minutes_generated",
        "meeting",
        mid,
        details,
    );

    let _ = session.insert("flash", "Minutes generated successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/minutes/{}", minutes_id)))
        .finish())
}
