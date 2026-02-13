use actix_session::Session;
use actix_web::{web, HttpResponse, Responder};
use askama::Template;
use serde::Deserialize;

use crate::db::DbPool;
use crate::models::audit;
use crate::auth::session::require_permission;
use crate::templates_structs::{PageContext, AuditListTemplate};

#[derive(Deserialize)]
pub struct AuditQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    q: Option<String>,
    action: Option<String>,
    target_type: Option<String>,
}

pub async fn list(
    pool: web::Data<DbPool>,
    session: Session,
    query: web::Query<AuditQuery>,
) -> impl Responder {
    if let Err(resp) = require_permission(&session, "audit.view") {
        return resp;
    }

    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };

    let ctx = PageContext::build(&session, &conn, "/audit");
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(25);
    let search = query.q.as_deref();
    let action_filter = query.action.as_deref();
    let target_type_filter = query.target_type.as_deref();

    let audit_page = audit::find_paginated(
        &conn,
        page,
        per_page,
        search,
        action_filter,
        target_type_filter,
    ).unwrap_or_else(|_| audit::AuditEntryPage {
        entries: vec![],
        page: 1,
        per_page: 25,
        total_count: 0,
        total_pages: 0,
    });

    let tmpl = AuditListTemplate {
        ctx,
        audit_page,
        search_query: query.q.clone(),
        action_filter: query.action.clone(),
        target_type_filter: query.target_type.clone(),
    };

    match tmpl.render() {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(_) => HttpResponse::InternalServerError().body("Template error"),
    }
}
