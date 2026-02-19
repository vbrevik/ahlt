use actix_session::Session;
use actix_web::{web, HttpResponse};
use std::collections::HashMap;

use crate::db::DbPool;
use crate::errors::{AppError, render};
use crate::auth::session::{require_permission, get_permissions};
use crate::models::meeting;
use crate::models::protocol;
use crate::models::workflow;
use crate::models::minutes;
use crate::templates_structs::{PageContext, MeetingDetailTemplate};

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

/// POST /tor/{id}/meetings/confirm — confirm a scheduled meeting.
pub async fn confirm(
    _pool: web::Data<DbPool>,
    _session: Session,
    _path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    Ok(HttpResponse::Ok().body("placeholder: confirm meeting"))
}

/// POST /tor/{id}/meetings/{mid}/transition — advance meeting lifecycle state.
pub async fn transition(
    _pool: web::Data<DbPool>,
    _session: Session,
    _path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    Ok(HttpResponse::Ok().body("placeholder: meeting transition"))
}

/// POST /tor/{id}/meetings/{mid}/agenda/assign — assign an agenda point to a meeting.
pub async fn assign_agenda(
    _pool: web::Data<DbPool>,
    _session: Session,
    _path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    Ok(HttpResponse::Ok().body("placeholder: assign agenda"))
}

/// POST /tor/{id}/meetings/{mid}/agenda/remove — remove an agenda point from a meeting.
pub async fn remove_agenda(
    _pool: web::Data<DbPool>,
    _session: Session,
    _path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    Ok(HttpResponse::Ok().body("placeholder: remove agenda"))
}

/// POST /tor/{id}/meetings/{mid}/minutes/generate — generate minutes for a meeting.
pub async fn generate_minutes(
    _pool: web::Data<DbPool>,
    _session: Session,
    _path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    Ok(HttpResponse::Ok().body("placeholder: generate minutes"))
}
