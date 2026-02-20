use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::models::tor;
use crate::models::protocol;
use crate::models::meeting;
use crate::auth::{csrf, validate};
use crate::auth::session::require_permission;
use crate::errors::{AppError, render};
use crate::handlers::auth_handlers::CsrfOnly;
use crate::templates_structs::{PageContext, TorFormTemplate, TorDetailTemplate, UserOption};

/// Convert newline-separated textarea text into a JSON array string.
/// Filters empty lines. Returns "[]" if no items.
fn lines_to_json(text: &str) -> String {
    let items: Vec<&str> = text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string())
}

pub async fn new_form(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.create")?;

    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/tor")?;

    let tmpl = TorFormTemplate {
        ctx,
        form_action: "/tor".to_string(),
        form_title: "Create Terms of Reference".to_string(),
        tor: None,
        errors: vec![],
    };
    render(tmpl)
}

pub async fn create(
    pool: web::Data<DbPool>,
    session: Session,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.create")?;
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    let conn = pool.get()?;

    let name = form.get("name").map(|s| s.as_str()).unwrap_or("");
    let label = form.get("label").map(|s| s.as_str()).unwrap_or("");
    let description = form.get("description").map(|s| s.as_str()).unwrap_or("");
    let status = form.get("status").map(|s| s.as_str()).unwrap_or("active");
    let meeting_cadence = form.get("meeting_cadence").map(|s| s.as_str()).unwrap_or("ad-hoc");
    let cadence_day = form.get("cadence_day").map(|s| s.as_str()).unwrap_or("");
    let cadence_time = form.get("cadence_time").map(|s| s.as_str()).unwrap_or("");
    let cadence_duration = form.get("cadence_duration_minutes").map(|s| s.as_str()).unwrap_or("60");
    let default_location = form.get("default_location").map(|s| s.as_str()).unwrap_or("");
    let remote_url = form.get("remote_url").map(|s| s.as_str()).unwrap_or("");
    let background_repo_url = form.get("background_repo_url").map(|s| s.as_str()).unwrap_or("");
    let tor_number = form.get("tor_number").map(|s| s.as_str()).unwrap_or("");
    let classification = form.get("classification").map(|s| s.as_str()).unwrap_or("");
    let version = form.get("version").map(|s| s.as_str()).unwrap_or("");
    let organization = form.get("organization").map(|s| s.as_str()).unwrap_or("");
    let focus_scope = form.get("focus_scope").map(|s| s.as_str()).unwrap_or("");
    let poc_contact = form.get("poc_contact").map(|s| s.as_str()).unwrap_or("");
    let phase_scheduling = form.get("phase_scheduling").map(|s| s.as_str()).unwrap_or("");
    let info_platform = form.get("info_platform").map(|s| s.as_str()).unwrap_or("");
    let invite_policy = form.get("invite_policy").map(|s| s.as_str()).unwrap_or("");
    let objectives_json = lines_to_json(form.get("objectives").map(|s| s.as_str()).unwrap_or(""));
    let inputs_json = lines_to_json(form.get("inputs_required").map(|s| s.as_str()).unwrap_or(""));
    let outputs_json = lines_to_json(form.get("outputs_expected").map(|s| s.as_str()).unwrap_or(""));

    // Validate
    let mut errors: Vec<String> = vec![];
    errors.extend(validate::validate_required(name, "Name", 50));
    errors.extend(validate::validate_required(label, "Label", 100));
    errors.extend(validate::validate_optional(description, "Description", 500));

    if !errors.is_empty() {
        let ctx = PageContext::build(&session, &conn, "/tor")?;
        let tmpl = TorFormTemplate {
            ctx,
            form_action: "/tor".to_string(),
            form_title: "Create Terms of Reference".to_string(),
            tor: None,
            errors,
        };
        return render(tmpl);
    }

    let props: Vec<(&str, &str)> = vec![
        ("description", description.trim()),
        ("status", status),
        ("meeting_cadence", meeting_cadence),
        ("cadence_day", cadence_day),
        ("cadence_time", cadence_time),
        ("cadence_duration_minutes", cadence_duration),
        ("default_location", default_location),
        ("remote_url", remote_url),
        ("background_repo_url", background_repo_url),
        ("tor_number", tor_number),
        ("classification", classification),
        ("version", version),
        ("organization", organization),
        ("focus_scope", focus_scope),
        ("objectives", &objectives_json),
        ("inputs_required", &inputs_json),
        ("outputs_expected", &outputs_json),
        ("poc_contact", poc_contact),
        ("phase_scheduling", phase_scheduling),
        ("info_platform", info_platform),
        ("invite_policy", invite_policy),
    ];

    match tor::create(&conn, name.trim(), label.trim(), &props) {
        Ok(tor_id) => {
            let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
            let details = serde_json::json!({
                "tor_name": name.trim(),
                "summary": format!("Created Terms of Reference '{}'", label.trim())
            });
            let _ = crate::audit::log(&conn, current_user_id, "tor.created", "tor", tor_id, details);

            let _ = session.insert("flash", "Terms of Reference created successfully");
            Ok(HttpResponse::SeeOther()
                .insert_header(("Location", format!("/tor/{tor_id}")))
                .finish())
        }
        Err(e) => {
            let msg = if e.to_string().contains("UNIQUE") {
                "A ToR with this name already exists".to_string()
            } else {
                format!("Error creating ToR: {e}")
            };
            let ctx = PageContext::build(&session, &conn, "/tor")?;
            let tmpl = TorFormTemplate {
                ctx,
                form_action: "/tor".to_string(),
                form_title: "Create Terms of Reference".to_string(),
                tor: None,
                errors: vec![msg],
            };
            render(tmpl)
        }
    }
}

pub async fn detail(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.list")?;

    let id = path.into_inner();
    let conn = pool.get()?;

    match tor::find_detail_by_id(&conn, id)? {
        Some(tor_detail) => {
            let ctx = PageContext::build(&session, &conn, "/tor")?;
            let members = tor::find_members(&conn, id)?;
            let functions = tor::find_functions(&conn, id)?;
            let protocol_steps = protocol::find_steps_for_tor(&conn, id)?;
            let non_members = tor::find_non_members(&conn, id)?;
            let available_users = non_members.into_iter()
                .map(|(id, name, label)| UserOption { id, name, label })
                .collect();
            let upstream_deps = tor::find_upstream(&conn, id)?;
            let downstream_deps = tor::find_downstream(&conn, id)?;
            let other_tors = tor::find_other_tors(&conn, id)?;
            let meetings = meeting::find_by_tor(&conn, id)?;

            let tmpl = TorDetailTemplate {
                ctx,
                tor: tor_detail,
                members,
                functions,
                protocol_steps,
                available_users,
                upstream_deps,
                downstream_deps,
                other_tors,
                meetings,
            };
            render(tmpl)
        }
        None => Err(AppError::NotFound),
    }
}

pub async fn edit_form(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;

    let id = path.into_inner();
    let conn = pool.get()?;

    match tor::find_detail_by_id(&conn, id)? {
        Some(t) => {
            let ctx = PageContext::build(&session, &conn, "/tor")?;
            let tmpl = TorFormTemplate {
                ctx,
                form_action: format!("/tor/{id}"),
                form_title: "Edit Terms of Reference".to_string(),
                tor: Some(t),
                errors: vec![],
            };
            render(tmpl)
        }
        None => Err(AppError::NotFound),
    }
}

pub async fn update(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    let id = path.into_inner();
    let conn = pool.get()?;

    let name = form.get("name").map(|s| s.as_str()).unwrap_or("");
    let label = form.get("label").map(|s| s.as_str()).unwrap_or("");
    let description = form.get("description").map(|s| s.as_str()).unwrap_or("");
    let status = form.get("status").map(|s| s.as_str()).unwrap_or("active");
    let meeting_cadence = form.get("meeting_cadence").map(|s| s.as_str()).unwrap_or("ad-hoc");
    let cadence_day = form.get("cadence_day").map(|s| s.as_str()).unwrap_or("");
    let cadence_time = form.get("cadence_time").map(|s| s.as_str()).unwrap_or("");
    let cadence_duration = form.get("cadence_duration_minutes").map(|s| s.as_str()).unwrap_or("60");
    let default_location = form.get("default_location").map(|s| s.as_str()).unwrap_or("");
    let remote_url = form.get("remote_url").map(|s| s.as_str()).unwrap_or("");
    let background_repo_url = form.get("background_repo_url").map(|s| s.as_str()).unwrap_or("");
    let tor_number = form.get("tor_number").map(|s| s.as_str()).unwrap_or("");
    let classification = form.get("classification").map(|s| s.as_str()).unwrap_or("");
    let version = form.get("version").map(|s| s.as_str()).unwrap_or("");
    let organization = form.get("organization").map(|s| s.as_str()).unwrap_or("");
    let focus_scope = form.get("focus_scope").map(|s| s.as_str()).unwrap_or("");
    let poc_contact = form.get("poc_contact").map(|s| s.as_str()).unwrap_or("");
    let phase_scheduling = form.get("phase_scheduling").map(|s| s.as_str()).unwrap_or("");
    let info_platform = form.get("info_platform").map(|s| s.as_str()).unwrap_or("");
    let invite_policy = form.get("invite_policy").map(|s| s.as_str()).unwrap_or("");
    let objectives_json = lines_to_json(form.get("objectives").map(|s| s.as_str()).unwrap_or(""));
    let inputs_json = lines_to_json(form.get("inputs_required").map(|s| s.as_str()).unwrap_or(""));
    let outputs_json = lines_to_json(form.get("outputs_expected").map(|s| s.as_str()).unwrap_or(""));

    // Validate
    let mut errors: Vec<String> = vec![];
    errors.extend(validate::validate_required(name, "Name", 50));
    errors.extend(validate::validate_required(label, "Label", 100));
    errors.extend(validate::validate_optional(description, "Description", 500));

    if !errors.is_empty() {
        let existing = tor::find_detail_by_id(&conn, id).ok().flatten();
        let ctx = PageContext::build(&session, &conn, "/tor")?;
        let tmpl = TorFormTemplate {
            ctx,
            form_action: format!("/tor/{id}"),
            form_title: "Edit Terms of Reference".to_string(),
            tor: existing,
            errors,
        };
        return render(tmpl);
    }

    let props: Vec<(&str, &str)> = vec![
        ("description", description.trim()),
        ("status", status),
        ("meeting_cadence", meeting_cadence),
        ("cadence_day", cadence_day),
        ("cadence_time", cadence_time),
        ("cadence_duration_minutes", cadence_duration),
        ("default_location", default_location),
        ("remote_url", remote_url),
        ("background_repo_url", background_repo_url),
        ("tor_number", tor_number),
        ("classification", classification),
        ("version", version),
        ("organization", organization),
        ("focus_scope", focus_scope),
        ("objectives", &objectives_json),
        ("inputs_required", &inputs_json),
        ("outputs_expected", &outputs_json),
        ("poc_contact", poc_contact),
        ("phase_scheduling", phase_scheduling),
        ("info_platform", info_platform),
        ("invite_policy", invite_policy),
    ];

    match tor::update(&conn, id, name.trim(), label.trim(), &props) {
        Ok(_) => {
            let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
            let details = serde_json::json!({
                "tor_name": name.trim(),
                "summary": format!("Updated Terms of Reference '{}'", label.trim())
            });
            let _ = crate::audit::log(&conn, current_user_id, "tor.updated", "tor", id, details);

            let _ = session.insert("flash", "Terms of Reference updated successfully");
            Ok(HttpResponse::SeeOther()
                .insert_header(("Location", format!("/tor/{id}")))
                .finish())
        }
        Err(e) => {
            let msg = if e.to_string().contains("UNIQUE") {
                "A ToR with this name already exists".to_string()
            } else {
                format!("Error updating ToR: {e}")
            };
            let existing = tor::find_detail_by_id(&conn, id).ok().flatten();
            let ctx = PageContext::build(&session, &conn, "/tor")?;
            let tmpl = TorFormTemplate {
                ctx,
                form_action: format!("/tor/{id}"),
                form_title: "Edit Terms of Reference".to_string(),
                tor: existing,
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
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let id = path.into_inner();
    let conn = pool.get()?;

    // Prevent deleting a ToR that has members
    let member_count = tor::count_members(&conn, id)?;
    if member_count > 0 {
        let _ = session.insert("flash", format!("Cannot delete ToR: {member_count} member(s) still assigned"));
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", "/tor"))
            .finish());
    }

    let tor_details = tor::find_detail_by_id(&conn, id).ok().flatten();

    match tor::delete(&conn, id) {
        Ok(_) => {
            let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
            if let Some(deleted) = tor_details {
                let details = serde_json::json!({
                    "tor_name": deleted.name,
                    "summary": format!("Deleted Terms of Reference '{}'", deleted.label)
                });
                let _ = crate::audit::log(&conn, current_user_id, "tor.deleted", "tor", id, details);
            }

            let _ = session.insert("flash", "Terms of Reference deleted");
            Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/tor"))
                .finish())
        }
        Err(_) => {
            let _ = session.insert("flash", "Error deleting Terms of Reference");
            Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/tor"))
                .finish())
        }
    }
}
