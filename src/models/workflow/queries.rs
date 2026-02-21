use sqlx::PgPool;
use crate::errors::AppError;
use crate::auth::session::Permissions;
use crate::models::{entity, relation};
use super::types::*;

// =====================================================================
// Runtime engine (used by suggestion/proposal/agenda handlers)
// =====================================================================

/// Find all available transitions from the current status,
/// filtered by user permissions and entity properties (conditions).
pub async fn find_available_transitions(
    pool: &PgPool,
    entity_type_scope: &str,
    current_status: &str,
    user_permissions: &Permissions,
    entity_properties: &std::collections::HashMap<String, String>,
) -> Result<Vec<AvailableTransition>, AppError> {
    // Find all transitions where transition_from matches current_status and entity_type_scope
    #[derive(sqlx::FromRow)]
    struct TransitionRow {
        required_permission: String,
        condition: Option<String>,
        requires_outcome: String,
        to_status_code: String,
        transition_label: String,
    }

    let all_rows: Vec<TransitionRow> = sqlx::query_as(
        "SELECT COALESCE(p_perm.value, '') AS required_permission, \
                p_cond.value AS condition, \
                COALESCE(p_outcome.value, 'false') AS requires_outcome, \
                COALESCE(p_to_code.value, '') AS to_status_code, \
                t.label AS transition_label \
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
           AND sp_from_code.value = $1 \
           AND sp_from_scope.value = $2"
    )
    .bind(current_status)
    .bind(entity_type_scope)
    .fetch_all(pool)
    .await
    .map_err(AppError::Db)?;

    let all_transitions: Vec<(String, Option<String>, AvailableTransition)> = all_rows.into_iter().map(|r| {
        (
            r.required_permission,
            r.condition,
            AvailableTransition {
                to_status_code: r.to_status_code,
                transition_label: r.transition_label,
                requires_outcome: r.requires_outcome == "true",
            },
        )
    }).collect();

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
pub async fn validate_transition(
    pool: &PgPool,
    entity_type_scope: &str,
    current_status: &str,
    new_status: &str,
    user_permissions: &Permissions,
    entity_properties: &std::collections::HashMap<String, String>,
) -> Result<AvailableTransition, AppError> {
    let available = find_available_transitions(
        pool, entity_type_scope, current_status, user_permissions, entity_properties,
    ).await?;

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
pub async fn list_workflow_scopes(pool: &PgPool) -> Result<Vec<WorkflowScope>, AppError> {
    let scopes: Vec<WorkflowScope> = sqlx::query_as(
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
         GROUP BY p.value, tc.transition_count \
         ORDER BY p.value"
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::Db)?;

    Ok(scopes)
}

/// List all statuses for a given workflow scope, ordered by `order` property.
pub async fn list_statuses_for_scope(pool: &PgPool, scope: &str) -> Result<Vec<WorkflowStatus>, AppError> {
    #[derive(sqlx::FromRow)]
    struct StatusRow {
        id: i64,
        name: String,
        status_code: String,
        status_label: String,
        status_order: String,
        is_initial: String,
        is_terminal: String,
    }

    let rows: Vec<StatusRow> = sqlx::query_as(
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
           AND p_scope.value = $1 \
         ORDER BY CAST(COALESCE(p_order.value, '0') AS BIGINT), e.id"
    )
    .bind(scope)
    .fetch_all(pool)
    .await
    .map_err(AppError::Db)?;

    let statuses = rows.into_iter().map(|r| {
        WorkflowStatus {
            id: r.id,
            name: r.name,
            entity_type_scope: scope.to_string(),
            status_code: r.status_code,
            label: r.status_label,
            order: r.status_order.parse::<i64>().unwrap_or(0),
            is_initial: r.is_initial == "true",
            is_terminal: r.is_terminal == "true",
        }
    }).collect();

    Ok(statuses)
}

/// List all transitions for a given workflow scope, with from/to status info.
pub async fn list_transitions_for_scope(pool: &PgPool, scope: &str) -> Result<Vec<WorkflowTransition>, AppError> {
    #[derive(sqlx::FromRow)]
    struct TransitionRow {
        id: i64,
        name: String,
        transition_label: String,
        from_status_id: i64,
        from_status_code: String,
        to_status_id: i64,
        to_status_code: String,
        required_permission: String,
        condition: Option<String>,
        requires_outcome: String,
    }

    let rows: Vec<TransitionRow> = sqlx::query_as(
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
           AND p_scope.value = $1 \
         ORDER BY from_status_code, to_status_code"
    )
    .bind(scope)
    .fetch_all(pool)
    .await
    .map_err(AppError::Db)?;

    let transitions = rows.into_iter().map(|r| {
        WorkflowTransition {
            id: r.id,
            name: r.name,
            entity_type_scope: scope.to_string(),
            from_status_code: r.from_status_code,
            to_status_code: r.to_status_code,
            from_status_id: r.from_status_id,
            to_status_id: r.to_status_id,
            required_permission: r.required_permission,
            condition: r.condition,
            requires_outcome: r.requires_outcome == "true",
            transition_label: r.transition_label,
        }
    }).collect();

    Ok(transitions)
}

/// Create a new workflow status entity with all required properties and relations.
pub async fn create_status(
    pool: &PgPool,
    scope: &str,
    status_code: &str,
    label: &str,
    order: i64,
    is_initial: bool,
    is_terminal: bool,
) -> Result<i64, AppError> {
    let name = format!("{}.{}", scope, status_code);
    let id = entity::create(pool, "workflow_status", &name, label)
        .await
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
    entity::set_properties(pool, id, &props).await.map_err(AppError::Db)?;

    Ok(id)
}

/// Update an existing workflow status entity's properties.
pub async fn update_status(
    pool: &PgPool,
    id: i64,
    label: &str,
    order: i64,
    is_initial: bool,
    is_terminal: bool,
) -> Result<(), AppError> {
    // Update entity label
    let ent = entity::find_by_id(pool, id).await.map_err(AppError::Db)?
        .ok_or(AppError::NotFound)?;
    entity::update(pool, id, &ent.name, label).await.map_err(AppError::Db)?;

    // Update properties
    entity::set_property(pool, id, "label", label).await.map_err(AppError::Db)?;
    let order_str = order.to_string();
    entity::set_property(pool, id, "order", &order_str).await.map_err(AppError::Db)?;

    if is_initial {
        entity::set_property(pool, id, "is_initial", "true").await.map_err(AppError::Db)?;
    } else {
        entity::delete_property(pool, id, "is_initial").await.map_err(AppError::Db)?;
    }
    if is_terminal {
        entity::set_property(pool, id, "is_terminal", "true").await.map_err(AppError::Db)?;
    } else {
        entity::delete_property(pool, id, "is_terminal").await.map_err(AppError::Db)?;
    }

    Ok(())
}

/// Delete a workflow status. Fails if any transitions reference it.
pub async fn delete_status(pool: &PgPool, id: i64) -> Result<(), AppError> {
    // Check if any transitions point to/from this status via relations
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM relations r \
         JOIN entities rt ON r.relation_type_id = rt.id \
         WHERE (rt.name = 'transition_from' OR rt.name = 'transition_to') \
           AND r.target_id = $1"
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(AppError::Db)?;
    let ref_count = row.0;

    if ref_count > 0 {
        return Err(AppError::PermissionDenied(
            "Cannot delete status: transitions still reference it. Delete the transitions first.".to_string()
        ));
    }

    entity::delete(pool, id).await.map_err(AppError::Db)?;
    Ok(())
}

/// Create a new workflow transition entity with properties and relations.
pub async fn create_transition(
    pool: &PgPool,
    scope: &str,
    from_status_id: i64,
    to_status_id: i64,
    label: &str,
    required_permission: &str,
    requires_outcome: bool,
    condition: &str,
) -> Result<i64, AppError> {
    // Read from/to status codes for the entity name and denormalized properties
    let from_code = entity::get_property(pool, from_status_id, "status_code")
        .await
        .map_err(AppError::Db)?
        .unwrap_or_default();
    let to_code = entity::get_property(pool, to_status_id, "status_code")
        .await
        .map_err(AppError::Db)?
        .unwrap_or_default();

    let name = format!("{}.{}_to_{}", scope, from_code, to_code);
    let id = entity::create(pool, "workflow_transition", &name, label)
        .await
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
    entity::set_properties(pool, id, &props).await.map_err(AppError::Db)?;

    // Create transition_from and transition_to relations
    relation::create(pool, "transition_from", id, from_status_id).await.map_err(AppError::Db)?;
    relation::create(pool, "transition_to", id, to_status_id).await.map_err(AppError::Db)?;

    Ok(id)
}

/// Update an existing workflow transition's properties.
/// Does NOT change from/to status -- delete and recreate for that.
pub async fn update_transition(
    pool: &PgPool,
    id: i64,
    label: &str,
    required_permission: &str,
    requires_outcome: bool,
    condition: &str,
) -> Result<(), AppError> {
    // Update entity label
    let ent = entity::find_by_id(pool, id).await.map_err(AppError::Db)?
        .ok_or(AppError::NotFound)?;
    entity::update(pool, id, &ent.name, label).await.map_err(AppError::Db)?;

    // Update properties
    entity::set_property(pool, id, "transition_label", label).await.map_err(AppError::Db)?;
    entity::set_property(pool, id, "required_permission", required_permission).await.map_err(AppError::Db)?;
    let requires_outcome_str = if requires_outcome { "true" } else { "false" };
    entity::set_property(pool, id, "requires_outcome", requires_outcome_str).await.map_err(AppError::Db)?;

    if condition.is_empty() {
        entity::delete_property(pool, id, "condition").await.map_err(AppError::Db)?;
    } else {
        entity::set_property(pool, id, "condition", condition).await.map_err(AppError::Db)?;
    }

    Ok(())
}

/// Delete a workflow transition and its relations.
pub async fn delete_transition(pool: &PgPool, id: i64) -> Result<(), AppError> {
    // Relations are CASCADE-deleted when the entity is deleted
    entity::delete(pool, id).await.map_err(AppError::Db)?;
    Ok(())
}
