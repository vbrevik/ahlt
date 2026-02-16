use rusqlite::{Connection, params};
use crate::errors::AppError;
use crate::auth::session::Permissions;
use super::types::*;

/// Find all available transitions from the current status,
/// filtered by user permissions and entity properties (conditions).
pub fn find_available_transitions(
    conn: &Connection,
    entity_type_scope: &str,
    current_status: &str,
    user_permissions: &Permissions,
    entity_properties: &std::collections::HashMap<String, String>,
) -> Result<Vec<AvailableTransition>, AppError> {
    // Find all transitions where transition_from matches current_status and entity_type_scope
    let mut stmt = conn.prepare(
        "SELECT t.id, t.label AS transition_label, \
                COALESCE(p_perm.value, '') AS required_permission, \
                p_cond.value AS condition, \
                COALESCE(p_outcome.value, 'false') AS requires_outcome, \
                COALESCE(p_to_code.value, '') AS to_status_code \
         FROM entities t \
         JOIN relations r_from ON t.id = r_from.source_id \
         JOIN entities rt_from ON r_from.relation_type_id = rt_from.id AND rt_from.name = 'transition_from' \
         JOIN entities s_from ON r_from.target_id = s_from.id \
         JOIN entity_properties sp_from_code ON s_from.id = sp_from_code.entity_id AND sp_from_code.key = 'status_code' \
         JOIN entity_properties sp_from_scope ON s_from.id = sp_from_scope.entity_id AND sp_from_scope.key = 'entity_type_scope' \
         JOIN relations r_to ON t.id = r_to.source_id \
         JOIN entities rt_to ON r_to.relation_type_id = rt_to.id AND rt_to.name = 'transition_to' \
         JOIN entities s_to ON r_to.target_id = s_to.id \
         JOIN entity_properties p_to_code ON s_to.id = p_to_code.entity_id AND p_to_code.key = 'status_code' \
         LEFT JOIN entity_properties p_perm ON t.id = p_perm.entity_id AND p_perm.key = 'required_permission' \
         LEFT JOIN entity_properties p_cond ON t.id = p_cond.entity_id AND p_cond.key = 'condition' \
         LEFT JOIN entity_properties p_outcome ON t.id = p_outcome.entity_id AND p_outcome.key = 'requires_outcome' \
         WHERE t.entity_type = 'workflow_transition' \
           AND sp_from_code.value = ?1 \
           AND sp_from_scope.value = ?2"
    ).map_err(AppError::Db)?;

    let all_transitions = stmt.query_map(params![current_status, entity_type_scope], |row| {
        let requires_outcome_str: String = row.get("requires_outcome")?;
        Ok((
            row.get::<_, String>("required_permission")?,
            row.get::<_, Option<String>>("condition")?,
            AvailableTransition {
                to_status_code: row.get("to_status_code")?,
                transition_label: row.get("transition_label")?,
                requires_outcome: requires_outcome_str == "true",
            },
        ))
    }).map_err(AppError::Db)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::Db)?;

    // Filter by permission and condition
    let mut available = Vec::new();
    for (required_perm, condition, transition) in all_transitions {
        // Check permission
        if !required_perm.is_empty() && !user_permissions.has(&required_perm) {
            continue;
        }

        // Check condition (format: "key=value")
        if let Some(cond) = &condition {
            if let Some((key, value)) = cond.split_once('=') {
                let actual = entity_properties.get(key).map(|s| s.as_str()).unwrap_or("");
                if actual != value {
                    continue;
                }
            }
        }

        available.push(transition);
    }

    Ok(available)
}

/// Validate a specific transition and return its info.
/// Returns error if transition is not valid or user lacks permission.
pub fn validate_transition(
    conn: &Connection,
    entity_type_scope: &str,
    current_status: &str,
    new_status: &str,
    user_permissions: &Permissions,
    entity_properties: &std::collections::HashMap<String, String>,
) -> Result<AvailableTransition, AppError> {
    let available = find_available_transitions(
        conn, entity_type_scope, current_status, user_permissions, entity_properties,
    )?;

    available.into_iter()
        .find(|t| t.to_status_code == new_status)
        .ok_or_else(|| AppError::PermissionDenied(
            format!("Invalid or unauthorized transition: {} -> {} for {}", current_status, new_status, entity_type_scope)
        ))
}
