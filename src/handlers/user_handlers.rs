use actix_session::Session;
use actix_web::{web, HttpResponse, Responder};
use askama::Template;

use crate::db::DbPool;
use crate::models::user::{self, UserForm};
use crate::models::role;
use crate::auth::password;
use crate::auth::session::{get_user_id, require_permission};
use crate::templates_structs::{PageContext, UserListTemplate, UserFormTemplate};

pub async fn list(
    pool: web::Data<DbPool>,
    session: Session,
) -> impl Responder {
    if let Err(resp) = require_permission(&session, "users.list") {
        return resp;
    }

    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };

    let ctx = PageContext::build(&session, &conn, "/users");
    let users = user::find_all_display(&conn).unwrap_or_default();

    let tmpl = UserListTemplate { ctx, users };
    match tmpl.render() {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(_) => HttpResponse::InternalServerError().body("Template error"),
    }
}

pub async fn new_form(
    pool: web::Data<DbPool>,
    session: Session,
) -> impl Responder {
    if let Err(resp) = require_permission(&session, "users.create") {
        return resp;
    }

    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };

    let ctx = PageContext::build(&session, &conn, "/users");
    let roles = role::find_all_display(&conn).unwrap_or_default();

    let tmpl = UserFormTemplate {
        ctx,
        form_action: "/users".to_string(),
        form_title: "Create User".to_string(),
        user: None,
        roles,
        errors: vec![],
    };
    match tmpl.render() {
        Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
        Err(_) => HttpResponse::InternalServerError().body("Template error"),
    }
}

pub async fn create(
    pool: web::Data<DbPool>,
    session: Session,
    form: web::Form<UserForm>,
) -> impl Responder {
    if let Err(resp) = require_permission(&session, "users.create") {
        return resp;
    }

    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };

    // Validate
    let mut errors = vec![];
    if form.username.trim().is_empty() {
        errors.push("Username is required".to_string());
    }
    if form.password.is_empty() {
        errors.push("Password is required".to_string());
    }
    if form.email.trim().is_empty() {
        errors.push("Email is required".to_string());
    }

    let role_id: i64 = match form.role_id.parse() {
        Ok(id) => id,
        Err(_) => {
            errors.push("Invalid role".to_string());
            0
        }
    };

    if !errors.is_empty() {
        let ctx = PageContext::build(&session, &conn, "/users");
        let roles = role::find_all_display(&conn).unwrap_or_default();
        let tmpl = UserFormTemplate {
            ctx,
            form_action: "/users".to_string(),
            form_title: "Create User".to_string(),
            user: None,
            roles,
            errors,
        };
        return match tmpl.render() {
            Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
            Err(_) => HttpResponse::InternalServerError().body("Template error"),
        };
    }

    let hashed = match password::hash_password(&form.password) {
        Ok(h) => h,
        Err(_) => return HttpResponse::InternalServerError().body("Password hash error"),
    };

    let new = user::NewUser {
        username: form.username.trim().to_string(),
        password: hashed,
        email: form.email.trim().to_string(),
        display_name: form.display_name.trim().to_string(),
        role_id,
    };

    match user::create(&conn, &new) {
        Ok(_) => {
            let _ = session.insert("flash", "User created successfully");
            HttpResponse::SeeOther()
                .insert_header(("Location", "/users"))
                .finish()
        }
        Err(e) => {
            let msg = if e.to_string().contains("UNIQUE") {
                "Username already exists".to_string()
            } else {
                format!("Error creating user: {e}")
            };
            let ctx = PageContext::build(&session, &conn, "/users");
            let roles = role::find_all_display(&conn).unwrap_or_default();
            let tmpl = UserFormTemplate {
                ctx,
                form_action: "/users".to_string(),
                form_title: "Create User".to_string(),
                user: None,
                roles,
                errors: vec![msg],
            };
            match tmpl.render() {
                Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
                Err(_) => HttpResponse::InternalServerError().body("Template error"),
            }
        }
    }
}

pub async fn edit_form(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> impl Responder {
    if let Err(resp) = require_permission(&session, "users.edit") {
        return resp;
    }

    let id = path.into_inner();

    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };

    match user::find_display_by_id(&conn, id) {
        Ok(Some(u)) => {
            let ctx = PageContext::build(&session, &conn, "/users");
            let roles = role::find_all_display(&conn).unwrap_or_default();
            let tmpl = UserFormTemplate {
                ctx,
                form_action: format!("/users/{id}"),
                form_title: "Edit User".to_string(),
                user: Some(u),
                roles,
                errors: vec![],
            };
            match tmpl.render() {
                Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
                Err(_) => HttpResponse::InternalServerError().body("Template error"),
            }
        }
        _ => HttpResponse::NotFound().body("User not found"),
    }
}

pub async fn update(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<UserForm>,
) -> impl Responder {
    if let Err(resp) = require_permission(&session, "users.edit") {
        return resp;
    }

    let id = path.into_inner();

    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };

    let new_role_id: i64 = form.role_id.parse().unwrap_or(0);

    // Last-admin protection: prevent changing the last admin's role away from admin
    if let Ok(Some(existing)) = user::find_display_by_id(&conn, id) {
        if existing.role_name == "admin" && existing.role_id != new_role_id {
            let admin_count = user::count_by_role_id(&conn, existing.role_id).unwrap_or(0);
            if admin_count <= 1 {
                let _ = session.insert("flash", "Cannot change role: this is the last administrator");
                return HttpResponse::SeeOther()
                    .insert_header(("Location", "/users"))
                    .finish();
            }
        }
    }

    // Hash password if provided
    let hashed = if form.password.is_empty() {
        None
    } else {
        match password::hash_password(&form.password) {
            Ok(h) => Some(h),
            Err(_) => return HttpResponse::InternalServerError().body("Password hash error"),
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
            let _ = session.insert("flash", "User updated successfully");
            HttpResponse::SeeOther()
                .insert_header(("Location", "/users"))
                .finish()
        }
        Err(e) => {
            let msg = if e.to_string().contains("UNIQUE") {
                "Username already exists".to_string()
            } else {
                format!("Error updating user: {e}")
            };
            let existing = user::find_display_by_id(&conn, id).ok().flatten();
            let ctx = PageContext::build(&session, &conn, "/users");
            let roles = role::find_all_display(&conn).unwrap_or_default();
            let tmpl = UserFormTemplate {
                ctx,
                form_action: format!("/users/{id}"),
                form_title: "Edit User".to_string(),
                user: existing,
                roles,
                errors: vec![msg],
            };
            match tmpl.render() {
                Ok(body) => HttpResponse::Ok().content_type("text/html").body(body),
                Err(_) => HttpResponse::InternalServerError().body("Template error"),
            }
        }
    }
}

pub async fn delete(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> impl Responder {
    if let Err(resp) = require_permission(&session, "users.delete") {
        return resp;
    }

    let id = path.into_inner();

    // Self-deletion protection
    let current_user_id = get_user_id(&session).unwrap_or(0);
    if id == current_user_id {
        let _ = session.insert("flash", "You cannot delete your own account");
        return HttpResponse::SeeOther()
            .insert_header(("Location", "/users"))
            .finish();
    }

    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Database error"),
    };

    // Last-admin protection
    if let Ok(Some(target)) = user::find_display_by_id(&conn, id) {
        if target.role_name == "admin" {
            let admin_count = user::count_by_role_id(&conn, target.role_id).unwrap_or(0);
            if admin_count <= 1 {
                let _ = session.insert("flash", "Cannot delete the last administrator");
                return HttpResponse::SeeOther()
                    .insert_header(("Location", "/users"))
                    .finish();
            }
        }
    }

    match user::delete(&conn, id) {
        Ok(_) => {
            let _ = session.insert("flash", "User deleted");
            HttpResponse::SeeOther()
                .insert_header(("Location", "/users"))
                .finish()
        }
        Err(_) => {
            let _ = session.insert("flash", "Error deleting user");
            HttpResponse::SeeOther()
                .insert_header(("Location", "/users"))
                .finish()
        }
    }
}
