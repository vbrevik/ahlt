use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::auth::session::require_permission;
use crate::db::DbPool;
use crate::errors::{render, AppError};
use crate::models::meeting;
use crate::templates_structs::{MeetingsListTemplate, PageContext};

/// GET /meetings — list all meetings across all ToRs (upcoming + past).
pub async fn list(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "meetings.view")?;
    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/meetings")?;

    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let upcoming = meeting::find_upcoming_all(&conn, &today)?;
    let past = meeting::find_past_all(&conn, &today)?;

    let tmpl = MeetingsListTemplate {
        ctx,
        upcoming,
        past,
    };
    render(tmpl)
}
