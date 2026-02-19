use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::auth::session::{require_permission};
use crate::errors::{AppError, render};
use crate::models::document;
use crate::templates_structs::{PageContext, DocumentListTemplate};

/// GET /documents
/// Lists all documents with optional search filtering.
pub async fn list(
    pool: web::Data<DbPool>,
    session: Session,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "document.list")?;

    let conn = pool.get()?;
    let search = query.get("q").map(|s| s.as_str());

    let documents = document::find_all(&conn, None, search)?;
    let total_count = document::count(&conn, None)?;

    let ctx = PageContext::build(&session, &conn, "/documents")?;
    let tmpl = DocumentListTemplate {
        ctx,
        documents,
        search_query: search.unwrap_or("").to_string(),
        total_count,
    };

    render(tmpl)
}
