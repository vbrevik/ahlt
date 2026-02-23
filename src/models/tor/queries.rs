use sqlx::PgPool;
use crate::errors::AppError;
use super::types::*;

pub async fn find_all_list_items(pool: &PgPool) -> Result<Vec<TorListItem>, sqlx::Error> {
    let items = sqlx::query_as::<_, TorListItem>(
        "SELECT e.id, e.name, e.label, \
                COALESCE(p_desc.value, '') AS description, \
                COALESCE(p_status.value, 'active') AS status, \
                COALESCE(p_cadence.value, 'ad-hoc') AS meeting_cadence, \
                (SELECT COUNT(DISTINCT r_fills.source_id) \
                 FROM relations r_tor \
                 JOIN relations r_fills ON r_tor.source_id = r_fills.target_id \
                 WHERE r_tor.target_id = e.id \
                   AND r_tor.relation_type_id = (\
                       SELECT id FROM entities \
                       WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
                   AND r_fills.relation_type_id = (\
                       SELECT id FROM entities \
                       WHERE entity_type = 'relation_type' AND name = 'fills_position') \
                ) AS member_count, \
                (SELECT COUNT(*) FROM relations r_func \
                 WHERE r_func.target_id = e.id \
                   AND r_func.relation_type_id = (\
                       SELECT id FROM entities \
                       WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
                ) AS function_count \
         FROM entities e \
         LEFT JOIN entity_properties p_desc \
             ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
         LEFT JOIN entity_properties p_status \
             ON e.id = p_status.entity_id AND p_status.key = 'status' \
         LEFT JOIN entity_properties p_cadence \
             ON e.id = p_cadence.entity_id AND p_cadence.key = 'meeting_cadence' \
         WHERE e.entity_type = 'tor' \
         ORDER BY e.sort_order, e.id",
    )
    .fetch_all(pool)
    .await?;

    Ok(items)
}

pub async fn find_detail_by_id(pool: &PgPool, id: i64) -> Result<Option<TorDetail>, sqlx::Error> {
    let detail = sqlx::query_as::<_, TorDetail>(
        "SELECT e.id, e.name, e.label, \
                COALESCE(p_desc.value, '') AS description, \
                COALESCE(p_status.value, 'active') AS status, \
                COALESCE(p_cadence.value, 'ad-hoc') AS meeting_cadence, \
                COALESCE(p_day.value, '') AS cadence_day, \
                COALESCE(p_time.value, '') AS cadence_time, \
                COALESCE(p_dur.value, '60') AS cadence_duration_minutes, \
                COALESCE(p_loc.value, '') AS default_location, \
                COALESCE(p_remote.value, '') AS remote_url, \
                COALESCE(p_repo.value, '') AS background_repo_url, \
                COALESCE(p_tornum.value, '') AS tor_number, \
                COALESCE(p_class.value, '') AS classification, \
                COALESCE(p_ver.value, '') AS version, \
                COALESCE(p_org.value, '') AS organization, \
                COALESCE(p_scope.value, '') AS focus_scope, \
                COALESCE(p_obj.value, '[]') AS objectives, \
                COALESCE(p_inp.value, '[]') AS inputs_required, \
                COALESCE(p_out.value, '[]') AS outputs_expected, \
                COALESCE(p_poc.value, '') AS poc_contact, \
                COALESCE(p_phase.value, '') AS phase_scheduling, \
                COALESCE(p_infop.value, '') AS info_platform, \
                COALESCE(p_invite.value, '') AS invite_policy \
         FROM entities e \
         LEFT JOIN entity_properties p_desc \
             ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
         LEFT JOIN entity_properties p_status \
             ON e.id = p_status.entity_id AND p_status.key = 'status' \
         LEFT JOIN entity_properties p_cadence \
             ON e.id = p_cadence.entity_id AND p_cadence.key = 'meeting_cadence' \
         LEFT JOIN entity_properties p_day \
             ON e.id = p_day.entity_id AND p_day.key = 'cadence_day' \
         LEFT JOIN entity_properties p_time \
             ON e.id = p_time.entity_id AND p_time.key = 'cadence_time' \
         LEFT JOIN entity_properties p_dur \
             ON e.id = p_dur.entity_id AND p_dur.key = 'cadence_duration_minutes' \
         LEFT JOIN entity_properties p_loc \
             ON e.id = p_loc.entity_id AND p_loc.key = 'default_location' \
         LEFT JOIN entity_properties p_remote \
             ON e.id = p_remote.entity_id AND p_remote.key = 'remote_url' \
         LEFT JOIN entity_properties p_repo \
             ON e.id = p_repo.entity_id AND p_repo.key = 'background_repo_url' \
         LEFT JOIN entity_properties p_tornum \
             ON e.id = p_tornum.entity_id AND p_tornum.key = 'tor_number' \
         LEFT JOIN entity_properties p_class \
             ON e.id = p_class.entity_id AND p_class.key = 'classification' \
         LEFT JOIN entity_properties p_ver \
             ON e.id = p_ver.entity_id AND p_ver.key = 'version' \
         LEFT JOIN entity_properties p_org \
             ON e.id = p_org.entity_id AND p_org.key = 'organization' \
         LEFT JOIN entity_properties p_scope \
             ON e.id = p_scope.entity_id AND p_scope.key = 'focus_scope' \
         LEFT JOIN entity_properties p_obj \
             ON e.id = p_obj.entity_id AND p_obj.key = 'objectives' \
         LEFT JOIN entity_properties p_inp \
             ON e.id = p_inp.entity_id AND p_inp.key = 'inputs_required' \
         LEFT JOIN entity_properties p_out \
             ON e.id = p_out.entity_id AND p_out.key = 'outputs_expected' \
         LEFT JOIN entity_properties p_poc \
             ON e.id = p_poc.entity_id AND p_poc.key = 'poc_contact' \
         LEFT JOIN entity_properties p_phase \
             ON e.id = p_phase.entity_id AND p_phase.key = 'phase_scheduling' \
         LEFT JOIN entity_properties p_infop \
             ON e.id = p_infop.entity_id AND p_infop.key = 'info_platform' \
         LEFT JOIN entity_properties p_invite \
             ON e.id = p_invite.entity_id AND p_invite.key = 'invite_policy' \
         WHERE e.id = $1 AND e.entity_type = 'tor'",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(detail)
}

pub async fn create(
    pool: &PgPool,
    name: &str,
    label: &str,
    props: &[(&str, &str)],
) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO entities (entity_type, name, label) VALUES ('tor', $1, $2) RETURNING id",
    )
    .bind(name)
    .bind(label)
    .fetch_one(pool)
    .await?;
    let tor_id = row.0;

    for &(key, value) in props {
        if !value.is_empty() {
            sqlx::query(
                "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, $2, $3)",
            )
            .bind(tor_id)
            .bind(key)
            .bind(value)
            .execute(pool)
            .await?;
        }
    }

    Ok(tor_id)
}

pub async fn update(
    pool: &PgPool,
    id: i64,
    name: &str,
    label: &str,
    props: &[(&str, &str)],
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE entities SET name = $1, label = $2, updated_at = NOW() \
         WHERE id = $3",
    )
    .bind(name)
    .bind(label)
    .bind(id)
    .execute(pool)
    .await?;

    for &(key, value) in props {
        sqlx::query(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, $2, $3) \
             ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value",
        )
        .bind(id)
        .bind(key)
        .bind(value)
        .execute(pool)
        .await?;
    }

    Ok(())
}

pub async fn delete(pool: &PgPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM entities WHERE id = $1 AND entity_type = 'tor'",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Find all positions in a ToR with their current holders.
/// Returns positions even when vacant (holder fields will be None).
pub async fn find_members(pool: &PgPool, tor_id: i64) -> Result<Vec<TorMember>, sqlx::Error> {
    let members = sqlx::query_as::<_, TorMember>(
        "SELECT f.id AS position_id, f.name AS position_name, f.label AS position_label, \
                COALESCE(p_mt.value, 'optional') AS membership_type, \
                u.id AS holder_id, u.name AS holder_name, u.label AS holder_label \
         FROM entities f \
         JOIN relations r_tor ON f.id = r_tor.source_id \
         LEFT JOIN relations r_fills ON f.id = r_fills.target_id \
             AND r_fills.relation_type_id = ( \
                 SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position') \
         LEFT JOIN entities u ON r_fills.source_id = u.id AND u.entity_type = 'user' \
         LEFT JOIN entity_properties p_mt ON f.id = p_mt.entity_id AND p_mt.key = 'membership_type' \
         WHERE r_tor.target_id = $1 \
           AND r_tor.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
           AND f.entity_type = 'tor_function' \
         ORDER BY CASE WHEN COALESCE(p_mt.value, 'optional') = 'mandatory' THEN 0 ELSE 1 END, f.label",
    )
    .bind(tor_id)
    .fetch_all(pool)
    .await?;

    Ok(members)
}

/// Assign a user to a position (creates fills_position relation).
pub async fn assign_to_position(
    pool: &PgPool,
    user_id: i64,
    position_id: i64,
    membership_type: &str,
) -> Result<(), sqlx::Error> {
    // Set the membership_type property on the position
    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'membership_type', $2) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value",
    )
    .bind(position_id)
    .bind(membership_type)
    .execute(pool)
    .await?;

    // Create fills_position relation
    sqlx::query(
        "INSERT INTO relations (relation_type_id, source_id, target_id) \
         VALUES ( \
             (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position'), \
             $1, $2) \
         ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(position_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Remove the current holder from a position.
pub async fn vacate_position(pool: &PgPool, position_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM relations WHERE target_id = $1 \
         AND relation_type_id = ( \
             SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position')",
    )
    .bind(position_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_functions(
    pool: &PgPool,
    tor_id: i64,
) -> Result<Vec<TorFunctionListItem>, sqlx::Error> {
    let functions: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT f.id, f.name, f.label, \
                COALESCE(p_cat.value, '') AS category \
         FROM relations r \
         JOIN entities f ON r.source_id = f.id \
         LEFT JOIN entity_properties p_cat \
             ON f.id = p_cat.entity_id AND p_cat.key = 'category' \
         WHERE r.target_id = $1 \
           AND r.relation_type_id = (\
               SELECT id FROM entities \
               WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
           AND f.entity_type = 'tor_function' \
         ORDER BY f.sort_order, f.id",
    )
    .bind(tor_id)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for (id, name, label, category) in functions {
        let assigned_rows: Vec<(String,)> = sqlx::query_as(
            "SELECT u.label \
             FROM relations r \
             JOIN entities u ON r.source_id = u.id \
             WHERE r.target_id = $1 \
               AND r.relation_type_id = ( \
                   SELECT id FROM entities \
                   WHERE entity_type = 'relation_type' AND name = 'fills_position') \
             ORDER BY u.label",
        )
        .bind(id)
        .fetch_all(pool)
        .await?;

        let assigned_to: Vec<String> = assigned_rows.into_iter().map(|r| r.0).collect();

        result.push(TorFunctionListItem {
            id,
            name,
            label,
            category,
            assigned_to,
        });
    }

    Ok(result)
}

/// Count positions with holders in a ToR (not vacant positions).
pub async fn count_members(pool: &PgPool, tor_id: i64) -> Result<i64, sqlx::Error> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT r_fills.source_id) \
         FROM entities f \
         JOIN relations r_tor ON f.id = r_tor.source_id \
         JOIN relations r_fills ON f.id = r_fills.target_id \
         WHERE r_tor.target_id = $1 \
           AND r_tor.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
           AND r_fills.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position') \
           AND f.entity_type = 'tor_function'",
    )
    .bind(tor_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Find users not currently filling any position in this ToR.
pub async fn find_non_members(
    pool: &PgPool,
    tor_id: i64,
) -> Result<Vec<(i64, String, String)>, sqlx::Error> {
    let users: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT e.id, e.name, e.label \
         FROM entities e \
         WHERE e.entity_type = 'user' \
           AND e.is_active = true \
           AND e.id NOT IN ( \
               SELECT r_fills.source_id \
               FROM relations r_fills \
               JOIN relations r_tor ON r_fills.target_id = r_tor.source_id \
               WHERE r_tor.target_id = $1 \
                 AND r_tor.relation_type_id = ( \
                     SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
                 AND r_fills.relation_type_id = ( \
                     SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position')) \
         ORDER BY e.label",
    )
    .bind(tor_id)
    .fetch_all(pool)
    .await?;

    Ok(users)
}

/// Verify user fills a position in the given ToR. Returns AppError::PermissionDenied if not.
pub async fn require_tor_membership(
    pool: &PgPool,
    user_id: i64,
    tor_id: i64,
) -> Result<(), AppError> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) \
         FROM relations r_fills \
         JOIN relations r_tor ON r_fills.target_id = r_tor.source_id \
         WHERE r_fills.source_id = $1 \
           AND r_tor.target_id = $2 \
           AND r_fills.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position') \
           AND r_tor.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor')",
    )
    .bind(user_id)
    .bind(tor_id)
    .fetch_one(pool)
    .await?;

    if row.0 == 0 {
        return Err(AppError::PermissionDenied("Not a member of this ToR".into()));
    }
    Ok(())
}

/// Find all ToRs where the given user fills a position.
/// Chain: user --(fills_position)--> tor_function --(belongs_to_tor)--> tor
pub async fn find_user_tors(pool: &PgPool, user_id: i64) -> Vec<UserTorMembership> {
    sqlx::query_as::<_, UserTorMembership>(
        "SELECT DISTINCT tor.id AS tor_id, tor.name AS tor_name, tor.label AS tor_label, \
                f.label AS position_label \
         FROM entities tor \
         JOIN relations r_tor ON tor.id = r_tor.target_id \
         JOIN entities f ON r_tor.source_id = f.id \
         JOIN relations r_fills ON f.id = r_fills.target_id \
         WHERE tor.entity_type = 'tor' \
           AND f.entity_type = 'tor_function' \
           AND r_tor.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor') \
           AND r_fills.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position') \
           AND r_fills.source_id = $1 \
         ORDER BY tor.label"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

/// Returns just the ToR IDs for a user (lightweight version of find_user_tors).
pub async fn find_tor_ids_for_user(pool: &PgPool, user_id: i64) -> Vec<i64> {
    find_user_tors(pool, user_id).await.into_iter().map(|t| t.tor_id).collect()
}

/// Get a ToR's display name (label) by ID.
pub async fn get_tor_name(pool: &PgPool, tor_id: i64) -> Result<String, AppError> {
    let row: (String,) = sqlx::query_as(
        "SELECT label FROM entities WHERE id = $1 AND entity_type = 'tor'",
    )
    .bind(tor_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}
