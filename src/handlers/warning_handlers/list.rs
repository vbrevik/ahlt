use actix_session::Session;
use actix_web::{web, HttpResponse};
use serde::Deserialize;

use crate::db::DbPool;
use crate::auth::session::get_user_id;
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, WarningListTemplate};
use crate::warnings::queries;

#[derive(Deserialize)]
pub struct WarningQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    category: Option<String>,
    severity: Option<String>,
    show_read: Option<String>,
    show_deleted: Option<String>,
}

pub async fn list(
    pool: web::Data<DbPool>,
    session: Session,
    query: web::Query<WarningQuery>,
) -> Result<HttpResponse, AppError> {
    let user_id = get_user_id(&session).ok_or_else(|| AppError::Session("No user".into()))?;
    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/warnings")?;

    let show_read = query.show_read.as_deref() == Some("true");
    let show_deleted = query.show_deleted.as_deref() == Some("true");

    let warning_page = queries::find_for_user(
        &conn,
        user_id,
        query.page.unwrap_or(1),
        query.per_page.unwrap_or(25),
        query.category.as_deref(),
        query.severity.as_deref(),
        show_read,
        show_deleted,
    )?;

    let tmpl = WarningListTemplate {
        ctx,
        warning_page,
        category_filter: query.category.clone(),
        severity_filter: query.severity.clone(),
        show_read,
        show_deleted,
    };

    render(tmpl)
}
