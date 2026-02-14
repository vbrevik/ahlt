use rusqlite::{Connection, params};
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
pub fn find_all_for_tor(conn: &Connection, tor_id: i64) -> Result<Vec<SuggestionListItem>, AppError> {
    let mut stmt = conn.prepare(
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
             ON CAST(p_by.value AS INTEGER) = u.id \
         LEFT JOIN entity_properties p_reason \
             ON e.id = p_reason.entity_id AND p_reason.key = 'rejection_reason' \
         LEFT JOIN relations r_spawn \
             ON e.id = r_spawn.source_id \
            AND r_spawn.relation_type_id = ( \
                SELECT id FROM entities \
                WHERE entity_type = 'relation_type' AND name = 'spawns_proposal') \
         WHERE e.entity_type = 'suggestion' AND r.target_id = ?1 \
         ORDER BY submitted_date DESC",
    )?;

    let items = stmt
        .query_map(params![tor_id], |row| {
            let description: String = row.get("description")?;
            let submitted_by_id_str: String = row.get("submitted_by_id")?;
            let submitted_by_id: i64 = submitted_by_id_str.parse().unwrap_or(0);
            let rejection_reason: Option<String> = row.get("rejection_reason")?;
            let spawned_proposal_id: Option<i64> = row.get("spawned_proposal_id")?;

            Ok(SuggestionListItem {
                id: row.get("id")?,
                description_preview: make_preview(&description, 100),
                description,
                submitted_by_id,
                submitted_by_name: row.get("submitted_by_name")?,
                submitted_date: row.get("submitted_date")?,
                status: row.get("status")?,
                rejection_reason,
                spawned_proposal_id,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(items)
}

/// Find a single suggestion by its entity id.
pub fn find_by_id(conn: &Connection, id: i64) -> Result<Option<SuggestionDetail>, AppError> {
    let mut stmt = conn.prepare(
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
             ON CAST(p_by.value AS INTEGER) = u.id \
         LEFT JOIN entity_properties p_reason \
             ON e.id = p_reason.entity_id AND p_reason.key = 'rejection_reason' \
         LEFT JOIN relations r_spawn \
             ON e.id = r_spawn.source_id \
            AND r_spawn.relation_type_id = ( \
                SELECT id FROM entities \
                WHERE entity_type = 'relation_type' AND name = 'spawns_proposal') \
         WHERE e.id = ?1 AND e.entity_type = 'suggestion'",
    )?;

    let mut rows = stmt.query_map(params![id], |row| {
        let description: String = row.get("description")?;
        let submitted_by_id_str: String = row.get("submitted_by_id")?;
        let submitted_by_id: i64 = submitted_by_id_str.parse().unwrap_or(0);
        let rejection_reason: Option<String> = row.get("rejection_reason")?;
        let spawned_proposal_id: Option<i64> = row.get("spawned_proposal_id")?;

        Ok(SuggestionDetail {
            id: row.get("id")?,
            description,
            submitted_by_id,
            submitted_by_name: row.get("submitted_by_name")?,
            submitted_date: row.get("submitted_date")?,
            status: row.get("status")?,
            rejection_reason,
            spawned_proposal_id,
        })
    })?;

    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Create a new suggestion entity linked to a ToR via `suggested_to`.
/// Returns the new entity id.
pub fn create(
    conn: &Connection,
    tor_id: i64,
    description: &str,
    submitted_by_id: i64,
    submitted_date: &str,
) -> Result<i64, AppError> {
    let name = format!("suggestion_{}_{}", submitted_date.replace('-', "_"), tor_id);
    let label = make_preview(description, 50);

    let suggestion_id = entity::create(conn, "suggestion", &name, &label)?;

    entity::set_property(conn, suggestion_id, "description", description)?;
    entity::set_property(conn, suggestion_id, "submitted_date", submitted_date)?;
    entity::set_property(conn, suggestion_id, "status", "open")?;
    entity::set_property(conn, suggestion_id, "submitted_by_id", &submitted_by_id.to_string())?;

    relation::create(conn, "suggested_to", suggestion_id, tor_id)?;

    Ok(suggestion_id)
}

/// Update the status of a suggestion (e.g. open -> accepted or rejected).
pub fn update_status(
    conn: &Connection,
    suggestion_id: i64,
    new_status: &str,
    rejection_reason: Option<&str>,
) -> Result<(), AppError> {
    entity::set_property(conn, suggestion_id, "status", new_status)?;

    if let Some(reason) = rejection_reason {
        entity::set_property(conn, suggestion_id, "rejection_reason", reason)?;
    }

    Ok(())
}
