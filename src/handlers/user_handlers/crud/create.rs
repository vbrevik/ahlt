use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

use crate::models::user::{self, UserForm};
use crate::auth::{csrf, password};
use crate::auth::session::require_permission;
use crate::errors::{AppError, render};
use crate::handlers::warning_handlers::ws::ConnectionMap;
use crate::templates_structs::{PageContext, UserFormTemplate};
use super::helpers;

pub async fn new_form(
    pool: web::Data<PgPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.create")?;

    let ctx = PageContext::build(&session, &pool, "/users").await?;

    let tmpl = UserFormTemplate {
        ctx,
        form_action: "/users".to_string(),
        form_title: "Create User".to_string(),
        user: None,
        errors: vec![],
    };
    render(tmpl)
}

pub async fn create(
    pool: web::Data<PgPool>,
    session: Session,
    form: web::Form<UserForm>,
    conn_map: web::Data<ConnectionMap>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.create")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let errors = helpers::validate_user_form(&form, true);

    if !errors.is_empty() {
        let ctx = PageContext::build(&session, &pool, "/users").await?;
        let tmpl = UserFormTemplate {
            ctx,
            form_action: "/users".to_string(),
            form_title: "Create User".to_string(),
            user: None,
            errors,
        };
        return render(tmpl);
    }

    let hashed = match password::hash_password(&form.password) {
        Ok(h) => h,
        Err(_) => return Err(AppError::Hash("Password hash error".to_string())),
    };

    let new = user::NewUser {
        username: form.username.trim().to_string(),
        password: hashed,
        email: form.email.trim().to_string(),
        display_name: form.display_name.trim().to_string(),
    };

    match user::create(&pool, &new).await {
        Ok(user_id) => {
            let _ = user::assign_default_role(&pool, user_id).await;

            let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
            let details = serde_json::json!({
                "email": new.email,
                "summary": format!("Created user '{}'", new.username)
            });
            let _ = crate::audit::log(&pool, current_user_id, "user.created",
                                      "user", user_id, details).await;

            let msg = format!("User '{}' was created", new.username);
            if let Ok(wid) = crate::warnings::create_warning(
                &pool, "info", "governance", "event.user.created", &msg, "", "system"
            ).await {
                let admins = crate::warnings::get_users_with_permission(&pool, "admin.settings").await.unwrap_or_default();
                if !admins.is_empty() {
                    let _ = crate::warnings::create_receipts(&pool, wid, &admins).await;
                    crate::handlers::warning_handlers::ws::notify_users(
                        &conn_map, &pool, &admins, wid, "info", &msg,
                    ).await;
                }
            }

            let _ = session.insert("flash", "User created successfully");
            Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/users"))
                .finish())
        }
        Err(e) => {
            let msg = if e.to_string().contains("UNIQUE") {
                "Username already exists".to_string()
            } else {
                format!("Error creating user: {e}")
            };
            let ctx = PageContext::build(&session, &pool, "/users").await?;
            let tmpl = UserFormTemplate {
                ctx,
                form_action: "/users".to_string(),
                form_title: "Create User".to_string(),
                user: None,
                errors: vec![msg],
            };
            render(tmpl)
        }
    }
}
