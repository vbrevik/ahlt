#![allow(dead_code)]
use rusqlite::{Connection, params};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Entity {
    pub id: i64,
    pub entity_type: String,
    pub name: String,
    pub label: String,
    pub sort_order: i64,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

fn row_to_entity(row: &rusqlite::Row) -> rusqlite::Result<Entity> {
    Ok(Entity {
        id: row.get("id")?,
        entity_type: row.get("entity_type")?,
        name: row.get("name")?,
        label: row.get("label")?,
        sort_order: row.get("sort_order")?,
        is_active: row.get::<_, i64>("is_active")? != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

/// Find all entities of a given type.
pub fn find_by_type(conn: &Connection, entity_type: &str) -> rusqlite::Result<Vec<Entity>> {
    let mut stmt = conn.prepare(
        "SELECT id, entity_type, name, label, sort_order, is_active, created_at, updated_at \
         FROM entities WHERE entity_type = ?1 ORDER BY sort_order, id"
    )?;
    let rows = stmt.query_map(params![entity_type], row_to_entity)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Find a single entity by type and name.
pub fn find_by_type_and_name(conn: &Connection, entity_type: &str, name: &str) -> rusqlite::Result<Option<Entity>> {
    let mut stmt = conn.prepare(
        "SELECT id, entity_type, name, label, sort_order, is_active, created_at, updated_at \
         FROM entities WHERE entity_type = ?1 AND name = ?2"
    )?;
    let mut rows = stmt.query_map(params![entity_type, name], row_to_entity)?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Find a single entity by id.
pub fn find_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<Entity>> {
    let mut stmt = conn.prepare(
        "SELECT id, entity_type, name, label, sort_order, is_active, created_at, updated_at \
         FROM entities WHERE id = ?1"
    )?;
    let mut rows = stmt.query_map(params![id], row_to_entity)?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Create a new entity, returning its id.
pub fn create(conn: &Connection, entity_type: &str, name: &str, label: &str) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES (?1, ?2, ?3)",
        params![entity_type, name, label],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Create a new entity with sort_order, returning its id.
pub fn create_with_sort(conn: &Connection, entity_type: &str, name: &str, label: &str, sort_order: i64) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label, sort_order) VALUES (?1, ?2, ?3, ?4)",
        params![entity_type, name, label, sort_order],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Update an entity's name and label.
pub fn update(conn: &Connection, id: i64, name: &str, label: &str) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE entities SET name = ?1, label = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%S','now') WHERE id = ?3",
        params![name, label, id],
    )?;
    Ok(())
}

/// Delete an entity (cascades to properties and relations).
pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM entities WHERE id = ?1", params![id])?;
    Ok(())
}

/// Count entities of a given type.
pub fn count_by_type(conn: &Connection, entity_type: &str) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM entities WHERE entity_type = ?1",
        params![entity_type],
        |row| row.get(0),
    )
}

// --- Property helpers ---

/// Get a single property value for an entity.
pub fn get_property(conn: &Connection, entity_id: i64, key: &str) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = ?2"
    )?;
    let mut rows = stmt.query_map(params![entity_id, key], |row| row.get::<_, String>(0))?;
    match rows.next() {
        Some(val) => Ok(Some(val?)),
        None => Ok(None),
    }
}

/// Get all properties for an entity as a HashMap.
pub fn get_properties(conn: &Connection, entity_id: i64) -> rusqlite::Result<HashMap<String, String>> {
    let mut stmt = conn.prepare(
        "SELECT key, value FROM entity_properties WHERE entity_id = ?1"
    )?;
    let rows = stmt.query_map(params![entity_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut map = HashMap::new();
    for row in rows {
        let (k, v) = row?;
        map.insert(k, v);
    }
    Ok(map)
}

/// Set a property (upsert).
pub fn set_property(conn: &Connection, entity_id: i64, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value",
        params![entity_id, key, value],
    )?;
    Ok(())
}

/// Delete a property.
pub fn delete_property(conn: &Connection, entity_id: i64, key: &str) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM entity_properties WHERE entity_id = ?1 AND key = ?2",
        params![entity_id, key],
    )?;
    Ok(())
}

/// Set multiple properties at once.
pub fn set_properties(conn: &Connection, entity_id: i64, props: &[(&str, &str)]) -> rusqlite::Result<()> {
    for (key, value) in props {
        set_property(conn, entity_id, key, value)?;
    }
    Ok(())
}
