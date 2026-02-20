use actix_session::Session;
use actix_web::{web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::db::DbPool;
use crate::models::{user, permission, setting};
use crate::auth::{csrf, password, rate_limit::RateLimiter};
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
    req: HttpRequest,
    pool: web::Data<DbPool>,
    session: Session,
    form: web::Form<LoginForm>,
    limiter: web::Data<RateLimiter>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;

    // Rate-limit check BEFORE any database access
    let ip = req.peer_addr()
        .map(|addr| addr.ip())
        .unwrap_or_else(|| std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));

    if limiter.is_blocked(ip) {
        let conn = pool.get()?;
        let app_name = setting::get_value(&conn, "app.name", "Ahlt");
        let csrf_token = csrf::get_or_create_token(&session);
        let tmpl = LoginTemplate {
            error: Some("Too many failed login attempts. Please try again later.".to_string()),
            app_name,
            csrf_token,
        };
        return render(tmpl);
    }

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
                    // Successful login — clear rate limit for this IP
                    limiter.clear(ip);

                    // Multi-role: aggregate permissions across all assigned roles
                    let perms = permission::find_codes_by_user_id(&conn, u.id)?;
                    let perms_csv = perms.join(",");

                    let _ = session.insert("user_id", u.id);
                    let _ = session.insert("username", &u.username);
                    let _ = session.insert("permissions", &perms_csv);
                    Ok(HttpResponse::SeeOther()
                        .insert_header(("Location", "/dashboard"))
                        .finish())
                }
                _ => {
                    limiter.record_failure(ip);
                    render_error("Invalid username or password")
                }
            }
        }
        None => {
            limiter.record_failure(ip);
            render_error("Invalid username or password")
        }
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
