use sqlx::PgPool;
use super::types::*;

/// Intermediate row struct for MinutesSection with is_auto_generated as String from DB.
#[derive(sqlx::FromRow)]
struct MinutesSectionRow {
    id: i64,
    name: String,
    label: String,
    section_type: String,
    sequence_order: i64,
    content: String,
    is_auto_generated: String,
}

/// Find minutes for a specific meeting.
pub async fn find_by_meeting(pool: &PgPool, meeting_id: i64) -> Result<Option<Minutes>, sqlx::Error> {
    let row = sqlx::query_as::<_, Minutes>(
        "SELECT m.id, m.name, m.label, \
                COALESCE(p_status.value, 'draft') AS status, \
                COALESCE(p_date.value, '') AS generated_date, \
                r.source_id AS meeting_id, \
                COALESCE(mtg.name, '') AS meeting_name, \
                COALESCE(p_appr_by.value, '') AS approved_by, \
                COALESCE(p_appr_date.value, '') AS approved_date, \
                COALESCE(p_dist.value, '[]') AS distribution_list, \
                COALESCE(p_att.value, '[]') AS structured_attendance, \
                COALESCE(p_ai.value, '[]') AS structured_action_items \
         FROM entities m \
         JOIN relations r ON m.id = r.target_id \
         JOIN entities mtg ON r.source_id = mtg.id \
         LEFT JOIN entity_properties p_status ON m.id = p_status.entity_id AND p_status.key = 'status' \
         LEFT JOIN entity_properties p_date ON m.id = p_date.entity_id AND p_date.key = 'generated_date' \
         LEFT JOIN entity_properties p_appr_by ON m.id = p_appr_by.entity_id AND p_appr_by.key = 'approved_by' \
         LEFT JOIN entity_properties p_appr_date ON m.id = p_appr_date.entity_id AND p_appr_date.key = 'approved_date' \
         LEFT JOIN entity_properties p_dist ON m.id = p_dist.entity_id AND p_dist.key = 'distribution_list' \
         LEFT JOIN entity_properties p_att ON m.id = p_att.entity_id AND p_att.key = 'structured_attendance' \
         LEFT JOIN entity_properties p_ai ON m.id = p_ai.entity_id AND p_ai.key = 'structured_action_items' \
         WHERE r.source_id = $1 \
           AND r.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'minutes_of') \
           AND m.entity_type = 'minutes'",
    )
    .bind(meeting_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Find minutes by ID.
pub async fn find_by_id(pool: &PgPool, minutes_id: i64) -> Result<Option<Minutes>, sqlx::Error> {
    let row = sqlx::query_as::<_, Minutes>(
        "SELECT m.id, m.name, m.label, \
                COALESCE(p_status.value, 'draft') AS status, \
                COALESCE(p_date.value, '') AS generated_date, \
                r.source_id AS meeting_id, \
                COALESCE(mtg.name, '') AS meeting_name, \
                COALESCE(p_appr_by.value, '') AS approved_by, \
                COALESCE(p_appr_date.value, '') AS approved_date, \
                COALESCE(p_dist.value, '[]') AS distribution_list, \
                COALESCE(p_att.value, '[]') AS structured_attendance, \
                COALESCE(p_ai.value, '[]') AS structured_action_items \
         FROM entities m \
         JOIN relations r ON m.id = r.target_id \
         JOIN entities mtg ON r.source_id = mtg.id \
         LEFT JOIN entity_properties p_status ON m.id = p_status.entity_id AND p_status.key = 'status' \
         LEFT JOIN entity_properties p_date ON m.id = p_date.entity_id AND p_date.key = 'generated_date' \
         LEFT JOIN entity_properties p_appr_by ON m.id = p_appr_by.entity_id AND p_appr_by.key = 'approved_by' \
         LEFT JOIN entity_properties p_appr_date ON m.id = p_appr_date.entity_id AND p_appr_date.key = 'approved_date' \
         LEFT JOIN entity_properties p_dist ON m.id = p_dist.entity_id AND p_dist.key = 'distribution_list' \
         LEFT JOIN entity_properties p_att ON m.id = p_att.entity_id AND p_att.key = 'structured_attendance' \
         LEFT JOIN entity_properties p_ai ON m.id = p_ai.entity_id AND p_ai.key = 'structured_action_items' \
         WHERE m.id = $1 \
           AND r.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'minutes_of') \
           AND m.entity_type = 'minutes'",
    )
    .bind(minutes_id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Find all sections of a minutes document, ordered by sequence.
pub async fn find_sections(pool: &PgPool, minutes_id: i64) -> Result<Vec<MinutesSection>, sqlx::Error> {
    let rows = sqlx::query_as::<_, MinutesSectionRow>(
        "SELECT s.id, s.name, s.label, \
                COALESCE(p_type.value, '') AS section_type, \
                CAST(COALESCE(p_order.value, '0') AS BIGINT) AS sequence_order, \
                COALESCE(p_content.value, '') AS content, \
                COALESCE(p_auto.value, 'false') AS is_auto_generated \
         FROM entities s \
         JOIN relations r ON s.id = r.source_id \
         LEFT JOIN entity_properties p_type ON s.id = p_type.entity_id AND p_type.key = 'section_type' \
         LEFT JOIN entity_properties p_order ON s.id = p_order.entity_id AND p_order.key = 'sequence_order' \
         LEFT JOIN entity_properties p_content ON s.id = p_content.entity_id AND p_content.key = 'content' \
         LEFT JOIN entity_properties p_auto ON s.id = p_auto.entity_id AND p_auto.key = 'is_auto_generated' \
         WHERE r.target_id = $1 \
           AND r.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'section_of') \
           AND s.entity_type = 'minutes_section' \
         ORDER BY CAST(COALESCE(p_order.value, '0') AS BIGINT)",
    )
    .bind(minutes_id)
    .fetch_all(pool)
    .await?;

    let sections = rows.into_iter().map(|r| {
        MinutesSection {
            id: r.id,
            name: r.name,
            label: r.label,
            section_type: r.section_type,
            sequence_order: r.sequence_order,
            content: r.content,
            is_auto_generated: r.is_auto_generated == "true",
        }
    }).collect();

    Ok(sections)
}

/// Generate a minutes scaffold for a meeting.
/// Creates the minutes entity and auto-generated sections.
pub async fn generate_scaffold(
    pool: &PgPool,
    meeting_id: i64,
    tor_id: i64,
    meeting_name: &str,
) -> Result<i64, sqlx::Error> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let minutes_name = format!("minutes_{}", meeting_name.to_lowercase().replace(' ', "_"));
    let minutes_label = format!("Minutes \u{2014} {}", meeting_name);

    // Create minutes entity
    let (minutes_id,): (i64,) = sqlx::query_as(
        "INSERT INTO entities (entity_type, name, label) VALUES ('minutes', $1, $2) RETURNING id",
    )
    .bind(&minutes_name)
    .bind(&minutes_label)
    .fetch_one(pool)
    .await?;

    // Set properties
    let props = vec![
        ("status", "draft"),
        ("generated_date", &today),
    ];
    for (key, value) in props {
        sqlx::query(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, $2, $3)",
        )
        .bind(minutes_id)
        .bind(key)
        .bind(value)
        .execute(pool)
        .await?;
    }

    // Link to meeting via minutes_of relation (meeting -> minutes)
    sqlx::query(
        "INSERT INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'minutes_of'), $1, $2)",
    )
    .bind(meeting_id)
    .bind(minutes_id)
    .execute(pool)
    .await?;

    // Generate sections
    let sections = [
        ("attendance", "Attendance", generate_attendance_content(pool, tor_id).await?),
        ("protocol", "Meeting Protocol", generate_protocol_content(pool, tor_id).await?),
        ("agenda_items", "Agenda Items", "No agenda items recorded.".to_string()),
        ("decisions", "Decisions", "No decisions recorded.".to_string()),
        ("action_items", "Action Items", "No action items recorded.".to_string()),
    ];

    for (i, (section_type, label, content)) in sections.iter().enumerate() {
        let section_name = format!("{}_{}", section_type, minutes_id);
        let (section_id,): (i64,) = sqlx::query_as(
            "INSERT INTO entities (entity_type, name, label) VALUES ('minutes_section', $1, $2) RETURNING id",
        )
        .bind(&section_name)
        .bind(label)
        .fetch_one(pool)
        .await?;

        let section_props = [
            ("section_type", section_type.to_string()),
            ("sequence_order", (i + 1).to_string()),
            ("content", content.to_string()),
            ("is_auto_generated", "true".to_string()),
        ];
        for (key, value) in &section_props {
            sqlx::query(
                "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, $2, $3)",
            )
            .bind(section_id)
            .bind(key)
            .bind(value)
            .execute(pool)
            .await?;
        }

        // Link section to minutes via section_of (section -> minutes)
        sqlx::query(
            "INSERT INTO relations (relation_type_id, source_id, target_id) \
             VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'section_of'), $1, $2)",
        )
        .bind(section_id)
        .bind(minutes_id)
        .execute(pool)
        .await?;
    }

    Ok(minutes_id)
}

/// Generate attendance content showing positions and their holders.
async fn generate_attendance_content(pool: &PgPool, tor_id: i64) -> Result<String, sqlx::Error> {
    use crate::models::tor;
    let members = tor::find_members(pool, tor_id).await?;
    let mut lines = Vec::new();
    lines.push("## Attendance\n".to_string());

    for m in &members {
        let holder = match &m.holder_label {
            Some(label) => format!("**{}**", label),
            None => {
                if m.membership_type == "mandatory" {
                    "**VACANT \u{2014} mandatory position**".to_string()
                } else {
                    "_Vacant_".to_string()
                }
            }
        };
        let mt_badge = if m.membership_type == "mandatory" { " [M]" } else { "" };
        lines.push(format!("- {} \u{2014} {}{}", m.position_label, holder, mt_badge));
    }

    Ok(lines.join("\n"))
}

/// Generate protocol content from ToR protocol steps.
async fn generate_protocol_content(pool: &PgPool, tor_id: i64) -> Result<String, sqlx::Error> {
    use crate::models::protocol;
    let steps = protocol::find_steps_for_tor(pool, tor_id).await?;
    let mut lines = Vec::new();
    lines.push("## Meeting Protocol\n".to_string());

    for step in &steps {
        let duration = match step.default_duration_minutes {
            Some(d) => format!(" ({} min)", d),
            None => String::new(),
        };
        let required = if step.is_required { " *" } else { "" };
        lines.push(format!("{}. {}{}{}", step.sequence_order, step.label, duration, required));
    }

    Ok(lines.join("\n"))
}

/// Update a section's content.
pub async fn update_section_content(pool: &PgPool, section_id: i64, content: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE entity_properties SET value = $1 WHERE entity_id = $2 AND key = 'content'",
    )
    .bind(content)
    .bind(section_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Update minutes status.
pub async fn update_status(pool: &PgPool, minutes_id: i64, new_status: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE entity_properties SET value = $1 WHERE entity_id = $2 AND key = 'status'",
    )
    .bind(new_status)
    .bind(minutes_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Upsert the distribution list (JSON string) for a minutes entity.
pub async fn update_distribution_list(pool: &PgPool, minutes_id: i64, json: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'distribution_list', $2) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(minutes_id)
    .bind(json)
    .execute(pool)
    .await?;
    Ok(())
}

/// Upsert the structured attendance (JSON string) for a minutes entity.
pub async fn update_structured_attendance(pool: &PgPool, minutes_id: i64, json: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'structured_attendance', $2) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(minutes_id)
    .bind(json)
    .execute(pool)
    .await?;
    Ok(())
}

/// Upsert the structured action items (JSON string) for a minutes entity.
pub async fn update_structured_action_items(pool: &PgPool, minutes_id: i64, json: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'structured_action_items', $2) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(minutes_id)
    .bind(json)
    .execute(pool)
    .await?;
    Ok(())
}
