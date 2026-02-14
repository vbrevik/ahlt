use actix_session::Session;
use actix_web::{web, HttpResponse};
use std::collections::HashMap;

use crate::db::DbPool;
use crate::auth::csrf;
use crate::auth::session::{require_permission, get_user_id};
use crate::errors::{AppError, render};
use crate::models::{tor, proposal};
use crate::models::proposal::ProposalForm;
use crate::templates_structs::{PageContext, ProposalFormTemplate, ProposalDetailTemplate};

// ---------------------------------------------------------------------------
// CRUD handlers (Task 16)
// ---------------------------------------------------------------------------

/// GET /tor/{tor_id}/proposals/{id}
/// Renders the proposal detail page.
pub async fn detail(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "proposal.view")?;

    let (tor_id, proposal_id) = path.into_inner();
    let conn = pool.get()?;
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&conn, user_id, tor_id)?;

    match proposal::find_by_id(&conn, proposal_id)? {
        Some(p) => {
            let tor_name = tor::get_tor_name(&conn, tor_id)?;
            let ctx = PageContext::build(&session, &conn, "/pipeline")?;
            let tmpl = ProposalDetailTemplate {
                ctx,
                tor_id,
                tor_name,
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
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "proposal.create")?;

    let tor_id = path.into_inner();
    let conn = pool.get()?;
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&conn, user_id, tor_id)?;

    let tor_name = tor::get_tor_name(&conn, tor_id)?;
    let ctx = PageContext::build(&session, &conn, "/pipeline")?;

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
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<ProposalForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "proposal.create")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let tor_id = path.into_inner();
    let conn = pool.get()?;
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&conn, user_id, tor_id)?;

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
        let tor_name = tor::get_tor_name(&conn, tor_id)?;
        let ctx = PageContext::build(&session, &conn, "/pipeline")?;
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

    // Get today's date from SQLite (zero external dependencies)
    let today: String = conn.query_row("SELECT date('now')", [], |row| row.get(0))?;

    let proposal_id = proposal::create(
        &conn, tor_id, title, description, rationale, user_id, &today, None,
    )?;

    // Audit log
    let details = serde_json::json!({
        "tor_id": tor_id,
        "title": title,
        "summary": format!("Created proposal '{}'", title)
    });
    let _ = crate::audit::log(&conn, user_id, "proposal.created", "proposal", proposal_id, details);

    let _ = session.insert("flash", "Proposal created successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/proposals/{proposal_id}")))
        .finish())
}

/// GET /tor/{tor_id}/proposals/{id}/edit
/// Renders the proposal edit form (only for draft or rejected proposals).
pub async fn edit_form(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "proposal.edit")?;

    let (tor_id, proposal_id) = path.into_inner();
    let conn = pool.get()?;
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&conn, user_id, tor_id)?;

    match proposal::find_by_id(&conn, proposal_id)? {
        Some(p) => {
            // Only draft or rejected proposals are editable
            if p.status != "draft" && p.status != "rejected" {
                let _ = session.insert("flash", "Only draft or rejected proposals can be edited");
                return Ok(HttpResponse::SeeOther()
                    .insert_header(("Location", format!("/tor/{tor_id}/proposals/{proposal_id}")))
                    .finish());
            }

            let tor_name = tor::get_tor_name(&conn, tor_id)?;
            let ctx = PageContext::build(&session, &conn, "/pipeline")?;
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
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<ProposalForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "proposal.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, proposal_id) = path.into_inner();
    let conn = pool.get()?;
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&conn, user_id, tor_id)?;

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
        let existing = proposal::find_by_id(&conn, proposal_id).ok().flatten();
        let tor_name = tor::get_tor_name(&conn, tor_id)?;
        let ctx = PageContext::build(&session, &conn, "/pipeline")?;
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

    proposal::update(&conn, proposal_id, title, description, rationale)?;

    // Audit log
    let details = serde_json::json!({
        "proposal_id": proposal_id,
        "title": title,
        "summary": format!("Updated proposal '{}'", title)
    });
    let _ = crate::audit::log(&conn, user_id, "proposal.updated", "proposal", proposal_id, details);

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
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<crate::handlers::auth_handlers::CsrfOnly>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "proposal.submit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, proposal_id) = path.into_inner();
    let conn = pool.get()?;
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&conn, user_id, tor_id)?;

    proposal::update_status(&conn, proposal_id, "submitted", None)?;

    // Audit log
    let details = serde_json::json!({
        "proposal_id": proposal_id,
        "new_status": "submitted",
        "summary": format!("Submitted proposal #{} for review", proposal_id)
    });
    let _ = crate::audit::log(&conn, user_id, "proposal.submitted", "proposal", proposal_id, details);

    let _ = session.insert("flash", "Proposal submitted for review");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/proposals/{proposal_id}")))
        .finish())
}

/// POST /tor/{tor_id}/proposals/{id}/review
/// Starts review of a submitted proposal.
pub async fn review(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<crate::handlers::auth_handlers::CsrfOnly>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "proposal.review")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, proposal_id) = path.into_inner();
    let conn = pool.get()?;
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&conn, user_id, tor_id)?;

    proposal::update_status(&conn, proposal_id, "under_review", None)?;

    // Audit log
    let details = serde_json::json!({
        "proposal_id": proposal_id,
        "new_status": "under_review",
        "summary": format!("Started review of proposal #{}", proposal_id)
    });
    let _ = crate::audit::log(&conn, user_id, "proposal.review_started", "proposal", proposal_id, details);

    let _ = session.insert("flash", "Proposal is now under review");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/proposals/{proposal_id}")))
        .finish())
}

/// POST /tor/{tor_id}/proposals/{id}/approve
/// Approves a proposal under review.
pub async fn approve(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<crate::handlers::auth_handlers::CsrfOnly>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "proposal.approve")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, proposal_id) = path.into_inner();
    let conn = pool.get()?;
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&conn, user_id, tor_id)?;

    proposal::update_status(&conn, proposal_id, "approved", None)?;

    // Audit log
    let details = serde_json::json!({
        "proposal_id": proposal_id,
        "new_status": "approved",
        "summary": format!("Approved proposal #{}", proposal_id)
    });
    let _ = crate::audit::log(&conn, user_id, "proposal.approved", "proposal", proposal_id, details);

    let _ = session.insert("flash", "Proposal approved");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/proposals/{proposal_id}")))
        .finish())
}

/// POST /tor/{tor_id}/proposals/{id}/reject
/// Rejects a proposal with a required reason.
pub async fn reject(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "proposal.approve")?;
    let csrf_token = form.get("csrf_token").map(|s| s.as_str()).unwrap_or("");
    csrf::validate_csrf(&session, csrf_token)?;

    let (tor_id, proposal_id) = path.into_inner();
    let conn = pool.get()?;
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&conn, user_id, tor_id)?;

    let rejection_reason = form.get("rejection_reason").map(|s| s.trim().to_string()).unwrap_or_default();
    if rejection_reason.is_empty() {
        let _ = session.insert("flash", "Rejection reason is required");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", format!("/tor/{tor_id}/proposals/{proposal_id}")))
            .finish());
    }

    proposal::update_status(&conn, proposal_id, "rejected", Some(&rejection_reason))?;

    // Audit log
    let details = serde_json::json!({
        "proposal_id": proposal_id,
        "new_status": "rejected",
        "rejection_reason": &rejection_reason,
        "summary": format!("Rejected proposal #{}", proposal_id)
    });
    let _ = crate::audit::log(&conn, user_id, "proposal.rejected", "proposal", proposal_id, details);

    let _ = session.insert("flash", "Proposal rejected");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/proposals/{proposal_id}")))
        .finish())
}
