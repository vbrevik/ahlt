use actix_session::Session;
use actix_web::{web, HttpResponse};
use sqlx::PgPool;

use crate::auth::session::{require_permission};
use crate::errors::{AppError, render};
use crate::models::document;
use crate::templates_structs::{PageContext, DocumentListTemplate};

/// GET /documents
/// Lists all documents with optional search filtering.
pub async fn list(
    pool: web::Data<PgPool>,
    session: Session,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "document.list")?;

    let search = query.get("q").map(|s| s.as_str());

    let documents = document::find_all(&pool, None, search).await?;
    let total_count = document::count(&pool, None).await?;

    let ctx = PageContext::build(&session, &pool, "/documents").await?;
    let tmpl = DocumentListTemplate {
        ctx,
        documents,
        search_query: search.unwrap_or("").to_string(),
        total_count,
    };

    render(tmpl)
}
