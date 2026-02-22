use sqlx::PgPool;
use crate::auth::session::get_user_id;
use crate::models::user::{self, UserForm};
use crate::errors::AppError;
use crate::auth::validate;
use actix_session::Session;

/// Validate user form data (used in both create and update flows)
pub fn validate_user_form(form: &UserForm, require_password: bool) -> Vec<String> {
    let mut errors = vec![];
    errors.extend(validate::validate_username(&form.username));
    errors.extend(validate::validate_email(&form.email));
    errors.extend(validate::validate_optional(&form.display_name, "Display name", 100));
    if require_password || !form.password.is_empty() {
        errors.extend(validate::validate_password(&form.password));
    }
    errors
}

/// Check if a user is the last admin in the system
pub async fn is_last_admin(pool: &PgPool, user_id: i64) -> Result<bool, AppError> {
    let has_admin_role: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM relations r \
         JOIN entities role_e ON r.target_id = role_e.id \
         WHERE r.source_id = $1 \
           AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
           AND role_e.name = 'admin'",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    if !has_admin_role {
        return Ok(false);
    }

    let admin_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT r.source_id) FROM relations r \
         JOIN entities role_e ON r.target_id = role_e.id \
         WHERE r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
           AND role_e.name = 'admin'",
    )
    .fetch_one(pool)
    .await?;

    Ok(admin_count <= 1)
}

/// Check if a user is the last admin (bulk operation version with safe defaults)
pub async fn is_last_admin_bulk(pool: &PgPool, user_id: i64) -> bool {
    let has_admin: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM relations r \
         JOIN entities role_e ON r.target_id = role_e.id \
         WHERE r.source_id = $1 \
           AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
           AND role_e.name = 'admin'",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap_or(true);

    if !has_admin {
        return false;
    }

    let admin_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT r.source_id) FROM relations r \
         JOIN entities role_e ON r.target_id = role_e.id \
         WHERE r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role') \
           AND role_e.name = 'admin'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(1);

    admin_count <= 1
}

/// Get current user ID with session validation
pub fn get_current_user_id(session: &Session) -> Result<i64, AppError> {
    get_user_id(session)
        .ok_or_else(|| AppError::Session("Not logged in".to_string()))
}

/// Get user details before deletion for audit logging
pub async fn fetch_user_for_audit(pool: &PgPool, user_id: i64) -> Option<user::UserDisplay> {
    user::find_display_by_id(pool, user_id).await.ok().flatten()
}
