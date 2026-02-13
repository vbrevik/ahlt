use actix_session::Session;
use actix_web::{web, HttpResponse};
use serde::Deserialize;

use crate::db::DbPool;
use crate::models::{user, role, permission, setting};
use crate::auth::{csrf, password};
use crate::errors::{AppError, render};
use crate::templates_structs::LoginTemplate;

#[derive(Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
    pub csrf_token: String,
}

#[derive(Deserialize)]
pub struct CsrfOnly {
    pub csrf_token: String,
}

pub async fn login_page(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    // If already logged in, redirect to dashboard
    if session.get::<i64>("user_id").unwrap_or(None).is_some() {
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", "/dashboard"))
            .finish());
    }

    let conn = pool.get()?;
    let app_name = setting::get_value(&conn, "app.name", "Ahlt");

    let csrf_token = csrf::get_or_create_token(&session);
    let tmpl = LoginTemplate { error: None, app_name, csrf_token };
    render(tmpl)
}

pub async fn login_submit(
    pool: web::Data<DbPool>,
    session: Session,
    form: web::Form<LoginForm>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let conn = pool.get()?;
    let app_name = setting::get_value(&conn, "app.name", "Ahlt");

    // Helper to render error page
    let render_error = |msg: &str| -> Result<HttpResponse, AppError> {
        let csrf_token = csrf::get_or_create_token(&session);
        let tmpl = LoginTemplate {
            error: Some(msg.to_string()),
            app_name: app_name.clone(),
            csrf_token,
        };
        render(tmpl)
    };

    // Look up user
    let found = user::find_by_username(&conn, &form.username)?;

    match found {
        Some(u) => {
            match password::verify_password(&form.password, &u.password) {
                Ok(true) => {
                    // Look up role label for display
                    let role_label = role::find_by_id(&conn, u.role_id)?
                        .map(|r| r.label)
                        .unwrap_or_default();

                    // Look up permissions for this role
                    let perms = permission::find_codes_by_role_id(&conn, u.role_id)?;
                    let perms_csv = perms.join(",");

                    let _ = session.insert("user_id", u.id);
                    let _ = session.insert("username", &u.username);
                    let _ = session.insert("role_id", u.role_id);
                    let _ = session.insert("role_label", &role_label);
                    let _ = session.insert("permissions", &perms_csv);
                    Ok(HttpResponse::SeeOther()
                        .insert_header(("Location", "/dashboard"))
                        .finish())
                }
                _ => render_error("Invalid username or password"),
            }
        }
        None => render_error("Invalid username or password"),
    }
}

pub async fn logout(
    session: Session,
    form: web::Form<CsrfOnly>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;
    session.purge();
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/login"))
        .finish())
}
