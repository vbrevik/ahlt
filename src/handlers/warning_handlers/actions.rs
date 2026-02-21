use actix_session::Session;
use actix_web::{web, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;

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
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<ReceiptForm>,
    conn_map: web::Data<ConnectionMap>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;
    let warning_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or_else(|| AppError::Session("No user".into()))?;

    if let Some(receipt_id) = queries::find_receipt_for_user(&pool, warning_id, user_id).await? {
        warnings::update_receipt_status(&pool, receipt_id, "deleted", user_id).await?;
    }

    send_count_update(&conn_map, &pool, user_id).await;

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/warnings"))
        .finish())
}

pub async fn forward(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<ForwardForm>,
    conn_map: web::Data<ConnectionMap>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;
    let warning_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or_else(|| AppError::Session("No user".into()))?;

    // Update sender's receipt to forwarded
    if let Some(receipt_id) = queries::find_receipt_for_user(&pool, warning_id, user_id).await? {
        warnings::update_receipt_status(&pool, receipt_id, "forwarded", user_id).await?;
        crate::models::relation::create(&pool, "forwarded_to_user", receipt_id, form.target_user_id).await?;
        warnings::create_event(&pool, receipt_id, "forwarded", user_id, None).await?;
    }

    // Create receipt for target user
    warnings::create_receipts(&pool, warning_id, &[form.target_user_id]).await?;

    // Notify target user via WS
    if let Some(w) = queries::get_warning_detail(&pool, warning_id).await? {
        crate::handlers::warning_handlers::ws::notify_users(
            &conn_map, &pool, &[form.target_user_id],
            warning_id, &w.severity, &w.message,
        ).await;
    }

    send_count_update(&conn_map, &pool, user_id).await;

    let location = format!("/warnings/{}", warning_id);
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", location.as_str()))
        .finish())
}
