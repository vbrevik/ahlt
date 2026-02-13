use actix_session::Session;
use actix_web::{web, HttpResponse, Responder};
use askama::Template;
use serde::Deserialize;

use crate::db::DbPool;
use crate::models::user;
use crate::auth::{csrf, password};
use crate::auth::session::get_user_id;
use crate::templates_structs::{PageContext, AccountTemplate};

#[derive(Deserialize)]
pub struct ChangePasswordForm {
    pub current_password: String,
    pub new_password: String,
    pub confirm_password: String,
    pub csrf_token: String,
}

pub async fn form(
    pool: web::Data<DbPool>,
    session: Session,
) -> impl Responder {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };

    let ctx = PageContext::build(&session, &conn, "/account");
    let tmpl = AccountTemplate { ctx, errors: vec![] };
    match tmpl.render() {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(_) => HttpResponse::InternalServerError().body("Template error"),
    }
}

pub async fn submit(
    pool: web::Data<DbPool>,
    session: Session,
    form: web::Form<ChangePasswordForm>,
) -> impl Responder {
    if let Err(resp) = csrf::validate_csrf(&session, &form.csrf_token) {
        return resp;
    }

    let user_id = match get_user_id(&session) {
        Some(id) => id,
        None => return HttpResponse::SeeOther()
            .insert_header(("Location", "/login"))
            .finish(),
    };

    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };

    // Validate inputs
    let mut errors = vec![];
    if form.new_password.is_empty() {
        errors.push("New password is required".to_string());
    }
    if form.new_password != form.confirm_password {
        errors.push("New passwords do not match".to_string());
    }

    if !errors.is_empty() {
        let ctx = PageContext::build(&session, &conn, "/account");
        let tmpl = AccountTemplate { ctx, errors };
        return match tmpl.render() {
            Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
            Err(_) => HttpResponse::InternalServerError().body("Template error"),
        };
    }

    // Verify current password
    let stored_hash = match user::find_password_hash_by_id(&conn, user_id) {
        Ok(Some(h)) => h,
        _ => {
            let ctx = PageContext::build(&session, &conn, "/account");
            let tmpl = AccountTemplate { ctx, errors: vec!["Could not verify current password".to_string()] };
            return match tmpl.render() {
                Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
                Err(_) => HttpResponse::InternalServerError().body("Template error"),
            };
        }
    };

    match password::verify_password(&form.current_password, &stored_hash) {
        Ok(true) => {}
        _ => {
            let ctx = PageContext::build(&session, &conn, "/account");
            let tmpl = AccountTemplate { ctx, errors: vec!["Current password is incorrect".to_string()] };
            return match tmpl.render() {
                Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
                Err(_) => HttpResponse::InternalServerError().body("Template error"),
            };
        }
    }

    // Hash and save new password
    let new_hash = match password::hash_password(&form.new_password) {
        Ok(h) => h,
        Err(_) => return HttpResponse::InternalServerError().body("Password hash error"),
    };

    match user::update_password(&conn, user_id, &new_hash) {
        Ok(_) => {
            let _ = session.insert("flash", "Password changed successfully");
            HttpResponse::SeeOther()
                .insert_header(("Location", "/account"))
                .finish()
        }
        Err(_) => {
            let ctx = PageContext::build(&session, &conn, "/account");
            let tmpl = AccountTemplate { ctx, errors: vec!["Error updating password".to_string()] };
            match tmpl.render() {
                Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
                Err(_) => HttpResponse::InternalServerError().body("Template error"),
            }
        }
    }
}
