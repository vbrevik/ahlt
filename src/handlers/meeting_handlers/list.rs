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
