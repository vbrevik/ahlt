use actix_session::Session;
use actix_web::{web, HttpResponse};
use std::collections::HashMap;
use sqlx::PgPool;

use crate::auth::csrf;
use crate::auth::session::{require_permission, get_user_id, get_permissions};
use crate::errors::AppError;
use crate::models::{tor, proposal, workflow};

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
