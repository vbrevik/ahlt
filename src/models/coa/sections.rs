use sqlx::PgPool;
use crate::errors::AppError;
use super::types::{CoaSection, CoaSubsection};

/// Intermediate row struct for section queries.
#[derive(sqlx::FromRow)]
struct SectionRow {
    id: i64,
    title: String,
    content: String,
    order_num: String,
}

/// Find all sections for a given COA via has_section relation.
pub async fn find_sections(pool: &PgPool, coa_id: i64) -> Result<Vec<CoaSection>, AppError> {
    let rows = sqlx::query_as::<_, SectionRow>(
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
         WHERE e.entity_type = 'coa_section' AND r.source_id = $1 \
         ORDER BY CAST(COALESCE(p_order.value, '0') AS BIGINT) ASC",
    )
    .bind(coa_id)
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for row in rows {
        let order: i32 = row.order_num.parse().unwrap_or(0);
        let subsections = find_subsections(pool, row.id).await?;
        result.push(CoaSection {
            id: row.id,
            title: row.title,
            content: row.content,
            order,
            subsections,
        });
    }

    Ok(result)
}

/// Find all subsections for a given section via has_subsection relation.
async fn find_subsections(pool: &PgPool, section_id: i64) -> Result<Vec<CoaSubsection>, AppError> {
    let rows = sqlx::query_as::<_, SectionRow>(
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
         WHERE e.entity_type = 'coa_section' AND r.source_id = $1 \
         ORDER BY CAST(COALESCE(p_order.value, '0') AS BIGINT) ASC",
    )
    .bind(section_id)
    .fetch_all(pool)
    .await?;

    let items = rows.into_iter().map(|row| {
        let order: i32 = row.order_num.parse().unwrap_or(0);
        CoaSubsection {
            id: row.id,
            title: row.title,
            content: row.content,
            order,
        }
    }).collect();

    Ok(items)
}
