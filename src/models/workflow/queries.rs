use rusqlite::{Connection, params};
use crate::errors::AppError;
use crate::auth::session::Permissions;
use crate::models::{entity, relation};
use super::types::*;

// =====================================================================
// Runtime engine (used by suggestion/proposal/agenda handlers)
// =====================================================================

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

// =====================================================================
// Builder queries (used by workflow builder UI)
// =====================================================================

/// List all distinct workflow scopes with their status and transition counts.
pub fn list_workflow_scopes(conn: &Connection) -> Result<Vec<WorkflowScope>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT p.value AS scope, \
                COUNT(DISTINCT e.id) AS status_count, \
                COALESCE(tc.transition_count, 0) AS transition_count \
         FROM entities e \
         JOIN entity_properties p ON e.id = p.entity_id AND p.key = 'entity_type_scope' \
         LEFT JOIN ( \
             SELECT pt.value AS scope, COUNT(*) AS transition_count \
             FROM entities et \
             JOIN entity_properties pt ON et.id = pt.entity_id AND pt.key = 'entity_type_scope' \
             WHERE et.entity_type = 'workflow_transition' \
             GROUP BY pt.value \
         ) tc ON tc.scope = p.value \
         WHERE e.entity_type = 'workflow_status' \
         GROUP BY p.value \
         ORDER BY p.value"
    ).map_err(AppError::Db)?;

    let scopes = stmt.query_map([], |row| {
        Ok(WorkflowScope {
            scope: row.get("scope")?,
            status_count: row.get("status_count")?,
            transition_count: row.get("transition_count")?,
        })
    }).map_err(AppError::Db)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::Db)?;

    Ok(scopes)
}

/// List all statuses for a given workflow scope, ordered by `order` property.
pub fn list_statuses_for_scope(conn: &Connection, scope: &str) -> Result<Vec<WorkflowStatus>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, \
                COALESCE(p_code.value, '') AS status_code, \
                COALESCE(p_label.value, e.label) AS status_label, \
                COALESCE(p_order.value, '0') AS status_order, \
                COALESCE(p_initial.value, '') AS is_initial, \
                COALESCE(p_terminal.value, '') AS is_terminal \
         FROM entities e \
         JOIN entity_properties p_scope ON e.id = p_scope.entity_id AND p_scope.key = 'entity_type_scope' \
         LEFT JOIN entity_properties p_code ON e.id = p_code.entity_id AND p_code.key = 'status_code' \
         LEFT JOIN entity_properties p_label ON e.id = p_label.entity_id AND p_label.key = 'label' \
         LEFT JOIN entity_properties p_order ON e.id = p_order.entity_id AND p_order.key = 'order' \
         LEFT JOIN entity_properties p_initial ON e.id = p_initial.entity_id AND p_initial.key = 'is_initial' \
         LEFT JOIN entity_properties p_terminal ON e.id = p_terminal.entity_id AND p_terminal.key = 'is_terminal' \
         WHERE e.entity_type = 'workflow_status' \
           AND p_scope.value = ?1 \
         ORDER BY CAST(COALESCE(p_order.value, '0') AS INTEGER), e.id"
    ).map_err(AppError::Db)?;

    let statuses = stmt.query_map(params![scope], |row| {
        let is_initial_str: String = row.get("is_initial")?;
        let is_terminal_str: String = row.get("is_terminal")?;
        let order_str: String = row.get("status_order")?;
        Ok(WorkflowStatus {
            id: row.get("id")?,
            name: row.get("name")?,
            entity_type_scope: scope.to_string(),
            status_code: row.get("status_code")?,
            label: row.get("status_label")?,
            order: order_str.parse::<i64>().unwrap_or(0),
            is_initial: is_initial_str == "true",
            is_terminal: is_terminal_str == "true",
        })
    }).map_err(AppError::Db)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::Db)?;

    Ok(statuses)
}

/// List all transitions for a given workflow scope, with from/to status info.
pub fn list_transitions_for_scope(conn: &Connection, scope: &str) -> Result<Vec<WorkflowTransition>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.label AS transition_label, \
                s_from.id AS from_status_id, \
                COALESCE(p_from_code.value, '') AS from_status_code, \
                s_to.id AS to_status_id, \
                COALESCE(p_to_code.value, '') AS to_status_code, \
                COALESCE(p_perm.value, '') AS required_permission, \
                p_cond.value AS condition, \
                COALESCE(p_outcome.value, 'false') AS requires_outcome \
         FROM entities t \
         JOIN entity_properties p_scope ON t.id = p_scope.entity_id AND p_scope.key = 'entity_type_scope' \
         JOIN relations r_from ON t.id = r_from.source_id \
         JOIN entities rt_from ON r_from.relation_type_id = rt_from.id AND rt_from.name = 'transition_from' \
         JOIN entities s_from ON r_from.target_id = s_from.id \
         JOIN entity_properties p_from_code ON s_from.id = p_from_code.entity_id AND p_from_code.key = 'status_code' \
         JOIN relations r_to ON t.id = r_to.source_id \
         JOIN entities rt_to ON r_to.relation_type_id = rt_to.id AND rt_to.name = 'transition_to' \
         JOIN entities s_to ON r_to.target_id = s_to.id \
         JOIN entity_properties p_to_code ON s_to.id = p_to_code.entity_id AND p_to_code.key = 'status_code' \
         LEFT JOIN entity_properties p_perm ON t.id = p_perm.entity_id AND p_perm.key = 'required_permission' \
         LEFT JOIN entity_properties p_cond ON t.id = p_cond.entity_id AND p_cond.key = 'condition' \
         LEFT JOIN entity_properties p_outcome ON t.id = p_outcome.entity_id AND p_outcome.key = 'requires_outcome' \
         WHERE t.entity_type = 'workflow_transition' \
           AND p_scope.value = ?1 \
         ORDER BY from_status_code, to_status_code"
    ).map_err(AppError::Db)?;

    let transitions = stmt.query_map(params![scope], |row| {
        let requires_outcome_str: String = row.get("requires_outcome")?;
        Ok(WorkflowTransition {
            id: row.get("id")?,
            name: row.get("name")?,
            entity_type_scope: scope.to_string(),
            from_status_code: row.get("from_status_code")?,
            to_status_code: row.get("to_status_code")?,
            from_status_id: row.get("from_status_id")?,
            to_status_id: row.get("to_status_id")?,
            required_permission: row.get("required_permission")?,
            condition: row.get("condition")?,
            requires_outcome: requires_outcome_str == "true",
            transition_label: row.get("transition_label")?,
        })
    }).map_err(AppError::Db)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::Db)?;

    Ok(transitions)
}

/// Create a new workflow status entity with all required properties and relations.
pub fn create_status(
    conn: &Connection,
    scope: &str,
    status_code: &str,
    label: &str,
    order: i64,
    is_initial: bool,
    is_terminal: bool,
) -> Result<i64, AppError> {
    let name = format!("{}.{}", scope, status_code);
    let id = entity::create(conn, "workflow_status", &name, label)
        .map_err(AppError::Db)?;

    let mut props: Vec<(&str, &str)> = vec![
        ("status_code", status_code),
        ("entity_type_scope", scope),
        ("label", label),
    ];
    let order_str = order.to_string();
    props.push(("order", &order_str));
    if is_initial {
        props.push(("is_initial", "true"));
    }
    if is_terminal {
        props.push(("is_terminal", "true"));
    }
    entity::set_properties(conn, id, &props).map_err(AppError::Db)?;

    Ok(id)
}

/// Update an existing workflow status entity's properties.
pub fn update_status(
    conn: &Connection,
    id: i64,
    label: &str,
    order: i64,
    is_initial: bool,
    is_terminal: bool,
) -> Result<(), AppError> {
    // Update entity label
    let ent = entity::find_by_id(conn, id).map_err(AppError::Db)?
        .ok_or(AppError::NotFound)?;
    entity::update(conn, id, &ent.name, label).map_err(AppError::Db)?;

    // Update properties
    entity::set_property(conn, id, "label", label).map_err(AppError::Db)?;
    let order_str = order.to_string();
    entity::set_property(conn, id, "order", &order_str).map_err(AppError::Db)?;

    if is_initial {
        entity::set_property(conn, id, "is_initial", "true").map_err(AppError::Db)?;
    } else {
        entity::delete_property(conn, id, "is_initial").map_err(AppError::Db)?;
    }
    if is_terminal {
        entity::set_property(conn, id, "is_terminal", "true").map_err(AppError::Db)?;
    } else {
        entity::delete_property(conn, id, "is_terminal").map_err(AppError::Db)?;
    }

    Ok(())
}

/// Delete a workflow status. Fails if any transitions reference it.
pub fn delete_status(conn: &Connection, id: i64) -> Result<(), AppError> {
    // Check if any transitions point to/from this status via relations
    let ref_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM relations r \
         JOIN entities rt ON r.relation_type_id = rt.id \
         WHERE (rt.name = 'transition_from' OR rt.name = 'transition_to') \
           AND r.target_id = ?1",
        params![id],
        |row| row.get(0),
    ).map_err(AppError::Db)?;

    if ref_count > 0 {
        return Err(AppError::PermissionDenied(
            "Cannot delete status: transitions still reference it. Delete the transitions first.".to_string()
        ));
    }

    entity::delete(conn, id).map_err(AppError::Db)?;
    Ok(())
}

/// Create a new workflow transition entity with properties and relations.
pub fn create_transition(
    conn: &Connection,
    scope: &str,
    from_status_id: i64,
    to_status_id: i64,
    label: &str,
    required_permission: &str,
    requires_outcome: bool,
    condition: &str,
) -> Result<i64, AppError> {
    // Read from/to status codes for the entity name and denormalized properties
    let from_code = entity::get_property(conn, from_status_id, "status_code")
        .map_err(AppError::Db)?
        .unwrap_or_default();
    let to_code = entity::get_property(conn, to_status_id, "status_code")
        .map_err(AppError::Db)?
        .unwrap_or_default();

    let name = format!("{}.{}_to_{}", scope, from_code, to_code);
    let id = entity::create(conn, "workflow_transition", &name, label)
        .map_err(AppError::Db)?;

    // Set properties (both canonical and denormalized)
    let requires_outcome_str = if requires_outcome { "true" } else { "false" };
    let mut props: Vec<(&str, &str)> = vec![
        ("entity_type_scope", scope),
        ("from_status_code", &from_code),
        ("to_status_code", &to_code),
        ("transition_label", label),
        ("required_permission", required_permission),
        ("requires_outcome", requires_outcome_str),
    ];
    if !condition.is_empty() {
        props.push(("condition", condition));
    }
    entity::set_properties(conn, id, &props).map_err(AppError::Db)?;

    // Create transition_from and transition_to relations
    relation::create(conn, "transition_from", id, from_status_id).map_err(AppError::Db)?;
    relation::create(conn, "transition_to", id, to_status_id).map_err(AppError::Db)?;

    Ok(id)
}

/// Update an existing workflow transition's properties.
/// Does NOT change from/to status — delete and recreate for that.
pub fn update_transition(
    conn: &Connection,
    id: i64,
    label: &str,
    required_permission: &str,
    requires_outcome: bool,
    condition: &str,
) -> Result<(), AppError> {
    // Update entity label
    let ent = entity::find_by_id(conn, id).map_err(AppError::Db)?
        .ok_or(AppError::NotFound)?;
    entity::update(conn, id, &ent.name, label).map_err(AppError::Db)?;

    // Update properties
    entity::set_property(conn, id, "transition_label", label).map_err(AppError::Db)?;
    entity::set_property(conn, id, "required_permission", required_permission).map_err(AppError::Db)?;
    let requires_outcome_str = if requires_outcome { "true" } else { "false" };
    entity::set_property(conn, id, "requires_outcome", requires_outcome_str).map_err(AppError::Db)?;

    if condition.is_empty() {
        entity::delete_property(conn, id, "condition").map_err(AppError::Db)?;
    } else {
        entity::set_property(conn, id, "condition", condition).map_err(AppError::Db)?;
    }

    Ok(())
}

/// Delete a workflow transition and its relations.
pub fn delete_transition(conn: &Connection, id: i64) -> Result<(), AppError> {
    // Relations are CASCADE-deleted when the entity is deleted
    entity::delete(conn, id).map_err(AppError::Db)?;
    Ok(())
}
