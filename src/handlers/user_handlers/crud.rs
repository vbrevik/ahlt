use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::models::user::{self, UserForm};
use crate::models::role;
use crate::auth::{csrf, password, validate};
use crate::auth::session::{get_user_id, require_permission};
use crate::errors::{AppError, render};
use crate::handlers::auth_handlers::CsrfOnly;
use crate::handlers::warning_handlers::ws::ConnectionMap;
use crate::templates_structs::{PageContext, UserFormTemplate};

pub async fn new_form(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.create")?;

    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/users")?;
    let roles = role::find_all_display(&conn)?;

    let tmpl = UserFormTemplate {
        ctx,
        form_action: "/users".to_string(),
        form_title: "Create User".to_string(),
        user: None,
        roles,
        errors: vec![],
    };
    render(tmpl)
}

pub async fn create(
    pool: web::Data<DbPool>,
    session: Session,
    form: web::Form<UserForm>,
    conn_map: web::Data<ConnectionMap>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.create")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let conn = pool.get()?;

    // Validate
    let mut errors: Vec<String> = vec![];
    errors.extend(validate::validate_username(&form.username));
    errors.extend(validate::validate_password(&form.password));
    errors.extend(validate::validate_email(&form.email));
    errors.extend(validate::validate_optional(&form.display_name, "Display name", 100));

    let role_id: i64 = match form.role_id.parse() {
        Ok(id) => id,
        Err(_) => {
            errors.push("Invalid role".to_string());
            0
        }
    };

    if !errors.is_empty() {
        let ctx = PageContext::build(&session, &conn, "/users")?;
        let roles = role::find_all_display(&conn)?;
        let tmpl = UserFormTemplate {
            ctx,
            form_action: "/users".to_string(),
            form_title: "Create User".to_string(),
            user: None,
            roles,
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
        role_id,
    };

    match user::create(&conn, &new) {
        Ok(user_id) => {
            // Audit log
            let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
            let details = serde_json::json!({
                "email": new.email,
                "role_id": new.role_id,
                "summary": format!("Created user '{}'", new.username)
            });
            let _ = crate::audit::log(&conn, current_user_id, "user.created",
                                      "user", user_id, details);

            // Generate info warning for admins
            let msg = format!("User '{}' was created", new.username);
            if let Ok(wid) = crate::warnings::create_warning(
                &conn, "info", "governance", "event.user.created", &msg, "", "system"
            ) {
                let admins = crate::warnings::get_users_with_permission(&conn, "admin.settings").unwrap_or_default();
                if !admins.is_empty() {
                    let _ = crate::warnings::create_receipts(&conn, wid, &admins);
                    crate::handlers::warning_handlers::ws::notify_users(
                        &conn_map, &pool, &admins, wid, "info", &msg,
                    );
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
            let ctx = PageContext::build(&session, &conn, "/users")?;
            let roles = role::find_all_display(&conn)?;
            let tmpl = UserFormTemplate {
                ctx,
                form_action: "/users".to_string(),
                form_title: "Create User".to_string(),
                user: None,
                roles,
                errors: vec![msg],
            };
            render(tmpl)
        }
    }
}

pub async fn edit_form(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.edit")?;

    let id = path.into_inner();
    let conn = pool.get()?;

    match user::find_display_by_id(&conn, id) {
        Ok(Some(u)) => {
            let ctx = PageContext::build(&session, &conn, "/users")?;
            let roles = role::find_all_display(&conn)?;
            let tmpl = UserFormTemplate {
                ctx,
                form_action: format!("/users/{id}"),
                form_title: "Edit User".to_string(),
                user: Some(u),
                roles,
                errors: vec![],
            };
            render(tmpl)
        }
        _ => Err(AppError::NotFound),
    }
}

pub async fn update(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<UserForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let id = path.into_inner();
    let conn = pool.get()?;

    // Validate
    let mut errors: Vec<String> = vec![];
    errors.extend(validate::validate_username(&form.username));
    errors.extend(validate::validate_email(&form.email));
    errors.extend(validate::validate_optional(&form.display_name, "Display name", 100));
    // Password is optional on update — only validate if provided
    if !form.password.is_empty() {
        errors.extend(validate::validate_password(&form.password));
    }

    let new_role_id: i64 = match form.role_id.parse() {
        Ok(id) => id,
        Err(_) => {
            errors.push("Invalid role".to_string());
            0
        }
    };

    if !errors.is_empty() {
        let existing = user::find_display_by_id(&conn, id).ok().flatten();
        let ctx = PageContext::build(&session, &conn, "/users")?;
        let roles = role::find_all_display(&conn)?;
        let tmpl = UserFormTemplate {
            ctx,
            form_action: format!("/users/{id}"),
            form_title: "Edit User".to_string(),
            user: existing,
            roles,
            errors,
        };
        return render(tmpl);
    }

    // Last-admin protection: prevent changing the last admin's role away from admin
    if let Ok(Some(existing)) = user::find_display_by_id(&conn, id) {
        if existing.role_name == "admin" && existing.role_id != new_role_id {
            let admin_count = user::count_by_role_id(&conn, existing.role_id).unwrap_or(0);
            if admin_count <= 1 {
                let _ = session.insert("flash", "Cannot change role: this is the last administrator");
                return Ok(HttpResponse::SeeOther()
                    .insert_header(("Location", "/users"))
                    .finish());
            }
        }
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
        &conn,
        id,
        form.username.trim(),
        hashed.as_deref(),
        form.email.trim(),
        form.display_name.trim(),
        new_role_id,
    ) {
        Ok(_) => {
            // Audit log
            let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
            let details = serde_json::json!({
                "username": form.username.trim(),
                "email": form.email.trim(),
                "role_id": new_role_id,
                "password_changed": !form.password.is_empty(),
                "summary": format!("Updated user '{}'", form.username.trim())
            });
            let _ = crate::audit::log(&conn, current_user_id, "user.updated",
                                      "user", id, details);

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
            let existing = user::find_display_by_id(&conn, id).ok().flatten();
            let ctx = PageContext::build(&session, &conn, "/users")?;
            let roles = role::find_all_display(&conn)?;
            let tmpl = UserFormTemplate {
                ctx,
                form_action: format!("/users/{id}"),
                form_title: "Edit User".to_string(),
                user: existing,
                roles,
                errors: vec![msg],
            };
            render(tmpl)
        }
    }
}

pub async fn delete(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<CsrfOnly>,
    conn_map: web::Data<ConnectionMap>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.delete")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let id = path.into_inner();

    // Self-deletion protection
    let current_user_id = get_user_id(&session).unwrap_or(0);
    if id == current_user_id {
        let _ = session.insert("flash", "You cannot delete your own account");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", "/users"))
            .finish());
    }

    let conn = pool.get()?;

    // Last-admin protection
    if let Ok(Some(target)) = user::find_display_by_id(&conn, id) {
        if target.role_name == "admin" {
            let admin_count = user::count_by_role_id(&conn, target.role_id).unwrap_or(0);
            if admin_count <= 1 {
                let _ = session.insert("flash", "Cannot delete the last administrator");
                return Ok(HttpResponse::SeeOther()
                    .insert_header(("Location", "/users"))
                    .finish());
            }
        }
    }

    // Fetch user details before deletion for audit log
    let user_details = user::find_display_by_id(&conn, id).ok().flatten();

    match user::delete(&conn, id) {
        Ok(_) => {
            // Audit log
            if let Some(deleted_user) = user_details {
                let details = serde_json::json!({
                    "username": deleted_user.username,
                    "summary": format!("Deleted user '{}'", deleted_user.username)
                });
                let _ = crate::audit::log(&conn, current_user_id, "user.deleted",
                                          "user", id, details);

                // Generate warning for admins
                let msg = format!("User '{}' was deleted", deleted_user.username);
                if let Ok(wid) = crate::warnings::create_warning(
                    &conn, "medium", "governance", "event.user.deleted", &msg, "", "system"
                ) {
                    let admins = crate::warnings::get_users_with_permission(&conn, "admin.settings").unwrap_or_default();
                    if !admins.is_empty() {
                        let _ = crate::warnings::create_receipts(&conn, wid, &admins);
                        crate::handlers::warning_handlers::ws::notify_users(
                            &conn_map, &pool, &admins, wid, "medium", &msg,
                        );
                    }
                }
            }

            let _ = session.insert("flash", "User deleted");
            Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/users"))
                .finish())
        }
        Err(_) => {
            let _ = session.insert("flash", "Error deleting user");
            Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/users"))
                .finish())
        }
    }
}


/// Bulk delete multiple users
#[derive(serde::Deserialize)]
pub struct BulkDeleteForm {
    pub csrf_token: String,
    pub user_ids: String, // JSON array string: "[1, 2, 3]"
}

pub async fn bulk_delete(
    pool: web::Data<DbPool>,
    session: Session,
    form: web::Form<BulkDeleteForm>,
    conn_map: web::Data<ConnectionMap>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.delete")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let current_user_id = get_user_id(&session).unwrap_or(0);
    
    // Parse the JSON array of user IDs
    let user_ids: Vec<i64> = match serde_json::from_str(&form.user_ids) {
        Ok(ids) => ids,
        Err(_) => {
            let _ = session.insert("flash", "Invalid user IDs");
            return Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/users"))
                .finish());
        }
    };

    if user_ids.is_empty() {
        let _ = session.insert("flash", "No users selected");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", "/users"))
            .finish());
    }

    let conn = pool.get()?;
    let mut deleted_count = 0;
    let mut error_count = 0;

    for id in user_ids {
        // Self-deletion protection
        if id == current_user_id {
            error_count += 1;
            continue;
        }

        // Last-admin protection
        if let Ok(Some(target)) = user::find_display_by_id(&conn, id) {
            if target.role_name == "admin" {
                let admin_count = user::count_by_role_id(&conn, target.role_id).unwrap_or(0);
                if admin_count <= 1 {
                    error_count += 1;
                    continue;
                }
            }
        }

        // Fetch user details before deletion for audit log
        let user_details = user::find_display_by_id(&conn, id).ok().flatten();

        if user::delete(&conn, id).is_ok() {
            deleted_count += 1;

            // Audit log
            if let Some(deleted_user) = user_details {
                let details = serde_json::json!({
                    "username": deleted_user.username,
                    "summary": format!("Deleted user '{}' (bulk delete)", deleted_user.username)
                });
                let _ = crate::audit::log(&conn, current_user_id, "user.deleted",
                                          "user", id, details);

                // Generate warning for admins
                let msg = format!("User '{}' was deleted", deleted_user.username);
                if let Ok(wid) = crate::warnings::create_warning(
                    &conn, "medium", "governance", "event.user.deleted", &msg, "", "system"
                ) {
                    let admins = crate::warnings::get_users_with_permission(&conn, "admin.settings").unwrap_or_default();
                    if !admins.is_empty() {
                        let _ = crate::warnings::create_receipts(&conn, wid, &admins);
                        crate::handlers::warning_handlers::ws::notify_users(
                            &conn_map, &pool, &admins, wid, "medium", &msg,
                        );
                    }
                }
            }
        } else {
            error_count += 1;
        }
    }

    // Build summary message
    let msg = if error_count == 0 {
        format!("Deleted {} user{}", deleted_count, if deleted_count == 1 { "" } else { "s" })
    } else if deleted_count == 0 {
        "Could not delete selected users".to_string()
    } else {
        format!("Deleted {} user{} ({} failed)", deleted_count, if deleted_count == 1 { "" } else { "s" }, error_count)
    };

    let _ = session.insert("flash", &msg);
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/users"))
        .finish())
}
