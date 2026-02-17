use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::models::role;
use crate::auth::{csrf, validate};
use crate::auth::session::require_permission;
use crate::errors::{AppError, render};
use crate::handlers::auth_handlers::CsrfOnly;
use crate::templates_structs::{PageContext, RoleFormTemplate};

use super::helpers::*;

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
    let mut errors: Vec<String> = vec![];
    errors.extend(validate::validate_username(name));
    errors.extend(validate::validate_required(label, "Label", 100));
    errors.extend(validate::validate_optional(description, "Description", 500));

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

    let user_count = role::count_users(&conn, id)?;
    if user_count > 0 {
        let _ = session.insert("flash", format!("Cannot delete role: {user_count} user(s) still assigned"));
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", "/roles"))
            .finish());
    }

    let role_details = role::find_detail_by_id(&conn, id).ok().flatten();

    match role::delete(&conn, id) {
        Ok(_) => {
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
