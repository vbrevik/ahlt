use actix_session::Session;
use actix_web::{web, HttpResponse, Responder};
use askama::Template;

use crate::db::DbPool;
use crate::models::user;
use crate::auth::session::{take_flash, get_username, get_permissions};
use crate::templates_structs::DashboardTemplate;

pub async fn index(
    pool: web::Data<DbPool>,
    session: Session,
) -> impl Responder {
    let username = get_username(&session);
    let role_label = session.get::<String>("role_label").unwrap_or(None).unwrap_or_default();
    let flash = take_flash(&session);
    let permissions = get_permissions(&session);

    let user_count = match pool.get() {
        Ok(conn) => user::count(&conn).unwrap_or(0),
        Err(_) => 0,
    };

    let tmpl = DashboardTemplate {
        username,
        role_label,
        user_count,
        flash,
        permissions,
    };

    match tmpl.render() {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(_) => HttpResponse::InternalServerError().body("Template error"),
    }
}
