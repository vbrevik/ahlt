use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::auth::{csrf, session::{require_permission, get_user_id}};
use crate::errors::{AppError, render};
use crate::models::document;
use crate::templates_structs::{PageContext, DocumentDetailTemplate, DocumentFormTemplate};

/// GET /documents/new
/// Renders the document creation form.
pub async fn new_form(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "document.create")?;

    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/documents")?;

    let tmpl = DocumentFormTemplate {
        ctx,
        form_title: "New Document".to_string(),
        form_action: "/documents".to_string(),
        document: None,
        errors: vec![],
    };
    render(tmpl)
}

/// POST /documents
/// Creates a new document.
pub async fn create(
    pool: web::Data<DbPool>,
    session: Session,
    form: web::Form<document::DocumentForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "document.create")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let conn = pool.get()?;
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;

    // Validate
    let title = form.title.trim();
    let body = form.body.trim();
    let doc_type = form.doc_type.trim();
    let mut errors = vec![];

    if title.is_empty() {
        errors.push("Title is required".to_string());
    }
    if body.is_empty() {
        errors.push("Body is required".to_string());
    }
    if doc_type.is_empty() {
        errors.push("Document type is required".to_string());
    }

    if !errors.is_empty() {
        let ctx = PageContext::build(&session, &conn, "/documents")?;
        let tmpl = DocumentFormTemplate {
            ctx,
            form_title: "New Document".to_string(),
            form_action: "/documents".to_string(),
            document: None,
            errors,
        };
        return render(tmpl);
    }

    let tor_id = form.tor_id.as_ref().and_then(|s| s.parse::<i64>().ok());
    let doc_id = document::create(&conn, title, doc_type, body, user_id, tor_id)?;

    // Audit log
    let details = serde_json::json!({
        "doc_id": doc_id,
        "title": title,
        "doc_type": doc_type,
        "tor_id": tor_id,
        "summary": format!("Created document '{}'", title)
    });
    let _ = crate::audit::log(&conn, user_id, "document.created", "document", doc_id, details);

    let _ = session.insert("flash", "Document created successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/documents/{}", doc_id)))
        .finish())
}

/// GET /documents/{id}
/// Renders the document detail page.
pub async fn detail(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "document.view")?;

    let doc_id = path.into_inner();
    let conn = pool.get()?;

    match document::find_by_id(&conn, doc_id)? {
        Some(doc) => {
            let ctx = PageContext::build(&session, &conn, "/documents")?;
            let tmpl = DocumentDetailTemplate { ctx, document: doc };
            render(tmpl)
        }
        None => Err(AppError::NotFound),
    }
}

/// GET /documents/{id}/edit
/// Renders the document edit form.
pub async fn edit_form(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "document.edit")?;

    let doc_id = path.into_inner();
    let conn = pool.get()?;

    match document::find_by_id(&conn, doc_id)? {
        Some(doc) => {
            let ctx = PageContext::build(&session, &conn, "/documents")?;
            let tmpl = DocumentFormTemplate {
                ctx,
                form_title: "Edit Document".to_string(),
                form_action: format!("/documents/{}", doc_id),
                document: Some(doc),
                errors: vec![],
            };
            render(tmpl)
        }
        None => Err(AppError::NotFound),
    }
}

/// POST /documents/{id}
/// Updates an existing document.
pub async fn update(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<document::DocumentForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "document.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let doc_id = path.into_inner();
    let conn = pool.get()?;
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;

    // Validate
    let title = form.title.trim();
    let body = form.body.trim();
    let doc_type = form.doc_type.trim();
    let mut errors = vec![];

    if title.is_empty() {
        errors.push("Title is required".to_string());
    }
    if body.is_empty() {
        errors.push("Body is required".to_string());
    }
    if doc_type.is_empty() {
        errors.push("Document type is required".to_string());
    }

    if !errors.is_empty() {
        let existing = document::find_by_id(&conn, doc_id).ok().flatten();
        let ctx = PageContext::build(&session, &conn, "/documents")?;
        let tmpl = DocumentFormTemplate {
            ctx,
            form_title: "Edit Document".to_string(),
            form_action: format!("/documents/{}", doc_id),
            document: existing,
            errors,
        };
        return render(tmpl);
    }

    document::update(&conn, doc_id, title, doc_type, body)?;

    // Audit log
    let details = serde_json::json!({
        "doc_id": doc_id,
        "title": title,
        "summary": format!("Updated document '{}'", title)
    });
    let _ = crate::audit::log(&conn, user_id, "document.updated", "document", doc_id, details);

    let _ = session.insert("flash", "Document updated successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/documents/{}", doc_id)))
        .finish())
}

/// POST /documents/{id}/delete
/// Deletes a document.
pub async fn delete(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "document.delete")?;

    let doc_id = path.into_inner();
    let conn = pool.get()?;
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;

    // Get document details before deletion for audit log
    let doc = document::find_by_id(&conn, doc_id)?.ok_or(AppError::NotFound)?;

    document::delete(&conn, doc_id)?;

    // Audit log
    let details = serde_json::json!({
        "doc_id": doc_id,
        "title": &doc.title,
        "summary": format!("Deleted document '{}'", &doc.title)
    });
    let _ = crate::audit::log(&conn, user_id, "document.deleted", "document", doc_id, details);

    let _ = session.insert("flash", "Document deleted successfully");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/documents"))
        .finish())
}
