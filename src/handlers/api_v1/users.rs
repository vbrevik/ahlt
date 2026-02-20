use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::db::DbPool;
use crate::models::user;
use crate::auth::{password, validate};
use crate::auth::session::{get_user_id, require_permission};
use crate::errors::AppError;
use crate::templates_structs::{
    PaginatedResponse, ApiUserResponse, ApiUserRequest, ApiErrorResponse,
};

/// GET /api/v1/users - List users with pagination
/// Query params: page (default 1), per_page (default 25)
pub async fn list(
    pool: web::Data<DbPool>,
    session: Session,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.list")?;

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
        .min(100); // Cap at 100

    let conn = pool.get()?;
    let user_page = user::find_paginated(&conn, page, per_page, &crate::models::table_filter::FilterTree::default(), &crate::models::table_filter::SortSpec::default())?;

    let response = PaginatedResponse {
        items: user_page
            .users
            .into_iter()
            .map(|u| ApiUserResponse::from(u))
            .collect(),
        page: user_page.page,
        per_page: user_page.per_page,
        total: user_page.total_count,
    };

    Ok(HttpResponse::Ok().json(response))
}

/// GET /api/v1/users/{id} - Get single user by ID
pub async fn read(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.list")?;

    let user_id = path.into_inner();
    let conn = pool.get()?;

    let user = user::find_display_by_id(&conn, user_id)?
        .ok_or(AppError::NotFound)?;

    Ok(HttpResponse::Ok().json(ApiUserResponse::from(user)))
}

/// POST /api/v1/users - Create new user
pub async fn create(
    pool: web::Data<DbPool>,
    session: Session,
    body: web::Json<ApiUserRequest>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.create")?;

    let conn = pool.get()?;

    // Validate request
    let mut errors = Vec::new();
    errors.extend(validate::validate_username(&body.username));
    if let Some(pwd) = &body.password {
        errors.extend(validate::validate_password(pwd));
    } else {
        errors.push("Password required for user creation".to_string());
    }
    errors.extend(validate::validate_email(&body.email));
    errors.extend(validate::validate_optional(&body.display_name, "Display name", 100));

    if !errors.is_empty() {
        return Ok(HttpResponse::BadRequest().json(ApiErrorResponse {
            error: "Validation failed".to_string(),
            details: Some(errors.join("; ")),
        }));
    }

    // Hash password
    let hashed = password::hash_password(body.password.as_ref().unwrap())
        .map_err(|_| AppError::Hash("Password hash failed".to_string()))?;

    // Create user
    let new_user = user::NewUser {
        username: body.username.clone(),
        password: hashed,
        email: body.email.clone(),
        display_name: body.display_name.clone(),
        role_id: body.role_id,
    };

    let created_id = user::create(&conn, &new_user)?;

    // Audit log
    let current_user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "username": body.username,
        "email": body.email,
        "display_name": body.display_name,
        "role_id": body.role_id,
        "summary": "User created via API"
    });
    let _ = crate::audit::log(&conn, current_user_id, "user.created", "user", created_id, details);

    // Fetch and return created user
    let created_user = user::find_display_by_id(&conn, created_id)?
        .ok_or(AppError::NotFound)?;

    Ok(HttpResponse::Created().json(ApiUserResponse::from(created_user)))
}

/// PUT /api/v1/users/{id} - Update user
pub async fn update(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    body: web::Json<ApiUserRequest>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.edit")?;

    let user_id = path.into_inner();
    let conn = pool.get()?;

    // Check if user exists
    let _existing = user::find_display_by_id(&conn, user_id)?
        .ok_or(AppError::NotFound)?;

    // Validate
    let mut errors = Vec::new();
    errors.extend(validate::validate_username(&body.username));
    if let Some(pwd) = &body.password {
        errors.extend(validate::validate_password(pwd));
    }
    errors.extend(validate::validate_email(&body.email));
    errors.extend(validate::validate_optional(&body.display_name, "Display name", 100));

    if !errors.is_empty() {
        return Ok(HttpResponse::BadRequest().json(ApiErrorResponse {
            error: "Validation failed".to_string(),
            details: Some(errors.join("; ")),
        }));
    }

    // Update user
    let hashed = if let Some(pwd) = &body.password {
        Some(password::hash_password(pwd)
            .map_err(|_| AppError::Hash("Password hash failed".to_string()))?)
    } else {
        None
    };

    user::update(
        &conn,
        user_id,
        &body.username,
        hashed.as_deref(),
        &body.email,
        &body.display_name,
        body.role_id,
    )?;

    // Audit log
    let current_user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "username": body.username,
        "email": body.email,
        "display_name": body.display_name,
        "role_id": body.role_id,
        "summary": "User updated via API"
    });
    let _ = crate::audit::log(&conn, current_user_id, "user.updated", "user", user_id, details);

    // Fetch and return updated user
    let updated_user = user::find_display_by_id(&conn, user_id)?
        .ok_or(AppError::NotFound)?;

    Ok(HttpResponse::Ok().json(ApiUserResponse::from(updated_user)))
}

/// DELETE /api/v1/users/{id} - Delete user
pub async fn delete(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "users.delete")?;

    let user_id = path.into_inner();
    let conn = pool.get()?;

    // Check if user exists
    user::find_display_by_id(&conn, user_id)?
        .ok_or(AppError::NotFound)?;

    // Prevent self-deletion and last-admin check (via model function)
    user::delete(&conn, user_id)?;

    // Audit log
    let current_user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "summary": "User deleted via API"
    });
    let _ = crate::audit::log(&conn, current_user_id, "user.deleted", "user", user_id, details);

    Ok(HttpResponse::NoContent().finish())
}
