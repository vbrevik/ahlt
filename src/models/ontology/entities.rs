use rusqlite::Connection;

/// Entity row for the data browser list view.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct EntityListItem {
    pub id: i64,
    pub entity_type: String,
    pub name: String,
    pub label: String,
}

/// A single property key-value pair.
#[derive(Debug, Clone)]
pub struct EntityProperty {
    pub key: String,
    pub value: String,
}

/// A related entity (used for both incoming and outgoing relations).
#[derive(Debug, Clone)]
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
pub fn find_entity_list(conn: &Connection, type_filter: Option<&str>) -> rusqlite::Result<Vec<EntityListItem>> {
    let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match type_filter {
        Some(t) if !t.is_empty() => (
            "SELECT id, entity_type, name, label FROM entities WHERE entity_type = ?1 ORDER BY entity_type, sort_order, id".to_string(),
            vec![Box::new(t.to_string()) as Box<dyn rusqlite::types::ToSql>],
        ),
        _ => (
            "SELECT id, entity_type, name, label FROM entities ORDER BY entity_type, sort_order, id".to_string(),
            vec![],
        ),
    };
    let mut stmt = conn.prepare(&sql)?;
    let params_ref: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let items = stmt.query_map(params_ref.as_slice(), |row| {
        Ok(EntityListItem {
            id: row.get(0)?,
            entity_type: row.get(1)?,
            name: row.get(2)?,
            label: row.get(3)?,
        })
    })?.collect::<Result<_, _>>()?;
    Ok(items)
}

/// Get full detail for a single entity by id.
pub fn find_entity_detail(conn: &Connection, id: i64) -> rusqlite::Result<Option<EntityDetail>> {
    let mut stmt = conn.prepare(
        "SELECT id, entity_type, name, label, sort_order, is_active, created_at, updated_at \
         FROM entities WHERE id = ?1"
    )?;
    let mut rows = stmt.query_map([id], |row| {
        Ok(EntityDetail {
            id: row.get(0)?,
            entity_type: row.get(1)?,
            name: row.get(2)?,
            label: row.get(3)?,
            sort_order: row.get(4)?,
            is_active: row.get::<_, i64>(5)? != 0,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
            properties: vec![],
            outgoing: vec![],
            incoming: vec![],
        })
    })?;

    let entity = match rows.next() {
        Some(Ok(e)) => e,
        _ => return Ok(None),
    };
    let mut entity = entity;

    // Properties
    let mut prop_stmt = conn.prepare(
        "SELECT key, value FROM entity_properties WHERE entity_id = ?1 ORDER BY key"
    )?;
    entity.properties = prop_stmt.query_map([id], |row| {
        Ok(EntityProperty {
            key: row.get(0)?,
            value: row.get(1)?,
        })
    })?.collect::<Result<_, _>>()?;

    // Outgoing relations (this entity is source)
    let mut out_stmt = conn.prepare(
        "SELECT tgt.id, tgt.entity_type, tgt.name, tgt.label, rt.name, rt.label \
         FROM relations r \
         JOIN entities tgt ON r.target_id = tgt.id \
         JOIN entities rt ON r.relation_type_id = rt.id \
         WHERE r.source_id = ?1 \
         ORDER BY rt.name, tgt.name"
    )?;
    entity.outgoing = out_stmt.query_map([id], |row| {
        Ok(RelatedEntity {
            id: row.get(0)?,
            entity_type: row.get(1)?,
            name: row.get(2)?,
            label: row.get(3)?,
            relation_type: row.get(4)?,
            relation_label: row.get(5)?,
        })
    })?.collect::<Result<_, _>>()?;

    // Incoming relations (this entity is target)
    let mut in_stmt = conn.prepare(
        "SELECT src.id, src.entity_type, src.name, src.label, rt.name, rt.label \
         FROM relations r \
         JOIN entities src ON r.source_id = src.id \
         JOIN entities rt ON r.relation_type_id = rt.id \
         WHERE r.target_id = ?1 \
         ORDER BY rt.name, src.name"
    )?;
    entity.incoming = in_stmt.query_map([id], |row| {
        Ok(RelatedEntity {
            id: row.get(0)?,
            entity_type: row.get(1)?,
            name: row.get(2)?,
            label: row.get(3)?,
            relation_type: row.get(4)?,
            relation_label: row.get(5)?,
        })
    })?.collect::<Result<_, _>>()?;

    Ok(Some(entity))
}

/// Get distinct entity types for filter UI.
#[allow(dead_code)]
pub fn find_entity_types(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT entity_type FROM entities ORDER BY entity_type"
    )?;
    let types = stmt.query_map([], |row| row.get(0))?.collect::<Result<_, _>>()?;
    Ok(types)
}
