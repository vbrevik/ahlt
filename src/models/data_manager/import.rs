use rusqlite::Connection;

use super::types::{ConflictMode, EntityImport, ImportError, ImportPayload, ImportResult, RelationImport};

/// Outcome of inserting/upserting a single entity.
enum EntityOutcome {
    Created,
    Updated,
    Skipped,
}

/// Parse a "type:name" reference string into (entity_type, name).
fn parse_ref(ref_str: &str) -> Result<(&str, &str), String> {
    ref_str
        .split_once(':')
        .ok_or_else(|| format!("invalid entity reference '{}' — expected 'type:name'", ref_str))
}

/// Resolve a "type:name" reference to an entity ID.
fn resolve_entity_ref(conn: &Connection, ref_str: &str) -> Result<i64, String> {
    let (entity_type, name) = parse_ref(ref_str)?;
    conn.query_row(
        "SELECT id FROM entities WHERE entity_type = ?1 AND name = ?2",
        rusqlite::params![entity_type, name],
        |row| row.get(0),
    )
    .map_err(|_| format!("entity not found: {}", ref_str))
}

/// Check if an entity with the given type+name already exists. Returns Some(id) if it does.
fn find_existing(conn: &Connection, entity: &EntityImport) -> Result<Option<i64>, String> {
    match conn.query_row(
        "SELECT id FROM entities WHERE entity_type = ?1 AND name = ?2",
        rusqlite::params![entity.entity_type, entity.name],
        |row| row.get::<_, i64>(0),
    ) {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("DB error checking existence: {}", e)),
    }
}

/// Insert a new entity with its properties.
fn insert_entity(conn: &Connection, entity: &EntityImport) -> Result<i64, String> {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label, sort_order) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![entity.entity_type, entity.name, entity.label, entity.sort_order],
    )
    .map_err(|e| format!("failed to insert entity {}:{} — {}", entity.entity_type, entity.name, e))?;

    let id = conn.last_insert_rowid();

    for (key, value) in &entity.properties {
        conn.execute(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
            rusqlite::params![id, key, value],
        )
        .map_err(|e| format!("failed to insert property {}.{} — {}", entity.name, key, e))?;
    }

    Ok(id)
}

/// Upsert an existing entity: update label, sort_order, and replace all properties.
fn upsert_entity(conn: &Connection, existing_id: i64, entity: &EntityImport) -> Result<(), String> {
    conn.execute(
        "UPDATE entities SET label = ?1, sort_order = ?2 WHERE id = ?3",
        rusqlite::params![entity.label, entity.sort_order, existing_id],
    )
    .map_err(|e| format!("failed to update entity {}:{} — {}", entity.entity_type, entity.name, e))?;

    // Delete all existing properties, then re-insert from import payload
    conn.execute(
        "DELETE FROM entity_properties WHERE entity_id = ?1",
        rusqlite::params![existing_id],
    )
    .map_err(|e| format!("failed to clear properties for {}:{} — {}", entity.entity_type, entity.name, e))?;

    for (key, value) in &entity.properties {
        conn.execute(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
            rusqlite::params![existing_id, key, value],
        )
        .map_err(|e| format!("failed to insert property {}.{} — {}", entity.name, key, e))?;
    }

    Ok(())
}

/// Process a single entity according to the conflict mode.
fn process_entity(
    conn: &Connection,
    entity: &EntityImport,
    mode: &ConflictMode,
) -> Result<EntityOutcome, String> {
    match find_existing(conn, entity)? {
        Some(existing_id) => match mode {
            ConflictMode::Skip => Ok(EntityOutcome::Skipped),
            ConflictMode::Upsert => {
                upsert_entity(conn, existing_id, entity)?;
                Ok(EntityOutcome::Updated)
            }
            ConflictMode::Fail => Err(format!(
                "entity already exists: {}:{}",
                entity.entity_type, entity.name
            )),
        },
        None => {
            insert_entity(conn, entity)?;
            Ok(EntityOutcome::Created)
        }
    }
}

/// Process a single relation import.
fn process_relation(conn: &Connection, rel: &RelationImport) -> Result<(), String> {
    let rel_type_id = conn
        .query_row(
            "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = ?1",
            rusqlite::params![rel.relation_type],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|_| format!("unknown relation type: {}", rel.relation_type))?;

    let source_id = resolve_entity_ref(conn, &rel.source)?;
    let target_id = resolve_entity_ref(conn, &rel.target)?;

    // Check if relation already exists (skip duplicates silently)
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM relations WHERE relation_type_id = ?1 AND source_id = ?2 AND target_id = ?3",
            rusqlite::params![rel_type_id, source_id, target_id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;

    if !exists {
        conn.execute(
            "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
            rusqlite::params![rel_type_id, source_id, target_id],
        )
        .map_err(|e| format!("failed to insert relation {} -> {} — {}", rel.source, rel.target, e))?;

        let relation_id = conn.last_insert_rowid();
        for (key, value) in &rel.properties {
            conn.execute(
                "INSERT INTO relation_properties (relation_id, key, value) VALUES (?1, ?2, ?3)",
                rusqlite::params![relation_id, key, value],
            )
            .map_err(|e| format!("failed to insert relation property {} — {}", key, e))?;
        }
    }

    Ok(())
}

/// Import data from an ImportPayload, applying conflict resolution.
/// The entire operation runs in a single transaction.
pub fn import_data(conn: &Connection, payload: &ImportPayload) -> Result<ImportResult, String> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("failed to begin transaction: {}", e))?;

    let mut result = ImportResult {
        created: 0,
        updated: 0,
        skipped: 0,
        errors: Vec::new(),
    };

    // Phase 1: Process entities
    for entity in &payload.entities {
        match process_entity(&tx, entity, &payload.conflict_mode) {
            Ok(EntityOutcome::Created) => result.created += 1,
            Ok(EntityOutcome::Updated) => result.updated += 1,
            Ok(EntityOutcome::Skipped) => result.skipped += 1,
            Err(reason) => {
                if payload.conflict_mode == ConflictMode::Fail {
                    // Rollback on first error in fail mode
                    tx.rollback()
                        .map_err(|e| format!("rollback failed: {}", e))?;
                    result.errors.push(ImportError {
                        item: serde_json::to_value(entity).unwrap_or_default(),
                        reason,
                    });
                    return Ok(result);
                }
                result.errors.push(ImportError {
                    item: serde_json::to_value(entity).unwrap_or_default(),
                    reason,
                });
            }
        }
    }

    // Phase 2: Process relations (after all entities exist)
    for rel in &payload.relations {
        if let Err(reason) = process_relation(&tx, rel) {
            result.errors.push(ImportError {
                item: serde_json::to_value(rel).unwrap_or_default(),
                reason,
            });
        }
    }

    tx.commit()
        .map_err(|e| format!("commit failed: {}", e))?;

    Ok(result)
}
