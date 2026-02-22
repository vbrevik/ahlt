/// Meeting update/mutation operations.
///
/// Handles POST requests for lifecycle transitions, agenda management,
/// minutes generation, and roll call data updates.

use actix_session::Session;
use actix_web::{web, HttpResponse};
use std::collections::HashMap;
use sqlx::PgPool;

use crate::auth::abac;
use crate::auth::csrf;
use crate::auth::session::{get_permissions, get_user_id};
use crate::errors::AppError;
use crate::models::meeting;
use crate::models::minutes;
use crate::models::workflow;

use super::forms::{TransitionForm, AgendaForm, CsrfOnly, RollCallForm};
use super::helpers::validate_meeting_tor_ownership;

// ---------------------------------------------------------------------------
// POST — transition meeting lifecycle state
// ---------------------------------------------------------------------------

/// POST /tor/{id}/meetings/{mid}/transition — advance meeting lifecycle state.
///
/// Validates the transition via the workflow engine, updates meeting status,
/// and logs the change to the audit trail.
pub async fn transition(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<TransitionForm>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, mid) = path.into_inner();
    abac::require_tor_capability(&pool, &session, tor_id, "can_call_meetings").await?;

    validate_meeting_tor_ownership(&pool, mid, tor_id).await?;
    let meeting_detail = meeting::find_by_id(&pool, mid).await?
        .ok_or(AppError::NotFound)?;

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
///
/// Associates an unassigned agenda point with a specific meeting.
pub async fn assign_agenda(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<AgendaForm>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, mid) = path.into_inner();
    abac::require_tor_capability(&pool, &session, tor_id, "can_manage_agenda").await?;

    validate_meeting_tor_ownership(&pool, mid, tor_id).await?;

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
///
/// Disassociates an agenda point from a meeting, returning it to the unassigned pool.
pub async fn remove_agenda(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<AgendaForm>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, mid) = path.into_inner();
    abac::require_tor_capability(&pool, &session, tor_id, "can_manage_agenda").await?;

    validate_meeting_tor_ownership(&pool, mid, tor_id).await?;

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
///
/// Creates a new minutes entity with a scaffold structure for a completed meeting.
/// Only available for meetings in "completed" status, and prevents duplicate generation.
pub async fn generate_minutes(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<CsrfOnly>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, mid) = path.into_inner();
    abac::require_tor_capability(&pool, &session, tor_id, "can_record_decisions").await?;

    validate_meeting_tor_ownership(&pool, mid, tor_id).await?;
    let meeting_detail = meeting::find_by_id(&pool, mid).await?
        .ok_or(AppError::NotFound)?;

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

/// POST /tor/{id}/meetings/{mid}/roll-call — upsert roll call JSON data for a meeting.
///
/// Stores structured roll call attendance data (typically attendees, status, etc.)
/// as JSON in the meeting record. The data structure is opaque to this handler;
/// validation/parsing happens on the frontend and template display layer.
pub async fn save_roll_call(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<RollCallForm>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;
    let (tor_id, meeting_id) = path.into_inner();
    abac::require_tor_capability(&pool, &session, tor_id, "can_record_decisions").await?;

    validate_meeting_tor_ownership(&pool, meeting_id, tor_id).await?;

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
