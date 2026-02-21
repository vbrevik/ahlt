use sqlx::PgPool;

/// Entity row for the data browser list view.
#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct EntityListItem {
    pub id: i64,
    pub entity_type: String,
    pub name: String,
    pub label: String,
}

/// A single property key-value pair.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EntityProperty {
    pub key: String,
    pub value: String,
}

/// A related entity (used for both incoming and outgoing relations).
#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct RelatedEntity {
    pub id: i64,
    pub entity_type: String,
    pub name: String,
    pub label: String,
    pub relation_type: String,
    pub relation_label: String,
}

/// Full detail of a single entity: base fields + properties + relations.
#[derive(Debug, Clone)]
pub struct EntityDetail {
    pub id: i64,
    pub entity_type: String,
    pub name: String,
    pub label: String,
    pub sort_order: i64,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
    pub properties: Vec<EntityProperty>,
    pub outgoing: Vec<RelatedEntity>,
    pub incoming: Vec<RelatedEntity>,
}

/// List all entities, optionally filtered by entity_type.
#[allow(dead_code)]
pub async fn find_entity_list(pool: &PgPool, type_filter: Option<&str>) -> Result<Vec<EntityListItem>, sqlx::Error> {
    match type_filter {
        Some(t) if !t.is_empty() => {
            sqlx::query_as::<_, EntityListItem>(
                "SELECT id, entity_type, name, label FROM entities WHERE entity_type = $1 ORDER BY entity_type, sort_order, id"
            )
            .bind(t)
            .fetch_all(pool)
            .await
        },
        _ => {
            sqlx::query_as::<_, EntityListItem>(
                "SELECT id, entity_type, name, label FROM entities ORDER BY entity_type, sort_order, id"
            )
            .fetch_all(pool)
            .await
        },
    }
}

/// Get full detail for a single entity by id.
pub async fn find_entity_detail(pool: &PgPool, id: i64) -> Result<Option<EntityDetail>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct EntityBaseRow {
        id: i64,
        entity_type: String,
        name: String,
        label: String,
        sort_order: i64,
        is_active: bool,
        created_at: String,
        updated_at: String,
    }

    let base: Option<EntityBaseRow> = sqlx::query_as(
        "SELECT id, entity_type, name, label, sort_order::BIGINT AS sort_order, is_active, \
         created_at::TEXT, updated_at::TEXT \
         FROM entities WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    let base = match base {
        Some(b) => b,
        None => return Ok(None),
    };

    let mut entity = EntityDetail {
        id: base.id,
        entity_type: base.entity_type,
        name: base.name,
        label: base.label,
        sort_order: base.sort_order,
        is_active: base.is_active,
        created_at: base.created_at,
        updated_at: base.updated_at,
        properties: vec![],
        outgoing: vec![],
        incoming: vec![],
    };

    // Properties
    entity.properties = sqlx::query_as::<_, EntityProperty>(
        "SELECT key, value FROM entity_properties WHERE entity_id = $1 ORDER BY key"
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    // Outgoing relations (this entity is source)
    entity.outgoing = sqlx::query_as::<_, RelatedEntity>(
        "SELECT tgt.id, tgt.entity_type, tgt.name, tgt.label, rt.name AS relation_type, rt.label AS relation_label \
         FROM relations r \
         JOIN entities tgt ON r.target_id = tgt.id \
         JOIN entities rt ON r.relation_type_id = rt.id \
         WHERE r.source_id = $1 \
         ORDER BY rt.name, tgt.name"
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    // Incoming relations (this entity is target)
    entity.incoming = sqlx::query_as::<_, RelatedEntity>(
        "SELECT src.id, src.entity_type, src.name, src.label, rt.name AS relation_type, rt.label AS relation_label \
         FROM relations r \
         JOIN entities src ON r.source_id = src.id \
         JOIN entities rt ON r.relation_type_id = rt.id \
         WHERE r.target_id = $1 \
         ORDER BY rt.name, src.name"
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    Ok(Some(entity))
}

/// Get distinct entity types for filter UI.
#[allow(dead_code)]
pub async fn find_entity_types(pool: &PgPool) -> Result<Vec<String>, sqlx::Error> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT entity_type FROM entities ORDER BY entity_type"
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(t,)| t).collect())
}
