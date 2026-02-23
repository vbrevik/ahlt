use actix_session::Session;
use actix_web::{web, HttpResponse};
use serde::Serialize;
use sqlx::PgPool;

use crate::auth::session::require_permission;
use crate::errors::AppError;
use crate::models::proposal;
use crate::templates_structs::PaginatedResponse;

#[derive(Serialize)]
pub struct ApiProposalItem {
    pub id: i64,
    pub tor_id: i64,
    pub tor_name: String,
    pub title: String,
    pub submitted_by_id: i64,
    pub submitted_by_name: String,
    pub submitted_date: String,
    pub status: String,
    pub rejection_reason: Option<String>,
}

/// GET /api/v1/proposals - List proposals with optional status and tor_id filters.
/// Query params: status (filter), tor_id (filter), page (default 1), per_page (default 25).
pub async fn list(
    pool: web::Data<PgPool>,
    session: Session,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "proposal.view")?;

    let status_filter = query.get("status").map(|s| s.as_str());
    let tor_id_filter = query.get("tor_id").and_then(|s| s.parse::<i64>().ok());
    let page = query
        .get("page")
        .and_then(|p| p.parse::<i64>().ok())
        .unwrap_or(1)
        .max(1);
    let per_page = query
        .get("per_page")
        .and_then(|p| p.parse::<i64>().ok())
        .unwrap_or(25)
        .max(1)
        .min(100);

    let all_items = proposal::find_all_cross_tor(&pool, None).await?;

    // Apply filters
    let filtered: Vec<_> = all_items
        .into_iter()
        .filter(|p| {
            if let Some(status) = status_filter {
                if p.status != status {
                    return false;
                }
            }
            if let Some(tid) = tor_id_filter {
                if p.tor_id != tid {
                    return false;
                }
            }
            true
        })
        .collect();

    let total = filtered.len() as i64;
    let offset = ((page - 1) * per_page) as usize;
    let items: Vec<ApiProposalItem> = filtered
        .into_iter()
        .skip(offset)
        .take(per_page as usize)
        .map(|p| ApiProposalItem {
            id: p.id,
            tor_id: p.tor_id,
            tor_name: p.tor_name,
            title: p.title,
            submitted_by_id: p.submitted_by_id,
            submitted_by_name: p.submitted_by_name,
            submitted_date: p.submitted_date,
            status: p.status,
            rejection_reason: p.rejection_reason,
        })
        .collect();

    Ok(HttpResponse::Ok().json(PaginatedResponse {
        items,
        page,
        per_page,
        total,
    }))
}
