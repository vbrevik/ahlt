use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

use crate::auth::session::require_permission;
use crate::errors::{render, AppError};
use crate::models::meeting;
use crate::templates_structs::{MeetingsListTemplate, PageContext};

/// GET /meetings — list all meetings across all ToRs (upcoming + past).
pub async fn list(
    pool: web::Data<PgPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "meetings.view")?;
    let ctx = PageContext::build(&session, &pool, "/meetings").await?;

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let upcoming = meeting::find_upcoming_all(&pool, &today).await?;
    let past = meeting::find_past_all(&pool, &today).await?;

    let tmpl = MeetingsListTemplate {
        ctx,
        upcoming,
        past,
    };
    render(tmpl)
}

/// GET /tor/{id}/meetings — list meetings for a specific ToR.
pub async fn list_for_tor(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    use crate::auth::session::get_user_id;
    use crate::models::tor;
    use crate::templates_structs::TorMeetingsListTemplate;

    require_permission(&session, "meetings.view")?;
    let tor_id = path.into_inner();
    let user_id = get_user_id(&session)
        .ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    let tor_name = tor::get_tor_name(&pool, tor_id).await?;
    let meetings = meeting::find_by_tor(&pool, tor_id).await.unwrap_or_default();

    let ctx = PageContext::build(&session, &pool, "/meetings").await?
        .with_tor(tor_id, &tor_name, "meetings");

    render(TorMeetingsListTemplate { ctx, tor_id, tor_name, meetings })
}
