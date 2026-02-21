use actix_session::Session;
use actix_web::{web, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;

use crate::models::{user, entity};
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

#[derive(Deserialize)]
pub struct ProfileUpdateForm {
    pub action: String, // "upload_avatar", "delete_avatar", "update_display_name"
    pub csrf_token: String,
    #[serde(default)]
    pub avatar_data_uri: String,
    #[serde(default)]
    pub display_name: String,
}

pub async fn form(
    pool: web::Data<PgPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    let ctx = PageContext::build(&session, &pool, "/account").await?;
    let tmpl = AccountTemplate { ctx, errors: vec![] };
    render(tmpl)
}

pub async fn submit(
    pool: web::Data<PgPool>,
    session: Session,
    form: web::Form<ChangePasswordForm>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let user_id = get_user_id(&session)
        .ok_or_else(|| AppError::Session("User not logged in".to_string()))?;

    // Validate inputs
    let mut errors: Vec<String> = vec![];
    errors.extend(validate::validate_password(&form.new_password));
    if form.new_password != form.confirm_password {
        errors.push("New passwords do not match".to_string());
    }

    if !errors.is_empty() {
        let ctx = PageContext::build(&session, &pool, "/account").await?;
        let tmpl = AccountTemplate { ctx, errors };
        return render(tmpl);
    }

    // Verify current password
    let stored_hash = user::find_password_hash_by_id(&pool, user_id).await?
        .ok_or_else(|| AppError::Session("Could not verify current password".to_string()))?;

    match password::verify_password(&form.current_password, &stored_hash) {
        Ok(true) => {}
        _ => {
            let ctx = PageContext::build(&session, &pool, "/account").await?;
            let tmpl = AccountTemplate { ctx, errors: vec!["Current password is incorrect".to_string()] };
            return render(tmpl);
        }
    }

    // Hash and save new password
    let new_hash = password::hash_password(&form.new_password)
        .map_err(AppError::Hash)?;
    user::update_password(&pool, user_id, &new_hash).await?;

    let _ = session.insert("flash", "Password changed successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/account"))
        .finish())
}

/// Handle profile updates (avatar upload, delete, display name change)
pub async fn update_profile(
    pool: web::Data<PgPool>,
    session: Session,
    form: web::Form<ProfileUpdateForm>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let user_id = get_user_id(&session)
        .ok_or_else(|| AppError::Session("User not logged in".to_string()))?;

    match form.action.as_str() {
        "upload_avatar" => {
            // Validate data URI
            if !form.avatar_data_uri.starts_with("data:image/") {
                return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Invalid image format"
                })));
            }

            // Limit size (data URI includes prefix, so estimate ~1.33x of binary size)
            if form.avatar_data_uri.len() > 200 * 1024 * 1.4 as usize {
                return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Avatar too large"
                })));
            }

            // Save avatar to entity_properties
            entity::set_property(&pool, user_id, "avatar_data_uri", &form.avatar_data_uri).await?;

            // Audit log
            let details = serde_json::json!({
                "summary": "User avatar uploaded"
            });
            let _ = crate::audit::log(&pool, user_id, "user.avatar_uploaded", "user", user_id, details).await;

            Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
        }
        "delete_avatar" => {
            // Delete avatar property
            entity::delete_property(&pool, user_id, "avatar_data_uri").await?;

            // Audit log
            let details = serde_json::json!({
                "summary": "User avatar deleted"
            });
            let _ = crate::audit::log(&pool, user_id, "user.avatar_deleted", "user", user_id, details).await;

            Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
        }
        "update_display_name" => {
            // Validate display name
            let mut errors = Vec::new();
            errors.extend(validate::validate_optional(&form.display_name, "Display name", 100));

            if !errors.is_empty() {
                return Ok(HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Validation failed",
                    "details": errors.join("; ")
                })));
            }

            // Get current user data
            let current_user = user::find_display_by_id(&pool, user_id).await?
                .ok_or(AppError::NotFound)?;

            // Update display name
            user::update(
                &pool,
                user_id,
                &current_user.username,
                None,
                &current_user.email,
                &form.display_name,
            ).await?;

            // Update session label
            let _ = session.insert("label", form.display_name.clone());

            // Audit log
            let details = serde_json::json!({
                "old_display_name": current_user.display_name,
                "new_display_name": form.display_name,
                "summary": "User display name updated"
            });
            let _ = crate::audit::log(&pool, user_id, "user.display_name_updated", "user", user_id, details).await;

            Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true })))
        }
        _ => Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Invalid action"
        })))
    }
}
