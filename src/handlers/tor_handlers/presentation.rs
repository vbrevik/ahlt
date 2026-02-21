use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

use crate::models::{presentation_template as pt, tor};
use crate::auth::csrf;
use crate::auth::session::require_permission;
use crate::errors::{AppError, render};
use crate::templates_structs::{PageContext, PresentationTemplatesTemplate};

/// List presentation templates for a ToR, with optional selected template's slides.
pub async fn list_templates(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;

    let tor_id = path.into_inner();

    let tor_detail = tor::find_detail_by_id(&pool, tor_id).await?
        .ok_or(AppError::NotFound)?;

    let templates = pt::find_templates_for_tor(&pool, tor_id).await?;

    let selected_id: Option<i64> = query.get("selected").and_then(|s| s.parse().ok());
    let (selected_template, slides) = if let Some(sel_id) = selected_id {
        let sel = templates.iter().find(|t| t.id == sel_id).cloned();
        let sl = if sel.is_some() { pt::find_slides(&pool, sel_id).await? } else { vec![] };
        (sel, sl)
    } else {
        (None, vec![])
    };

    let tmpl = PresentationTemplatesTemplate {
        ctx: PageContext::build(&session, &pool, "/tor").await?,
        tor_id,
        tor_label: tor_detail.label,
        templates,
        selected_template,
        slides,
    };
    render(tmpl)
}

/// Create a new presentation template.
pub async fn create_template(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    let tor_id = path.into_inner();

    let name = form.get("name").map(|s| s.as_str()).unwrap_or("");
    let label = form.get("label").map(|s| s.as_str()).unwrap_or("");
    let description = form.get("description").map(|s| s.as_str()).unwrap_or("");

    if name.trim().is_empty() || label.trim().is_empty() {
        let _ = session.insert("flash", "Name and label are required");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", format!("/tor/{tor_id}/templates")))
            .finish());
    }

    let template_id = pt::create_template(&pool, tor_id, name.trim(), label.trim(), description).await?;

    let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "template_id": template_id,
        "name": name.trim(),
        "summary": "Created presentation template"
    });
    let _ = crate::audit::log(&pool, current_user_id, "tor.template_created", "tor", tor_id, details).await;

    let _ = session.insert("flash", "Template created");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/templates?selected={template_id}")))
        .finish())
}

/// Delete a presentation template.
pub async fn delete_template(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    let (tor_id, template_id) = path.into_inner();

    pt::delete_template(&pool, template_id).await?;

    let current_user_id = crate::auth::session::get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({ "template_id": template_id, "summary": "Deleted presentation template" });
    let _ = crate::audit::log(&pool, current_user_id, "tor.template_deleted", "tor", tor_id, details).await;

    let _ = session.insert("flash", "Template deleted");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/templates")))
        .finish())
}

/// Add a slide to a template.
pub async fn handle_add_slide(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    let (tor_id, template_id) = path.into_inner();

    let name = form.get("name").map(|s| s.as_str()).unwrap_or("");
    let label = form.get("label").map(|s| s.as_str()).unwrap_or("");
    let slide_order: i64 = form.get("slide_order").and_then(|s| s.parse().ok()).unwrap_or(99);
    let required_content = form.get("required_content").map(|s| s.as_str()).unwrap_or("");
    let notes = form.get("notes").map(|s| s.as_str()).unwrap_or("");

    if name.trim().is_empty() || label.trim().is_empty() {
        let _ = session.insert("flash", "Name and label are required");
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", format!("/tor/{tor_id}/templates?selected={template_id}")))
            .finish());
    }

    pt::add_slide(&pool, template_id, name.trim(), label.trim(), slide_order, required_content, notes).await?;

    let _ = session.insert("flash", "Slide added");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/templates?selected={template_id}")))
        .finish())
}

/// Delete a slide from a template.
pub async fn handle_delete_slide(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64, i64)>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    let (tor_id, template_id, slide_id) = path.into_inner();

    pt::delete_slide(&pool, slide_id).await?;

    let _ = session.insert("flash", "Slide deleted");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/templates?selected={template_id}")))
        .finish())
}

/// Move a slide up or down.
pub async fn handle_move_slide(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<(i64, i64, i64)>,
    form: web::Form<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, form.get("csrf_token").map(|s| s.as_str()).unwrap_or(""))?;

    let (tor_id, template_id, slide_id) = path.into_inner();

    let direction = form.get("direction").map(|s| s.as_str()).unwrap_or("");
    let slides = pt::find_slides(&pool, template_id).await?;

    if let Some(pos) = slides.iter().position(|s| s.id == slide_id) {
        let swap_with = match direction {
            "up" if pos > 0 => Some(slides[pos - 1].id),
            "down" if pos < slides.len() - 1 => Some(slides[pos + 1].id),
            _ => None,
        };
        if let Some(other_id) = swap_with {
            pt::reorder_slides(&pool, slide_id, other_id).await?;
        }
    }

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{tor_id}/templates?selected={template_id}")))
        .finish())
}
