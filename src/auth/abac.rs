//! Attribute-Based Access Control (ABAC) for resource-scoped capabilities.
//!
//! Provides capability checks for EAV-graph resources. Currently used for ToR
//! member roles (Chairperson, Secretary) that need fine-grained operation access
//! without global `tor.edit` permission.
//!
//! ## Graph model
//!
//! ```text
//! user --(fills_position)--> tor_function --(belongs_to_tor)--> tor
//!                            tor_function has entity_properties:
//!                              can_call_meetings      = 'true' | 'false'
//!                              can_manage_agenda      = 'true' | 'false'
//!                              can_record_decisions   = 'true' | 'false'
//!                              can_review_suggestions = 'true' | 'false'
//!                              can_create_proposals   = 'true' | 'false'
//!                              can_approve_proposals  = 'true' | 'false'
//! ```
//!
//! `load_tor_capabilities` returns all six capability keys (using `LIKE 'can_%'`),
//! not just the three used by Split 2 meeting handlers. This forward-compatibility
//! allows future splits for suggestion/proposal ABAC without modifying this function.

use crate::auth::session::{get_user_id, require_permission, Permissions};
use crate::errors::AppError;
use actix_session::Session;
use sqlx::PgPool;

/// Check whether a user holds a specific capability in a given resource,
/// by traversing the EAV graph:
///   user --(fills_position)--> tor_function --(belongs_to_rel)--> resource
///
/// Returns `Ok(true)` if ANY of the user's positions in the resource
/// has the capability property set to `'true'`.
/// Returns `Ok(false)` for non-members, wrong-resource, or missing/false flag.
/// Returns `Err` on database error.
///
/// Fail-closed: a misspelled `belongs_to_rel` causes the scalar subquery to
/// return NULL, so the WHERE clause evaluates to UNKNOWN (false in SQL
/// three-valued logic), and the function returns `Ok(false)`.
pub async fn has_resource_capability(
    pool: &PgPool,
    user_id: i64,
    resource_id: i64,
    belongs_to_rel: &str,
    capability: &str,
) -> Result<bool, AppError> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)
         FROM entity_properties ep
         JOIN entities func
             ON ep.entity_id = func.id
             AND func.entity_type = 'tor_function'
         JOIN relations r_fills
             ON r_fills.target_id = func.id
             AND r_fills.source_id = $1
             AND r_fills.relation_type_id = (
                 SELECT id FROM entities
                 WHERE entity_type = 'relation_type' AND name = 'fills_position'
             )
         JOIN relations r_belongs
             ON r_belongs.source_id = func.id
             AND r_belongs.target_id = $2
             AND r_belongs.relation_type_id = (
                 SELECT id FROM entities
                 WHERE entity_type = 'relation_type' AND name = $3
             )
         WHERE ep.key = $4
           AND ep.value = 'true'",
    )
    .bind(user_id)
    .bind(resource_id)
    .bind(belongs_to_rel)
    .bind(capability)
    .fetch_one(pool)
    .await?;
    Ok(row.0 > 0)
}

/// Load all capability keys the user holds in a specific ToR, in a single
/// database query. Returns only keys whose value is `'true'`.
///
/// Used at page-render time to populate template contexts with ABAC capabilities
/// (e.g., `ctx.tor_capabilities.has("can_call_meetings")`).
///
/// The `LIKE 'can_%'` filter captures all six capability types, making this
/// function forward-compatible when new capabilities are added.
pub async fn load_tor_capabilities(
    pool: &PgPool,
    user_id: i64,
    tor_id: i64,
) -> Result<Permissions, AppError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT ep.key
         FROM entity_properties ep
         JOIN entities func
             ON ep.entity_id = func.id
             AND func.entity_type = 'tor_function'
         JOIN relations r_fills
             ON r_fills.target_id = func.id
             AND r_fills.source_id = $1
             AND r_fills.relation_type_id = (
                 SELECT id FROM entities
                 WHERE entity_type = 'relation_type' AND name = 'fills_position'
             )
         JOIN relations r_belongs
             ON r_belongs.source_id = func.id
             AND r_belongs.target_id = $2
             AND r_belongs.relation_type_id = (
                 SELECT id FROM entities
                 WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor'
             )
         WHERE ep.key LIKE 'can_%'
           AND ep.value = 'true'",
    )
    .bind(user_id)
    .bind(tor_id)
    .fetch_all(pool)
    .await?;
    let keys: Vec<String> = rows.into_iter().map(|r| r.0).collect();
    Ok(Permissions(keys))
}

/// Handler-level guard for ToR resource capabilities.
///
/// Two-phase check:
/// 1. If the session has global `tor.edit`, access is granted immediately
///    (admin bypass — no DB query needed).
/// 2. Otherwise, look up the user's ABAC capability via `has_resource_capability`.
///    Returns `Ok(())` if the user holds the capability, or `Err(PermissionDenied)`
///    if not.
///
/// Error semantics:
/// - Unauthenticated session → `AppError::Session`
/// - Capability not held → `AppError::PermissionDenied(capability)`
/// - Database error → `AppError::Db`
pub async fn require_tor_capability(
    pool: &PgPool,
    session: &Session,
    tor_id: i64,
    capability: &str,
) -> Result<(), AppError> {
    // Unauthenticated sessions fail immediately before any permission logic.
    let user_id = get_user_id(session)
        .ok_or_else(|| AppError::Session("Not authenticated".to_string()))?;
    // Phase 1: global bypass for users with tor.edit (no DB query needed).
    if require_permission(session, "tor.edit").is_ok() {
        return Ok(());
    }
    // Phase 2: resource-level capability check via ABAC graph traversal.
    // ToR-specific: the resource link relation is always 'belongs_to_tor'.
    if has_resource_capability(pool, user_id, tor_id, "belongs_to_tor", capability).await? {
        Ok(())
    } else {
        Err(AppError::PermissionDenied(capability.to_string()))
    }
}
