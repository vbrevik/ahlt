use actix_session::Session;
use actix_web::{web, HttpResponse};
use serde::Deserialize;

use crate::db::DbPool;
use crate::models::user;
use crate::auth::session::require_permission;
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, UserListTemplate};

#[derive(Deserialize)]
pub struct PaginationQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    q: Option<String>,
}

pub async fn list(
    pool: web::Data<DbPool>,
    session: Session,
    query: web::Query<PaginationQuery>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.list")?;

    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/users")?;
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(25);
    let search = query.q.as_deref();
    let user_page = user::find_paginated(&conn, page, per_page, search)?;

    let tmpl = UserListTemplate { ctx, user_page, search_query: query.q.clone() };
    render(tmpl)
}
