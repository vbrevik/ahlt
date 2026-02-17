use actix_session::Session;
use actix_web::{web, HttpResponse};
use serde::Deserialize;

use crate::db::DbPool;
use crate::models::user;
use crate::auth::{csrf, password, validate};
use crate::auth::session::get_user_id;
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, AccountTemplate};

#[derive(Deserialize)]
pub struct ChangePasswordForm {
    pub current_password: String,
    pub new_password: String,
    pub confirm_password: String,
    pub csrf_token: String,
}

pub async fn form(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/account")?;
    let tmpl = AccountTemplate { ctx, errors: vec![] };
    render(tmpl)
}

pub async fn submit(
    pool: web::Data<DbPool>,
    session: Session,
    form: web::Form<ChangePasswordForm>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let user_id = get_user_id(&session)
        .ok_or_else(|| AppError::Session("User not logged in".to_string()))?;

    let conn = pool.get()?;

    // Validate inputs
    let mut errors: Vec<String> = vec![];
    errors.extend(validate::validate_password(&form.new_password));
    if form.new_password != form.confirm_password {
        errors.push("New passwords do not match".to_string());
    }

    if !errors.is_empty() {
        let ctx = PageContext::build(&session, &conn, "/account")?;
        let tmpl = AccountTemplate { ctx, errors };
        return render(tmpl);
    }

    // Verify current password
    let stored_hash = user::find_password_hash_by_id(&conn, user_id)?
        .ok_or_else(|| AppError::Session("Could not verify current password".to_string()))?;

    match password::verify_password(&form.current_password, &stored_hash) {
        Ok(true) => {}
        _ => {
            let ctx = PageContext::build(&session, &conn, "/account")?;
            let tmpl = AccountTemplate { ctx, errors: vec!["Current password is incorrect".to_string()] };
            return render(tmpl);
        }
    }

    // Hash and save new password
    let new_hash = password::hash_password(&form.new_password)
        .map_err(AppError::Hash)?;
    user::update_password(&conn, user_id, &new_hash)?;

    let _ = session.insert("flash", "Password changed successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/account"))
        .finish())
}
