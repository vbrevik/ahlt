use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

use crate::auth::session::get_user_id;
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, WarningDetailTemplate, UserOption};
use crate::warnings::queries;

pub async fn detail(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let warning_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or_else(|| AppError::Session("No user".into()))?;
    let ctx = PageContext::build(&session, &pool, "/warnings").await?;

    let warning = queries::get_warning_detail(&pool, warning_id).await?
        .ok_or(AppError::NotFound)?;

    let recipients = queries::get_recipients(&pool, warning_id).await?;

    // Get timeline for current user's receipt
    let receipt_id = queries::find_receipt_for_user(&pool, warning_id, user_id).await?
        .unwrap_or(0);
    let timeline = if receipt_id > 0 {
        queries::get_receipt_timeline(&pool, receipt_id).await?
    } else {
        Vec::new()
    };

    // Get users for forward dropdown (exclude self)
    let users: Vec<UserOption> = sqlx::query_as(
        "SELECT id, name, label FROM entities WHERE entity_type = 'user' AND id != $1 ORDER BY name",
    )
    .bind(user_id)
    .fetch_all(pool.get_ref())
    .await?;

    // Auto-mark as read when viewing
    if receipt_id > 0 {
        let current_status: String = sqlx::query_as::<_, (String,)>(
            "SELECT value FROM entity_properties WHERE entity_id = $1 AND key = 'status'",
        )
        .bind(receipt_id)
        .fetch_optional(pool.get_ref())
        .await?
        .map(|r| r.0)
        .unwrap_or_default();

        if current_status == "unread" {
            crate::warnings::update_receipt_status(&pool, receipt_id, "read", user_id).await?;
        }
    }

    let tmpl = WarningDetailTemplate {
        ctx,
        warning,
        recipients,
        timeline,
        user_receipt_id: receipt_id,
        users,
    };

    render(tmpl)
}
