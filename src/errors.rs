use actix_web::{HttpResponse, ResponseError};
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    Db(rusqlite::Error),
    Pool(r2d2::Error),
    Template(askama::Error),
    Hash(String),
    NotFound,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Db(e) => write!(f, "Database error: {e}"),
            AppError::Pool(e) => write!(f, "Pool error: {e}"),
            AppError::Template(e) => write!(f, "Template error: {e}"),
            AppError::Hash(e) => write!(f, "Hash error: {e}"),
            AppError::NotFound => write!(f, "Not found"),
        }
    }
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        match self {
            AppError::NotFound => HttpResponse::NotFound().body("Not Found"),
            _ => {
                log::error!("{self}");
                HttpResponse::InternalServerError().body("Internal Server Error")
            }
        }
    }
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
