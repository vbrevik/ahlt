use rusqlite::{Connection, params};
use crate::errors::AppError;
use crate::models::{entity, relation};
use super::types::*;

/// Generate a slug-style name from a title.
fn name_from_title(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c == ' ' { '_' } else { c })
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// Find all documents, optionally filtered by ToR or search term.
pub fn find_all(
    conn: &Connection,
    tor_id: Option<i64>,
    search: Option<&str>,
) -> Result<Vec<DocumentListItem>, AppError> {
    let base_sql = "SELECT e.id, \
                           COALESCE(p_title.value, '') AS title, \
                           COALESCE(p_type.value, 'ad_hoc') AS doc_type, \
                           COALESCE(p_by.value, '0') AS created_by_id, \
                           COALESCE(u.label, '') AS created_by_name, \
                           COALESCE(p_date.value, '') AS created_date, \
                           r.target_id AS tor_id, \
                           COALESCE(t.label, '') AS tor_name \
                    FROM entities e \
                    LEFT JOIN entity_properties p_title \
                        ON e.id = p_title.entity_id AND p_title.key = 'title' \
                    LEFT JOIN entity_properties p_type \
                        ON e.id = p_type.entity_id AND p_type.key = 'doc_type' \
                    LEFT JOIN entity_properties p_by \
                        ON e.id = p_by.entity_id AND p_by.key = 'created_by_id' \
                    LEFT JOIN entities u \
                        ON CAST(p_by.value AS INTEGER) = u.id \
                    LEFT JOIN entity_properties p_date \
                        ON e.id = p_date.entity_id AND p_date.key = 'created_date' \
                    LEFT JOIN relations r ON e.id = r.source_id \
                        AND r.relation_type_id = ( \
                            SELECT id FROM entities \
                            WHERE entity_type = 'relation_type' AND name = 'scoped_to_tor') \
                    LEFT JOIN entities t ON r.target_id = t.id \
                    WHERE e.entity_type = 'document'";

    let mut where_clause = String::new();
    let mut params_list: Vec<String> = vec![];

    if let Some(tid) = tor_id {
        where_clause.push_str(" AND r.target_id = ?");
        params_list.push(tid.to_string());
    }

    if let Some(q) = search {
        where_clause.push_str(" AND (p_title.value LIKE ? OR p_type.value LIKE ?)");
        let search_pattern = format!("%{}%", q);
        params_list.push(search_pattern.clone());
        params_list.push(search_pattern);
    }

    let sql = format!("{}{} ORDER BY p_date.value DESC", base_sql, where_clause);

    let mut stmt = conn.prepare(&sql)?;
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![];
    for p in &params_list {
        params.push(p);
    }

    if tor_id.is_some() {
        params.insert(0, &tor_id);
    }

    let items = stmt
        .query_map(rusqlite::params_from_iter(params_list.iter()), |row| {
            let created_by_id_str: String = row.get(3)?;
            let created_by_id: i64 = created_by_id_str.parse().unwrap_or(0);
            let tor_id_val: Option<i64> = row.get(6).ok();
            let tor_name_val: Option<String> = row.get(7).ok();

            Ok(DocumentListItem {
                id: row.get(0)?,
                title: row.get(1)?,
                doc_type: row.get(2)?,
                created_by_id,
                created_by_name: row.get(4)?,
                created_date: row.get(5)?,
                tor_id: tor_id_val,
                tor_name: tor_name_val,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(items)
}

/// Find a single document by ID.
pub fn find_by_id(conn: &Connection, id: i64) -> Result<Option<DocumentDetail>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT e.id, \
                COALESCE(p_title.value, '') AS title, \
                COALESCE(p_type.value, 'ad_hoc') AS doc_type, \
                COALESCE(p_body.value, '') AS body, \
                COALESCE(p_by.value, '0') AS created_by_id, \
                COALESCE(u.label, '') AS created_by_name, \
                COALESCE(p_date.value, '') AS created_date, \
                COALESCE(p_updated.value, '') AS updated_date, \
                COALESCE(r.target_id, 0) AS tor_id, \
                COALESCE(t.label, '') AS tor_name \
         FROM entities e \
         LEFT JOIN entity_properties p_title \
             ON e.id = p_title.entity_id AND p_title.key = 'title' \
         LEFT JOIN entity_properties p_type \
             ON e.id = p_type.entity_id AND p_type.key = 'doc_type' \
         LEFT JOIN entity_properties p_body \
             ON e.id = p_body.entity_id AND p_body.key = 'body' \
         LEFT JOIN entity_properties p_by \
             ON e.id = p_by.entity_id AND p_by.key = 'created_by_id' \
         LEFT JOIN entities u \
             ON CAST(p_by.value AS INTEGER) = u.id \
         LEFT JOIN entity_properties p_date \
             ON e.id = p_date.entity_id AND p_date.key = 'created_date' \
         LEFT JOIN entity_properties p_updated \
             ON e.id = p_updated.entity_id AND p_updated.key = 'updated_date' \
         LEFT JOIN relations r ON e.id = r.source_id \
             AND r.relation_type_id = ( \
                 SELECT id FROM entities \
                 WHERE entity_type = 'relation_type' AND name = 'scoped_to_tor') \
         LEFT JOIN entities t ON r.target_id = t.id \
         WHERE e.id = ?1 AND e.entity_type = 'document'",
    )?;

    let mut rows = stmt.query_map(params![id], |row| {
        let created_by_id_str: String = row.get(4)?;
        let created_by_id: i64 = created_by_id_str.parse().unwrap_or(0);

        Ok(DocumentDetail {
            id: row.get(0)?,
            title: row.get(1)?,
            doc_type: row.get(2)?,
            body: row.get(3)?,
            created_by_id,
            created_by_name: row.get(5)?,
            created_date: row.get(6)?,
            updated_date: row.get(7)?,
            tor_id: row.get(8)?,
            tor_name: row.get(9)?,
        })
    })?;

    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Create a new document.
pub fn create(
    conn: &Connection,
    title: &str,
    doc_type: &str,
    body: &str,
    created_by_id: i64,
    tor_id: Option<i64>,
) -> Result<i64, AppError> {
    let name = name_from_title(title);
    let doc_id = entity::create(conn, "document", &name, title)?;

    // Get today's date from SQLite
    let today: String = conn.query_row("SELECT date('now')", [], |row| row.get(0))?;

    entity::set_property(conn, doc_id, "title", title)?;
    entity::set_property(conn, doc_id, "doc_type", doc_type)?;
    entity::set_property(conn, doc_id, "body", body)?;
    entity::set_property(conn, doc_id, "created_by_id", &created_by_id.to_string())?;
    entity::set_property(conn, doc_id, "created_date", &today)?;

    if let Some(tid) = tor_id {
        relation::create(conn, "scoped_to_tor", doc_id, tid)?;
    }

    Ok(doc_id)
}

/// Update an existing document.
pub fn update(
    conn: &Connection,
    doc_id: i64,
    title: &str,
    doc_type: &str,
    body: &str,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE entities SET label = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%S','now') WHERE id = ?2",
        params![title, doc_id],
    )?;

    entity::set_property(conn, doc_id, "title", title)?;
    entity::set_property(conn, doc_id, "doc_type", doc_type)?;
    entity::set_property(conn, doc_id, "body", body)?;

    // Get today's date from SQLite for updated_date property
    let today: String = conn.query_row("SELECT date('now')", [], |row| row.get(0))?;
    entity::set_property(conn, doc_id, "updated_date", &today)?;

    Ok(())
}

/// Delete a document and its relations.
pub fn delete(conn: &Connection, doc_id: i64) -> Result<(), AppError> {
    entity::delete(conn, doc_id)?;
    Ok(())
}

/// Count documents, optionally by ToR.
pub fn count(conn: &Connection, tor_id: Option<i64>) -> Result<i64, AppError> {
    if let Some(tid) = tor_id {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM entities e \
             LEFT JOIN relations r ON e.id = r.source_id \
                 AND r.relation_type_id = ( \
                     SELECT id FROM entities \
                     WHERE entity_type = 'relation_type' AND name = 'scoped_to_tor') \
             WHERE e.entity_type = 'document' AND r.target_id = ?1",
            params![tid],
            |row| row.get(0),
        )?;
        Ok(count)
    } else {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM entities WHERE entity_type = 'document'",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }
}
