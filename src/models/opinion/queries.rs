use sqlx::PgPool;
use crate::errors::AppError;
use crate::models::{entity, relation};
use super::types::*;

/// Intermediate row for find_opinions_for_agenda_point query.
#[derive(sqlx::FromRow)]
struct OpinionListRow {
    id: i64,
    recorded_by_id: String,
    recorded_by_name: String,
    preferred_coa_id: String,
    commentary: String,
    created_date: String,
}

/// Intermediate row for find_opinion_by_id query.
#[derive(sqlx::FromRow)]
struct OpinionDetailRow {
    id: i64,
    agenda_point_id: String,
    recorded_by_id: String,
    recorded_by_name: String,
    preferred_coa_id: String,
    coa_title: String,
    commentary: String,
    created_date: String,
}

/// Record a new opinion on an agenda point.
/// Creates an opinion entity with properties and relations to the user and agenda point.
/// Returns the new opinion entity id.
pub async fn record_opinion(
    pool: &PgPool,
    agenda_point_id: i64,
    recorded_by_id: i64,
    preferred_coa_id: i64,
    commentary: &str,
) -> Result<i64, AppError> {
    let name = format!("opinion_ap{}_by{}", agenda_point_id, recorded_by_id);

    let opinion_id = entity::create(pool, "opinion", &name, &name).await
        .map_err(|e| AppError::Db(e))?;

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    entity::set_property(pool, opinion_id, "agenda_point_id", &agenda_point_id.to_string()).await
        .map_err(|e| AppError::Db(e))?;
    entity::set_property(pool, opinion_id, "recorded_by_id", &recorded_by_id.to_string()).await
        .map_err(|e| AppError::Db(e))?;
    entity::set_property(pool, opinion_id, "preferred_coa_id", &preferred_coa_id.to_string()).await
        .map_err(|e| AppError::Db(e))?;
    entity::set_property(pool, opinion_id, "commentary", commentary).await
        .map_err(|e| AppError::Db(e))?;
    entity::set_property(pool, opinion_id, "created_date", &now).await
        .map_err(|e| AppError::Db(e))?;

    // Create relations: opinion_by (user -> opinion), opinion_on (opinion -> agenda_point), prefers_coa (opinion -> coa)
    relation::create(pool, "opinion_by", recorded_by_id, opinion_id).await
        .map_err(|e| AppError::Db(e))?;
    relation::create(pool, "opinion_on", opinion_id, agenda_point_id).await
        .map_err(|e| AppError::Db(e))?;
    relation::create(pool, "prefers_coa", opinion_id, preferred_coa_id).await
        .map_err(|e| AppError::Db(e))?;

    Ok(opinion_id)
}

/// Find all opinions recorded for a specific agenda point.
///
/// Handles two creation paths:
/// - Programmatic (`record_opinion()`): stored as entity_properties (recorded_by_id, preferred_coa_id,
///   commentary) with an `opinion_by` relation where source=user, target=opinion.
/// - Seeded: only relations exist -- `opinion_by` (source=opinion, target=user),
///   `opinion_on` (source=opinion, target=agenda_point), `prefers_coa` (source=opinion, target=coa).
///   Property key is `rationale` not `commentary`.
///
/// COALESCE fallbacks resolve user and COA from whichever path was used.
pub async fn find_opinions_for_agenda_point(
    pool: &PgPool,
    agenda_point_id: i64,
) -> Result<Vec<OpinionListItem>, AppError> {
    let rows = sqlx::query_as::<_, OpinionListRow>(
        "SELECT e.id, \
                COALESCE(p_by.value, \
                    CAST(r_by_seed.target_id AS TEXT), \
                    CAST(r_by_prog.source_id AS TEXT), \
                    '0') AS recorded_by_id, \
                COALESCE(u_prop.label, u_seed.label, u_prog.label, '') AS recorded_by_name, \
                COALESCE(p_coa.value, \
                    CAST(r_pref.target_id AS TEXT), \
                    '0') AS preferred_coa_id, \
                COALESCE(p_comment.value, p_rationale.value, '') AS commentary, \
                COALESCE(p_date.value, '') AS created_date \
         FROM entities e \
         JOIN relations r ON e.id = r.source_id \
         JOIN entities rt ON r.relation_type_id = rt.id AND rt.name = 'opinion_on' \
         LEFT JOIN entity_properties p_by \
             ON e.id = p_by.entity_id AND p_by.key = 'recorded_by_id' \
         LEFT JOIN entities u_prop ON CAST(p_by.value AS BIGINT) = u_prop.id \
         LEFT JOIN relations r_by_seed ON r_by_seed.source_id = e.id \
             AND r_by_seed.relation_type_id = ( \
                 SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'opinion_by') \
         LEFT JOIN entities u_seed ON u_seed.id = r_by_seed.target_id AND u_seed.entity_type = 'user' \
         LEFT JOIN relations r_by_prog ON r_by_prog.target_id = e.id \
             AND r_by_prog.relation_type_id = ( \
                 SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'opinion_by') \
         LEFT JOIN entities u_prog ON u_prog.id = r_by_prog.source_id AND u_prog.entity_type = 'user' \
         LEFT JOIN entity_properties p_coa \
             ON e.id = p_coa.entity_id AND p_coa.key = 'preferred_coa_id' \
         LEFT JOIN relations r_pref ON r_pref.source_id = e.id \
             AND r_pref.relation_type_id = ( \
                 SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'prefers_coa') \
         LEFT JOIN entity_properties p_comment \
             ON e.id = p_comment.entity_id AND p_comment.key = 'commentary' \
         LEFT JOIN entity_properties p_rationale \
             ON e.id = p_rationale.entity_id AND p_rationale.key = 'rationale' \
         LEFT JOIN entity_properties p_date \
             ON e.id = p_date.entity_id AND p_date.key = 'created_date' \
         WHERE e.entity_type = 'opinion' AND r.target_id = $1 \
         ORDER BY COALESCE(p_date.value, '') ASC",
    )
    .bind(agenda_point_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::Db)?;

    let items = rows.into_iter().map(|row| {
        let recorded_by: i64 = row.recorded_by_id.parse().unwrap_or(0);
        let preferred_coa_id: i64 = row.preferred_coa_id.parse().unwrap_or(0);
        OpinionListItem {
            id: row.id,
            recorded_by,
            recorded_by_name: row.recorded_by_name,
            preferred_coa_id,
            commentary: row.commentary,
            created_date: row.created_date,
        }
    }).collect();

    Ok(items)
}

/// Find a single opinion by id with full details including COA title.
pub async fn find_opinion_by_id(
    pool: &PgPool,
    id: i64,
) -> Result<Option<OpinionDetail>, AppError> {
    let row = sqlx::query_as::<_, OpinionDetailRow>(
        "SELECT e.id, \
                COALESCE(p_ap.value, '0') AS agenda_point_id, \
                COALESCE(p_by.value, '0') AS recorded_by_id, \
                COALESCE(u.label, '') AS recorded_by_name, \
                COALESCE(p_coa.value, '0') AS preferred_coa_id, \
                COALESCE(coa.label, '') AS coa_title, \
                COALESCE(p_comment.value, '') AS commentary, \
                COALESCE(p_date.value, '') AS created_date \
         FROM entities e \
         LEFT JOIN entity_properties p_ap \
             ON e.id = p_ap.entity_id AND p_ap.key = 'agenda_point_id' \
         LEFT JOIN entity_properties p_by \
             ON e.id = p_by.entity_id AND p_by.key = 'recorded_by_id' \
         LEFT JOIN entities u \
             ON CAST(p_by.value AS BIGINT) = u.id \
         LEFT JOIN entity_properties p_coa \
             ON e.id = p_coa.entity_id AND p_coa.key = 'preferred_coa_id' \
         LEFT JOIN entities coa \
             ON CAST(p_coa.value AS BIGINT) = coa.id \
         LEFT JOIN entity_properties p_comment \
             ON e.id = p_comment.entity_id AND p_comment.key = 'commentary' \
         LEFT JOIN entity_properties p_date \
             ON e.id = p_date.entity_id AND p_date.key = 'created_date' \
         WHERE e.id = $1 AND e.entity_type = 'opinion'",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Db(e))?;

    match row {
        Some(r) => {
            let recorded_by: i64 = r.recorded_by_id.parse().unwrap_or(0);
            let preferred_coa_id: i64 = r.preferred_coa_id.parse().unwrap_or(0);
            let agenda_point_id: i64 = r.agenda_point_id.parse().unwrap_or(0);
            Ok(Some(OpinionDetail {
                id: r.id,
                agenda_point_id,
                recorded_by,
                recorded_by_name: r.recorded_by_name,
                preferred_coa_id,
                coa_title: r.coa_title,
                commentary: r.commentary,
                created_date: r.created_date,
            }))
        }
        None => Ok(None),
    }
}

/// Update an existing opinion's preferred COA and commentary.
pub async fn update_opinion(
    pool: &PgPool,
    id: i64,
    preferred_coa_id: i64,
    commentary: &str,
) -> Result<(), AppError> {
    entity::set_property(pool, id, "preferred_coa_id", &preferred_coa_id.to_string()).await
        .map_err(|e| AppError::Db(e))?;
    entity::set_property(pool, id, "commentary", commentary).await
        .map_err(|e| AppError::Db(e))?;

    // Update the prefers_coa relation by deleting old and creating new
    relation::delete_all_from_source(pool, id, "prefers_coa").await
        .map_err(|e| AppError::Db(e))?;
    relation::create(pool, "prefers_coa", id, preferred_coa_id).await
        .map_err(|e| AppError::Db(e))?;

    Ok(())
}

/// Check if a user has already recorded an opinion on a specific agenda point.
pub async fn find_opinion_by_user_and_agenda_point(
    pool: &PgPool,
    user_id: i64,
    agenda_point_id: i64,
) -> Result<Option<i64>, AppError> {
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT e.id \
         FROM entities e \
         JOIN entity_properties p_by ON e.id = p_by.entity_id AND p_by.key = 'recorded_by_id' \
         JOIN entity_properties p_ap ON e.id = p_ap.entity_id AND p_ap.key = 'agenda_point_id' \
         WHERE e.entity_type = 'opinion' \
           AND CAST(p_by.value AS BIGINT) = $1 \
           AND CAST(p_ap.value AS BIGINT) = $2",
    )
    .bind(user_id)
    .bind(agenda_point_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Db(e))?;

    Ok(row.map(|(id,)| id))
}

/// Record a final decision on an agenda point.
/// Creates a decision entity with properties and updates the agenda point status to "voted".
/// Returns the new decision entity id.
pub async fn record_decision(
    pool: &PgPool,
    agenda_point_id: i64,
    decided_by_id: i64,
    selected_coa_id: i64,
    decision_rationale: &str,
) -> Result<i64, AppError> {
    let name = format!("decision_ap{}_by{}", agenda_point_id, decided_by_id);

    let decision_id = entity::create(pool, "decision", &name, &name).await
        .map_err(|e| AppError::Db(e))?;

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    entity::set_property(pool, decision_id, "agenda_point_id", &agenda_point_id.to_string()).await
        .map_err(|e| AppError::Db(e))?;
    entity::set_property(pool, decision_id, "decided_by_id", &decided_by_id.to_string()).await
        .map_err(|e| AppError::Db(e))?;
    entity::set_property(pool, decision_id, "selected_coa_id", &selected_coa_id.to_string()).await
        .map_err(|e| AppError::Db(e))?;
    entity::set_property(pool, decision_id, "decision_rationale", decision_rationale).await
        .map_err(|e| AppError::Db(e))?;
    entity::set_property(pool, decision_id, "decided_date", &now).await
        .map_err(|e| AppError::Db(e))?;

    // Update agenda point status to "voted"
    entity::set_property(pool, agenda_point_id, "status", "voted").await
        .map_err(|e| AppError::Db(e))?;

    Ok(decision_id)
}

/// Get a summary of opinions grouped by preferred COA for an agenda point.
/// Returns a list of (coa_id, count) tuples showing how many people prefer each COA.
pub async fn get_opinions_summary(
    pool: &PgPool,
    agenda_point_id: i64,
) -> Result<Vec<(i64, i32)>, AppError> {
    let results: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT CAST(p_coa.value AS BIGINT) AS coa_id, COUNT(*) AS count \
         FROM entities e \
         JOIN entity_properties p_ap ON e.id = p_ap.entity_id AND p_ap.key = 'agenda_point_id' \
         JOIN entity_properties p_coa ON e.id = p_coa.entity_id AND p_coa.key = 'preferred_coa_id' \
         WHERE e.entity_type = 'opinion' \
           AND CAST(p_ap.value AS BIGINT) = $1 \
         GROUP BY coa_id \
         ORDER BY count DESC",
    )
    .bind(agenda_point_id)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Db(e))?;

    Ok(results.into_iter().map(|(coa_id, count)| (coa_id, count as i32)).collect())
}
