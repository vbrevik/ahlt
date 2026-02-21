use actix_session::Session;
use actix_web::{web, HttpResponse};
use chrono::{Datelike, Local, NaiveDate};
use serde::Deserialize;
use sqlx::PgPool;

use crate::auth::session::require_permission;
use crate::errors::{render, AppError};
use crate::models::tor;
use crate::templates_structs::{PageContext, TorOutlookTemplate};

#[derive(Deserialize)]
pub struct CalendarQuery {
    pub start: Option<String>,
    pub end: Option<String>,
}

pub async fn calendar_api(
    pool: web::Data<PgPool>,
    session: Session,
    query: web::Query<CalendarQuery>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.list")?;

    let today = Local::now().date_naive();
    let start = query
        .start
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or(today);
    let end = query
        .end
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| start + chrono::Duration::days(6));

    // Cap range to 90 days
    let max_end = start + chrono::Duration::days(90);
    if end > max_end {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Date range must not exceed 90 days"
        })));
    }

    let events = tor::compute_meetings(&pool, start, end).await?;
    Ok(HttpResponse::Ok().json(events))
}

pub async fn outlook(
    pool: web::Data<PgPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.list")?;

    let ctx = PageContext::build(&session, &pool, "/tor/outlook").await?;

    // Compute initial week (Mon-Sun containing today)
    let today = Local::now().date_naive();
    let days_since_monday = today.weekday().num_days_from_monday();
    let week_start = today - chrono::Duration::days(days_since_monday as i64);
    let week_end = week_start + chrono::Duration::days(6);

    let events = tor::compute_meetings(&pool, week_start, week_end).await?;
    let events_json =
        serde_json::to_string(&events).unwrap_or_else(|_| "[]".to_string());
    let today_str = today.format("%Y-%m-%d").to_string();
    let week_start_str = week_start.format("%Y-%m-%d").to_string();

    let tmpl = TorOutlookTemplate {
        ctx,
        events_json,
        today: today_str,
        week_start: week_start_str,
    };
    render(tmpl)
}
