use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

use std::collections::HashMap;
use crate::auth::csrf;
use crate::auth::session::{require_permission, get_user_id, get_permissions};
use crate::errors::{AppError, render};
use crate::models::{entity, tor, agenda_point, coa, opinion, workflow};
use crate::models::agenda_point::AgendaPointForm;
use crate::templates_structs::{PageContext, AgendaPointFormTemplate, AgendaPointDetailTemplate};

#[derive(serde::Deserialize)]
pub struct AgendaTransitionForm {
    pub csrf_token: String,
    pub to_status: String,
}

#[derive(serde::Deserialize)]
pub struct AgendaDeleteForm {
    pub csrf_token: String,
}

// ---------------------------------------------------------------------------
// CRUD handlers (Task 13)
// ---------------------------------------------------------------------------

/// GET /tor/{id}/workflow/agenda/new
/// Renders the agenda point creation form.
pub async fn new_form(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "agenda.create")?;

    let tor_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    let ctx = PageContext::build(&session, &pool, "/workflow").await?;

    let tmpl = AgendaPointFormTemplate {
        ctx,
        tor_id,
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
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<AgendaPointForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "agenda.create")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let tor_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Validate
    let title = form.title.trim();
    let description = form.description.trim();
    let item_type = form.item_type.trim();
    let scheduled_date = form.scheduled_date.trim();
    let presenter = form.presenter.as_deref().unwrap_or("").trim();
    let priority = form.priority.as_deref().unwrap_or("").trim();
    let pre_read_url = form.pre_read_url.as_deref().unwrap_or("").trim();
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
        let ctx = PageContext::build(&session, &pool, "/workflow").await?;
        let tmpl = AgendaPointFormTemplate {
            ctx,
            tor_id,
            form_action: format!("/tor/{tor_id}/workflow/agenda"),
            form_title: "New Agenda Point".to_string(),
            agenda_point: None,
            errors,
        };
        return render(tmpl);
    }

    let agenda_point_id = agenda_point::create(
        &pool, tor_id, title, description, item_type, scheduled_date, time_allocation_minutes, user_id,
        presenter, priority, pre_read_url,
    ).await?;

    // Audit log
    let details = serde_json::json!({
        "tor_id": tor_id,
        "title": title,
        "summary": format!("Created agenda point '{}'", title)
    });
    let _ = crate::audit::log(&pool, user_id, "agenda_point.created", "agenda_point", agenda_point_id, details).await;

    let _ = session.insert("flash", "Agenda point created successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/workflow/agenda/{agenda_point_id}")))
        .finish())
}

/// GET /tor/{id}/workflow/agenda/{agenda_id}
/// Renders the agenda point detail page.
pub async fn detail(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "agenda.view")?;

    let (tor_id, agenda_point_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    match agenda_point::find_by_id(&pool, agenda_point_id).await? {
        Some(ap) => {
            let ctx = PageContext::build(&session, &pool, "/workflow").await?;

            // Fetch related COAs
            let mut coas = vec![];
            for coa_id in &ap.coa_ids {
                if let Ok(coa_detail) = coa::find_by_id(&pool, *coa_id).await {
                    coas.push(coa_detail);
                }
            }

            // Fetch opinions
            let opinions_list = opinion::find_opinions_for_agenda_point(&pool, agenda_point_id).await?;

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
                    preference_pct: 0,
                    opinions: coa_opinions,
                });
            }

            // Compute preference percentages for the COA preference bar
            let total_prefs: i32 = opinions.iter().map(|s| s.preference_count).sum();
            for summary in &mut opinions {
                summary.preference_pct = if total_prefs > 0 {
                    (summary.preference_count * 100 / total_prefs) as u32
                } else {
                    0
                };
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
                &pool,
                "agenda_point",
                &ap.status,
                &permissions,
                &entity_properties,
            ).await?;

            let tmpl = AgendaPointDetailTemplate {
                ctx,
                tor_id,
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
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "agenda.manage")?;

    let (tor_id, agenda_point_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    match agenda_point::find_by_id(&pool, agenda_point_id).await? {
        Some(ap) => {
            let ctx = PageContext::build(&session, &pool, "/workflow").await?;
            let tmpl = AgendaPointFormTemplate {
                ctx,
                tor_id,
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
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<AgendaPointForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "agenda.manage")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, agenda_point_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Validate
    let title = form.title.trim();
    let description = form.description.trim();
    let item_type = form.item_type.trim();
    let scheduled_date = form.scheduled_date.trim();
    let presenter = form.presenter.as_deref().unwrap_or("").trim();
    let priority = form.priority.as_deref().unwrap_or("").trim();
    let pre_read_url = form.pre_read_url.as_deref().unwrap_or("").trim();
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
        let existing = agenda_point::find_by_id(&pool, agenda_point_id).await.ok().flatten();
        let ctx = PageContext::build(&session, &pool, "/workflow").await?;
        let tmpl = AgendaPointFormTemplate {
            ctx,
            tor_id,
            form_action: format!("/tor/{tor_id}/workflow/agenda/{agenda_point_id}"),
            form_title: "Edit Agenda Point".to_string(),
            agenda_point: existing,
            errors,
        };
        return render(tmpl);
    }

    agenda_point::update(&pool, agenda_point_id, title, description, item_type, scheduled_date, time_allocation_minutes, presenter, priority, pre_read_url).await?;

    // Audit log
    let details = serde_json::json!({
        "agenda_point_id": agenda_point_id,
        "title": title,
        "summary": format!("Updated agenda point '{}'", title)
    });
    let _ = crate::audit::log(&pool, user_id, "agenda_point.updated", "agenda_point", agenda_point_id, details).await;

    let _ = session.insert("flash", "Agenda point updated successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/workflow/agenda/{agenda_point_id}")))
        .finish())
}

/// POST /tor/{id}/workflow/agenda/{agenda_id}/transition
/// Advance an agenda point through its workflow states via the workflow engine.
pub async fn transition(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<AgendaTransitionForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "agenda.manage")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, agenda_point_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    let ap = agenda_point::find_by_id(&pool, agenda_point_id).await?
        .ok_or(AppError::NotFound)?;

    let permissions = get_permissions(&session)
        .map_err(|e| AppError::Session(format!("Failed to get permissions: {}", e)))?;

    let mut entity_properties = HashMap::new();
    entity_properties.insert("item_type".to_string(), ap.item_type.clone());

    // Validate the transition via the workflow engine
    workflow::validate_transition(
        &pool,
        "agenda_point",
        &ap.status,
        &form.to_status,
        &permissions,
        &entity_properties,
    ).await?;

    // Update status
    entity::set_property(&pool, agenda_point_id, "status", &form.to_status)
        .await
        .map_err(AppError::Db)?;

    // Audit log
    let details = serde_json::json!({
        "agenda_point_id": agenda_point_id,
        "tor_id": tor_id,
        "from_status": &ap.status,
        "to_status": &form.to_status,
        "summary": format!("Agenda point '{}' transitioned from {} to {}", ap.title, ap.status, form.to_status),
    });
    let _ = crate::audit::log(&pool, user_id, "agenda_point.transition", "agenda_point", agenda_point_id, details).await;

    let _ = session.insert("flash", format!("Status changed to {}", &form.to_status));
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/workflow/agenda/{agenda_point_id}")))
        .finish())
}

/// POST /tor/{id}/workflow/agenda/{agenda_id}/delete
pub async fn delete(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<AgendaDeleteForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "agenda.manage")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, agenda_point_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    let ap = agenda_point::find_by_id(&pool, agenda_point_id).await?
        .ok_or(AppError::NotFound)?;
    let ap_title = ap.title.clone();

    entity::delete(&pool, agenda_point_id).await.map_err(AppError::Db)?;

    let details = serde_json::json!({
        "agenda_point_id": agenda_point_id,
        "tor_id": tor_id,
        "title": ap_title,
        "summary": format!("Deleted agenda point '{}'", ap_title),
    });
    let _ = crate::audit::log(&pool, user_id, "agenda_point.deleted", "agenda_point", agenda_point_id, details).await;

    let _ = session.insert("flash", "Agenda point deleted");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/workflow?tab=agenda_points")))
        .finish())
}
