use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::models::entity;
use crate::auth::session::{get_user_id, require_permission};
use crate::errors::AppError;
use crate::templates_structs::{
    PaginatedResponse, ApiEntityResponse, ApiEntityRequest, ApiEntityProperty, ApiErrorResponse,
};

/// GET /api/v1/entities - List entities with optional type filter and pagination
/// Query params: entity_type (filter), page (default 1), per_page (default 25)
pub async fn list(
    pool: web::Data<DbPool>,
    session: Session,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "entities.list")?;

    let entity_type_filter = query.get("entity_type").map(|s| s.as_str());
    let page = query
        .get("page")
        .and_then(|p| p.parse::<i64>().ok())
        .unwrap_or(1)
        .max(1);
    let per_page = query
        .get("per_page")
        .and_then(|p| p.parse::<i64>().ok())
        .unwrap_or(25)
        .max(1)
        .min(100);

    let conn = pool.get()?;

    // Get all entities of requested type (or all if no filter)
    let all_entities = if let Some(et) = entity_type_filter {
        entity::find_by_type(&conn, et)?
    } else {
        // Get all entities by querying directly
        let mut stmt = conn.prepare("SELECT id, entity_type, name, label, sort_order, is_active, created_at, updated_at FROM entities ORDER BY id")?;
        let entities = stmt.query_map([], |row| {
            Ok(entity::Entity {
                id: row.get(0)?,
                entity_type: row.get(1)?,
                name: row.get(2)?,
                label: row.get(3)?,
                sort_order: row.get(4)?,
                is_active: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
        entities
    };

    // Apply pagination
    let total_count = all_entities.len() as i64;
    let offset = ((page - 1) * per_page) as usize;
    let paginated: Vec<_> = all_entities
        .into_iter()
        .skip(offset)
        .take(per_page as usize)
        .collect();

    // Convert to API response format
    let items: Vec<ApiEntityResponse> = paginated
        .into_iter()
        .map(|e| {
            // Fetch properties for each entity
            let props = entity::get_properties(&conn, e.id)
                .unwrap_or_default()
                .into_iter()
                .map(|(k, v)| ApiEntityProperty { key: k, value: v })
                .collect();

            ApiEntityResponse {
                id: e.id,
                entity_type: e.entity_type,
                name: e.name,
                label: if e.label.is_empty() {
                    None
                } else {
                    Some(e.label)
                },
                properties: props,
            }
        })
        .collect();

    let response = PaginatedResponse {
        items,
        page,
        per_page,
        total: total_count,
    };

    Ok(HttpResponse::Ok().json(response))
}

/// GET /api/v1/entities/{id} - Get single entity by ID with properties
pub async fn read(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "entities.list")?;

    let entity_id = path.into_inner();
    let conn = pool.get()?;

    let entity = entity::find_by_id(&conn, entity_id)?.ok_or(AppError::NotFound)?;

    let props = entity::get_properties(&conn, entity_id)?
        .into_iter()
        .map(|(k, v)| ApiEntityProperty { key: k, value: v })
        .collect();

    let response = ApiEntityResponse {
        id: entity.id,
        entity_type: entity.entity_type,
        name: entity.name,
        label: if entity.label.is_empty() {
            None
        } else {
            Some(entity.label)
        },
        properties: props,
    };

    Ok(HttpResponse::Ok().json(response))
}

/// POST /api/v1/entities - Create new entity
pub async fn create(
    pool: web::Data<DbPool>,
    session: Session,
    body: web::Json<ApiEntityRequest>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "entities.create")?;

    let conn = pool.get()?;

    // Validate request
    let mut errors = Vec::new();
    if body.entity_type.trim().is_empty() {
        errors.push("Entity type is required".to_string());
    }
    if body.name.trim().is_empty() {
        errors.push("Name is required".to_string());
    }
    if let Some(label) = &body.label {
        if label.len() > 500 {
            errors.push("Label must be 500 characters or less".to_string());
        }
    }

    if !errors.is_empty() {
        return Ok(HttpResponse::BadRequest().json(ApiErrorResponse {
            error: "Validation failed".to_string(),
            details: Some(errors.join("; ")),
        }));
    }

    // Create entity with label (use provided label or empty string)
    let entity_id = entity::create(
        &conn,
        &body.entity_type,
        &body.name,
        body.label.as_deref().unwrap_or(""),
    )?;

    // Set properties if provided
    if let Some(props) = &body.properties {
        for prop in props {
            entity::set_property(&conn, entity_id, &prop.key, &prop.value)?;
        }
    }

    // Audit log
    let current_user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "entity_type": body.entity_type,
        "name": body.name,
        "label": body.label,
        "summary": "Entity created via API"
    });
    let _ = crate::audit::log(&conn, current_user_id, "entity.created", "entity", entity_id, details);

    // Fetch and return created entity
    let created_entity = entity::find_by_id(&conn, entity_id)?.ok_or(AppError::NotFound)?;
    let props = entity::get_properties(&conn, entity_id)?
        .into_iter()
        .map(|(k, v)| ApiEntityProperty { key: k, value: v })
        .collect();

    let response = ApiEntityResponse {
        id: created_entity.id,
        entity_type: created_entity.entity_type,
        name: created_entity.name,
        label: if created_entity.label.is_empty() {
            None
        } else {
            Some(created_entity.label)
        },
        properties: props,
    };

    Ok(HttpResponse::Created().json(response))
}

/// PUT /api/v1/entities/{id} - Update entity
pub async fn update(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    body: web::Json<ApiEntityRequest>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "entities.edit")?;

    let entity_id = path.into_inner();
    let conn = pool.get()?;

    // Check if entity exists
    let _existing = entity::find_by_id(&conn, entity_id)?.ok_or(AppError::NotFound)?;

    // Validate
    let mut errors = Vec::new();
    if body.name.trim().is_empty() {
        errors.push("Name is required".to_string());
    }
    if let Some(label) = &body.label {
        if label.len() > 500 {
            errors.push("Label must be 500 characters or less".to_string());
        }
    }

    if !errors.is_empty() {
        return Ok(HttpResponse::BadRequest().json(ApiErrorResponse {
            error: "Validation failed".to_string(),
            details: Some(errors.join("; ")),
        }));
    }

    // Update entity name and label
    conn.execute(
        "UPDATE entities SET name = ?1, label = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%S','now') WHERE id = ?3",
        rusqlite::params![&body.name, body.label.as_deref().unwrap_or(""), entity_id],
    )?;

    // Update properties if provided
    if let Some(props) = &body.properties {
        for prop in props {
            entity::set_property(&conn, entity_id, &prop.key, &prop.value)?;
        }
    }

    // Audit log
    let current_user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "name": body.name,
        "label": body.label,
        "summary": "Entity updated via API"
    });
    let _ = crate::audit::log(&conn, current_user_id, "entity.updated", "entity", entity_id, details);

    // Fetch and return updated entity
    let updated_entity = entity::find_by_id(&conn, entity_id)?.ok_or(AppError::NotFound)?;
    let props = entity::get_properties(&conn, entity_id)?
        .into_iter()
        .map(|(k, v)| ApiEntityProperty { key: k, value: v })
        .collect();

    let response = ApiEntityResponse {
        id: updated_entity.id,
        entity_type: updated_entity.entity_type,
        name: updated_entity.name,
        label: if updated_entity.label.is_empty() {
            None
        } else {
            Some(updated_entity.label)
        },
        properties: props,
    };

    Ok(HttpResponse::Ok().json(response))
}

/// DELETE /api/v1/entities/{id} - Delete entity
pub async fn delete(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "entities.delete")?;

    let entity_id = path.into_inner();
    let conn = pool.get()?;

    // Check if entity exists
    entity::find_by_id(&conn, entity_id)?.ok_or(AppError::NotFound)?;

    // Delete entity
    entity::delete(&conn, entity_id)?;

    // Audit log
    let current_user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "summary": "Entity deleted via API"
    });
    let _ = crate::audit::log(&conn, current_user_id, "entity.deleted", "entity", entity_id, details);

    Ok(HttpResponse::NoContent().finish())
}
