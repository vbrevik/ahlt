use actix_session::Session;
use actix_web::{web, HttpResponse};
use std::collections::HashMap;
use sqlx::PgPool;

use crate::auth::csrf;
use crate::auth::session::{require_permission, get_user_id, get_permissions};
use crate::errors::{AppError, render};
use crate::models::{tor, proposal, workflow};
use crate::models::proposal::ProposalForm;
use crate::templates_structs::{PageContext, ProposalFormTemplate, ProposalDetailTemplate};

// ---------------------------------------------------------------------------
// CRUD handlers (Task 16)
// ---------------------------------------------------------------------------

/// GET /tor/{tor_id}/proposals/{id}
/// Renders the proposal detail page.
pub async fn detail(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "proposal.view")?;

    let (tor_id, proposal_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    match proposal::find_by_id(&pool, proposal_id).await? {
        Some(p) => {
            let tor_name = tor::get_tor_name(&pool, tor_id).await?;
            let ctx = PageContext::build(&session, &pool, "/workflow").await?
                .with_tor(tor_id, &tor_name, "workflow");
            let tmpl = ProposalDetailTemplate {
                ctx,
                tor_id,
                proposal: p,
            };
            render(tmpl)
        }
        None => Err(AppError::NotFound),
    }
}

/// GET /tor/{tor_id}/proposals/new
/// Renders the proposal creation form.
pub async fn new_form(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "proposal.create")?;

    let tor_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    let tor_name = tor::get_tor_name(&pool, tor_id).await?;
    let ctx = PageContext::build(&session, &pool, "/workflow").await?
        .with_tor(tor_id, &tor_name, "workflow");

    let tmpl = ProposalFormTemplate {
        ctx,
        tor_id,
        tor_name,
        form_action: format!("/tor/{tor_id}/proposals"),
        form_title: "New Proposal".to_string(),
        proposal: None,
        errors: vec![],
    };
    render(tmpl)
}

/// POST /tor/{tor_id}/proposals
/// Creates a new proposal linked to the ToR.
pub async fn create(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<ProposalForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "proposal.create")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let tor_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Validate
    let title = form.title.trim();
    let description = form.description.trim();
    let rationale = form.rationale.trim();
    let mut errors = vec![];

    if title.is_empty() {
        errors.push("Title is required".to_string());
    }
    if description.is_empty() {
        errors.push("Description is required".to_string());
    }

    if !errors.is_empty() {
        let tor_name = tor::get_tor_name(&pool, tor_id).await?;
        let ctx = PageContext::build(&session, &pool, "/workflow").await?;
        let tmpl = ProposalFormTemplate {
            ctx,
            tor_id,
            tor_name,
            form_action: format!("/tor/{tor_id}/proposals"),
            form_title: "New Proposal".to_string(),
            proposal: None,
            errors,
        };
        return render(tmpl);
    }

    // Get today's date from PostgreSQL
    let today: String = sqlx::query_scalar("SELECT CURRENT_DATE::text")
        .fetch_one(pool.get_ref())
        .await?;

    let proposal_id = proposal::create(
        &pool, tor_id, title, description, rationale, user_id, &today, None,
    ).await?;

    // Audit log
    let details = serde_json::json!({
        "tor_id": tor_id,
        "title": title,
        "summary": format!("Created proposal '{}'", title)
    });
    let _ = crate::audit::log(&pool, user_id, "proposal.created", "proposal", proposal_id, details).await;

    let _ = session.insert("flash", "Proposal created successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/proposals/{proposal_id}")))
        .finish())
}

/// GET /tor/{tor_id}/proposals/{id}/edit
/// Renders the proposal edit form (only for draft or rejected proposals).
pub async fn edit_form(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "proposal.edit")?;

    let (tor_id, proposal_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    match proposal::find_by_id(&pool, proposal_id).await? {
        Some(p) => {
            // Check via workflow engine if editing is allowed for this status
            // Only draft and rejected proposals should allow editing
            if p.status != "draft" && p.status != "rejected" {
                let _ = session.insert("flash", "Only draft or rejected proposals can be edited");
                return Ok(HttpResponse::SeeOther()
                    .insert_header(("Location", format!("/tor/{tor_id}/proposals/{proposal_id}")))
                    .finish());
            }

            let tor_name = tor::get_tor_name(&pool, tor_id).await?;
            let ctx = PageContext::build(&session, &pool, "/workflow").await?
                .with_tor(tor_id, &tor_name, "workflow");
            let tmpl = ProposalFormTemplate {
                ctx,
                tor_id,
                tor_name,
                form_action: format!("/tor/{tor_id}/proposals/{proposal_id}"),
                form_title: "Edit Proposal".to_string(),
                proposal: Some(p),
                errors: vec![],
            };
            render(tmpl)
        }
        None => Err(AppError::NotFound),
    }
}

/// POST /tor/{tor_id}/proposals/{id}
/// Updates an existing proposal's title, description, and rationale.
pub async fn update(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<ProposalForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "proposal.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, proposal_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Validate
    let title = form.title.trim();
    let description = form.description.trim();
    let rationale = form.rationale.trim();
    let mut errors = vec![];

    if title.is_empty() {
        errors.push("Title is required".to_string());
    }
    if description.is_empty() {
        errors.push("Description is required".to_string());
    }

    if !errors.is_empty() {
        let existing = proposal::find_by_id(&pool, proposal_id).await.ok().flatten();
        let tor_name = tor::get_tor_name(&pool, tor_id).await?;
        let ctx = PageContext::build(&session, &pool, "/workflow").await?;
        let tmpl = ProposalFormTemplate {
            ctx,
            tor_id,
            tor_name,
            form_action: format!("/tor/{tor_id}/proposals/{proposal_id}"),
            form_title: "Edit Proposal".to_string(),
            proposal: existing,
            errors,
        };
        return render(tmpl);
    }

    proposal::update(&pool, proposal_id, title, description, rationale).await?;

    // Audit log
    let details = serde_json::json!({
        "proposal_id": proposal_id,
        "title": title,
        "summary": format!("Updated proposal '{}'", title)
    });
    let _ = crate::audit::log(&pool, user_id, "proposal.updated", "proposal", proposal_id, details).await;

    let _ = session.insert("flash", "Proposal updated successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/proposals/{proposal_id}")))
        .finish())
}

// ---------------------------------------------------------------------------
// Status workflow handlers (Task 17)
// ---------------------------------------------------------------------------

/// POST /tor/{tor_id}/proposals/{id}/submit
/// Submits a draft proposal for review.
pub async fn submit(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<crate::handlers::auth_handlers::CsrfOnly>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "proposal.submit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, proposal_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Get current status for workflow validation
    let current_proposal = proposal::find_by_id(&pool, proposal_id).await?
        .ok_or(AppError::NotFound)?;
    let user_permissions = get_permissions(&session)
        .map_err(|e| AppError::Session(e))?;
    let entity_props = HashMap::new();

    // Validate workflow transition via workflow engine
    workflow::validate_transition(
        &pool,
        "proposal",
        &current_proposal.status,
        "submitted",
        &user_permissions,
        &entity_props,
    ).await?;

    proposal::update_status(&pool, proposal_id, "submitted", None).await?;

    // Audit log
    let details = serde_json::json!({
        "proposal_id": proposal_id,
        "new_status": "submitted",
        "summary": format!("Submitted proposal #{} for review", proposal_id)
    });
    let _ = crate::audit::log(&pool, user_id, "proposal.submitted", "proposal", proposal_id, details).await;

    let _ = session.insert("flash", "Proposal submitted for review");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/proposals/{proposal_id}")))
        .finish())
}

/// POST /tor/{tor_id}/proposals/{id}/review
/// Starts review of a submitted proposal.
pub async fn review(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<crate::handlers::auth_handlers::CsrfOnly>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "proposal.review")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, proposal_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Get current status for workflow validation
    let current_proposal = proposal::find_by_id(&pool, proposal_id).await?
        .ok_or(AppError::NotFound)?;
    let user_permissions = get_permissions(&session)
        .map_err(|e| AppError::Session(e))?;
    let entity_props = HashMap::new();

    // Validate workflow transition via workflow engine
    workflow::validate_transition(
        &pool,
        "proposal",
        &current_proposal.status,
        "under_review",
        &user_permissions,
        &entity_props,
    ).await?;

    proposal::update_status(&pool, proposal_id, "under_review", None).await?;

    // Audit log
    let details = serde_json::json!({
        "proposal_id": proposal_id,
        "new_status": "under_review",
        "summary": format!("Started review of proposal #{}", proposal_id)
    });
    let _ = crate::audit::log(&pool, user_id, "proposal.review_started", "proposal", proposal_id, details).await;

    let _ = session.insert("flash", "Proposal is now under review");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/proposals/{proposal_id}")))
        .finish())
}

/// POST /tor/{tor_id}/proposals/{id}/approve
/// Approves a proposal under review.
pub async fn approve(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<crate::handlers::auth_handlers::CsrfOnly>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "proposal.approve")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, proposal_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Get current status for workflow validation
    let current_proposal = proposal::find_by_id(&pool, proposal_id).await?
        .ok_or(AppError::NotFound)?;
    let user_permissions = get_permissions(&session)
        .map_err(|e| AppError::Session(e))?;
    let entity_props = HashMap::new();

    // Validate workflow transition via workflow engine
    workflow::validate_transition(
        &pool,
        "proposal",
        &current_proposal.status,
        "approved",
        &user_permissions,
        &entity_props,
    ).await?;

    proposal::update_status(&pool, proposal_id, "approved", None).await?;

    // Audit log
    let details = serde_json::json!({
        "proposal_id": proposal_id,
        "new_status": "approved",
        "summary": format!("Approved proposal #{}", proposal_id)
    });
    let _ = crate::audit::log(&pool, user_id, "proposal.approved", "proposal", proposal_id, details).await;

    let _ = session.insert("flash", "Proposal approved");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/proposals/{proposal_id}")))
        .finish())
}

/// POST /tor/{tor_id}/proposals/{id}/reject
/// Rejects a proposal with a required reason.
pub async fn reject(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "proposal.approve")?;
    let csrf_token = form.get("csrf_token").map(|s| s.as_str()).unwrap_or("");
    csrf::validate_csrf(&session, csrf_token)?;

    let (tor_id, proposal_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    let rejection_reason = form.get("rejection_reason").map(|s| s.trim().to_string()).unwrap_or_default();
    if rejection_reason.is_empty() {
        let _ = session.insert("flash", "Rejection reason is required");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", format!("/tor/{tor_id}/proposals/{proposal_id}")))
            .finish());
    }

    // Get current status for workflow validation
    let current_proposal = proposal::find_by_id(&pool, proposal_id).await?
        .ok_or(AppError::NotFound)?;
    let user_permissions = get_permissions(&session)
        .map_err(|e| AppError::Session(e))?;
    let entity_props = HashMap::new();

    // Validate workflow transition via workflow engine
    workflow::validate_transition(
        &pool,
        "proposal",
        &current_proposal.status,
        "rejected",
        &user_permissions,
        &entity_props,
    ).await?;

    proposal::update_status(&pool, proposal_id, "rejected", Some(&rejection_reason)).await?;

    // Audit log
    let details = serde_json::json!({
        "proposal_id": proposal_id,
        "new_status": "rejected",
        "rejection_reason": &rejection_reason,
        "summary": format!("Rejected proposal #{}", proposal_id)
    });
    let _ = crate::audit::log(&pool, user_id, "proposal.rejected", "proposal", proposal_id, details).await;

    let _ = session.insert("flash", "Proposal rejected");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/proposals/{proposal_id}")))
        .finish())
}
