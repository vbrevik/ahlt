use sqlx::PgPool;
use crate::errors::AppError;
use crate::models::{entity, relation};
use super::types::*;

/// Count proposals with a given status (e.g. "draft", "submitted", "approved").
pub async fn count_by_status(pool: &PgPool, status: &str) -> i64 {
    let result: Result<(i64,), _> = sqlx::query_as(
        "SELECT COUNT(*) FROM entities e
         JOIN entity_properties p ON e.id = p.entity_id AND p.key = 'status'
         WHERE e.entity_type = 'proposal' AND p.value = $1",
    )
    .bind(status)
    .fetch_one(pool)
    .await;

    result.map(|r| r.0).unwrap_or(0)
}

/// Generate a slug-style name from a title: lowercase, spaces to underscores,
/// keep only alphanumeric and underscores.
fn name_from_title(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c == ' ' { '_' } else { c })
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// Find all proposals linked to a ToR via the `submitted_to` relation.
pub async fn find_all_for_tor(pool: &PgPool, tor_id: i64) -> Result<Vec<ProposalListItem>, AppError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: i64,
        title: String,
        submitted_date: String,
        status: String,
        submitted_by_id: String,
        submitted_by_name: String,
        rejection_reason: Option<String>,
        related_suggestion_id: Option<i64>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT e.id, \
                COALESCE(p_title.value, '') AS title, \
                COALESCE(p_date.value, '') AS submitted_date, \
                COALESCE(p_status.value, 'draft') AS status, \
                COALESCE(p_by.value, '0') AS submitted_by_id, \
                COALESCE(u.label, '') AS submitted_by_name, \
                p_reason.value AS rejection_reason, \
                r_spawn.source_id AS related_suggestion_id \
         FROM entities e \
         JOIN relations r ON e.id = r.source_id \
         JOIN entities rt ON r.relation_type_id = rt.id AND rt.name = 'submitted_to' \
         LEFT JOIN entity_properties p_title \
             ON e.id = p_title.entity_id AND p_title.key = 'title' \
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
             ON e.id = r_spawn.target_id \
            AND r_spawn.relation_type_id = ( \
                SELECT id FROM entities \
                WHERE entity_type = 'relation_type' AND name = 'spawns_proposal') \
         WHERE e.entity_type = 'proposal' AND r.target_id = $1 \
         ORDER BY submitted_date DESC",
    )
    .bind(tor_id)
    .fetch_all(pool)
    .await?;

    let items = rows
        .into_iter()
        .map(|row| {
            let submitted_by_id: i64 = row.submitted_by_id.parse().unwrap_or(0);
            ProposalListItem {
                id: row.id,
                title: row.title,
                submitted_by_id,
                submitted_by_name: row.submitted_by_name,
                submitted_date: row.submitted_date,
                status: row.status,
                rejection_reason: row.rejection_reason,
                related_suggestion_id: row.related_suggestion_id,
            }
        })
        .collect();

    Ok(items)
}

/// Find all proposals across all ToRs (or filtered to ToRs a user fills a position in).
///
/// `user_id = None`  -> returns every proposal across all ToRs.
/// `user_id = Some(id)` -> returns only proposals for ToRs the user fills a position in.
pub async fn find_all_cross_tor(pool: &PgPool, user_id: Option<i64>) -> Result<Vec<CrossTorProposalItem>, AppError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        tor_id: i64,
        tor_name: String,
        id: i64,
        title: String,
        submitted_date: String,
        status: String,
        submitted_by_id: String,
        submitted_by_name: String,
        rejection_reason: Option<String>,
        related_suggestion_id: Option<i64>,
    }

    let base_sql = "SELECT tor.id AS tor_id, tor.label AS tor_name, e.id, \
                           COALESCE(p_title.value, '') AS title, \
                           COALESCE(p_date.value, '') AS submitted_date, \
                           COALESCE(p_status.value, 'draft') AS status, \
                           COALESCE(p_by.value, '0') AS submitted_by_id, \
                           COALESCE(u.label, '') AS submitted_by_name, \
                           p_reason.value AS rejection_reason, \
                           r_spawn.source_id AS related_suggestion_id \
                    FROM entities e \
                    JOIN relations r ON e.id = r.source_id \
                    JOIN entities rt ON r.relation_type_id = rt.id AND rt.name = 'submitted_to' \
                    JOIN entities tor ON tor.id = r.target_id AND tor.entity_type = 'tor' \
                    LEFT JOIN entity_properties p_title \
                        ON e.id = p_title.entity_id AND p_title.key = 'title' \
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
                        ON e.id = r_spawn.target_id \
                       AND r_spawn.relation_type_id = ( \
                           SELECT id FROM entities \
                           WHERE entity_type = 'relation_type' AND name = 'spawns_proposal') \
                    WHERE e.entity_type = 'proposal'";

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
            CrossTorProposalItem {
                tor_id: row.tor_id,
                tor_name: row.tor_name,
                id: row.id,
                title: row.title,
                submitted_by_id,
                submitted_by_name: row.submitted_by_name,
                submitted_date: row.submitted_date,
                status: row.status,
                rejection_reason: row.rejection_reason,
                related_suggestion_id: row.related_suggestion_id,
            }
        })
        .collect();

    Ok(items)
}

/// Find a single proposal by its entity id.
pub async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<ProposalDetail>, AppError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: i64,
        title: String,
        description: String,
        rationale: String,
        submitted_date: String,
        status: String,
        submitted_by_id: String,
        submitted_by_name: String,
        rejection_reason: Option<String>,
        related_suggestion_id: Option<i64>,
    }

    let row = sqlx::query_as::<_, Row>(
        "SELECT e.id, \
                COALESCE(p_title.value, '') AS title, \
                COALESCE(p_desc.value, '') AS description, \
                COALESCE(p_rat.value, '') AS rationale, \
                COALESCE(p_date.value, '') AS submitted_date, \
                COALESCE(p_status.value, 'draft') AS status, \
                COALESCE(p_by.value, '0') AS submitted_by_id, \
                COALESCE(u.label, '') AS submitted_by_name, \
                p_reason.value AS rejection_reason, \
                r_spawn.source_id AS related_suggestion_id \
         FROM entities e \
         LEFT JOIN entity_properties p_title \
             ON e.id = p_title.entity_id AND p_title.key = 'title' \
         LEFT JOIN entity_properties p_desc \
             ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
         LEFT JOIN entity_properties p_rat \
             ON e.id = p_rat.entity_id AND p_rat.key = 'rationale' \
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
             ON e.id = r_spawn.target_id \
            AND r_spawn.relation_type_id = ( \
                SELECT id FROM entities \
                WHERE entity_type = 'relation_type' AND name = 'spawns_proposal') \
         WHERE e.id = $1 AND e.entity_type = 'proposal'",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| {
        let submitted_by_id: i64 = r.submitted_by_id.parse().unwrap_or(0);
        ProposalDetail {
            id: r.id,
            title: r.title,
            description: r.description,
            rationale: r.rationale,
            submitted_by_id,
            submitted_by_name: r.submitted_by_name,
            submitted_date: r.submitted_date,
            status: r.status,
            rejection_reason: r.rejection_reason,
            related_suggestion_id: r.related_suggestion_id,
        }
    }))
}

/// Create a new proposal entity linked to a ToR via `submitted_to`.
/// Optionally links to a source suggestion via `spawns_proposal`.
/// Returns the new entity id.
pub async fn create(
    pool: &PgPool,
    tor_id: i64,
    title: &str,
    description: &str,
    rationale: &str,
    submitted_by_id: i64,
    submitted_date: &str,
    related_suggestion_id: Option<i64>,
) -> Result<i64, AppError> {
    let name = name_from_title(title);

    let proposal_id = entity::create(pool, "proposal", &name, title).await?;

    entity::set_property(pool, proposal_id, "title", title).await?;
    entity::set_property(pool, proposal_id, "description", description).await?;
    entity::set_property(pool, proposal_id, "rationale", rationale).await?;
    entity::set_property(pool, proposal_id, "submitted_date", submitted_date).await?;
    entity::set_property(pool, proposal_id, "status", "draft").await?;
    entity::set_property(pool, proposal_id, "submitted_by_id", &submitted_by_id.to_string()).await?;

    relation::create(pool, "submitted_to", proposal_id, tor_id).await?;

    if let Some(suggestion_id) = related_suggestion_id {
        relation::create(pool, "spawns_proposal", suggestion_id, proposal_id).await?;
    }

    Ok(proposal_id)
}

/// Update an existing proposal's title, description, and rationale.
pub async fn update(
    pool: &PgPool,
    proposal_id: i64,
    title: &str,
    description: &str,
    rationale: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE entities SET label = $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(title)
    .bind(proposal_id)
    .execute(pool)
    .await?;

    entity::set_property(pool, proposal_id, "title", title).await?;
    entity::set_property(pool, proposal_id, "description", description).await?;
    entity::set_property(pool, proposal_id, "rationale", rationale).await?;

    Ok(())
}

/// Update the status of a proposal (e.g. draft -> submitted, under_review -> approved/rejected).
/// When rejecting, supply a rejection_reason. For any other status, the rejection_reason
/// property is cleared.
pub async fn update_status(
    pool: &PgPool,
    proposal_id: i64,
    new_status: &str,
    rejection_reason: Option<&str>,
) -> Result<(), AppError> {
    entity::set_property(pool, proposal_id, "status", new_status).await?;

    if let Some(reason) = rejection_reason {
        entity::set_property(pool, proposal_id, "rejection_reason", reason).await?;
    } else if new_status != "rejected" {
        sqlx::query(
            "DELETE FROM entity_properties WHERE entity_id = $1 AND key = 'rejection_reason'",
        )
        .bind(proposal_id)
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Auto-create a proposal from an accepted suggestion.
/// Copies the suggestion's description and metadata, linking the two together.
/// Returns the new proposal id.
pub async fn auto_create_from_suggestion(
    pool: &PgPool,
    suggestion_id: i64,
    tor_id: i64,
) -> Result<i64, AppError> {
    let suggestion = crate::models::suggestion::find_by_id(pool, suggestion_id).await?
        .ok_or(AppError::NotFound)?;

    // Use first 100 chars of description as title
    let title: String = suggestion.description.chars().take(100).collect();

    create(
        pool,
        tor_id,
        &title,
        &suggestion.description,
        "Auto-created from accepted suggestion",
        suggestion.submitted_by_id,
        &suggestion.submitted_date,
        Some(suggestion_id),
    )
    .await
}

/// Mark a proposal as ready for agenda.
/// Sets the ready_for_agenda property to "true", indicating it can be queued
/// and scheduled into agenda points.
pub async fn mark_ready_for_agenda(
    pool: &PgPool,
    proposal_id: i64,
) -> Result<(), AppError> {
    entity::set_property(pool, proposal_id, "ready_for_agenda", "true").await?;
    Ok(())
}

/// Find all queued proposals for a ToR that haven't yet been scheduled into agenda points.
/// Queued proposals are those with ready_for_agenda="true" that don't have a
/// spawns_agenda_point relation yet.
pub async fn find_queued_proposals(
    pool: &PgPool,
    tor_id: i64,
) -> Result<Vec<ProposalListItem>, AppError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: i64,
        title: String,
        submitted_date: String,
        status: String,
        submitted_by_id: String,
        submitted_by_name: String,
        rejection_reason: Option<String>,
        related_suggestion_id: Option<i64>,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT e.id, \
                COALESCE(p_title.value, '') AS title, \
                COALESCE(p_date.value, '') AS submitted_date, \
                COALESCE(p_status.value, 'draft') AS status, \
                COALESCE(p_by.value, '0') AS submitted_by_id, \
                COALESCE(u.label, '') AS submitted_by_name, \
                p_reason.value AS rejection_reason, \
                r_spawn.source_id AS related_suggestion_id \
         FROM entities e \
         JOIN relations r ON e.id = r.source_id \
         JOIN entities rt ON r.relation_type_id = rt.id AND rt.name = 'submitted_to' \
         LEFT JOIN entity_properties p_title \
             ON e.id = p_title.entity_id AND p_title.key = 'title' \
         LEFT JOIN entity_properties p_date \
             ON e.id = p_date.entity_id AND p_date.key = 'submitted_date' \
         LEFT JOIN entity_properties p_status \
             ON e.id = p_status.entity_id AND p_status.key = 'status' \
         LEFT JOIN entity_properties p_ready \
             ON e.id = p_ready.entity_id AND p_ready.key = 'ready_for_agenda' \
         LEFT JOIN entity_properties p_by \
             ON e.id = p_by.entity_id AND p_by.key = 'submitted_by_id' \
         LEFT JOIN entities u \
             ON CAST(p_by.value AS BIGINT) = u.id \
         LEFT JOIN entity_properties p_reason \
             ON e.id = p_reason.entity_id AND p_reason.key = 'rejection_reason' \
         LEFT JOIN relations r_spawn \
             ON e.id = r_spawn.target_id \
            AND r_spawn.relation_type_id = ( \
                SELECT id FROM entities \
                WHERE entity_type = 'relation_type' AND name = 'spawns_proposal') \
         WHERE e.entity_type = 'proposal' \
            AND r.target_id = $1 \
            AND COALESCE(p_status.value, 'draft') = 'approved' \
            AND COALESCE(p_ready.value, 'false') = 'true' \
            AND NOT EXISTS ( \
                SELECT 1 FROM relations spawns_ap \
                WHERE spawns_ap.source_id = e.id \
                  AND spawns_ap.relation_type_id = ( \
                      SELECT id FROM entities \
                      WHERE entity_type = 'relation_type' AND name = 'spawns_agenda_point') \
            ) \
         ORDER BY COALESCE(p_date.value, '') DESC",
    )
    .bind(tor_id)
    .fetch_all(pool)
    .await?;

    let items = rows
        .into_iter()
        .map(|row| {
            let submitted_by_id: i64 = row.submitted_by_id.parse().unwrap_or(0);
            ProposalListItem {
                id: row.id,
                title: row.title,
                submitted_by_id,
                submitted_by_name: row.submitted_by_name,
                submitted_date: row.submitted_date,
                status: row.status,
                rejection_reason: row.rejection_reason,
                related_suggestion_id: row.related_suggestion_id,
            }
        })
        .collect();

    Ok(items)
}

/// Remove a proposal from the queue by setting ready_for_agenda="false".
pub async fn unqueue_proposal(
    pool: &PgPool,
    proposal_id: i64,
) -> Result<(), AppError> {
    entity::set_property(pool, proposal_id, "ready_for_agenda", "false").await?;
    Ok(())
}
