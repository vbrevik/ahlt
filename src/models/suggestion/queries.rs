use sqlx::PgPool;
use crate::errors::AppError;
use crate::models::{entity, relation};
use super::types::*;

/// Truncate a string to `max_len` chars, appending "..." if truncated.
fn make_preview(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}

/// Find all suggestions related to a given ToR via the `suggested_to` relation.
pub async fn find_all_for_tor(pool: &PgPool, tor_id: i64) -> Result<Vec<SuggestionListItem>, AppError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: i64,
        description: String,
        submitted_date: String,
        status: String,
        submitted_by_id: String,
        submitted_by_name: String,
        rejection_reason: Option<String>,
        spawned_proposal_id: Option<i64>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT e.id, \
                COALESCE(p_desc.value, '') AS description, \
                COALESCE(p_date.value, '') AS submitted_date, \
                COALESCE(p_status.value, 'open') AS status, \
                COALESCE(p_by.value, '0') AS submitted_by_id, \
                COALESCE(u.label, '') AS submitted_by_name, \
                p_reason.value AS rejection_reason, \
                r_spawn.target_id AS spawned_proposal_id \
         FROM entities e \
         JOIN relations r ON e.id = r.source_id \
         JOIN entities rt ON r.relation_type_id = rt.id AND rt.name = 'suggested_to' \
         LEFT JOIN entity_properties p_desc \
             ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
         LEFT JOIN entity_properties p_date \
             ON e.id = p_date.entity_id AND p_date.key = 'submitted_date' \
         LEFT JOIN entity_properties p_status \
             ON e.id = p_status.entity_id AND p_status.key = 'status' \
         LEFT JOIN entity_properties p_by \
             ON e.id = p_by.entity_id AND p_by.key = 'submitted_by_id' \
         LEFT JOIN entities u \
             ON CAST(p_by.value AS BIGINT) = u.id \
         LEFT JOIN entity_properties p_reason \
             ON e.id = p_reason.entity_id AND p_reason.key = 'rejection_reason' \
         LEFT JOIN relations r_spawn \
             ON e.id = r_spawn.source_id \
            AND r_spawn.relation_type_id = ( \
                SELECT id FROM entities \
                WHERE entity_type = 'relation_type' AND name = 'spawns_proposal') \
         WHERE e.entity_type = 'suggestion' AND r.target_id = $1 \
         ORDER BY submitted_date DESC",
    )
    .bind(tor_id)
    .fetch_all(pool)
    .await?;

    let items = rows
        .into_iter()
        .map(|row| {
            let submitted_by_id: i64 = row.submitted_by_id.parse().unwrap_or(0);
            SuggestionListItem {
                id: row.id,
                description_preview: make_preview(&row.description, 100),
                description: row.description,
                submitted_by_id,
                submitted_by_name: row.submitted_by_name,
                submitted_date: row.submitted_date,
                status: row.status,
                rejection_reason: row.rejection_reason,
                spawned_proposal_id: row.spawned_proposal_id,
            }
        })
        .collect();

    Ok(items)
}

/// Find all suggestions across all ToRs (or filtered to ToRs a user fills a position in).
///
/// `user_id = None`  -> returns every suggestion across all ToRs.
/// `user_id = Some(id)` -> returns only suggestions for ToRs the user fills a position in.
pub async fn find_all_cross_tor(pool: &PgPool, user_id: Option<i64>) -> Result<Vec<CrossTorSuggestionItem>, AppError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        tor_id: i64,
        tor_name: String,
        id: i64,
        description: String,
        submitted_date: String,
        status: String,
        submitted_by_id: String,
        submitted_by_name: String,
        rejection_reason: Option<String>,
        spawned_proposal_id: Option<i64>,
    }

    let base_sql = "SELECT tor.id AS tor_id, tor.label AS tor_name, e.id, \
                           COALESCE(p_desc.value, '') AS description, \
                           COALESCE(p_date.value, '') AS submitted_date, \
                           COALESCE(p_status.value, 'open') AS status, \
                           COALESCE(p_by.value, '0') AS submitted_by_id, \
                           COALESCE(u.label, '') AS submitted_by_name, \
                           p_reason.value AS rejection_reason, \
                           r_spawn.target_id AS spawned_proposal_id \
                    FROM entities e \
                    JOIN relations r ON e.id = r.source_id \
                    JOIN entities rt ON r.relation_type_id = rt.id AND rt.name = 'suggested_to' \
                    JOIN entities tor ON tor.id = r.target_id AND tor.entity_type = 'tor' \
                    LEFT JOIN entity_properties p_desc \
                        ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
                    LEFT JOIN entity_properties p_date \
                        ON e.id = p_date.entity_id AND p_date.key = 'submitted_date' \
                    LEFT JOIN entity_properties p_status \
                        ON e.id = p_status.entity_id AND p_status.key = 'status' \
                    LEFT JOIN entity_properties p_by \
                        ON e.id = p_by.entity_id AND p_by.key = 'submitted_by_id' \
                    LEFT JOIN entities u \
                        ON CAST(p_by.value AS BIGINT) = u.id \
                    LEFT JOIN entity_properties p_reason \
                        ON e.id = p_reason.entity_id AND p_reason.key = 'rejection_reason' \
                    LEFT JOIN relations r_spawn \
                        ON e.id = r_spawn.source_id \
                       AND r_spawn.relation_type_id = ( \
                           SELECT id FROM entities \
                           WHERE entity_type = 'relation_type' AND name = 'spawns_proposal') \
                    WHERE e.entity_type = 'suggestion'";

    let rows = if let Some(uid) = user_id {
        let sql = format!(
            "{} AND EXISTS (\
                SELECT 1 FROM relations r_fills \
                JOIN relations r_tor ON r_fills.target_id = r_tor.source_id \
                WHERE r_fills.source_id = $1 \
                  AND r_tor.target_id = tor.id \
                  AND r_fills.relation_type_id = (\
                      SELECT id FROM entities \
                      WHERE entity_type = 'relation_type' AND name = 'fills_position') \
                  AND r_tor.relation_type_id = (\
                      SELECT id FROM entities \
                      WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor')\
            ) ORDER BY tor.label ASC, submitted_date DESC",
            base_sql
        );
        sqlx::query_as::<_, Row>(&sql)
            .bind(uid)
            .fetch_all(pool)
            .await?
    } else {
        let sql = format!("{} ORDER BY tor.label ASC, submitted_date DESC", base_sql);
        sqlx::query_as::<_, Row>(&sql)
            .fetch_all(pool)
            .await?
    };

    let items = rows
        .into_iter()
        .map(|row| {
            let submitted_by_id: i64 = row.submitted_by_id.parse().unwrap_or(0);
            CrossTorSuggestionItem {
                tor_id: row.tor_id,
                tor_name: row.tor_name,
                id: row.id,
                description_preview: make_preview(&row.description, 100),
                description: row.description,
                submitted_by_id,
                submitted_by_name: row.submitted_by_name,
                submitted_date: row.submitted_date,
                status: row.status,
                rejection_reason: row.rejection_reason,
                spawned_proposal_id: row.spawned_proposal_id,
            }
        })
        .collect();

    Ok(items)
}

/// Find a single suggestion by its entity id.
pub async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<SuggestionDetail>, AppError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: i64,
        description: String,
        submitted_date: String,
        status: String,
        submitted_by_id: String,
        submitted_by_name: String,
        rejection_reason: Option<String>,
        spawned_proposal_id: Option<i64>,
    }

    let row = sqlx::query_as::<_, Row>(
        "SELECT e.id, \
                COALESCE(p_desc.value, '') AS description, \
                COALESCE(p_date.value, '') AS submitted_date, \
                COALESCE(p_status.value, 'open') AS status, \
                COALESCE(p_by.value, '0') AS submitted_by_id, \
                COALESCE(u.label, '') AS submitted_by_name, \
                p_reason.value AS rejection_reason, \
                r_spawn.target_id AS spawned_proposal_id \
         FROM entities e \
         LEFT JOIN entity_properties p_desc \
             ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
         LEFT JOIN entity_properties p_date \
             ON e.id = p_date.entity_id AND p_date.key = 'submitted_date' \
         LEFT JOIN entity_properties p_status \
             ON e.id = p_status.entity_id AND p_status.key = 'status' \
         LEFT JOIN entity_properties p_by \
             ON e.id = p_by.entity_id AND p_by.key = 'submitted_by_id' \
         LEFT JOIN entities u \
             ON CAST(p_by.value AS BIGINT) = u.id \
         LEFT JOIN entity_properties p_reason \
             ON e.id = p_reason.entity_id AND p_reason.key = 'rejection_reason' \
         LEFT JOIN relations r_spawn \
             ON e.id = r_spawn.source_id \
            AND r_spawn.relation_type_id = ( \
                SELECT id FROM entities \
                WHERE entity_type = 'relation_type' AND name = 'spawns_proposal') \
         WHERE e.id = $1 AND e.entity_type = 'suggestion'",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| {
        let submitted_by_id: i64 = r.submitted_by_id.parse().unwrap_or(0);
        SuggestionDetail {
            id: r.id,
            description: r.description,
            submitted_by_id,
            submitted_by_name: r.submitted_by_name,
            submitted_date: r.submitted_date,
            status: r.status,
            rejection_reason: r.rejection_reason,
            spawned_proposal_id: r.spawned_proposal_id,
        }
    }))
}

/// Create a new suggestion entity linked to a ToR via `suggested_to`.
/// Returns the new entity id.
pub async fn create(
    pool: &PgPool,
    tor_id: i64,
    description: &str,
    submitted_by_id: i64,
    submitted_date: &str,
) -> Result<i64, AppError> {
    let name = format!("suggestion_{}_{}", submitted_date.replace('-', "_"), tor_id);
    let label = make_preview(description, 50);

    let suggestion_id = entity::create(pool, "suggestion", &name, &label).await?;

    entity::set_property(pool, suggestion_id, "description", description).await?;
    entity::set_property(pool, suggestion_id, "submitted_date", submitted_date).await?;
    entity::set_property(pool, suggestion_id, "status", "open").await?;
    entity::set_property(pool, suggestion_id, "submitted_by_id", &submitted_by_id.to_string()).await?;

    relation::create(pool, "suggested_to", suggestion_id, tor_id).await?;

    Ok(suggestion_id)
}

/// Update the status of a suggestion (e.g. open -> accepted or rejected).
pub async fn update_status(
    pool: &PgPool,
    suggestion_id: i64,
    new_status: &str,
    rejection_reason: Option<&str>,
) -> Result<(), AppError> {
    entity::set_property(pool, suggestion_id, "status", new_status).await?;

    if let Some(reason) = rejection_reason {
        entity::set_property(pool, suggestion_id, "rejection_reason", reason).await?;
    }

    Ok(())
}
