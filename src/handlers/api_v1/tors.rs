use actix_session::Session;
use actix_web::{web, HttpResponse};
use serde::Serialize;
use sqlx::PgPool;

use crate::auth::session::require_permission;
use crate::errors::AppError;
use crate::models::tor;
use crate::templates_structs::PaginatedResponse;

#[derive(Serialize)]
pub struct ApiTorListItem {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub description: String,
    pub status: String,
    pub meeting_cadence: String,
    pub member_count: i64,
    pub function_count: i64,
}

#[derive(Serialize)]
pub struct ApiTorDetail {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub description: String,
    pub status: String,
    pub meeting_cadence: String,
    pub cadence_day: String,
    pub cadence_time: String,
    pub cadence_duration_minutes: String,
    pub default_location: String,
    pub member_count: i64,
}

/// GET /api/v1/tors - List Terms of Reference with optional status filter and pagination.
/// Query params: status (filter), page (default 1), per_page (default 25).
pub async fn list(
    pool: web::Data<PgPool>,
    session: Session,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.list")?;

    let status_filter = query.get("status").map(|s| s.as_str());
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

    let all_items = tor::find_all_list_items(&pool).await?;

    // Apply status filter if provided
    let filtered: Vec<_> = if let Some(status) = status_filter {
        all_items.into_iter().filter(|t| t.status == status).collect()
    } else {
        all_items
    };

    let total = filtered.len() as i64;
    let offset = ((page - 1) * per_page) as usize;
    let items: Vec<ApiTorListItem> = filtered
        .into_iter()
        .skip(offset)
        .take(per_page as usize)
        .map(|t| ApiTorListItem {
            id: t.id,
            name: t.name,
            label: t.label,
            description: t.description,
            status: t.status,
            meeting_cadence: t.meeting_cadence,
            member_count: t.member_count,
            function_count: t.function_count,
        })
        .collect();

    Ok(HttpResponse::Ok().json(PaginatedResponse {
        items,
        page,
        per_page,
        total,
    }))
}

/// GET /api/v1/tors/{id} - Get ToR detail with member count.
pub async fn detail(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.list")?;

    let tor_id = path.into_inner();
    let tor = tor::find_detail_by_id(&pool, tor_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let member_count = tor::count_members(&pool, tor_id).await.unwrap_or(0);

    Ok(HttpResponse::Ok().json(ApiTorDetail {
        id: tor.id,
        name: tor.name,
        label: tor.label,
        description: tor.description,
        status: tor.status,
        meeting_cadence: tor.meeting_cadence,
        cadence_day: tor.cadence_day,
        cadence_time: tor.cadence_time,
        cadence_duration_minutes: tor.cadence_duration_minutes,
        default_location: tor.default_location,
        member_count,
    }))
}
