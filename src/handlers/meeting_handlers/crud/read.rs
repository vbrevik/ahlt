/// Meeting detail read operations.
///
/// Handles GET requests to retrieve and display meeting information.

use actix_session::Session;
use actix_web::{web, HttpResponse};
use std::collections::HashMap;
use sqlx::PgPool;

use crate::auth::abac;
use crate::auth::session::{get_permissions, get_user_id, require_permission};
use crate::errors::{render, AppError};
use crate::models::meeting;
use crate::models::minutes;
use crate::models::protocol;
use crate::models::tor;
use crate::models::workflow;
use crate::templates_structs::{MeetingDetailTemplate, PageContext};

// ---------------------------------------------------------------------------
// GET — meeting detail
// ---------------------------------------------------------------------------

/// GET /tor/{id}/meetings/{mid} — meeting detail page.
///
/// Displays comprehensive meeting information including:
/// - Meeting metadata (date, status, location, etc.)
/// - Agenda points assigned to this meeting
/// - Unassigned agenda points available for this ToR
/// - Protocol steps
/// - Available workflow transitions
/// - Existing minutes (if any)
/// - User capabilities (ABAC) for conditional UI rendering
pub async fn detail(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "meetings.view")?;
    let (tor_id, mid) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;

    let meeting = meeting::find_by_id(&pool, mid).await?
        .ok_or(AppError::NotFound)?;

    // Verify the meeting belongs to the requested ToR
    if meeting.tor_id != tor_id {
        return Err(AppError::NotFound);
    }

    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    let tor_name = tor::get_tor_name(&pool, tor_id).await?;
    let ctx = PageContext::build(&session, &pool, "/meetings").await?
        .with_tor(tor_id, &tor_name, "meetings");

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
