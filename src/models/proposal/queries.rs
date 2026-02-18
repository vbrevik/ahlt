use rusqlite::{Connection, params};
use crate::errors::AppError;
use crate::models::{entity, relation};
use super::types::*;

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
pub fn find_all_for_tor(conn: &Connection, tor_id: i64) -> Result<Vec<ProposalListItem>, AppError> {
    let mut stmt = conn.prepare(
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
             ON CAST(p_by.value AS INTEGER) = u.id \
         LEFT JOIN entity_properties p_reason \
             ON e.id = p_reason.entity_id AND p_reason.key = 'rejection_reason' \
         LEFT JOIN relations r_spawn \
             ON e.id = r_spawn.target_id \
            AND r_spawn.relation_type_id = ( \
                SELECT id FROM entities \
                WHERE entity_type = 'relation_type' AND name = 'spawns_proposal') \
         WHERE e.entity_type = 'proposal' AND r.target_id = ?1 \
         ORDER BY submitted_date DESC",
    )?;

    let items = stmt
        .query_map(params![tor_id], |row| {
            let submitted_by_id_str: String = row.get("submitted_by_id")?;
            let submitted_by_id: i64 = submitted_by_id_str.parse().unwrap_or(0);
            let rejection_reason: Option<String> = row.get("rejection_reason")?;
            let related_suggestion_id: Option<i64> = row.get("related_suggestion_id")?;

            Ok(ProposalListItem {
                id: row.get("id")?,
                title: row.get("title")?,
                submitted_by_id,
                submitted_by_name: row.get("submitted_by_name")?,
                submitted_date: row.get("submitted_date")?,
                status: row.get("status")?,
                rejection_reason,
                related_suggestion_id,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(items)
}

/// Find all proposals across all ToRs (or filtered to ToRs a user fills a position in).
///
/// `user_id = None`  → returns every proposal across all ToRs.
/// `user_id = Some(id)` → returns only proposals for ToRs the user fills a position in.
pub fn find_all_cross_tor(conn: &Connection, user_id: Option<i64>) -> Result<Vec<CrossTorProposalItem>, AppError> {
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
                        ON CAST(p_by.value AS INTEGER) = u.id \
                    LEFT JOIN entity_properties p_reason \
                        ON e.id = p_reason.entity_id AND p_reason.key = 'rejection_reason' \
                    LEFT JOIN relations r_spawn \
                        ON e.id = r_spawn.target_id \
                       AND r_spawn.relation_type_id = ( \
                           SELECT id FROM entities \
                           WHERE entity_type = 'relation_type' AND name = 'spawns_proposal') \
                    WHERE e.entity_type = 'proposal'";

    let row_to_item = |row: &rusqlite::Row<'_>| {
        let submitted_by_id_str: String = row.get("submitted_by_id")?;
        let submitted_by_id: i64 = submitted_by_id_str.parse().unwrap_or(0);
        let rejection_reason: Option<String> = row.get("rejection_reason")?;
        let related_suggestion_id: Option<i64> = row.get("related_suggestion_id")?;

        Ok(CrossTorProposalItem {
            tor_id: row.get("tor_id")?,
            tor_name: row.get("tor_name")?,
            id: row.get("id")?,
            title: row.get("title")?,
            submitted_by_id,
            submitted_by_name: row.get("submitted_by_name")?,
            submitted_date: row.get("submitted_date")?,
            status: row.get("status")?,
            rejection_reason,
            related_suggestion_id,
        })
    };

    let items = if let Some(uid) = user_id {
        let sql = format!(
            "{} AND EXISTS (\
                SELECT 1 FROM relations r_fills \
                JOIN relations r_tor ON r_fills.target_id = r_tor.source_id \
                WHERE r_fills.source_id = ?1 \
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
        let mut stmt = conn.prepare(&sql)?;
        stmt.query_map(params![uid], row_to_item)?
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let sql = format!("{} ORDER BY tor.label ASC, submitted_date DESC", base_sql);
        let mut stmt = conn.prepare(&sql)?;
        stmt.query_map([], row_to_item)?
            .collect::<Result<Vec<_>, _>>()?
    };

    Ok(items)
}

/// Find a single proposal by its entity id.
pub fn find_by_id(conn: &Connection, id: i64) -> Result<Option<ProposalDetail>, AppError> {
    let mut stmt = conn.prepare(
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
             ON CAST(p_by.value AS INTEGER) = u.id \
         LEFT JOIN entity_properties p_reason \
             ON e.id = p_reason.entity_id AND p_reason.key = 'rejection_reason' \
         LEFT JOIN relations r_spawn \
             ON e.id = r_spawn.target_id \
            AND r_spawn.relation_type_id = ( \
                SELECT id FROM entities \
                WHERE entity_type = 'relation_type' AND name = 'spawns_proposal') \
         WHERE e.id = ?1 AND e.entity_type = 'proposal'",
    )?;

    let mut rows = stmt.query_map(params![id], |row| {
        let submitted_by_id_str: String = row.get("submitted_by_id")?;
        let submitted_by_id: i64 = submitted_by_id_str.parse().unwrap_or(0);
        let rejection_reason: Option<String> = row.get("rejection_reason")?;
        let related_suggestion_id: Option<i64> = row.get("related_suggestion_id")?;

        Ok(ProposalDetail {
            id: row.get("id")?,
            title: row.get("title")?,
            description: row.get("description")?,
            rationale: row.get("rationale")?,
            submitted_by_id,
            submitted_by_name: row.get("submitted_by_name")?,
            submitted_date: row.get("submitted_date")?,
            status: row.get("status")?,
            rejection_reason,
            related_suggestion_id,
        })
    })?;

    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Create a new proposal entity linked to a ToR via `submitted_to`.
/// Optionally links to a source suggestion via `spawns_proposal`.
/// Returns the new entity id.
pub fn create(
    conn: &Connection,
    tor_id: i64,
    title: &str,
    description: &str,
    rationale: &str,
    submitted_by_id: i64,
    submitted_date: &str,
    related_suggestion_id: Option<i64>,
) -> Result<i64, AppError> {
    let name = name_from_title(title);

    let proposal_id = entity::create(conn, "proposal", &name, title)?;

    entity::set_property(conn, proposal_id, "title", title)?;
    entity::set_property(conn, proposal_id, "description", description)?;
    entity::set_property(conn, proposal_id, "rationale", rationale)?;
    entity::set_property(conn, proposal_id, "submitted_date", submitted_date)?;
    entity::set_property(conn, proposal_id, "status", "draft")?;
    entity::set_property(conn, proposal_id, "submitted_by_id", &submitted_by_id.to_string())?;

    relation::create(conn, "submitted_to", proposal_id, tor_id)?;

    if let Some(suggestion_id) = related_suggestion_id {
        relation::create(conn, "spawns_proposal", suggestion_id, proposal_id)?;
    }

    Ok(proposal_id)
}

/// Update an existing proposal's title, description, and rationale.
pub fn update(
    conn: &Connection,
    proposal_id: i64,
    title: &str,
    description: &str,
    rationale: &str,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE entities SET label = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%S','now') WHERE id = ?2",
        params![title, proposal_id],
    )?;

    entity::set_property(conn, proposal_id, "title", title)?;
    entity::set_property(conn, proposal_id, "description", description)?;
    entity::set_property(conn, proposal_id, "rationale", rationale)?;

    Ok(())
}

/// Update the status of a proposal (e.g. draft -> submitted, under_review -> approved/rejected).
/// When rejecting, supply a rejection_reason. For any other status, the rejection_reason
/// property is cleared.
pub fn update_status(
    conn: &Connection,
    proposal_id: i64,
    new_status: &str,
    rejection_reason: Option<&str>,
) -> Result<(), AppError> {
    entity::set_property(conn, proposal_id, "status", new_status)?;

    if let Some(reason) = rejection_reason {
        entity::set_property(conn, proposal_id, "rejection_reason", reason)?;
    } else if new_status != "rejected" {
        conn.execute(
            "DELETE FROM entity_properties WHERE entity_id = ?1 AND key = 'rejection_reason'",
            params![proposal_id],
        )?;
    }

    Ok(())
}

/// Auto-create a proposal from an accepted suggestion.
/// Copies the suggestion's description and metadata, linking the two together.
/// Returns the new proposal id.
pub fn auto_create_from_suggestion(
    conn: &Connection,
    suggestion_id: i64,
    tor_id: i64,
) -> Result<i64, AppError> {
    let suggestion = crate::models::suggestion::find_by_id(conn, suggestion_id)?
        .ok_or(AppError::NotFound)?;

    // Use first 100 chars of description as title
    let title: String = suggestion.description.chars().take(100).collect();

    create(
        conn,
        tor_id,
        &title,
        &suggestion.description,
        "Auto-created from accepted suggestion",
        suggestion.submitted_by_id,
        &suggestion.submitted_date,
        Some(suggestion_id),
    )
}

/// Mark a proposal as ready for agenda.
/// Sets the ready_for_agenda property to "true", indicating it can be queued
/// and scheduled into agenda points.
pub fn mark_ready_for_agenda(
    conn: &Connection,
    proposal_id: i64,
) -> Result<(), AppError> {
    entity::set_property(conn, proposal_id, "ready_for_agenda", "true")?;
    Ok(())
}

/// Find all queued proposals for a ToR that haven't yet been scheduled into agenda points.
/// Queued proposals are those with ready_for_agenda="true" that don't have a
/// spawns_agenda_point relation yet.
pub fn find_queued_proposals(
    conn: &Connection,
    tor_id: i64,
) -> Result<Vec<ProposalListItem>, AppError> {
    let mut stmt = conn.prepare(
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
             ON CAST(p_by.value AS INTEGER) = u.id \
         LEFT JOIN entity_properties p_reason \
             ON e.id = p_reason.entity_id AND p_reason.key = 'rejection_reason' \
         LEFT JOIN relations r_spawn \
             ON e.id = r_spawn.target_id \
            AND r_spawn.relation_type_id = ( \
                SELECT id FROM entities \
                WHERE entity_type = 'relation_type' AND name = 'spawns_proposal') \
         WHERE e.entity_type = 'proposal' \
            AND r.target_id = ?1 \
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
    )?;

    let items = stmt
        .query_map(params![tor_id], |row| {
            let submitted_by_id_str: String = row.get("submitted_by_id")?;
            let submitted_by_id: i64 = submitted_by_id_str.parse().unwrap_or(0);
            let rejection_reason: Option<String> = row.get("rejection_reason")?;
            let related_suggestion_id: Option<i64> = row.get("related_suggestion_id")?;

            Ok(ProposalListItem {
                id: row.get("id")?,
                title: row.get("title")?,
                submitted_by_id,
                submitted_by_name: row.get("submitted_by_name")?,
                submitted_date: row.get("submitted_date")?,
                status: row.get("status")?,
                rejection_reason,
                related_suggestion_id,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(items)
}

/// Remove a proposal from the queue by setting ready_for_agenda="false".
pub fn unqueue_proposal(
    conn: &Connection,
    proposal_id: i64,
) -> Result<(), AppError> {
    entity::set_property(conn, proposal_id, "ready_for_agenda", "false")?;
    Ok(())
}

