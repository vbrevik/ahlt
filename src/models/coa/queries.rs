use rusqlite::{Connection, params};
use crate::errors::AppError;
use crate::models::{entity, relation};
use super::types::*;
use super::sections;

/// Find all COAs considered for a specific agenda point via considers_coa relation.
pub fn find_all_for_agenda_point(conn: &Connection, agenda_point_id: i64) -> Result<Vec<CoaListItem>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT e.id, \
                COALESCE(p_title.value, '') AS title, \
                COALESCE(p_type.value, 'simple') AS coa_type, \
                COALESCE(p_created_by.value, '0') AS created_by, \
                COALESCE(p_created_date.value, '') AS created_date \
         FROM entities e \
         JOIN relations r ON e.id = r.target_id \
         JOIN entities rt ON r.relation_type_id = rt.id AND rt.name = 'considers_coa' \
         LEFT JOIN entity_properties p_title \
             ON e.id = p_title.entity_id AND p_title.key = 'title' \
         LEFT JOIN entity_properties p_type \
             ON e.id = p_type.entity_id AND p_type.key = 'coa_type' \
         LEFT JOIN entity_properties p_created_by \
             ON e.id = p_created_by.entity_id AND p_created_by.key = 'created_by' \
         LEFT JOIN entity_properties p_created_date \
             ON e.id = p_created_date.entity_id AND p_created_date.key = 'created_date' \
         WHERE e.entity_type = 'coa' AND r.source_id = ?1 \
         ORDER BY p_created_date.value DESC",
    )?;

    let items = stmt
        .query_map(params![agenda_point_id], |row| {
            let created_by_str: String = row.get("created_by")?;
            let created_by: i64 = created_by_str.parse().unwrap_or(0);

            Ok(CoaListItem {
                id: row.get("id")?,
                title: row.get("title")?,
                coa_type: row.get("coa_type")?,
                created_by,
                created_date: row.get("created_date")?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(items)
}

/// Find a single COA by its entity id, loading all sections and subsections.
pub fn find_by_id(conn: &Connection, id: i64) -> Result<CoaDetail, AppError> {
    let mut stmt = conn.prepare(
        "SELECT e.id, \
                COALESCE(p_title.value, '') AS title, \
                COALESCE(p_desc.value, '') AS description, \
                COALESCE(p_type.value, 'simple') AS coa_type, \
                COALESCE(p_created_by.value, '0') AS created_by, \
                COALESCE(p_created_date.value, '') AS created_date \
         FROM entities e \
         LEFT JOIN entity_properties p_title \
             ON e.id = p_title.entity_id AND p_title.key = 'title' \
         LEFT JOIN entity_properties p_desc \
             ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
         LEFT JOIN entity_properties p_type \
             ON e.id = p_type.entity_id AND p_type.key = 'coa_type' \
         LEFT JOIN entity_properties p_created_by \
             ON e.id = p_created_by.entity_id AND p_created_by.key = 'created_by' \
         LEFT JOIN entity_properties p_created_date \
             ON e.id = p_created_date.entity_id AND p_created_date.key = 'created_date' \
         WHERE e.id = ?1 AND e.entity_type = 'coa'",
    )?;

    let mut rows = stmt.query_map(params![id], |row| {
        let created_by_str: String = row.get("created_by")?;
        let created_by: i64 = created_by_str.parse().unwrap_or(0);

        Ok((
            row.get::<_, i64>("id")?,
            row.get::<_, String>("title")?,
            row.get::<_, String>("description")?,
            row.get::<_, String>("coa_type")?,
            created_by,
            row.get::<_, String>("created_date")?,
        ))
    })?;

    match rows.next() {
        Some(Ok((coa_id, title, description, coa_type, created_by, created_date))) => {
            let sections = sections::find_sections(conn, coa_id)?;
            Ok(CoaDetail {
                id: coa_id,
                title,
                description,
                coa_type,
                created_by,
                created_date,
                sections,
            })
        }
        Some(Err(e)) => Err(AppError::Db(e)),
        None => Err(AppError::NotFound),
    }
}

/// Create a new COA entity and set its properties.
/// Returns the new COA id.
pub fn create(
    conn: &Connection,
    title: &str,
    description: &str,
    coa_type: &str,  // "simple" or "complex"
    created_by_id: i64,
) -> Result<i64, AppError> {
    let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let name = format!("coa_{}_{}", title.replace(' ', "_").to_lowercase(), now.replace(':', "_"));
    let label = title.to_string();

    let coa_id = entity::create(conn, "coa", &name, &label).map_err(AppError::Db)?;

    entity::set_property(conn, coa_id, "title", title).map_err(AppError::Db)?;
    entity::set_property(conn, coa_id, "description", description).map_err(AppError::Db)?;
    entity::set_property(conn, coa_id, "coa_type", coa_type).map_err(AppError::Db)?;
    entity::set_property(conn, coa_id, "created_by", &created_by_id.to_string()).map_err(AppError::Db)?;
    entity::set_property(conn, coa_id, "created_date", &now).map_err(AppError::Db)?;

    Ok(coa_id)
}

/// Update COA title and description.
pub fn update(
    conn: &Connection,
    id: i64,
    title: &str,
    description: &str,
) -> Result<(), AppError> {
    entity::set_property(conn, id, "title", title).map_err(AppError::Db)?;
    entity::set_property(conn, id, "description", description).map_err(AppError::Db)?;
    Ok(())
}

/// Add a section to a COA via has_section relation.
/// Returns the new section entity id.
pub fn add_section(
    conn: &Connection,
    coa_id: i64,
    title: &str,
    content: &str,
    order: i32,
) -> Result<i64, AppError> {
    let name = format!("coa_section_{}_{}_{}", coa_id, order, title.replace(' ', "_").to_lowercase());
    let label = title.to_string();

    let section_id = entity::create(conn, "coa_section", &name, &label).map_err(AppError::Db)?;

    entity::set_property(conn, section_id, "title", title).map_err(AppError::Db)?;
    entity::set_property(conn, section_id, "content", content).map_err(AppError::Db)?;
    entity::set_property(conn, section_id, "order", &order.to_string()).map_err(AppError::Db)?;

    relation::create(conn, "has_section", coa_id, section_id).map_err(AppError::Db)?;

    Ok(section_id)
}

