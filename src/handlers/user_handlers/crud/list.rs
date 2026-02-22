use serde::Deserialize;
use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

use crate::auth::session::{require_permission, get_user_id};
use crate::errors::AppError;

#[derive(Deserialize)]
pub struct ExportQuery {
    filter: Option<String>,
    sort: Option<String>,
    dir: Option<String>,
}

pub async fn export_csv(
    pool: web::Data<PgPool>,
    session: Session,
    query: web::Query<ExportQuery>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.list")?;

    let filter = query.filter.as_deref()
        .and_then(|s| crate::models::table_filter::FilterTree::from_json(s).ok())
        .unwrap_or_default();
    let sort = crate::models::table_filter::SortSpec::from_params(
        query.sort.as_deref(), query.dir.as_deref()
    );

    let users = crate::models::user::find_all_filtered(&pool, &filter, &sort).await?;

    // Audit log
    let uid = crate::auth::session::get_user_id(&session).unwrap_or(0);
    let _ = crate::audit::log(&pool, uid, "users.export", "user", 0,
        serde_json::json!({ "count": users.len(), "format": "csv" })).await;

    // Get today's date for filename
    let today: String = sqlx::query_scalar("SELECT CURRENT_DATE::text")
        .fetch_one(pool.get_ref())
        .await
        .unwrap_or_else(|_| "unknown".to_string());

    fn escape_csv(s: &str) -> String {
        if s.contains(',') || s.contains('"') || s.contains('\n') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    }

    let mut csv = String::from("id,username,display_name,email,role,created_at,updated_at\n");
    for u in &users {
        csv.push_str(&format!("{},{},{},{},{},{},{}\n",
            u.id,
            escape_csv(&u.username),
            escape_csv(&u.display_name),
            escape_csv(&u.email),
            escape_csv(&u.role_labels),
            u.created_at,
            u.updated_at,
        ));
    }

    Ok(HttpResponse::Ok()
        .content_type("text/csv; charset=utf-8")
        .insert_header(("Content-Disposition",
            format!("attachment; filename=\"users-{today}.csv\"")))
        .body(csv))
}

#[derive(Deserialize)]
pub struct SaveColumnsForm {
    pub columns: String,
    pub set_global: Option<String>,
    pub csrf_token: String,
    pub redirect_to: Option<String>,
}

pub async fn save_columns(
    pool: web::Data<PgPool>,
    session: Session,
    form: web::Form<SaveColumnsForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.list")?;
    crate::auth::csrf::validate_csrf(&session, &form.csrf_token)?;

    let user_id = get_user_id(&session)
        .ok_or_else(|| AppError::Session("Not logged in".to_string()))?;

    // Validate: only known column keys allowed
    const VALID_KEYS: &[&str] = &["user", "email", "status", "created_at", "updated_at", "actions"];
    let sanitized: String = form.columns.split(',')
        .map(str::trim)
        .filter(|k| VALID_KEYS.contains(k))
        .collect::<Vec<_>>()
        .join(",");

    // Always include always-visible columns
    let pref = if !sanitized.contains("user") {
        format!("user,{sanitized}")
    } else { sanitized.clone() };
    let pref = if !pref.contains("actions") {
        format!("{pref},actions")
    } else { pref };

    crate::models::table_filter::columns::save_user_columns(user_id, "users", &pref, &pool).await?;

    // Optionally save global default
    if form.set_global.as_deref() == Some("true") {
        require_permission(&session, "settings.manage")?;
        crate::models::table_filter::columns::save_global_columns("users", &pref, &pool).await?;
    }

    let redirect = form.redirect_to.as_deref().unwrap_or("/users");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", redirect.to_string()))
        .finish())
}
