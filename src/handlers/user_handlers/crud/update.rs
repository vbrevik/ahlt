use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

use crate::models::user::{self, UserForm};
use crate::auth::{csrf, password, validate};
use crate::auth::session::require_permission;
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, UserFormTemplate};

pub async fn update(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<UserForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let id = path.into_inner();

    // Validate
    let mut errors: Vec<String> = vec![];
    errors.extend(validate::validate_username(&form.username));
    errors.extend(validate::validate_email(&form.email));
    errors.extend(validate::validate_optional(&form.display_name, "Display name", 100));
    // Password is optional on update — only validate if provided
    if !form.password.is_empty() {
        errors.extend(validate::validate_password(&form.password));
    }

    if !errors.is_empty() {
        let existing = user::find_display_by_id(&pool, id).await.ok().flatten();
        let ctx = PageContext::build(&session, &pool, "/users").await?;
        let tmpl = UserFormTemplate {
            ctx,
            form_action: format!("/users/{id}"),
            form_title: "Edit User".to_string(),
            user: existing,
            errors,
        };
        return render(tmpl);
    }

    // Hash password if provided
    let hashed = if form.password.is_empty() {
        None
    } else {
        match password::hash_password(&form.password) {
            Ok(h) => Some(h),
            Err(_) => return Err(AppError::Hash("Password hash error".to_string())),
        }
    };

    match user::update(
        &pool,
        id,
        form.username.trim(),
        hashed.as_deref(),
        form.email.trim(),
        form.display_name.trim(),
    ).await {
        Ok(_) => {
            // Audit log
            let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
            let details = serde_json::json!({
                "username": form.username.trim(),
                "email": form.email.trim(),
                "password_changed": !form.password.is_empty(),
                "summary": format!("Updated user '{}'", form.username.trim())
            });
            let _ = crate::audit::log(&pool, current_user_id, "user.updated",
                                      "user", id, details).await;

            let _ = session.insert("flash", "User updated successfully");
            Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/users"))
                .finish())
        }
        Err(e) => {
            let msg = if e.to_string().contains("UNIQUE") {
                "Username already exists".to_string()
            } else {
                format!("Error updating user: {e}")
            };
            let existing = user::find_display_by_id(&pool, id).await.ok().flatten();
            let ctx = PageContext::build(&session, &pool, "/users").await?;
            let tmpl = UserFormTemplate {
                ctx,
                form_action: format!("/users/{id}"),
                form_title: "Edit User".to_string(),
                user: existing,
                errors: vec![msg],
            };
            render(tmpl)
        }
    }
}
