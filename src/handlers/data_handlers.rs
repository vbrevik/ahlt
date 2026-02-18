use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::auth::csrf;
use crate::auth::session::require_permission;
use crate::db::DbPool;
use crate::errors::{render, AppError};
use crate::models::data_manager::{export, import, jsonld};
use crate::templates_structs::{DataManagerTemplate, PageContext};

/// Query params for the export endpoint.
#[derive(serde::Deserialize)]
pub struct ExportQuery {
    pub format: Option<String>,
    pub types: Option<String>,
}

/// GET /data-manager — admin page
pub async fn data_manager_page(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "settings.manage")?;

    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/data-manager")?;

    // Collect distinct entity types for the export filter
    let mut stmt = conn.prepare(
        "SELECT DISTINCT entity_type FROM entities ORDER BY entity_type",
    )?;
    let entity_types: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    let tmpl = DataManagerTemplate { ctx, entity_types };
    render(tmpl)
}

/// POST /api/data/import — import entities and relations
pub async fn import_data(
    pool: web::Data<DbPool>,
    session: Session,
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "settings.manage")?;

    // CSRF validation: expect token in the JSON body or X-CSRF-Token header
    // For API calls, we check the JSON body field "csrf_token"
    if let Some(token) = body.get("csrf_token").and_then(|v| v.as_str()) {
        csrf::validate_csrf(&session, token)?;
    } else {
        return Err(AppError::Csrf("missing csrf_token in request body".to_string()));
    }

    let conn = pool.get()?;

    // Auto-detect JSON-LD vs native format
    let payload = if body.get("@context").is_some() || body.get("@graph").is_some() {
        jsonld::parse_jsonld(&body)
            .map_err(|e| AppError::Session(format!("Invalid JSON-LD: {}", e)))?
    } else {
        // Parse native format, stripping the csrf_token field
        let mut native = body.into_inner();
        if let Some(obj) = native.as_object_mut() {
            obj.remove("csrf_token");
        }
        serde_json::from_value(native)
            .map_err(|e| AppError::Session(format!("Invalid import payload: {}", e)))?
    };

    let result = import::import_data(&conn, &payload)
        .map_err(|e| AppError::Session(format!("Import failed: {}", e)))?;

    Ok(HttpResponse::Ok().json(result))
}

/// GET /api/data/export — export entity graph
pub async fn export_data(
    pool: web::Data<DbPool>,
    session: Session,
    query: web::Query<ExportQuery>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "settings.manage")?;

    let conn = pool.get()?;

    let types_filter: Option<Vec<String>> = query.types.as_ref().map(|t| {
        t.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    });
    let types_ref = types_filter.as_deref();

    let format = query.format.as_deref().unwrap_or("json");

    match format {
        "jsonld" => {
            let data = jsonld::export_jsonld(&conn, types_ref)?;
            Ok(HttpResponse::Ok()
                .content_type("application/ld+json")
                .json(data))
        }
        "sql" => {
            let sql = export::export_sql(&conn, types_ref)?;
            Ok(HttpResponse::Ok()
                .content_type("text/plain; charset=utf-8")
                .body(sql))
        }
        _ => {
            let data = export::export_entities(&conn, types_ref)?;
            Ok(HttpResponse::Ok().json(data))
        }
    }
}

/// GET /api/data/schema — return the JSON-LD @context
pub async fn schema(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "settings.manage")?;

    let conn = pool.get()?;
    let context = jsonld::build_context(&conn)?;

    Ok(HttpResponse::Ok()
        .content_type("application/ld+json")
        .json(context))
}
