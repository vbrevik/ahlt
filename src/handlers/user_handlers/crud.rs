use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::models::user::{self, UserForm};
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

    if !errors.is_empty() {
        let ctx = PageContext::build(&session, &conn, "/users")?;
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

    match user::create(&conn, &new) {
        Ok(user_id) => {
            // Assign default viewer role
            let _ = user::assign_default_role(&conn, user_id);

            // Audit log
            let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
            let details = serde_json::json!({
                "email": new.email,
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
            let tmpl = UserFormTemplate {
                ctx,
                form_action: format!("/users/{id}"),
                form_title: "Edit User".to_string(),
                user: Some(u),
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

    if !errors.is_empty() {
        let existing = user::find_display_by_id(&conn, id).ok().flatten();
        let ctx = PageContext::build(&session, &conn, "/users")?;
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
        &conn,
        id,
        form.username.trim(),
        hashed.as_deref(),
        form.email.trim(),
        form.display_name.trim(),
    ) {
        Ok(_) => {
            // Audit log
            let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
            let details = serde_json::json!({
                "username": form.username.trim(),
                "email": form.email.trim(),
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

#[derive(serde::Deserialize)]
pub struct ExportQuery {
    filter: Option<String>,
    sort: Option<String>,
    dir: Option<String>,
}

pub async fn export_csv(
    pool: web::Data<DbPool>,
    session: Session,
    query: web::Query<ExportQuery>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.list")?;

    let conn = pool.get()?;

    let filter = query.filter.as_deref()
        .and_then(|s| crate::models::table_filter::FilterTree::from_json(s).ok())
        .unwrap_or_default();
    let sort = crate::models::table_filter::SortSpec::from_params(
        query.sort.as_deref(), query.dir.as_deref()
    );

    let users = crate::models::user::find_all_filtered(&conn, &filter, &sort)?;

    // Audit log
    let uid = crate::auth::session::get_user_id(&session).unwrap_or(0);
    let _ = crate::audit::log(&conn, uid, "users.export", "user", 0,
        serde_json::json!({ "count": users.len(), "format": "csv" }));

    // Get today's date for filename
    let today: String = conn.query_row("SELECT DATE('now')", [], |r| r.get(0))
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
            escape_csv(&u.role_label),
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

#[derive(serde::Deserialize)]
pub struct SaveColumnsForm {
    pub columns: String,
    pub set_global: Option<String>,
    pub csrf_token: String,
    pub redirect_to: Option<String>,
}

pub async fn save_columns(
    pool: web::Data<DbPool>,
    session: Session,
    form: web::Form<SaveColumnsForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.list")?;
    crate::auth::csrf::validate_csrf(&session, &form.csrf_token)?;

    let conn = pool.get()?;
    let user_id = crate::auth::session::get_user_id(&session)
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

    crate::models::table_filter::columns::save_user_columns(user_id, "users", &pref, &conn)?;

    // Optionally save global default
    if form.set_global.as_deref() == Some("true") {
        require_permission(&session, "settings.manage")?;
        crate::models::table_filter::columns::save_global_columns("users", &pref, &conn)?;
    }

    let redirect = form.redirect_to.as_deref().unwrap_or("/users");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", redirect.to_string()))
        .finish())
}
