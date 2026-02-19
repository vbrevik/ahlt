use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::audit;
use crate::auth::csrf;
use crate::auth::session::{require_permission, get_user_id};
use crate::errors::{AppError, render};
use crate::models::{entity, workflow};
use crate::handlers::role_handlers::helpers::{parse_form_body, get_field};
use crate::templates_structs::{PageContext, WorkflowBuilderListTemplate, WorkflowBuilderDetailTemplate};

/// GET /workflow/builder
pub async fn list(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "workflow.manage")?;
    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/workflow/builder")?;
    let scopes = workflow::list_workflow_scopes(&conn)?;
    render(WorkflowBuilderListTemplate { ctx, scopes })
}

/// GET /workflow/builder/{scope}
pub async fn detail(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "workflow.manage")?;
    let scope = path.into_inner();
    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/workflow/builder")?;
    let statuses = workflow::list_statuses_for_scope(&conn, &scope)?;
    let transitions = workflow::list_transitions_for_scope(&conn, &scope)?;
    let permissions = entity::find_by_type(&conn, "permission").map_err(AppError::Db)?;
    render(WorkflowBuilderDetailTemplate { ctx, scope, statuses, transitions, permissions })
}

/// POST /workflow/builder/{scope}/statuses — create a new status
pub async fn create_status(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<String>,
    body: web::Bytes,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "workflow.manage")?;
    let scope = path.into_inner();
    let body_str = String::from_utf8_lossy(&body);
    let params = parse_form_body(&body_str);
    csrf::validate_csrf(&session, get_field(&params, "csrf_token"))?;

    let status_code = get_field(&params, "status_code");
    let label = get_field(&params, "label");
    let order: i64 = get_field(&params, "order").parse().unwrap_or(0);
    let is_initial = get_field(&params, "is_initial") == "true";
    let is_terminal = get_field(&params, "is_terminal") == "true";

    if status_code.is_empty() || label.is_empty() {
        session.insert("flash", "Status code and label are required.".to_string()).ok();
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", format!("/workflow/builder/{}", scope)))
            .finish());
    }

    let conn = pool.get()?;
    let id = workflow::create_status(&conn, &scope, status_code, label, order, is_initial, is_terminal)?;

    let user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "scope": scope, "status_code": status_code, "label": label,
        "summary": format!("Created workflow status '{}'", label)
    });
    let _ = audit::log(&conn, user_id, "workflow_status.create", "workflow_status", id, details);

    session.insert("flash", format!("Status '{}' created.", label)).ok();
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/workflow/builder/{}", scope)))
        .finish())
}

/// POST /workflow/builder/{scope}/statuses/{id}/update
pub async fn update_status(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(String, i64)>,
    body: web::Bytes,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "workflow.manage")?;
    let (scope, status_id) = path.into_inner();
    let body_str = String::from_utf8_lossy(&body);
    let params = parse_form_body(&body_str);
    csrf::validate_csrf(&session, get_field(&params, "csrf_token"))?;

    let label = get_field(&params, "label").to_string();
    let order: i64 = get_field(&params, "order").parse().unwrap_or(0);
    let is_initial = get_field(&params, "is_initial") == "true";
    let is_terminal = get_field(&params, "is_terminal") == "true";

    let conn = pool.get()?;
    workflow::update_status(&conn, status_id, &label, order, is_initial, is_terminal)?;

    let user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "scope": scope, "status_id": status_id, "label": label,
        "summary": format!("Updated workflow status '{}'", label)
    });
    let _ = audit::log(&conn, user_id, "workflow_status.update", "workflow_status", status_id, details);

    session.insert("flash", format!("Status '{}' updated.", label)).ok();
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/workflow/builder/{}", scope)))
        .finish())
}

/// POST /workflow/builder/{scope}/statuses/{id}/delete
pub async fn delete_status(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(String, i64)>,
    body: web::Bytes,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "workflow.manage")?;
    let (scope, status_id) = path.into_inner();
    let body_str = String::from_utf8_lossy(&body);
    let params = parse_form_body(&body_str);
    csrf::validate_csrf(&session, get_field(&params, "csrf_token"))?;

    let conn = pool.get()?;
    let ent = entity::find_by_id(&conn, status_id).map_err(AppError::Db)?.ok_or(AppError::NotFound)?;

    match workflow::delete_status(&conn, status_id) {
        Ok(()) => {
            let user_id = get_user_id(&session).unwrap_or(0);
            let details = serde_json::json!({
                "scope": scope, "status_id": status_id, "label": ent.label,
                "summary": format!("Deleted workflow status '{}'", ent.label)
            });
            let _ = audit::log(&conn, user_id, "workflow_status.delete", "workflow_status", status_id, details);
            session.insert("flash", format!("Status '{}' deleted.", ent.label)).ok();
        }
        Err(e) => {
            session.insert("flash", format!("Cannot delete: {}", e)).ok();
        }
    }

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/workflow/builder/{}", scope)))
        .finish())
}

/// POST /workflow/builder/{scope}/transitions — create a new transition
pub async fn create_transition(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<String>,
    body: web::Bytes,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "workflow.manage")?;
    let scope = path.into_inner();
    let body_str = String::from_utf8_lossy(&body);
    let params = parse_form_body(&body_str);
    csrf::validate_csrf(&session, get_field(&params, "csrf_token"))?;

    let from_status_id: i64 = get_field(&params, "from_status_id").parse().unwrap_or(0);
    let to_status_id: i64 = get_field(&params, "to_status_id").parse().unwrap_or(0);
    let label = get_field(&params, "label");
    let required_permission = get_field(&params, "required_permission");
    let requires_outcome = get_field(&params, "requires_outcome") == "true";
    let condition = get_field(&params, "condition");

    if from_status_id == 0 || to_status_id == 0 || label.is_empty() {
        session.insert("flash", "From status, to status, and label are required.".to_string()).ok();
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", format!("/workflow/builder/{}", scope)))
            .finish());
    }

    let conn = pool.get()?;
    let id = workflow::create_transition(
        &conn, &scope, from_status_id, to_status_id,
        label, required_permission, requires_outcome, condition,
    )?;

    let user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "scope": scope, "label": label,
        "summary": format!("Created workflow transition '{}'", label)
    });
    let _ = audit::log(&conn, user_id, "workflow_transition.create", "workflow_transition", id, details);

    session.insert("flash", format!("Transition '{}' created.", label)).ok();
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/workflow/builder/{}", scope)))
        .finish())
}

/// POST /workflow/builder/{scope}/transitions/{id}/update
pub async fn update_transition(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(String, i64)>,
    body: web::Bytes,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "workflow.manage")?;
    let (scope, transition_id) = path.into_inner();
    let body_str = String::from_utf8_lossy(&body);
    let params = parse_form_body(&body_str);
    csrf::validate_csrf(&session, get_field(&params, "csrf_token"))?;

    let label = get_field(&params, "label").to_string();
    let required_permission = get_field(&params, "required_permission").to_string();
    let requires_outcome = get_field(&params, "requires_outcome") == "true";
    let condition = get_field(&params, "condition").to_string();

    let conn = pool.get()?;
    workflow::update_transition(&conn, transition_id, &label, &required_permission, requires_outcome, &condition)?;

    let user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "scope": scope, "transition_id": transition_id, "label": label,
        "summary": format!("Updated workflow transition '{}'", label)
    });
    let _ = audit::log(&conn, user_id, "workflow_transition.update", "workflow_transition", transition_id, details);

    session.insert("flash", format!("Transition '{}' updated.", label)).ok();
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/workflow/builder/{}", scope)))
        .finish())
}

/// POST /workflow/builder/{scope}/transitions/{id}/delete
pub async fn delete_transition(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(String, i64)>,
    body: web::Bytes,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "workflow.manage")?;
    let (scope, transition_id) = path.into_inner();
    let body_str = String::from_utf8_lossy(&body);
    let params = parse_form_body(&body_str);
    csrf::validate_csrf(&session, get_field(&params, "csrf_token"))?;

    let conn = pool.get()?;
    let ent = entity::find_by_id(&conn, transition_id).map_err(AppError::Db)?.ok_or(AppError::NotFound)?;

    workflow::delete_transition(&conn, transition_id)?;

    let user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "scope": scope, "transition_id": transition_id, "label": ent.label,
        "summary": format!("Deleted workflow transition '{}'", ent.label)
    });
    let _ = audit::log(&conn, user_id, "workflow_transition.delete", "workflow_transition", transition_id, details);

    session.insert("flash", format!("Transition '{}' deleted.", ent.label)).ok();
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/workflow/builder/{}", scope)))
        .finish())
}
