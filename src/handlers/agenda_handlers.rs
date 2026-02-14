use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::auth::csrf;
use crate::auth::session::{require_permission, get_user_id};
use crate::errors::{AppError, render};
use crate::models::{tor, agenda_point, coa, opinion, workflow};
use crate::models::agenda_point::AgendaPointForm;
use crate::templates_structs::{PageContext, AgendaPointFormTemplate, AgendaPointDetailTemplate};

// ---------------------------------------------------------------------------
// CRUD handlers (Task 13)
// ---------------------------------------------------------------------------

/// GET /tor/{id}/workflow/agenda/new
/// Renders the agenda point creation form.
pub async fn new_form(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "agenda.create")?;

    let tor_id = path.into_inner();
    let conn = pool.get()?;
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&conn, user_id, tor_id)?;

    let tor_name = tor::get_tor_name(&conn, tor_id)?;
    let ctx = PageContext::build(&session, &conn, "/workflow")?;

    let tmpl = AgendaPointFormTemplate {
        ctx,
        tor_id,
        tor_name,
        form_action: format!("/tor/{tor_id}/workflow/agenda"),
        form_title: "New Agenda Point".to_string(),
        agenda_point: None,
        errors: vec![],
    };
    render(tmpl)
}

/// POST /tor/{id}/workflow/agenda
/// Creates a new agenda point linked to the ToR.
pub async fn create(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<AgendaPointForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "agenda.create")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let tor_id = path.into_inner();
    let conn = pool.get()?;
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&conn, user_id, tor_id)?;

    // Validate
    let title = form.title.trim();
    let description = form.description.trim();
    let item_type = form.item_type.trim();
    let scheduled_date = form.scheduled_date.trim();
    let mut errors = vec![];

    if title.is_empty() {
        errors.push("Title is required".to_string());
    }
    if description.is_empty() {
        errors.push("Description is required".to_string());
    }
    if item_type.is_empty() {
        errors.push("Item type is required".to_string());
    }
    if scheduled_date.is_empty() {
        errors.push("Scheduled date is required".to_string());
    }

    // Parse time allocation
    let time_allocation_minutes: i32 = form.time_allocation_minutes.trim().parse().unwrap_or(0);
    if time_allocation_minutes < 0 {
        errors.push("Time allocation must be a non-negative number".to_string());
    }

    if !errors.is_empty() {
        let tor_name = tor::get_tor_name(&conn, tor_id)?;
        let ctx = PageContext::build(&session, &conn, "/workflow")?;
        let tmpl = AgendaPointFormTemplate {
            ctx,
            tor_id,
            tor_name,
            form_action: format!("/tor/{tor_id}/workflow/agenda"),
            form_title: "New Agenda Point".to_string(),
            agenda_point: None,
            errors,
        };
        return render(tmpl);
    }

    let agenda_point_id = agenda_point::create(
        &conn, tor_id, title, description, item_type, scheduled_date, time_allocation_minutes, user_id,
    )?;

    // Audit log
    let details = serde_json::json!({
        "tor_id": tor_id,
        "title": title,
        "summary": format!("Created agenda point '{}'", title)
    });
    let _ = crate::audit::log(&conn, user_id, "agenda_point.created", "agenda_point", agenda_point_id, details);

    let _ = session.insert("flash", "Agenda point created successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/workflow/agenda/{agenda_point_id}")))
        .finish())
}

/// GET /tor/{id}/workflow/agenda/{agenda_id}
/// Renders the agenda point detail page.
pub async fn detail(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "agenda.view")?;

    let (tor_id, agenda_point_id) = path.into_inner();
    let conn = pool.get()?;
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&conn, user_id, tor_id)?;

    match agenda_point::find_by_id(&conn, agenda_point_id)? {
        Some(ap) => {
            let tor_name = tor::get_tor_name(&conn, tor_id)?;
            let ctx = PageContext::build(&session, &conn, "/workflow")?;

            // Fetch related COAs
            let mut coas = vec![];
            for coa_id in &ap.coa_ids {
                if let Ok(coa_detail) = coa::find_by_id(&conn, *coa_id) {
                    coas.push(coa_detail);
                }
            }

            // Fetch opinions
            let opinions_list = opinion::find_opinions_for_agenda_point(&conn, agenda_point_id)?;

            // Build opinions summary grouped by COA
            let mut opinions: Vec<crate::models::opinion::OpinionSummary> = vec![];
            for coa_detail in &coas {
                let coa_opinions: Vec<_> = opinions_list.iter()
                    .filter(|op| op.preferred_coa_id == coa_detail.id)
                    .cloned()
                    .collect();

                opinions.push(crate::models::opinion::OpinionSummary {
                    coa_id: coa_detail.id,
                    coa_title: coa_detail.title.clone(),
                    preference_count: coa_opinions.len() as i32,
                    opinions: coa_opinions,
                });
            }

            // Get user permissions for workflow transitions
            let permissions = crate::auth::session::get_permissions(&session)
                .map_err(|e| AppError::Session(e))?;

            // Get agenda point entity properties for transition validation
            let mut entity_properties = std::collections::HashMap::new();
            entity_properties.insert("status".to_string(), ap.status.clone());
            entity_properties.insert("item_type".to_string(), ap.item_type.clone());

            // Get available transitions
            let available_transitions = workflow::find_available_transitions(
                &conn,
                "agenda_point",
                &ap.status,
                &permissions,
                &entity_properties,
            )?;

            let tmpl = AgendaPointDetailTemplate {
                ctx,
                tor_id,
                tor_name,
                agenda_point: ap,
                coas,
                opinions,
                available_transitions,
            };
            render(tmpl)
        }
        None => Err(AppError::NotFound),
    }
}

/// GET /tor/{id}/workflow/agenda/{agenda_id}/edit
/// Renders the agenda point edit form.
pub async fn edit_form(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "agenda.manage")?;

    let (tor_id, agenda_point_id) = path.into_inner();
    let conn = pool.get()?;
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&conn, user_id, tor_id)?;

    match agenda_point::find_by_id(&conn, agenda_point_id)? {
        Some(ap) => {
            let tor_name = tor::get_tor_name(&conn, tor_id)?;
            let ctx = PageContext::build(&session, &conn, "/workflow")?;
            let tmpl = AgendaPointFormTemplate {
                ctx,
                tor_id,
                tor_name,
                form_action: format!("/tor/{tor_id}/workflow/agenda/{agenda_point_id}"),
                form_title: "Edit Agenda Point".to_string(),
                agenda_point: Some(ap),
                errors: vec![],
            };
            render(tmpl)
        }
        None => Err(AppError::NotFound),
    }
}

/// POST /tor/{id}/workflow/agenda/{agenda_id}
/// Updates an existing agenda point's title, description, item_type, scheduled_date, and time allocation.
pub async fn update(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<AgendaPointForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "agenda.manage")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, agenda_point_id) = path.into_inner();
    let conn = pool.get()?;
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&conn, user_id, tor_id)?;

    // Validate
    let title = form.title.trim();
    let description = form.description.trim();
    let item_type = form.item_type.trim();
    let scheduled_date = form.scheduled_date.trim();
    let mut errors = vec![];

    if title.is_empty() {
        errors.push("Title is required".to_string());
    }
    if description.is_empty() {
        errors.push("Description is required".to_string());
    }
    if item_type.is_empty() {
        errors.push("Item type is required".to_string());
    }
    if scheduled_date.is_empty() {
        errors.push("Scheduled date is required".to_string());
    }

    // Parse time allocation
    let time_allocation_minutes: i32 = form.time_allocation_minutes.trim().parse().unwrap_or(0);
    if time_allocation_minutes < 0 {
        errors.push("Time allocation must be a non-negative number".to_string());
    }

    if !errors.is_empty() {
        let existing = agenda_point::find_by_id(&conn, agenda_point_id).ok().flatten();
        let tor_name = tor::get_tor_name(&conn, tor_id)?;
        let ctx = PageContext::build(&session, &conn, "/workflow")?;
        let tmpl = AgendaPointFormTemplate {
            ctx,
            tor_id,
            tor_name,
            form_action: format!("/tor/{tor_id}/workflow/agenda/{agenda_point_id}"),
            form_title: "Edit Agenda Point".to_string(),
            agenda_point: existing,
            errors,
        };
        return render(tmpl);
    }

    agenda_point::update(&conn, agenda_point_id, title, description, item_type, scheduled_date, time_allocation_minutes)?;

    // Audit log
    let details = serde_json::json!({
        "agenda_point_id": agenda_point_id,
        "title": title,
        "summary": format!("Updated agenda point '{}'", title)
    });
    let _ = crate::audit::log(&conn, user_id, "agenda_point.updated", "agenda_point", agenda_point_id, details);

    let _ = session.insert("flash", "Agenda point updated successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/workflow/agenda/{agenda_point_id}")))
        .finish())
}

/// POST /tor/{id}/workflow/agenda/{agenda_id}/transition
/// Generic workflow transition handler using the workflow engine.
pub async fn transition(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<crate::handlers::auth_handlers::CsrfOnly>,
) -> Result<HttpResponse, AppError> {
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, agenda_point_id) = path.into_inner();
    let conn = pool.get()?;
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&conn, user_id, tor_id)?;

    // For now, this is a stub. Workflow transitions will be fully implemented
    // as part of a future task when the workflow engine is more mature.

    let _ = session.insert("flash", "Transition handler not yet implemented");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/workflow/agenda/{agenda_point_id}")))
        .finish())
}
