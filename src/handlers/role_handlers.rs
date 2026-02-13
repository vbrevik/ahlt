use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::models::role;
use crate::auth::csrf;
use crate::auth::session::require_permission;
use crate::errors::{AppError, render};
use crate::handlers::auth_handlers::CsrfOnly;
use crate::templates_structs::{PageContext, RoleListTemplate, RoleFormTemplate};

/// Decode a URL-encoded string (form data): `+` → space, `%HH` → byte.
fn url_decode(s: &str) -> String {
    let s = s.replace('+', " ");
    let mut out = Vec::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i+1..i+3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_default()
}

/// Parse URL-encoded form body, supporting duplicate keys (e.g. checkboxes).
fn parse_form_body(body: &str) -> Vec<(String, String)> {
    body.split('&')
        .filter(|s| !s.is_empty())
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            Some((url_decode(k), url_decode(v)))
        })
        .collect()
}

fn get_field<'a>(params: &'a [(String, String)], key: &str) -> &'a str {
    params.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .unwrap_or("")
}

fn get_all<'a>(params: &'a [(String, String)], key: &str) -> Vec<&'a str> {
    params.iter()
        .filter(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .collect()
}

pub async fn list(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.manage")?;

    let conn = pool.get()?;

    let ctx = PageContext::build(&session, &conn, "/roles")?;
    let roles = role::find_all_list_items(&conn)?;

    let tmpl = RoleListTemplate { ctx, roles };
    render(tmpl)
}

pub async fn new_form(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.manage")?;

    let conn = pool.get()?;

    let ctx = PageContext::build(&session, &conn, "/roles")?;
    let permissions = role::find_permission_checkboxes(&conn, 0)?;

    let tmpl = RoleFormTemplate {
        ctx,
        form_action: "/roles".to_string(),
        form_title: "Create Role".to_string(),
        role: None,
        permissions,
        errors: vec![],
    };
    render(tmpl)
}

pub async fn create(
    pool: web::Data<DbPool>,
    session: Session,
    body: String,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.manage")?;

    let params = parse_form_body(&body);
    csrf::validate_csrf(&session, get_field(&params, "csrf_token"))?;

    let conn = pool.get()?;

    let name = get_field(&params, "name");
    let label = get_field(&params, "label");
    let description = get_field(&params, "description");
    let perm_ids: Vec<i64> = get_all(&params, "permissions")
        .iter()
        .filter_map(|s| s.parse().ok())
        .collect();

    // Validate
    let mut errors = vec![];
    if name.trim().is_empty() {
        errors.push("Name is required".to_string());
    }
    if label.trim().is_empty() {
        errors.push("Label is required".to_string());
    }

    if !errors.is_empty() {
        let ctx = PageContext::build(&session, &conn, "/roles")?;
        let permissions = role::find_permission_checkboxes(&conn, 0)?;
        let tmpl = RoleFormTemplate {
            ctx,
            form_action: "/roles".to_string(),
            form_title: "Create Role".to_string(),
            role: None,
            permissions,
            errors,
        };
        return render(tmpl);
    }

    match role::create(&conn, name.trim(), label.trim(), description.trim(), &perm_ids) {
        Ok(role_id) => {
            // Audit log
            let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
            let details = serde_json::json!({
                "role_name": name.trim(),
                "permission_count": perm_ids.len(),
                "summary": format!("Created role '{}'", label.trim())
            });
            let _ = crate::audit::log(&conn, current_user_id, "role.created",
                                      "role", role_id, details);

            let _ = session.insert("flash", "Role created successfully");
            Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/roles"))
                .finish())
        }
        Err(e) => {
            let msg = if e.to_string().contains("UNIQUE") {
                "A role with this name already exists".to_string()
            } else {
                format!("Error creating role: {e}")
            };
            let ctx = PageContext::build(&session, &conn, "/roles")?;
            let permissions = role::find_permission_checkboxes(&conn, 0)?;
            let tmpl = RoleFormTemplate {
                ctx,
                form_action: "/roles".to_string(),
                form_title: "Create Role".to_string(),
                role: None,
                permissions,
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
    require_permission(&session, "roles.manage")?;

    let id = path.into_inner();

    let conn = pool.get()?;

    match role::find_detail_by_id(&conn, id)? {
        Some(r) => {
            let ctx = PageContext::build(&session, &conn, "/roles")?;
            let permissions = role::find_permission_checkboxes(&conn, id)?;
            let tmpl = RoleFormTemplate {
                ctx,
                form_action: format!("/roles/{id}"),
                form_title: "Edit Role".to_string(),
                role: Some(r),
                permissions,
                errors: vec![],
            };
            render(tmpl)
        }
        None => Ok(HttpResponse::NotFound().body("Role not found")),
    }
}

pub async fn update(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    body: String,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.manage")?;

    let params = parse_form_body(&body);
    csrf::validate_csrf(&session, get_field(&params, "csrf_token"))?;

    let id = path.into_inner();

    let conn = pool.get()?;

    let name = get_field(&params, "name");
    let label = get_field(&params, "label");
    let description = get_field(&params, "description");
    let perm_ids: Vec<i64> = get_all(&params, "permissions")
        .iter()
        .filter_map(|s| s.parse().ok())
        .collect();

    // Validate
    let mut errors = vec![];
    if name.trim().is_empty() {
        errors.push("Name is required".to_string());
    }
    if label.trim().is_empty() {
        errors.push("Label is required".to_string());
    }

    if !errors.is_empty() {
        let existing = role::find_detail_by_id(&conn, id).ok().flatten();
        let ctx = PageContext::build(&session, &conn, "/roles")?;
        let permissions = role::find_permission_checkboxes(&conn, id)?;
        let tmpl = RoleFormTemplate {
            ctx,
            form_action: format!("/roles/{id}"),
            form_title: "Edit Role".to_string(),
            role: existing,
            permissions,
            errors,
        };
        return render(tmpl);
    }

    match role::update(&conn, id, name.trim(), label.trim(), description.trim(), &perm_ids) {
        Ok(_) => {
            // Audit log for permission changes
            let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
            let details = serde_json::json!({
                "role_name": name.trim(),
                "new_permission_count": perm_ids.len(),
                "summary": format!("Updated permissions for role '{}'", label.trim())
            });
            let _ = crate::audit::log(&conn, current_user_id, "role.permissions_changed",
                                      "role", id, details);

            let _ = session.insert("flash", "Role updated successfully");
            Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/roles"))
                .finish())
        }
        Err(e) => {
            let msg = if e.to_string().contains("UNIQUE") {
                "A role with this name already exists".to_string()
            } else {
                format!("Error updating role: {e}")
            };
            let existing = role::find_detail_by_id(&conn, id).ok().flatten();
            let ctx = PageContext::build(&session, &conn, "/roles")?;
            let permissions = role::find_permission_checkboxes(&conn, id)?;
            let tmpl = RoleFormTemplate {
                ctx,
                form_action: format!("/roles/{id}"),
                form_title: "Edit Role".to_string(),
                role: existing,
                permissions,
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
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.manage")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let id = path.into_inner();

    let conn = pool.get()?;

    // Prevent deleting a role that has users assigned
    let user_count = role::count_users(&conn, id)?;
    if user_count > 0 {
        let _ = session.insert("flash", format!("Cannot delete role: {user_count} user(s) still assigned"));
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", "/roles"))
            .finish());
    }

    // Fetch role details before deletion for audit log
    let role_details = role::find_detail_by_id(&conn, id).ok().flatten();

    match role::delete(&conn, id) {
        Ok(_) => {
            // Audit log
            let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
            if let Some(deleted_role) = role_details {
                let details = serde_json::json!({
                    "role_name": deleted_role.name,
                    "summary": format!("Deleted role '{}'", deleted_role.label)
                });
                let _ = crate::audit::log(&conn, current_user_id, "role.deleted",
                                          "role", id, details);
            }

            let _ = session.insert("flash", "Role deleted");
            Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/roles"))
                .finish())
        }
        Err(_) => {
            let _ = session.insert("flash", "Error deleting role");
            Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/roles"))
                .finish())
        }
    }
}
