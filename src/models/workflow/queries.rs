use rusqlite::{Connection, params};
use crate::errors::AppError;
use crate::auth::session::Permissions;
use super::types::*;

/// Find all workflow statuses for a given entity type scope.
pub fn find_statuses_for_type(conn: &Connection, entity_type_scope: &str) -> Result<Vec<WorkflowStatus>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.label, \
                COALESCE(p_scope.value, '') AS entity_type_scope, \
                COALESCE(p_code.value, '') AS status_code, \
                COALESCE(p_initial.value, 'false') AS is_initial, \
                COALESCE(p_terminal.value, 'false') AS is_terminal \
         FROM entities e \
         LEFT JOIN entity_properties p_scope ON e.id = p_scope.entity_id AND p_scope.key = 'entity_type_scope' \
         LEFT JOIN entity_properties p_code ON e.id = p_code.entity_id AND p_code.key = 'status_code' \
         LEFT JOIN entity_properties p_initial ON e.id = p_initial.entity_id AND p_initial.key = 'is_initial' \
         LEFT JOIN entity_properties p_terminal ON e.id = p_terminal.entity_id AND p_terminal.key = 'is_terminal' \
         WHERE e.entity_type = 'workflow_status' \
           AND p_scope.value = ?1 \
         ORDER BY e.sort_order, e.id"
    ).map_err(AppError::Db)?;

    let items = stmt.query_map(params![entity_type_scope], |row| {
        let is_initial_str: String = row.get("is_initial")?;
        let is_terminal_str: String = row.get("is_terminal")?;
        Ok(WorkflowStatus {
            id: row.get("id")?,
            entity_type_scope: row.get("entity_type_scope")?,
            status_code: row.get("status_code")?,
            label: row.get("label")?,
            is_initial: is_initial_str == "true",
            is_terminal: is_terminal_str == "true",
        })
    }).map_err(AppError::Db)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::Db)?;

    Ok(items)
}

/// Get the initial status code for an entity type.
pub fn get_initial_status(conn: &Connection, entity_type_scope: &str) -> Result<String, AppError> {
    let statuses = find_statuses_for_type(conn, entity_type_scope)?;
    statuses.into_iter()
        .find(|s| s.is_initial)
        .map(|s| s.status_code)
        .ok_or_else(|| AppError::Session(format!("No initial workflow status for {}", entity_type_scope)))
}

/// Get the label for a status code of a given entity type.
pub fn get_status_label(conn: &Connection, entity_type_scope: &str, status_code: &str) -> Result<String, AppError> {
    let statuses = find_statuses_for_type(conn, entity_type_scope)?;
    statuses.into_iter()
        .find(|s| s.status_code == status_code)
        .map(|s| s.label)
        .ok_or_else(|| AppError::Session(format!("Unknown status '{}' for {}", status_code, entity_type_scope)))
}

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
