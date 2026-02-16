use actix_session::Session;
use actix_web::{web, HttpResponse};
use std::collections::HashMap;

use crate::db::DbPool;
use crate::auth::session::{require_permission, get_user_id};
use crate::errors::{AppError, render};
use crate::models::{tor, suggestion, proposal, agenda_point};
use crate::templates_structs::{PageContext, WorkflowTemplate};

/// GET /tor/{tor_id}/workflow
/// Renders the workflow view with suggestions and proposals tabs.
pub async fn view(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "suggestion.view")?;

    let tor_id = path.into_inner();
    let conn = pool.get()?;
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&conn, user_id, tor_id)?;

    let tor_name = tor::get_tor_name(&conn, tor_id)?;
    let active_tab = query
        .get("tab")
        .cloned()
        .unwrap_or_else(|| "suggestions".to_string());

    let suggestions = suggestion::find_all_for_tor(&conn, tor_id)?;
    let proposals = proposal::find_all_for_tor(&conn, tor_id)?;
    let agenda_points = agenda_point::find_all_for_tor(&conn, tor_id)?;

    let ctx = PageContext::build(&session, &conn, "/workflow")?;

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
