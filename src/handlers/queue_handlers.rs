use actix_session::Session;
use actix_web::{web, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;

use crate::auth::csrf;
use crate::auth::session::{require_permission, get_user_id};
use crate::errors::{AppError, render};
use crate::models::{tor, proposal, agenda_point, relation};
use crate::templates_structs::{PageContext, QueueTemplate};

// ---------------------------------------------------------------------------
// Form Structures
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct MarkReadyForm {
    pub csrf_token: String,
}

#[derive(Debug, Deserialize)]
pub struct UnqueueForm {
    pub proposal_id: i64,
    pub csrf_token: String,
}

#[derive(Debug, Deserialize)]
pub struct BulkScheduleForm {
    pub csrf_token: String,
    #[serde(default)]
    pub proposal_ids: Vec<i64>,
    pub scheduled_date: String,
    pub time_allocation_minutes: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /tor/{id}/workflow/queue
/// View the queue of proposals ready to be scheduled into agenda points.
/// Requires: agenda.queue permission and ToR membership
pub async fn view_queue(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "agenda.queue")?;

    let tor_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Fetch queued proposals
    let queued_proposals = proposal::find_queued_proposals(&pool, tor_id).await?;

    let tor_name = tor::get_tor_name(&pool, tor_id).await?;
    let ctx = PageContext::build(&session, &pool, "/workflow").await?;

    let tmpl = QueueTemplate {
        ctx,
        tor_id,
        tor_name,
        queued_proposals,
    };

    render(tmpl)
}

/// POST /tor/{id}/proposals/{pid}/ready-for-agenda
/// Mark a proposal as ready for the agenda queue.
/// Requires: agenda.queue permission
pub async fn mark_ready(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<MarkReadyForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "agenda.queue")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, proposal_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Verify the proposal exists and is approved
    let proposal = proposal::find_by_id(&pool, proposal_id).await?
        .ok_or(AppError::NotFound)?;

    if proposal.status != "approved" {
        return Err(AppError::PermissionDenied(
            "Proposal must be approved before marking ready for agenda".to_string()
        ));
    }

    // Mark as ready for agenda
    proposal::mark_ready_for_agenda(&pool, proposal_id).await?;

    // Audit log
    let details = serde_json::json!({
        "proposal_id": proposal_id,
        "title": proposal.title,
        "summary": format!("Marked proposal '{}' as ready for agenda", proposal.title)
    });
    let _ = crate::audit::log(&pool, user_id, "proposal.marked_ready_for_agenda", "proposal", proposal_id, details).await;

    let _ = session.insert("flash", "Proposal marked ready for agenda");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/workflow?tab=proposals")))
        .finish())
}

/// POST /tor/{id}/workflow/queue/unqueue
/// Remove a proposal from the queue by unsetting ready_for_agenda flag.
/// Requires: agenda.queue permission
pub async fn unqueue_proposal(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<UnqueueForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "agenda.queue")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let tor_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    let proposal_id = form.proposal_id;

    // Verify the proposal exists
    let proposal = proposal::find_by_id(&pool, proposal_id).await?
        .ok_or(AppError::NotFound)?;

    // Unqueue the proposal
    proposal::unqueue_proposal(&pool, proposal_id).await?;

    // Audit log
    let details = serde_json::json!({
        "proposal_id": proposal_id,
        "title": proposal.title,
        "summary": format!("Unqueued proposal '{}'", proposal.title)
    });
    let _ = crate::audit::log(&pool, user_id, "proposal.unqueued", "proposal", proposal_id, details).await;

    let _ = session.insert("flash", "Proposal removed from queue");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/workflow/queue")))
        .finish())
}

/// GET /tor/{id}/workflow/queue/schedule
/// Show the scheduling form with queued proposals and date/time picker.
/// Requires: agenda.manage permission
pub async fn schedule_form(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "agenda.manage")?;

    let tor_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Fetch queued proposals
    let queued_proposals = proposal::find_queued_proposals(&pool, tor_id).await?;

    let tor_name = tor::get_tor_name(&pool, tor_id).await?;
    let ctx = PageContext::build(&session, &pool, "/workflow").await?;

    let tmpl = QueueTemplate {
        ctx,
        tor_id,
        tor_name,
        queued_proposals,
    };

    render(tmpl)
}

/// POST /tor/{id}/workflow/queue/schedule
/// Bulk schedule selected proposals into agenda points for a specific date/time.
/// Requires: agenda.manage permission
pub async fn bulk_schedule(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<BulkScheduleForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "agenda.manage")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let tor_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Validation
    let mut errors = vec![];

    if form.proposal_ids.is_empty() {
        errors.push("Please select at least one proposal to schedule".to_string());
    }

    if form.scheduled_date.trim().is_empty() {
        errors.push("Scheduled date is required".to_string());
    }

    // Parse time allocation
    let time_allocation_minutes: i32 = form.time_allocation_minutes
        .trim()
        .parse()
        .unwrap_or(0);

    if time_allocation_minutes <= 0 {
        errors.push("Time allocation must be greater than 0 minutes".to_string());
    }

    // Validate that scheduled_date is not in the past
    let today: String = sqlx::query_scalar("SELECT CURRENT_DATE::text")
        .fetch_one(pool.get_ref())
        .await
        .map_err(AppError::Db)?;
    if form.scheduled_date < today {
        errors.push("Scheduled date cannot be in the past".to_string());
    }

    if !errors.is_empty() {
        let queued_proposals = proposal::find_queued_proposals(&pool, tor_id).await?;
        let tor_name = tor::get_tor_name(&pool, tor_id).await?;
        let ctx = PageContext::build(&session, &pool, "/workflow").await?;

        let tmpl = QueueTemplate {
            ctx,
            tor_id,
            tor_name,
            queued_proposals,
        };

        // For now, return the form with template (errors will need to be added to QueueTemplate in future)
        return render(tmpl);
    }

    // Bulk schedule: create agenda points for each proposal
    let mut scheduled_count = 0;
    for proposal_id in &form.proposal_ids {
        // Get the proposal to copy metadata
        let proposal = proposal::find_by_id(&pool, *proposal_id).await?
            .ok_or(AppError::NotFound)?;

        // Create agenda point entity
        let agenda_point_id = agenda_point::create(
            &pool,
            tor_id,
            &proposal.title,
            &format!("From proposal: {}", proposal.title),
            "informative", // Default type; can be customized per proposal in future
            &form.scheduled_date,
            time_allocation_minutes,
            user_id,
            "", // presenter
            "", // priority
            "", // pre_read_url
        ).await?;

        // Create spawns_agenda_point relation: proposal → agenda_point
        relation::create(&pool, "spawns_agenda_point", *proposal_id, agenda_point_id).await?;

        // Remove proposal from queue (set ready_for_agenda=false)
        proposal::unqueue_proposal(&pool, *proposal_id).await?;

        scheduled_count += 1;
    }

    // Audit log
    let details = serde_json::json!({
        "scheduled_date": form.scheduled_date,
        "count": scheduled_count,
        "time_allocation_minutes": time_allocation_minutes,
        "summary": format!("Scheduled {} proposals for {}", scheduled_count, form.scheduled_date)
    });
    let _ = crate::audit::log(&pool, user_id, "queue.bulk_scheduled", "agenda_point", tor_id, details).await;

    let _ = session.insert("flash", format!("Scheduled {} proposals", scheduled_count));
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/workflow?tab=agenda")))
        .finish())
}
