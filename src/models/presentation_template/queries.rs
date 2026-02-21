use sqlx::PgPool;
use super::types::*;

/// Find all presentation templates for a ToR.
pub async fn find_templates_for_tor(pool: &PgPool, tor_id: i64) -> Result<Vec<PresentationTemplate>, sqlx::Error> {
    let templates = sqlx::query_as::<_, PresentationTemplate>(
        "SELECT t.id, t.name, t.label, \
                COALESCE(p_desc.value, '') AS description, \
                (SELECT COUNT(*) FROM relations r_slide \
                 JOIN entities s ON r_slide.source_id = s.id \
                 WHERE r_slide.target_id = t.id \
                   AND r_slide.relation_type_id = ( \
                       SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'slide_of') \
                   AND s.entity_type = 'template_slide') AS slide_count \
         FROM entities t \
         JOIN relations r ON t.id = r.source_id \
         LEFT JOIN entity_properties p_desc ON t.id = p_desc.entity_id AND p_desc.key = 'description' \
         WHERE r.target_id = $1 \
           AND r.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'template_of') \
           AND t.entity_type = 'presentation_template' \
         ORDER BY t.label",
    )
    .bind(tor_id)
    .fetch_all(pool)
    .await?;

    Ok(templates)
}

/// Find all slides for a template, ordered by slide_order.
pub async fn find_slides(pool: &PgPool, template_id: i64) -> Result<Vec<TemplateSlide>, sqlx::Error> {
    let slides = sqlx::query_as::<_, TemplateSlide>(
        "SELECT s.id, s.name, s.label, \
                CAST(COALESCE(p_order.value, '0') AS BIGINT) AS slide_order, \
                COALESCE(p_content.value, '') AS required_content, \
                COALESCE(p_notes.value, '') AS notes \
         FROM entities s \
         JOIN relations r ON s.id = r.source_id \
         LEFT JOIN entity_properties p_order ON s.id = p_order.entity_id AND p_order.key = 'slide_order' \
         LEFT JOIN entity_properties p_content ON s.id = p_content.entity_id AND p_content.key = 'required_content' \
         LEFT JOIN entity_properties p_notes ON s.id = p_notes.entity_id AND p_notes.key = 'notes' \
         WHERE r.target_id = $1 \
           AND r.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'slide_of') \
           AND s.entity_type = 'template_slide' \
         ORDER BY CAST(COALESCE(p_order.value, '0') AS BIGINT)",
    )
    .bind(template_id)
    .fetch_all(pool)
    .await?;

    Ok(slides)
}

/// Create a presentation template for a ToR.
pub async fn create_template(
    pool: &PgPool,
    tor_id: i64,
    name: &str,
    label: &str,
    description: &str,
) -> Result<i64, sqlx::Error> {
    let template_id: (i64,) = sqlx::query_as(
        "INSERT INTO entities (entity_type, name, label) VALUES ('presentation_template', $1, $2) RETURNING id",
    )
    .bind(name)
    .bind(label)
    .fetch_one(pool)
    .await?;
    let template_id = template_id.0;

    if !description.is_empty() {
        sqlx::query(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'description', $2)",
        )
        .bind(template_id)
        .bind(description)
        .execute(pool)
        .await?;
    }

    // Link to ToR via template_of
    sqlx::query(
        "INSERT INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'template_of'), $1, $2)",
    )
    .bind(template_id)
    .bind(tor_id)
    .execute(pool)
    .await?;

    Ok(template_id)
}

/// Delete a presentation template.
pub async fn delete_template(pool: &PgPool, template_id: i64) -> Result<(), sqlx::Error> {
    // CASCADE will delete slides, properties, and relations
    sqlx::query(
        "DELETE FROM entities WHERE id = $1 AND entity_type = 'presentation_template'",
    )
    .bind(template_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Add a slide to a template.
pub async fn add_slide(
    pool: &PgPool,
    template_id: i64,
    name: &str,
    label: &str,
    slide_order: i64,
    required_content: &str,
    notes: &str,
) -> Result<i64, sqlx::Error> {
    let slide_id: (i64,) = sqlx::query_as(
        "INSERT INTO entities (entity_type, name, label) VALUES ('template_slide', $1, $2) RETURNING id",
    )
    .bind(name)
    .bind(label)
    .fetch_one(pool)
    .await?;
    let slide_id = slide_id.0;

    let props: [(&str, String); 3] = [
        ("slide_order", slide_order.to_string()),
        ("required_content", required_content.to_string()),
        ("notes", notes.to_string()),
    ];

    for (key, value) in &props {
        if !value.is_empty() {
            sqlx::query(
                "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, $2, $3)",
            )
            .bind(slide_id)
            .bind(key)
            .bind(value)
            .execute(pool)
            .await?;
        }
    }

    // Link to template via slide_of
    sqlx::query(
        "INSERT INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'slide_of'), $1, $2)",
    )
    .bind(slide_id)
    .bind(template_id)
    .execute(pool)
    .await?;

    Ok(slide_id)
}

/// Delete a slide.
pub async fn delete_slide(pool: &PgPool, slide_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM entities WHERE id = $1 AND entity_type = 'template_slide'",
    )
    .bind(slide_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Reorder two slides by swapping their slide_order values.
pub async fn reorder_slides(pool: &PgPool, slide_a_id: i64, slide_b_id: i64) -> Result<(), sqlx::Error> {
    let order_a: (String,) = sqlx::query_as(
        "SELECT value FROM entity_properties WHERE entity_id = $1 AND key = 'slide_order'",
    )
    .bind(slide_a_id)
    .fetch_one(pool)
    .await?;

    let order_b: (String,) = sqlx::query_as(
        "SELECT value FROM entity_properties WHERE entity_id = $1 AND key = 'slide_order'",
    )
    .bind(slide_b_id)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        "UPDATE entity_properties SET value = $1 WHERE entity_id = $2 AND key = 'slide_order'",
    )
    .bind(&order_b.0)
    .bind(slide_a_id)
    .execute(pool)
    .await?;

    sqlx::query(
        "UPDATE entity_properties SET value = $1 WHERE entity_id = $2 AND key = 'slide_order'",
    )
    .bind(&order_a.0)
    .bind(slide_b_id)
    .execute(pool)
    .await?;

    Ok(())
}
