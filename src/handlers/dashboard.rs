use actix_session::Session;
use actix_web::{web, HttpResponse};
use chrono::{Local, Timelike};

use crate::db::DbPool;
use crate::models::{user, entity, audit, proposal};
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, DashboardTemplate};

fn time_greeting(username: &str) -> String {
    let hour = Local::now().hour();
    let period = match hour {
        5..=11 => "Good morning",
        12..=16 => "Good afternoon",
        17..=21 => "Good evening",
        _ => "Good evening",
    };
    format!("{}, {}", period, username)
}

pub async fn index(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    let role_label = session.get::<String>("role_label").unwrap_or(None).unwrap_or_default();

    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/dashboard")?;

    let greeting = time_greeting(&ctx.username);
    let user_count = user::count(&conn)?;
    let role_count = entity::count_by_type(&conn, "role")?;
    let proposal_count = proposal::count_by_status(&conn, "submitted");
    let tor_position_count = entity::count_by_type(&conn, "tor_function")?;
    let audit_entry_count = entity::count_by_type(&conn, "audit_entry")?;

    let recent_activity = audit::find_recent(&conn, 5).unwrap_or_default();

    let tmpl = DashboardTemplate {
        ctx,
        role_label,
        greeting,
        user_count,
        role_count,
        proposal_count,
        tor_position_count,
        audit_entry_count,
        recent_activity,
    };
    render(tmpl)
}
