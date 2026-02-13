use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::models::user;
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, DashboardTemplate};

pub async fn index(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    let role_label = session.get::<String>("role_label").unwrap_or(None).unwrap_or_default();

    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/dashboard")?;
    let user_count = user::count(&conn)?;

    let tmpl = DashboardTemplate { ctx, role_label, user_count };
    render(tmpl)
}
