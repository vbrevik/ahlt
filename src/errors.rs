use actix_web::{HttpResponse, ResponseError};
use askama::Template;
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    Db(rusqlite::Error),
    Pool(r2d2::Error),
    Template(askama::Error),
    Hash(String),
    NotFound,
    PermissionDenied(String),
    Session(String),
    Csrf(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Db(e) => write!(f, "Database error: {e}"),
            AppError::Pool(e) => write!(f, "Pool error: {e}"),
            AppError::Template(e) => write!(f, "Template error: {e}"),
            AppError::Hash(e) => write!(f, "Hash error: {e}"),
            AppError::NotFound => write!(f, "Not found"),
            AppError::PermissionDenied(perm) => write!(f, "Permission denied: {}", perm),
            AppError::Session(msg) => write!(f, "Session error: {}", msg),
            AppError::Csrf(msg) => write!(f, "CSRF error: {}", msg),
        }
    }
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        match self {
            AppError::PermissionDenied(_) | AppError::Csrf(_) => {
                HttpResponse::Forbidden()
                    .content_type("text/html; charset=utf-8")
                    .body("<h1>403 Forbidden</h1><p>You don't have permission to access this resource.</p>")
            }
            AppError::NotFound => {
                let html = include_str!("../templates/errors/404.html");
                HttpResponse::NotFound()
                    .content_type("text/html; charset=utf-8")
                    .body(html)
            }
            _ => {
                log::error!("{self}");
                let html = include_str!("../templates/errors/500.html");
                HttpResponse::InternalServerError()
                    .content_type("text/html; charset=utf-8")
                    .body(html)
            }
        }
    }
}

/// Helper to render Askama templates with automatic error conversion.
pub fn render<T: Template>(tmpl: T) -> Result<HttpResponse, AppError> {
    let body = tmpl.render()?;  // Uses From<askama::Error> for AppError
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(body))
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Db(e)
    }
}

impl From<r2d2::Error> for AppError {
    fn from(e: r2d2::Error) -> Self {
        AppError::Pool(e)
    }
}

impl From<askama::Error> for AppError {
    fn from(e: askama::Error) -> Self {
        AppError::Template(e)
    }
}
