use rusqlite::{Connection, params};
use super::types::*;

/// Find all presentation templates for a ToR.
pub fn find_templates_for_tor(conn: &Connection, tor_id: i64) -> rusqlite::Result<Vec<PresentationTemplate>> {
    let mut stmt = conn.prepare(
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
         WHERE r.target_id = ?1 \
           AND r.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'template_of') \
           AND t.entity_type = 'presentation_template' \
         ORDER BY t.label",
    )?;

    let templates = stmt
        .query_map(params![tor_id], |row| {
            Ok(PresentationTemplate {
                id: row.get("id")?,
                name: row.get("name")?,
                label: row.get("label")?,
                description: row.get("description")?,
                slide_count: row.get("slide_count")?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(templates)
}

/// Find all slides for a template, ordered by slide_order.
pub fn find_slides(conn: &Connection, template_id: i64) -> rusqlite::Result<Vec<TemplateSlide>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.name, s.label, \
                CAST(COALESCE(p_order.value, '0') AS INTEGER) AS slide_order, \
                COALESCE(p_content.value, '') AS required_content, \
                COALESCE(p_notes.value, '') AS notes \
         FROM entities s \
         JOIN relations r ON s.id = r.source_id \
         LEFT JOIN entity_properties p_order ON s.id = p_order.entity_id AND p_order.key = 'slide_order' \
         LEFT JOIN entity_properties p_content ON s.id = p_content.entity_id AND p_content.key = 'required_content' \
         LEFT JOIN entity_properties p_notes ON s.id = p_notes.entity_id AND p_notes.key = 'notes' \
         WHERE r.target_id = ?1 \
           AND r.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'slide_of') \
           AND s.entity_type = 'template_slide' \
         ORDER BY CAST(COALESCE(p_order.value, '0') AS INTEGER)",
    )?;

    let slides = stmt
        .query_map(params![template_id], |row| {
            Ok(TemplateSlide {
                id: row.get("id")?,
                name: row.get("name")?,
                label: row.get("label")?,
                slide_order: row.get("slide_order")?,
                required_content: row.get("required_content")?,
                notes: row.get("notes")?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(slides)
}

/// Create a presentation template for a ToR.
pub fn create_template(
    conn: &Connection,
    tor_id: i64,
    name: &str,
    label: &str,
    description: &str,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('presentation_template', ?1, ?2)",
        params![name, label],
    )?;
    let template_id = conn.last_insert_rowid();

    if !description.is_empty() {
        conn.execute(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'description', ?2)",
            params![template_id, description],
        )?;
    }

    // Link to ToR via template_of
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'template_of'), ?1, ?2)",
        params![template_id, tor_id],
    )?;

    Ok(template_id)
}

/// Delete a presentation template.
pub fn delete_template(conn: &Connection, template_id: i64) -> rusqlite::Result<()> {
    // CASCADE will delete slides, properties, and relations
    conn.execute(
        "DELETE FROM entities WHERE id = ?1 AND entity_type = 'presentation_template'",
        params![template_id],
    )?;
    Ok(())
}

/// Add a slide to a template.
pub fn add_slide(
    conn: &Connection,
    template_id: i64,
    name: &str,
    label: &str,
    slide_order: i64,
    required_content: &str,
    notes: &str,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('template_slide', ?1, ?2)",
        params![name, label],
    )?;
    let slide_id = conn.last_insert_rowid();

    let props: [(&str, String); 3] = [
        ("slide_order", slide_order.to_string()),
        ("required_content", required_content.to_string()),
        ("notes", notes.to_string()),
    ];

    for (key, value) in &props {
        if !value.is_empty() {
            conn.execute(
                "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
                params![slide_id, key, value],
            )?;
        }
    }

    // Link to template via slide_of
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'slide_of'), ?1, ?2)",
        params![slide_id, template_id],
    )?;

    Ok(slide_id)
}

/// Delete a slide.
pub fn delete_slide(conn: &Connection, slide_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM entities WHERE id = ?1 AND entity_type = 'template_slide'",
        params![slide_id],
    )?;
    Ok(())
}

/// Reorder two slides by swapping their slide_order values.
pub fn reorder_slides(conn: &Connection, slide_a_id: i64, slide_b_id: i64) -> rusqlite::Result<()> {
    let order_a: String = conn.query_row(
        "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = 'slide_order'",
        params![slide_a_id],
        |row| row.get(0),
    )?;
    let order_b: String = conn.query_row(
        "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = 'slide_order'",
        params![slide_b_id],
        |row| row.get(0),
    )?;

    conn.execute(
        "UPDATE entity_properties SET value = ?1 WHERE entity_id = ?2 AND key = 'slide_order'",
        params![order_b, slide_a_id],
    )?;
    conn.execute(
        "UPDATE entity_properties SET value = ?1 WHERE entity_id = ?2 AND key = 'slide_order'",
        params![order_a, slide_b_id],
    )?;

    Ok(())
}
