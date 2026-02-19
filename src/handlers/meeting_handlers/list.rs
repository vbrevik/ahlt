use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::errors::AppError;

/// GET /meetings — list all meetings across all ToRs.
pub async fn list(
    _pool: web::Data<DbPool>,
    _session: Session,
) -> Result<HttpResponse, AppError> {
    Ok(HttpResponse::Ok().body("placeholder: meeting list"))
}
