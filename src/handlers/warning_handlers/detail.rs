use actix_session::Session;
use actix_web::{web, HttpResponse};
use rusqlite::params;

use crate::db::DbPool;
use crate::auth::session::get_user_id;
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, WarningDetailTemplate, UserOption};
use crate::warnings::queries;

pub async fn detail(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let warning_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or_else(|| AppError::Session("No user".into()))?;
    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/warnings")?;

    let warning = queries::get_warning_detail(&conn, warning_id)?
        .ok_or(AppError::NotFound)?;

    let recipients = queries::get_recipients(&conn, warning_id)?;

    // Get timeline for current user's receipt
    let receipt_id = queries::find_receipt_for_user(&conn, warning_id, user_id)?
        .unwrap_or(0);
    let timeline = if receipt_id > 0 {
        queries::get_receipt_timeline(&conn, receipt_id)?
    } else {
        Vec::new()
    };

    // Get users for forward dropdown (exclude self)
    let mut stmt = conn.prepare(
        "SELECT id, name, label FROM entities WHERE entity_type = 'user' AND id != ?1 ORDER BY name"
    )?;
    let users = stmt.query_map(params![user_id], |row| {
        Ok(UserOption {
            id: row.get(0)?,
            name: row.get(1)?,
            label: row.get(2)?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;

    // Auto-mark as read when viewing
    if receipt_id > 0 {
        let current_status: String = conn.query_row(
            "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = 'status'",
            params![receipt_id],
            |row| row.get(0),
        ).unwrap_or_default();
        if current_status == "unread" {
            crate::warnings::update_receipt_status(&conn, receipt_id, "read", user_id)?;
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
