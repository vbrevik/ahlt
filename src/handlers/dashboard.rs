use actix_session::Session;
use actix_web::{web, HttpResponse, Responder};
use askama::Template;

use crate::db::DbPool;
use crate::models::user;
use crate::templates_structs::{PageContext, DashboardTemplate};

pub async fn index(
    pool: web::Data<DbPool>,
    session: Session,
) -> impl Responder {
    let role_label = session.get::<String>("role_label").unwrap_or(None).unwrap_or_default();

    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };

    let ctx = PageContext::build(&session, &conn, "/dashboard");
    let user_count = user::count(&conn).unwrap_or(0);

    let tmpl = DashboardTemplate { ctx, role_label, user_count };
    match tmpl.render() {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(_) => HttpResponse::InternalServerError().body("Template error"),
    }
}
