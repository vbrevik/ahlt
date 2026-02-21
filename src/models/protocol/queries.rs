use sqlx::PgPool;
use super::types::*;

pub async fn find_steps_for_tor(pool: &PgPool, tor_id: i64) -> Result<Vec<ProtocolStep>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: i64,
        name: String,
        label: String,
        step_type: String,
        sequence_order: i64,
        duration: Option<i64>,
        description: String,
        is_required: String,
        responsible: String,
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT e.id, e.name, e.label, \
                COALESCE(p_type.value, 'procedural') AS step_type, \
                CAST(COALESCE(p_order.value, '0') AS BIGINT) AS sequence_order, \
                CASE WHEN p_dur.value IS NOT NULL THEN CAST(p_dur.value AS BIGINT) ELSE NULL END AS duration, \
                COALESCE(p_desc.value, '') AS description, \
                COALESCE(p_req.value, 'true') AS is_required, \
                COALESCE(p_resp.value, '') AS responsible \
         FROM entities e \
         JOIN relations r ON e.id = r.source_id \
         LEFT JOIN entity_properties p_type ON e.id = p_type.entity_id AND p_type.key = 'step_type' \
         LEFT JOIN entity_properties p_order ON e.id = p_order.entity_id AND p_order.key = 'sequence_order' \
         LEFT JOIN entity_properties p_dur ON e.id = p_dur.entity_id AND p_dur.key = 'default_duration_minutes' \
         LEFT JOIN entity_properties p_desc ON e.id = p_desc.entity_id AND p_desc.key = 'description' \
         LEFT JOIN entity_properties p_req ON e.id = p_req.entity_id AND p_req.key = 'is_required' \
         LEFT JOIN entity_properties p_resp ON e.id = p_resp.entity_id AND p_resp.key = 'responsible' \
         WHERE r.target_id = $1 \
           AND r.relation_type_id = ( \
               SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'protocol_of') \
           AND e.entity_type = 'protocol_step' \
         ORDER BY CAST(COALESCE(p_order.value, '0') AS BIGINT)",
    )
    .bind(tor_id)
    .fetch_all(pool)
    .await?;

    let steps = rows
        .into_iter()
        .map(|row| ProtocolStep {
            id: row.id,
            name: row.name,
            label: row.label,
            step_type: row.step_type,
            sequence_order: row.sequence_order,
            default_duration_minutes: row.duration,
            description: row.description,
            is_required: row.is_required == "true",
            responsible: row.responsible,
        })
        .collect();

    Ok(steps)
}

pub async fn create_step(
    pool: &PgPool,
    tor_id: i64,
    name: &str,
    label: &str,
    step_type: &str,
    sequence_order: i64,
    default_duration_minutes: Option<i64>,
    description: &str,
    is_required: bool,
    responsible: &str,
) -> Result<i64, sqlx::Error> {
    let step_id: (i64,) = sqlx::query_as(
        "INSERT INTO entities (entity_type, name, label) VALUES ('protocol_step', $1, $2) RETURNING id",
    )
    .bind(name)
    .bind(label)
    .fetch_one(pool)
    .await?;
    let step_id = step_id.0;

    let props: Vec<(&str, String)> = vec![
        ("step_type", step_type.to_string()),
        ("sequence_order", sequence_order.to_string()),
        ("description", description.to_string()),
        ("is_required", if is_required { "true" } else { "false" }.to_string()),
        ("responsible", responsible.to_string()),
    ];

    for (key, value) in &props {
        if !value.is_empty() {
            sqlx::query(
                "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, $2, $3)",
            )
            .bind(step_id)
            .bind(key)
            .bind(value)
            .execute(pool)
            .await?;
        }
    }

    if let Some(dur) = default_duration_minutes {
        sqlx::query(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'default_duration_minutes', $2)",
        )
        .bind(step_id)
        .bind(dur.to_string())
        .execute(pool)
        .await?;
    }

    // Link to ToR via protocol_of relation
    sqlx::query(
        "INSERT INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'protocol_of'), $1, $2)",
    )
    .bind(step_id)
    .bind(tor_id)
    .execute(pool)
    .await?;

    Ok(step_id)
}

pub async fn delete_step(pool: &PgPool, step_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM entities WHERE id = $1 AND entity_type = 'protocol_step'",
    )
    .bind(step_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Swap sequence_order of two steps.
pub async fn reorder_steps(pool: &PgPool, step_a_id: i64, step_b_id: i64) -> Result<(), sqlx::Error> {
    let order_a: (String,) = sqlx::query_as(
        "SELECT value FROM entity_properties WHERE entity_id = $1 AND key = 'sequence_order'",
    )
    .bind(step_a_id)
    .fetch_one(pool)
    .await?;

    let order_b: (String,) = sqlx::query_as(
        "SELECT value FROM entity_properties WHERE entity_id = $1 AND key = 'sequence_order'",
    )
    .bind(step_b_id)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        "UPDATE entity_properties SET value = $1 WHERE entity_id = $2 AND key = 'sequence_order'",
    )
    .bind(&order_b.0)
    .bind(step_a_id)
    .execute(pool)
    .await?;

    sqlx::query(
        "UPDATE entity_properties SET value = $1 WHERE entity_id = $2 AND key = 'sequence_order'",
    )
    .bind(&order_a.0)
    .bind(step_b_id)
    .execute(pool)
    .await?;

    Ok(())
}
