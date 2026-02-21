use sqlx::PgPool;
use crate::errors::AppError;
use crate::models::{entity, relation};
use super::types::*;
use super::sections;

/// Intermediate row struct for find_all_for_agenda_point query.
#[derive(sqlx::FromRow)]
struct CoaListRow {
    id: i64,
    title: String,
    coa_type: String,
    created_by: String,
    created_date: String,
}

/// Intermediate row struct for find_by_id query.
#[derive(sqlx::FromRow)]
struct CoaDetailRow {
    id: i64,
    title: String,
    description: String,
    coa_type: String,
    created_by: String,
    created_date: String,
}

/// Find all COAs considered for a specific agenda point via considers_coa relation.
pub async fn find_all_for_agenda_point(pool: &PgPool, agenda_point_id: i64) -> Result<Vec<CoaListItem>, AppError> {
    let rows = sqlx::query_as::<_, CoaListRow>(
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
         WHERE e.entity_type = 'coa' AND r.source_id = $1 \
         ORDER BY p_created_date.value DESC",
    )
    .bind(agenda_point_id)
    .fetch_all(pool)
    .await?;

    let items = rows.into_iter().map(|r| {
        CoaListItem {
            id: r.id,
            title: r.title,
            coa_type: r.coa_type,
            created_by: r.created_by.parse().unwrap_or(0),
            created_date: r.created_date,
        }
    }).collect();

    Ok(items)
}

/// Find a single COA by its entity id, loading all sections and subsections.
pub async fn find_by_id(pool: &PgPool, id: i64) -> Result<CoaDetail, AppError> {
    let row = sqlx::query_as::<_, CoaDetailRow>(
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
         WHERE e.id = $1 AND e.entity_type = 'coa'",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => {
            let created_by: i64 = r.created_by.parse().unwrap_or(0);
            let coa_sections = sections::find_sections(pool, r.id).await?;
            Ok(CoaDetail {
                id: r.id,
                title: r.title,
                description: r.description,
                coa_type: r.coa_type,
                created_by,
                created_date: r.created_date,
                sections: coa_sections,
            })
        }
        None => Err(AppError::NotFound),
    }
}

/// Create a new COA entity and set its properties.
/// Returns the new COA id.
pub async fn create(
    pool: &PgPool,
    title: &str,
    description: &str,
    coa_type: &str,  // "simple" or "complex"
    created_by_id: i64,
) -> Result<i64, AppError> {
    let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let name = format!("coa_{}_{}", title.replace(' ', "_").to_lowercase(), now.replace(':', "_"));
    let label = title.to_string();

    let coa_id = entity::create(pool, "coa", &name, &label).await.map_err(AppError::Db)?;

    entity::set_property(pool, coa_id, "title", title).await.map_err(AppError::Db)?;
    entity::set_property(pool, coa_id, "description", description).await.map_err(AppError::Db)?;
    entity::set_property(pool, coa_id, "coa_type", coa_type).await.map_err(AppError::Db)?;
    entity::set_property(pool, coa_id, "created_by", &created_by_id.to_string()).await.map_err(AppError::Db)?;
    entity::set_property(pool, coa_id, "created_date", &now).await.map_err(AppError::Db)?;

    Ok(coa_id)
}

/// Update COA title and description.
pub async fn update(
    pool: &PgPool,
    id: i64,
    title: &str,
    description: &str,
) -> Result<(), AppError> {
    entity::set_property(pool, id, "title", title).await.map_err(AppError::Db)?;
    entity::set_property(pool, id, "description", description).await.map_err(AppError::Db)?;
    Ok(())
}

/// Add a section to a COA via has_section relation.
/// Returns the new section entity id.
pub async fn add_section(
    pool: &PgPool,
    coa_id: i64,
    title: &str,
    content: &str,
    order: i32,
) -> Result<i64, AppError> {
    let name = format!("coa_section_{}_{}_{}", coa_id, order, title.replace(' ', "_").to_lowercase());
    let label = title.to_string();

    let section_id = entity::create(pool, "coa_section", &name, &label).await.map_err(AppError::Db)?;

    entity::set_property(pool, section_id, "title", title).await.map_err(AppError::Db)?;
    entity::set_property(pool, section_id, "content", content).await.map_err(AppError::Db)?;
    entity::set_property(pool, section_id, "order", &order.to_string()).await.map_err(AppError::Db)?;

    relation::create(pool, "has_section", coa_id, section_id).await.map_err(AppError::Db)?;

    Ok(section_id)
}
