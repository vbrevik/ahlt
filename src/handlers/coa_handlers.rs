use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

use crate::auth::csrf;
use crate::auth::session::{require_permission, get_user_id};
use crate::errors::{AppError, render};
use crate::models::{tor, agenda_point, coa, relation};
use crate::models::coa::CoaForm;
use crate::templates_structs::{PageContext, CoaFormTemplate};

// ---------------------------------------------------------------------------
// COA CRUD handlers (Task 15)
// ---------------------------------------------------------------------------

/// GET /tor/{id}/workflow/agenda/{agenda_id}/coa/new
/// Renders the COA creation form.
pub async fn new_form(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "coa.create")?;

    let (tor_id, agenda_point_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Verify agenda point exists in this ToR
    match agenda_point::find_by_id(&pool, agenda_point_id).await {
        Ok(_) => {
            let tor_name = tor::get_tor_name(&pool, tor_id).await?;
            let ctx = PageContext::build(&session, &pool, "/workflow").await?
                .with_tor(tor_id, &tor_name, "workflow");

            let tmpl = CoaFormTemplate {
                ctx,
                tor_id,
                form_action: format!("/tor/{tor_id}/workflow/agenda/{agenda_point_id}/coa"),
                form_title: "New Course of Action".to_string(),
                coa: None,
                errors: vec![],
            };
            render(tmpl)
        }
        Err(_) => Err(AppError::NotFound),
    }
}

/// POST /tor/{id}/workflow/agenda/{agenda_id}/coa
/// Creates a new COA linked to an agenda point.
pub async fn create(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<CoaForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "coa.create")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, agenda_point_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Verify agenda point exists
    if agenda_point::find_by_id(&pool, agenda_point_id).await.is_err() {
        return Err(AppError::NotFound);
    }

    // Validate
    let title = form.title.trim();
    let description = form.description.trim();
    let coa_type = form.coa_type.trim();
    let mut errors = vec![];

    if title.is_empty() {
        errors.push("Title is required".to_string());
    }
    if description.is_empty() {
        errors.push("Description is required".to_string());
    }
    if coa_type.is_empty() {
        errors.push("COA type is required".to_string());
    }
    if coa_type != "simple" && coa_type != "complex" {
        errors.push("COA type must be 'simple' or 'complex'".to_string());
    }

    if !errors.is_empty() {
        let ctx = PageContext::build(&session, &pool, "/workflow").await?;
        let tmpl = CoaFormTemplate {
            ctx,
            tor_id,
            form_action: format!("/tor/{tor_id}/workflow/agenda/{agenda_point_id}/coa"),
            form_title: "New Course of Action".to_string(),
            coa: None,
            errors,
        };
        return render(tmpl);
    }

    // Create COA
    let coa_id = coa::create(&pool, title, description, coa_type, user_id).await?;

    // Create considers_coa relation (agenda_point → coa)
    relation::create(&pool, "considers_coa", agenda_point_id, coa_id).await.map_err(AppError::Db)?;

    // Audit log
    let details = serde_json::json!({
        "tor_id": tor_id,
        "agenda_point_id": agenda_point_id,
        "title": title,
        "coa_type": coa_type,
        "summary": format!("Created COA '{}' ({}) for agenda point", title, coa_type)
    });
    let _ = crate::audit::log(&pool, user_id, "coa.created", "coa", coa_id, details).await;

    let _ = session.insert("flash", "Course of Action created successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/workflow/agenda/{agenda_point_id}/coa/{coa_id}/edit")))
        .finish())
}

/// GET /tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}/edit
/// Renders the COA edit form.
pub async fn edit_form(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64, i64)>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "coa.edit")?;

    let (tor_id, agenda_point_id, coa_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Verify agenda point exists and COA is linked to it
    if agenda_point::find_by_id(&pool, agenda_point_id).await.is_err() {
        return Err(AppError::NotFound);
    }

    match coa::find_by_id(&pool, coa_id).await {
        Ok(coa_detail) => {
            let tor_name = tor::get_tor_name(&pool, tor_id).await?;
            let ctx = PageContext::build(&session, &pool, "/workflow").await?
                .with_tor(tor_id, &tor_name, "workflow");

            let tmpl = CoaFormTemplate {
                ctx,
                tor_id,
                form_action: format!("/tor/{tor_id}/workflow/agenda/{agenda_point_id}/coa/{coa_id}"),
                form_title: format!("Edit: {}", &coa_detail.title),
                coa: Some(coa_detail),
                errors: vec![],
            };
            render(tmpl)
        }
        Err(_) => Err(AppError::NotFound),
    }
}

/// POST /tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}
/// Updates an existing COA.
pub async fn update(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64, i64)>,
    form: web::Form<CoaForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "coa.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, agenda_point_id, coa_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Verify agenda point exists and COA exists
    if agenda_point::find_by_id(&pool, agenda_point_id).await.is_err() {
        return Err(AppError::NotFound);
    }
    if coa::find_by_id(&pool, coa_id).await.is_err() {
        return Err(AppError::NotFound);
    }

    // Validate
    let title = form.title.trim();
    let description = form.description.trim();
    let mut errors = vec![];

    if title.is_empty() {
        errors.push("Title is required".to_string());
    }
    if description.is_empty() {
        errors.push("Description is required".to_string());
    }

    if !errors.is_empty() {
        let coa_detail = coa::find_by_id(&pool, coa_id).await.ok();
        let ctx = PageContext::build(&session, &pool, "/workflow").await?;
        let tmpl = CoaFormTemplate {
            ctx,
            tor_id,
            form_action: format!("/tor/{tor_id}/workflow/agenda/{agenda_point_id}/coa/{coa_id}"),
            form_title: format!("Edit: {}", &coa_detail.as_ref().map(|c| &c.title).unwrap_or(&"COA".to_string())),
            coa: coa_detail,
            errors,
        };
        return render(tmpl);
    }

    // Update COA
    coa::update(&pool, coa_id, title, description).await?;

    // Audit log
    let details = serde_json::json!({
        "tor_id": tor_id,
        "agenda_point_id": agenda_point_id,
        "title": title,
        "summary": format!("Updated COA '{}'", title)
    });
    let _ = crate::audit::log(&pool, user_id, "coa.updated", "coa", coa_id, details).await;

    let _ = session.insert("flash", "Course of Action updated successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/workflow/agenda/{agenda_point_id}/coa/{coa_id}/edit")))
        .finish())
}

/// POST /tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}/delete
/// Deletes a COA.
pub async fn delete(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64, i64)>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "coa.edit")?;

    let (tor_id, agenda_point_id, coa_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Verify agenda point exists and COA exists
    if agenda_point::find_by_id(&pool, agenda_point_id).await.is_err() {
        return Err(AppError::NotFound);
    }

    // Get COA details before deleting for audit log
    let coa_detail = coa::find_by_id(&pool, coa_id).await?;
    let coa_title = coa_detail.title.clone();

    // Delete COA (cascades to sections and relations)
    crate::models::entity::delete(&pool, coa_id).await.map_err(AppError::Db)?;

    // Audit log
    let details = serde_json::json!({
        "tor_id": tor_id,
        "agenda_point_id": agenda_point_id,
        "coa_id": coa_id,
        "title": coa_title,
        "summary": format!("Deleted COA '{}'", coa_title)
    });
    let _ = crate::audit::log(&pool, user_id, "coa.deleted", "coa", coa_id, details).await;

    let _ = session.insert("flash", "Course of Action deleted successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/workflow/agenda/{agenda_point_id}")))
        .finish())
}

/// POST /tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}/sections
/// Adds a section to a complex COA.
pub async fn add_section(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64, i64)>,
    form: web::Form<AddSectionForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "coa.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, agenda_point_id, coa_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Verify COA exists and is complex type
    let coa_detail = coa::find_by_id(&pool, coa_id).await;
    match coa_detail {
        Ok(coa) if coa.coa_type == "complex" => {
            // Validate
            let title = form.section_title.trim();
            let content = form.section_content.trim();
            let order: i32 = form.section_order.trim().parse().unwrap_or(0);

            if title.is_empty() {
                return Err(AppError::Session("Section title is required".to_string()));
            }
            if content.is_empty() {
                return Err(AppError::Session("Section content is required".to_string()));
            }

            // Create section
            let _section_id = coa::add_section(&pool, coa_id, title, content, order).await?;

            // Audit log
            let details = serde_json::json!({
                "coa_id": coa_id,
                "section_title": title,
                "summary": format!("Added section '{}' to COA", title)
            });
            let _ = crate::audit::log(&pool, user_id, "coa.section_added", "coa", coa_id, details).await;

            let _ = session.insert("flash", "Section added successfully");
            Ok(HttpResponse::SeeOther()
                .insert_header(("Location", format!("/tor/{tor_id}/workflow/agenda/{agenda_point_id}/coa/{coa_id}/edit")))
                .finish())
        }
        Ok(_) => Err(AppError::Session("Cannot add sections to simple COAs".to_string())),
        Err(_) => Err(AppError::NotFound),
    }
}

/// POST /tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}/sections/{section_id}
/// Updates a section of a COA.
pub async fn update_section(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64, i64, i64)>,
    form: web::Form<AddSectionForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "coa.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let (tor_id, agenda_point_id, coa_id, section_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Verify COA and section exist
    if coa::find_by_id(&pool, coa_id).await.is_err() {
        return Err(AppError::NotFound);
    }

    // Validate
    let title = form.section_title.trim();
    let content = form.section_content.trim();
    let order: i32 = form.section_order.trim().parse().unwrap_or(0);

    if title.is_empty() {
        return Err(AppError::Session("Section title is required".to_string()));
    }
    if content.is_empty() {
        return Err(AppError::Session("Section content is required".to_string()));
    }

    // Update section properties
    crate::models::entity::set_property(&pool, section_id, "title", title).await.map_err(AppError::Db)?;
    crate::models::entity::set_property(&pool, section_id, "content", content).await.map_err(AppError::Db)?;
    crate::models::entity::set_property(&pool, section_id, "order", &order.to_string()).await.map_err(AppError::Db)?;

    // Audit log
    let details = serde_json::json!({
        "coa_id": coa_id,
        "section_id": section_id,
        "section_title": title,
        "summary": format!("Updated section '{}' in COA", title)
    });
    let _ = crate::audit::log(&pool, user_id, "coa.section_updated", "coa", coa_id, details).await;

    let _ = session.insert("flash", "Section updated successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/workflow/agenda/{agenda_point_id}/coa/{coa_id}/edit")))
        .finish())
}

/// POST /tor/{id}/workflow/agenda/{agenda_id}/coa/{coa_id}/sections/{section_id}/delete
/// Deletes a section from a COA.
pub async fn delete_section(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64, i64, i64)>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "coa.edit")?;

    let (tor_id, agenda_point_id, coa_id, section_id) = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    // Verify COA exists
    if coa::find_by_id(&pool, coa_id).await.is_err() {
        return Err(AppError::NotFound);
    }

    // Delete section (cascades to subsections via relations)
    crate::models::entity::delete(&pool, section_id).await.map_err(AppError::Db)?;

    // Audit log
    let details = serde_json::json!({
        "coa_id": coa_id,
        "section_id": section_id,
        "summary": "Deleted section from COA"
    });
    let _ = crate::audit::log(&pool, user_id, "coa.section_deleted", "coa", coa_id, details).await;

    let _ = session.insert("flash", "Section deleted successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/workflow/agenda/{agenda_point_id}/coa/{coa_id}/edit")))
        .finish())
}

// ---------------------------------------------------------------------------
// Form structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AddSectionForm {
    pub section_title: String,
    pub section_content: String,
    pub section_order: String,
    pub csrf_token: String,
}
