use actix_session::Session;
use actix_web::{web, HttpResponse};
use serde::Serialize;
use sqlx::PgPool;

use crate::auth::session::{get_user_id, require_permission};
use crate::errors::AppError;
use crate::templates_structs::PaginatedResponse;
use crate::warnings;

#[derive(Serialize)]
pub struct ApiWarningItem {
    pub warning_id: i64,
    pub receipt_id: i64,
    pub severity: String,
    pub category: String,
    pub message: String,
    pub status: String,
    pub created_at: String,
}

/// GET /api/v1/warnings - List warnings scoped to the calling user.
/// Query params: severity (filter), page (default 1), per_page (default 25).
pub async fn list(
    pool: web::Data<PgPool>,
    session: Session,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "warnings.view")?;

    let user_id = get_user_id(&session)
        .ok_or_else(|| AppError::Session("User not logged in".to_string()))?;

    let severity_filter = query.get("severity").map(|s| s.as_str());
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

    let warning_page = warnings::queries::find_for_user(
        &pool,
        user_id,
        page,
        per_page,
        None,              // no category filter via API (keep simple)
        severity_filter,
        true,              // show read warnings
        false,             // hide deleted warnings
    )
    .await?;

    let items: Vec<ApiWarningItem> = warning_page
        .items
        .into_iter()
        .map(|w| ApiWarningItem {
            warning_id: w.warning_id,
            receipt_id: w.receipt_id,
            severity: w.severity,
            category: w.category,
            message: w.message,
            status: w.status,
            created_at: w.created_at,
        })
        .collect();

    Ok(HttpResponse::Ok().json(PaginatedResponse {
        items,
        page,
        per_page,
        total: warning_page.total_count,
    }))
}
