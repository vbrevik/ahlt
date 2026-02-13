use actix_session::Session;
use actix_web::{web, HttpResponse, Responder};
use askama::Template;
use serde::Deserialize;

use crate::db::DbPool;
use crate::models::{user, role, permission};
use crate::auth::password;
use crate::templates_structs::LoginTemplate;

#[derive(Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

pub async fn login_page(session: Session) -> impl Responder {
    // If already logged in, redirect to dashboard
    if session.get::<i64>("user_id").unwrap_or(None).is_some() {
        return HttpResponse::SeeOther()
            .insert_header(("Location", "/dashboard"))
            .finish();
    }

    let tmpl = LoginTemplate { error: None };
    match tmpl.render() {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(_) => HttpResponse::InternalServerError().body("Template error"),
    }
}

pub async fn login_submit(
    pool: web::Data<DbPool>,
    session: Session,
    form: web::Form<LoginForm>,
) -> impl Responder {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };

    // Look up user
    let found = user::find_by_username(&conn, &form.username);
    let render_error = |msg: &str| {
        let tmpl = LoginTemplate {
            error: Some(msg.to_string()),
        };
        match tmpl.render() {
            Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
            Err(_) => HttpResponse::InternalServerError().body("Template error"),
        }
    };

    match found {
        Ok(Some(u)) => {
            match password::verify_password(&form.password, &u.password) {
                Ok(true) => {
                    // Look up role label for display
                    let role_label = role::find_by_id(&conn, u.role_id)
                        .ok()
                        .flatten()
                        .map(|r| r.label)
                        .unwrap_or_default();

                    // Look up permissions for this role
                    let perms = permission::find_codes_by_role_id(&conn, u.role_id)
                        .unwrap_or_default();
                    let perms_csv = perms.join(",");

                    let _ = session.insert("user_id", u.id);
                    let _ = session.insert("username", &u.username);
                    let _ = session.insert("role_id", u.role_id);
                    let _ = session.insert("role_label", &role_label);
                    let _ = session.insert("permissions", &perms_csv);
                    HttpResponse::SeeOther()
                        .insert_header(("Location", "/dashboard"))
                        .finish()
                }
                _ => render_error("Invalid username or password"),
            }
        }
        _ => render_error("Invalid username or password"),
    }
}

pub async fn logout(session: Session) -> impl Responder {
    session.purge();
    HttpResponse::SeeOther()
        .insert_header(("Location", "/login"))
        .finish()
}
