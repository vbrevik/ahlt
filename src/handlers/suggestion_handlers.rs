use actix_session::Session;
use actix_web::{web, HttpResponse};
use std::collections::HashMap;
use sqlx::PgPool;

use crate::auth::csrf;
use crate::auth::session::{require_permission, get_user_id, get_permissions};
use crate::errors::{AppError, render};
use crate::models::{tor, suggestion, proposal, workflow};
use crate::models::suggestion::SuggestionForm;
use crate::templates_structs::{PageContext, SuggestionFormTemplate};

/// GET /tor/{tor_id}/suggestions/new
/// Renders the suggestion creation form.
pub async fn new_form(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "suggestion.create")?;

    let tor_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    let tor_name = tor::get_tor_name(&pool, tor_id).await?;
    let ctx = PageContext::build(&session, &pool, "/workflow").await?;

    let tmpl = SuggestionFormTemplate {
        ctx,
        tor_id,
        tor_name,
        errors: vec![],
    };
    render(tmpl)
}

/// POST /tor/{tor_id}/suggestions
/// Creates a new suggestion linked to the ToR.
pub async fn create(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<SuggestionForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "suggestion.create")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let tor_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Validate
    let description = form.description.trim();
    if description.is_empty() {
        let tor_name = tor::get_tor_name(&pool, tor_id).await?;
        let ctx = PageContext::build(&session, &pool, "/workflow").await?;
        let tmpl = SuggestionFormTemplate {
            ctx,
            tor_id,
            tor_name,
            errors: vec!["Description is required".to_string()],
        };
        return render(tmpl);
    }

    // Get today's date from PostgreSQL
    let today: String = sqlx::query_scalar("SELECT CURRENT_DATE::text")
        .fetch_one(pool.get_ref())
        .await?;

    let suggestion_id = suggestion::create(&pool, tor_id, description, user_id, &today).await?;

    // Audit log
    let details = serde_json::json!({
        "tor_id": tor_id,
        "description_preview": &description[..description.len().min(100)],
        "summary": "Created new suggestion"
    });
    let _ = crate::audit::log(&pool, user_id, "suggestion.created", "suggestion", suggestion_id, details).await;

    let _ = session.insert("flash", "Suggestion submitted successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/workflow?tab=suggestions")))
        .finish())
}

/// POST /tor/{tor_id}/suggestions/{id}/accept
/// Accepts a suggestion and auto-creates a draft proposal.
pub async fn accept(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<crate::handlers::auth_handlers::CsrfOnly>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "suggestion.review")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, suggestion_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Get current status for workflow validation
    let current_suggestion = suggestion::find_by_id(&pool, suggestion_id).await?
        .ok_or(AppError::NotFound)?;
    let user_permissions = get_permissions(&session)
        .map_err(|e| AppError::Session(e))?;
    let entity_props = HashMap::new();

    // Validate workflow transition via workflow engine
    workflow::validate_transition(
        &pool,
        "suggestion",
        &current_suggestion.status,
        "accepted",
        &user_permissions,
        &entity_props,
    ).await?;

    suggestion::update_status(&pool, suggestion_id, "accepted", None).await?;
    let proposal_id = proposal::auto_create_from_suggestion(&pool, suggestion_id, tor_id).await?;

    // Audit log
    let details = serde_json::json!({
        "suggestion_id": suggestion_id,
        "spawned_proposal_id": proposal_id,
        "summary": format!("Accepted suggestion #{} and created draft proposal #{}", suggestion_id, proposal_id)
    });
    let _ = crate::audit::log(&pool, user_id, "suggestion.accepted", "suggestion", suggestion_id, details).await;

    let _ = session.insert("flash", "Suggestion accepted and draft proposal created");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/workflow?tab=suggestions")))
        .finish())
}

/// POST /tor/{tor_id}/suggestions/{id}/reject
/// Rejects a suggestion with a required reason.
pub async fn reject(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "suggestion.review")?;
    let csrf_token = form.get("csrf_token").map(|s| s.as_str()).unwrap_or("");
    csrf::validate_csrf(&session, csrf_token)?;

    let (tor_id, suggestion_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    let rejection_reason = form.get("rejection_reason").map(|s| s.trim().to_string()).unwrap_or_default();
    if rejection_reason.is_empty() {
        let _ = session.insert("flash", "Rejection reason is required");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", format!("/tor/{tor_id}/workflow?tab=suggestions")))
            .finish());
    }

    // Get current status for workflow validation
    let current_suggestion = suggestion::find_by_id(&pool, suggestion_id).await?
        .ok_or(AppError::NotFound)?;
    let user_permissions = get_permissions(&session)
        .map_err(|e| AppError::Session(e))?;
    let entity_props = HashMap::new();

    // Validate workflow transition via workflow engine
    workflow::validate_transition(
        &pool,
        "suggestion",
        &current_suggestion.status,
        "rejected",
        &user_permissions,
        &entity_props,
    ).await?;

    suggestion::update_status(&pool, suggestion_id, "rejected", Some(&rejection_reason)).await?;

    // Audit log
    let details = serde_json::json!({
        "suggestion_id": suggestion_id,
        "rejection_reason": &rejection_reason,
        "summary": format!("Rejected suggestion #{}", suggestion_id)
    });
    let _ = crate::audit::log(&pool, user_id, "suggestion.rejected", "suggestion", suggestion_id, details).await;

    let _ = session.insert("flash", "Suggestion rejected");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/workflow?tab=suggestions")))
        .finish())
}
