use actix_session::Session;
use actix_web::{web, HttpResponse};
use std::collections::HashMap;
use sqlx::PgPool;

use crate::auth::session::{require_permission, get_user_id, get_permissions};
use crate::errors::{AppError, render};
use crate::models::{tor, suggestion, proposal, agenda_point};
use crate::templates_structs::{PageContext, WorkflowTemplate, WorkflowIndexTemplate};

/// GET /tor/{tor_id}/workflow
/// Renders the workflow view with suggestions and proposals tabs.
pub async fn view(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
    query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "suggestion.view")?;

    let tor_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    let tor_name = tor::get_tor_name(&pool, tor_id).await?;
    let active_tab = query
        .get("tab")
        .cloned()
        .unwrap_or_else(|| "suggestions".to_string());

    let suggestions = suggestion::find_all_for_tor(&pool, tor_id).await?;
    let proposals = proposal::find_all_for_tor(&pool, tor_id).await?;
    let agenda_points = agenda_point::find_all_for_tor(&pool, tor_id).await?;

    let ctx = PageContext::build(&session, &pool, "/workflow").await?
        .with_tor(tor_id, &tor_name, "workflow");

    let tmpl = WorkflowTemplate {
        ctx,
        tor_id,
        tor_name,
        active_tab,
        suggestions,
        proposals,
        agenda_points,
    };
    render(tmpl)
}

/// GET /workflow
/// Cross-ToR workflow index page showing suggestions, proposals, and agenda points
/// across all ToRs the user is a member of (or all ToRs for workflow managers).
pub async fn index(
    pool: web::Data<PgPool>,
    session: Session,
    query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "suggestion.view")?;
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".into()))?;
    let permissions = get_permissions(&session).map_err(|e| AppError::Session(e))?;
    let active_tab = query.get("tab").cloned().unwrap_or_else(|| "suggestions".to_string());
    let filter_id = if permissions.has("workflow.manage") { None } else { Some(user_id) };
    let suggestions = suggestion::find_all_cross_tor(&pool, filter_id).await?;
    let proposals = proposal::find_all_cross_tor(&pool, filter_id).await?;
    let agenda_points = agenda_point::find_all_cross_tor(&pool, filter_id).await?;
    let ctx = PageContext::build(&session, &pool, "/workflow").await?;
    render(WorkflowIndexTemplate { ctx, active_tab, suggestions, proposals, agenda_points })
}
