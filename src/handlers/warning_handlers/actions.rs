use actix_session::Session;
use actix_web::{web, HttpResponse};
use serde::Deserialize;

use crate::db::DbPool;
use crate::auth::{csrf, session::get_user_id};
use crate::errors::AppError;
use crate::warnings::{self, queries};
use crate::handlers::warning_handlers::ws::{ConnectionMap, send_count_update};

#[derive(Deserialize)]
pub struct ReceiptForm {
    pub csrf_token: String,
    pub receipt_id: i64,
}

#[derive(Deserialize)]
pub struct ForwardForm {
    pub csrf_token: String,
    pub target_user_id: i64,
}

pub async fn mark_deleted(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<ReceiptForm>,
    conn_map: web::Data<ConnectionMap>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;
    let warning_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or_else(|| AppError::Session("No user".into()))?;
    let conn = pool.get()?;

    if let Some(receipt_id) = queries::find_receipt_for_user(&conn, warning_id, user_id)? {
        warnings::update_receipt_status(&conn, receipt_id, "deleted", user_id)?;
    }

    send_count_update(&conn_map, &pool, user_id);

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/warnings"))
        .finish())
}

pub async fn forward(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<ForwardForm>,
    conn_map: web::Data<ConnectionMap>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;
    let warning_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or_else(|| AppError::Session("No user".into()))?;
    let conn = pool.get()?;

    // Update sender's receipt to forwarded
    if let Some(receipt_id) = queries::find_receipt_for_user(&conn, warning_id, user_id)? {
        warnings::update_receipt_status(&conn, receipt_id, "forwarded", user_id)?;
        crate::models::relation::create(&conn, "forwarded_to_user", receipt_id, form.target_user_id)?;
        warnings::create_event(&conn, receipt_id, "forwarded", user_id, None)?;
    }

    // Create receipt for target user
    warnings::create_receipts(&conn, warning_id, &[form.target_user_id])?;

    // Notify target user via WS
    if let Some(w) = queries::get_warning_detail(&conn, warning_id)? {
        crate::handlers::warning_handlers::ws::notify_users(
            &conn_map, &pool, &[form.target_user_id],
            warning_id, &w.severity, &w.message,
        );
    }

    send_count_update(&conn_map, &pool, user_id);

    let location = format!("/warnings/{}", warning_id);
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", location.as_str()))
        .finish())
}
