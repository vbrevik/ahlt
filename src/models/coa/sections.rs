use rusqlite::{Connection, params};
use crate::errors::AppError;
use crate::models::entity;
use super::types::{CoaSection, CoaSubsection};

/// Find all sections for a given COA via has_section relation.
pub fn find_sections(conn: &Connection, coa_id: i64) -> Result<Vec<CoaSection>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT e.id, \
                COALESCE(p_title.value, '') AS title, \
                COALESCE(p_content.value, '') AS content, \
                COALESCE(p_order.value, '0') AS order_num \
         FROM entities e \
         JOIN relations r ON e.id = r.target_id \
         JOIN entities rt ON r.relation_type_id = rt.id AND rt.name = 'has_section' \
         LEFT JOIN entity_properties p_title \
             ON e.id = p_title.entity_id AND p_title.key = 'title' \
         LEFT JOIN entity_properties p_content \
             ON e.id = p_content.entity_id AND p_content.key = 'content' \
         LEFT JOIN entity_properties p_order \
             ON e.id = p_order.entity_id AND p_order.key = 'order' \
         WHERE e.entity_type = 'coa_section' AND r.source_id = ?1 \
         ORDER BY CAST(COALESCE(p_order.value, '0') AS INTEGER) ASC",
    )?;

    let sections = stmt
        .query_map(params![coa_id], |row| {
            let order_str: String = row.get("order_num")?;
            let order: i32 = order_str.parse().unwrap_or(0);

            Ok((
                row.get::<_, i64>("id")?,
                row.get::<_, String>("title")?,
                row.get::<_, String>("content")?,
                order,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut result = Vec::new();
    for (section_id, title, content, order) in sections {
        let subsections = find_subsections(conn, section_id)?;
        result.push(CoaSection {
            id: section_id,
            title,
            content,
            order,
            subsections,
        });
    }

    Ok(result)
}

/// Find all subsections for a given section via has_subsection relation.
fn find_subsections(conn: &Connection, section_id: i64) -> Result<Vec<CoaSubsection>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT e.id, \
                COALESCE(p_title.value, '') AS title, \
                COALESCE(p_content.value, '') AS content, \
                COALESCE(p_order.value, '0') AS order_num \
         FROM entities e \
         JOIN relations r ON e.id = r.target_id \
         JOIN entities rt ON r.relation_type_id = rt.id AND rt.name = 'has_subsection' \
         LEFT JOIN entity_properties p_title \
             ON e.id = p_title.entity_id AND p_title.key = 'title' \
         LEFT JOIN entity_properties p_content \
             ON e.id = p_content.entity_id AND p_content.key = 'content' \
         LEFT JOIN entity_properties p_order \
             ON e.id = p_order.entity_id AND p_order.key = 'order' \
         WHERE e.entity_type = 'coa_section' AND r.source_id = ?1 \
         ORDER BY CAST(COALESCE(p_order.value, '0') AS INTEGER) ASC",
    )?;

    let items = stmt
        .query_map(params![section_id], |row| {
            let order_str: String = row.get("order_num")?;
            let order: i32 = order_str.parse().unwrap_or(0);

            Ok(CoaSubsection {
                id: row.get("id")?,
                title: row.get("title")?,
                content: row.get("content")?,
                order,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(items)
}

/// Find a single section by its entity id.
pub fn find_section_by_id(conn: &Connection, section_id: i64) -> Result<CoaSection, AppError> {
    let mut stmt = conn.prepare(
        "SELECT e.id, \
                COALESCE(p_title.value, '') AS title, \
                COALESCE(p_content.value, '') AS content, \
                COALESCE(p_order.value, '0') AS order_num \
         FROM entities e \
         WHERE e.id = ?1 AND e.entity_type = 'coa_section'",
    )?;

    let mut rows = stmt.query_map(params![section_id], |row| {
        let order_str: String = row.get("order_num")?;
        let order: i32 = order_str.parse().unwrap_or(0);

        Ok((
            row.get::<_, i64>("id")?,
            row.get::<_, String>("title")?,
            row.get::<_, String>("content")?,
            order,
        ))
    })?;

    match rows.next() {
        Some(Ok((id, title, content, order))) => {
            let subsections = find_subsections(conn, id)?;
            Ok(CoaSection {
                id,
                title,
                content,
                order,
                subsections,
            })
        }
        Some(Err(e)) => Err(AppError::Db(e)),
        None => Err(AppError::NotFound),
    }
}

/// Delete a section by its entity id (cascades to subsections via relations).
pub fn delete_section(conn: &Connection, section_id: i64) -> Result<(), AppError> {
    entity::delete(conn, section_id).map_err(AppError::Db)?;
    Ok(())
}
