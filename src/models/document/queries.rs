use sqlx::PgPool;
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
pub async fn find_all(
    pool: &PgPool,
    tor_id: Option<i64>,
    search: Option<&str>,
) -> Result<Vec<DocumentListItem>, AppError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: i64,
        title: String,
        doc_type: String,
        created_by_id: String,
        created_by_name: String,
        created_date: String,
        tor_id: Option<i64>,
        tor_name: Option<String>,
    }

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
                        ON CAST(p_by.value AS BIGINT) = u.id \
                    LEFT JOIN entity_properties p_date \
                        ON e.id = p_date.entity_id AND p_date.key = 'created_date' \
                    LEFT JOIN relations r ON e.id = r.source_id \
                        AND r.relation_type_id = ( \
                            SELECT id FROM entities \
                            WHERE entity_type = 'relation_type' AND name = 'scoped_to_tor') \
                    LEFT JOIN entities t ON r.target_id = t.id \
                    WHERE e.entity_type = 'document'";

    // Build dynamic SQL with numbered parameters
    let mut where_clause = String::new();
    let mut param_index = 1u32;

    if tor_id.is_some() {
        where_clause.push_str(&format!(" AND r.target_id = ${}", param_index));
        param_index += 1;
    }

    if search.is_some() {
        where_clause.push_str(&format!(
            " AND (p_title.value LIKE ${} OR p_type.value LIKE ${})",
            param_index,
            param_index + 1
        ));
        // param_index += 2; // not needed after last use
    }

    let sql = format!("{}{} ORDER BY p_date.value DESC", base_sql, where_clause);

    // We need to dynamically bind parameters based on which filters are active
    let mut query = sqlx::query_as::<_, Row>(&sql);

    if let Some(tid) = tor_id {
        query = query.bind(tid);
    }

    if let Some(q) = search {
        let search_pattern = format!("%{}%", q);
        query = query.bind(search_pattern.clone());
        query = query.bind(search_pattern);
    }

    let rows = query.fetch_all(pool).await?;

    let items = rows
        .into_iter()
        .map(|row| {
            let created_by_id: i64 = row.created_by_id.parse().unwrap_or(0);
            DocumentListItem {
                id: row.id,
                title: row.title,
                doc_type: row.doc_type,
                created_by_id,
                created_by_name: row.created_by_name,
                created_date: row.created_date,
                tor_id: row.tor_id,
                tor_name: row.tor_name,
            }
        })
        .collect();

    Ok(items)
}

/// Find a single document by ID.
pub async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<DocumentDetail>, AppError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: i64,
        title: String,
        doc_type: String,
        body: String,
        created_by_id: String,
        created_by_name: String,
        created_date: String,
        updated_date: String,
        tor_id: i64,
        tor_name: String,
    }

    let row = sqlx::query_as::<_, Row>(
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
             ON CAST(p_by.value AS BIGINT) = u.id \
         LEFT JOIN entity_properties p_date \
             ON e.id = p_date.entity_id AND p_date.key = 'created_date' \
         LEFT JOIN entity_properties p_updated \
             ON e.id = p_updated.entity_id AND p_updated.key = 'updated_date' \
         LEFT JOIN relations r ON e.id = r.source_id \
             AND r.relation_type_id = ( \
                 SELECT id FROM entities \
                 WHERE entity_type = 'relation_type' AND name = 'scoped_to_tor') \
         LEFT JOIN entities t ON r.target_id = t.id \
         WHERE e.id = $1 AND e.entity_type = 'document'",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| {
        let created_by_id: i64 = r.created_by_id.parse().unwrap_or(0);
        DocumentDetail {
            id: r.id,
            title: r.title,
            doc_type: r.doc_type,
            body: r.body,
            created_by_id,
            created_by_name: r.created_by_name,
            created_date: r.created_date,
            updated_date: r.updated_date,
            tor_id: r.tor_id,
            tor_name: r.tor_name,
        }
    }))
}

/// Create a new document.
pub async fn create(
    pool: &PgPool,
    title: &str,
    doc_type: &str,
    body: &str,
    created_by_id: i64,
    tor_id: Option<i64>,
) -> Result<i64, AppError> {
    let name = name_from_title(title);
    let doc_id = entity::create(pool, "document", &name, title).await?;

    // Get today's date from PostgreSQL
    let today: (String,) = sqlx::query_as("SELECT CURRENT_DATE::TEXT")
        .fetch_one(pool)
        .await?;

    entity::set_property(pool, doc_id, "title", title).await?;
    entity::set_property(pool, doc_id, "doc_type", doc_type).await?;
    entity::set_property(pool, doc_id, "body", body).await?;
    entity::set_property(pool, doc_id, "created_by_id", &created_by_id.to_string()).await?;
    entity::set_property(pool, doc_id, "created_date", &today.0).await?;

    if let Some(tid) = tor_id {
        relation::create(pool, "scoped_to_tor", doc_id, tid).await?;
    }

    Ok(doc_id)
}

/// Update an existing document.
pub async fn update(
    pool: &PgPool,
    doc_id: i64,
    title: &str,
    doc_type: &str,
    body: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE entities SET label = $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(title)
    .bind(doc_id)
    .execute(pool)
    .await?;

    entity::set_property(pool, doc_id, "title", title).await?;
    entity::set_property(pool, doc_id, "doc_type", doc_type).await?;
    entity::set_property(pool, doc_id, "body", body).await?;

    // Get today's date from PostgreSQL for updated_date property
    let today: (String,) = sqlx::query_as("SELECT CURRENT_DATE::TEXT")
        .fetch_one(pool)
        .await?;
    entity::set_property(pool, doc_id, "updated_date", &today.0).await?;

    Ok(())
}

/// Delete a document and its relations.
pub async fn delete(pool: &PgPool, doc_id: i64) -> Result<(), AppError> {
    entity::delete(pool, doc_id).await?;
    Ok(())
}

/// Count documents, optionally by ToR.
pub async fn count(pool: &PgPool, tor_id: Option<i64>) -> Result<i64, AppError> {
    if let Some(tid) = tor_id {
        let result: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM entities e \
             LEFT JOIN relations r ON e.id = r.source_id \
                 AND r.relation_type_id = ( \
                     SELECT id FROM entities \
                     WHERE entity_type = 'relation_type' AND name = 'scoped_to_tor') \
             WHERE e.entity_type = 'document' AND r.target_id = $1",
        )
        .bind(tid)
        .fetch_one(pool)
        .await?;
        Ok(result.0)
    } else {
        let result: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM entities WHERE entity_type = 'document'",
        )
        .fetch_one(pool)
        .await?;
        Ok(result.0)
    }
}
