use sqlx::{Acquire, PgPool, Postgres};

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
async fn resolve_entity_ref(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    ref_str: &str,
) -> Result<i64, String> {
    let (entity_type, name) = parse_ref(ref_str)?;
    let row: (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE entity_type = $1 AND name = $2",
    )
    .bind(entity_type)
    .bind(name)
    .fetch_one(tx.acquire().await.map_err(|e| format!("acquire error: {}", e))?)
    .await
    .map_err(|_| format!("entity not found: {}", ref_str))?;
    Ok(row.0)
}

/// Check if an entity with the given type+name already exists. Returns Some(id) if it does.
async fn find_existing(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    entity: &EntityImport,
) -> Result<Option<i64>, String> {
    let result: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM entities WHERE entity_type = $1 AND name = $2",
    )
    .bind(&entity.entity_type)
    .bind(&entity.name)
    .fetch_optional(tx.acquire().await.map_err(|e| format!("acquire error: {}", e))?)
    .await
    .map_err(|e| format!("DB error checking existence: {}", e))?;
    Ok(result.map(|r| r.0))
}

/// Insert a new entity with its properties.
async fn insert_entity(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    entity: &EntityImport,
) -> Result<i64, String> {
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO entities (entity_type, name, label, sort_order) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(&entity.entity_type)
    .bind(&entity.name)
    .bind(&entity.label)
    .bind(entity.sort_order)
    .fetch_one(tx.acquire().await.map_err(|e| format!("acquire error: {}", e))?)
    .await
    .map_err(|e| format!("failed to insert entity {}:{} — {}", entity.entity_type, entity.name, e))?;

    let id = row.0;

    for (key, value) in &entity.properties {
        sqlx::query(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, $2, $3)",
        )
        .bind(id)
        .bind(key)
        .bind(value)
        .execute(tx.acquire().await.map_err(|e| format!("acquire error: {}", e))?)
        .await
        .map_err(|e| format!("failed to insert property {}.{} — {}", entity.name, key, e))?;
    }

    Ok(id)
}

/// Upsert an existing entity: update label, sort_order, and replace all properties.
async fn upsert_entity(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    existing_id: i64,
    entity: &EntityImport,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE entities SET label = $1, sort_order = $2 WHERE id = $3",
    )
    .bind(&entity.label)
    .bind(entity.sort_order)
    .bind(existing_id)
    .execute(tx.acquire().await.map_err(|e| format!("acquire error: {}", e))?)
    .await
    .map_err(|e| format!("failed to update entity {}:{} — {}", entity.entity_type, entity.name, e))?;

    // Delete all existing properties, then re-insert from import payload
    sqlx::query(
        "DELETE FROM entity_properties WHERE entity_id = $1",
    )
    .bind(existing_id)
    .execute(tx.acquire().await.map_err(|e| format!("acquire error: {}", e))?)
    .await
    .map_err(|e| format!("failed to clear properties for {}:{} — {}", entity.entity_type, entity.name, e))?;

    for (key, value) in &entity.properties {
        sqlx::query(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, $2, $3)",
        )
        .bind(existing_id)
        .bind(key)
        .bind(value)
        .execute(tx.acquire().await.map_err(|e| format!("acquire error: {}", e))?)
        .await
        .map_err(|e| format!("failed to insert property {}.{} — {}", entity.name, key, e))?;
    }

    Ok(())
}

/// Process a single entity according to the conflict mode.
async fn process_entity(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    entity: &EntityImport,
    mode: &ConflictMode,
) -> Result<EntityOutcome, String> {
    match find_existing(tx, entity).await? {
        Some(existing_id) => match mode {
            ConflictMode::Skip => Ok(EntityOutcome::Skipped),
            ConflictMode::Upsert => {
                upsert_entity(tx, existing_id, entity).await?;
                Ok(EntityOutcome::Updated)
            }
            ConflictMode::Fail => Err(format!(
                "entity already exists: {}:{}",
                entity.entity_type, entity.name
            )),
        },
        None => {
            insert_entity(tx, entity).await?;
            Ok(EntityOutcome::Created)
        }
    }
}

/// Process a single relation import.
async fn process_relation(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    rel: &RelationImport,
) -> Result<(), String> {
    let row: (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = $1",
    )
    .bind(&rel.relation_type)
    .fetch_one(tx.acquire().await.map_err(|e| format!("acquire error: {}", e))?)
    .await
    .map_err(|_| format!("unknown relation type: {}", rel.relation_type))?;
    let rel_type_id = row.0;

    let source_id = resolve_entity_ref(tx, &rel.source).await?;
    let target_id = resolve_entity_ref(tx, &rel.target).await?;

    // Check if relation already exists (skip duplicates silently)
    let count_row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM relations WHERE relation_type_id = $1 AND source_id = $2 AND target_id = $3",
    )
    .bind(rel_type_id)
    .bind(source_id)
    .bind(target_id)
    .fetch_one(tx.acquire().await.map_err(|e| format!("acquire error: {}", e))?)
    .await
    .unwrap_or((0,));
    let exists = count_row.0 > 0;

    if !exists {
        let rel_row: (i64,) = sqlx::query_as(
            "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES ($1, $2, $3) RETURNING id",
        )
        .bind(rel_type_id)
        .bind(source_id)
        .bind(target_id)
        .fetch_one(tx.acquire().await.map_err(|e| format!("acquire error: {}", e))?)
        .await
        .map_err(|e| format!("failed to insert relation {} -> {} — {}", rel.source, rel.target, e))?;

        let relation_id = rel_row.0;
        for (key, value) in &rel.properties {
            sqlx::query(
                "INSERT INTO relation_properties (relation_id, key, value) VALUES ($1, $2, $3)",
            )
            .bind(relation_id)
            .bind(key)
            .bind(value)
            .execute(tx.acquire().await.map_err(|e| format!("acquire error: {}", e))?)
            .await
            .map_err(|e| format!("failed to insert relation property {} — {}", key, e))?;
        }
    }

    Ok(())
}

/// Import data from an ImportPayload, applying conflict resolution.
/// The entire operation runs in a single transaction.
pub async fn import_data(pool: &PgPool, payload: &ImportPayload) -> Result<ImportResult, String> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("failed to begin transaction: {}", e))?;

    let mut result = ImportResult {
        created: 0,
        updated: 0,
        skipped: 0,
        errors: Vec::new(),
    };

    // Phase 1: Process entities
    for entity in &payload.entities {
        match process_entity(&mut tx, entity, &payload.conflict_mode).await {
            Ok(EntityOutcome::Created) => result.created += 1,
            Ok(EntityOutcome::Updated) => result.updated += 1,
            Ok(EntityOutcome::Skipped) => result.skipped += 1,
            Err(reason) => {
                if payload.conflict_mode == ConflictMode::Fail {
                    // Rollback on first error in fail mode
                    tx.rollback()
                        .await
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
        if let Err(reason) = process_relation(&mut tx, rel).await {
            result.errors.push(ImportError {
                item: serde_json::to_value(rel).unwrap_or_default(),
                reason,
            });
        }
    }

    tx.commit()
        .await
        .map_err(|e| format!("commit failed: {}", e))?;

    Ok(result)
}
