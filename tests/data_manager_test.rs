//! Data manager import/export tests — covers entity import, conflict modes,
//! relation import, entity export, and filtered export.
//!
//! Tests the data_manager model layer:
//! - Entity import with properties
//! - Skip conflict mode (duplicate handling)
//! - Relation import using seeded relation types
//! - Full entity export round-trip
//! - Type-filtered export

mod common;

use std::collections::HashMap;

use ahlt::models::data_manager::{
    export,
    import,
    types::{ConflictMode, EntityImport, ImportPayload, RelationImport},
};
use common::setup_test_db;

/// Helper: build an EntityImport with optional properties.
fn make_entity(
    entity_type: &str,
    name: &str,
    label: &str,
    properties: Vec<(&str, &str)>,
) -> EntityImport {
    EntityImport {
        entity_type: entity_type.to_string(),
        name: name.to_string(),
        label: label.to_string(),
        sort_order: 0,
        properties: properties
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

/// Helper: build a RelationImport.
fn make_relation(
    relation_type: &str,
    source: &str,
    target: &str,
) -> RelationImport {
    RelationImport {
        relation_type: relation_type.to_string(),
        source: source.to_string(),
        target: target.to_string(),
        properties: HashMap::new(),
    }
}

// ────────────────────────────────────────────────────────────────────
// 1. Import entities with properties
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_import_entities() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let payload = ImportPayload {
        conflict_mode: ConflictMode::Skip,
        entities: vec![
            make_entity(
                "dm_test",
                "item_alpha",
                "Item Alpha",
                vec![("color", "red"), ("weight", "10")],
            ),
            make_entity(
                "dm_test",
                "item_beta",
                "Item Beta",
                vec![("color", "blue")],
            ),
        ],
        relations: vec![],
    };

    let result = import::import_data(pool, &payload).await.expect("import failed");

    assert_eq!(result.created, 2, "should create 2 entities");
    assert_eq!(result.updated, 0);
    assert_eq!(result.skipped, 0);
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    // Verify entities exist in DB
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM entities WHERE entity_type = 'dm_test'",
    )
    .fetch_one(pool)
    .await
    .expect("count query failed");
    assert_eq!(row.0, 2, "should have 2 dm_test entities in DB");

    // Verify properties were stored
    let props: Vec<(String, String)> = sqlx::query_as(
        "SELECT ep.key, ep.value FROM entity_properties ep \
         JOIN entities e ON ep.entity_id = e.id \
         WHERE e.entity_type = 'dm_test' AND e.name = 'item_alpha' \
         ORDER BY ep.key",
    )
    .fetch_all(pool)
    .await
    .expect("property query failed");

    assert_eq!(props.len(), 2);
    assert_eq!(props[0], ("color".to_string(), "red".to_string()));
    assert_eq!(props[1], ("weight".to_string(), "10".to_string()));
}

// ────────────────────────────────────────────────────────────────────
// 2. Skip conflict mode — duplicate entity is skipped
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_import_skip_mode() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let entity = make_entity(
        "dm_skip",
        "dup_item",
        "Dup Item Original",
        vec![("version", "1")],
    );

    // First import — entity is created
    let payload1 = ImportPayload {
        conflict_mode: ConflictMode::Skip,
        entities: vec![entity.clone()],
        relations: vec![],
    };
    let r1 = import::import_data(pool, &payload1).await.expect("first import failed");
    assert_eq!(r1.created, 1);
    assert_eq!(r1.skipped, 0);

    // Second import — same entity, skip mode
    let modified = make_entity(
        "dm_skip",
        "dup_item",
        "Dup Item Modified",
        vec![("version", "2")],
    );
    let payload2 = ImportPayload {
        conflict_mode: ConflictMode::Skip,
        entities: vec![modified],
        relations: vec![],
    };
    let r2 = import::import_data(pool, &payload2).await.expect("second import failed");
    assert_eq!(r2.created, 0);
    assert_eq!(r2.skipped, 1, "duplicate should be skipped");
    assert!(r2.errors.is_empty());

    // Verify original label is preserved (not overwritten)
    let row: (String,) = sqlx::query_as(
        "SELECT label FROM entities WHERE entity_type = 'dm_skip' AND name = 'dup_item'",
    )
    .fetch_one(pool)
    .await
    .expect("label query failed");
    assert_eq!(row.0, "Dup Item Original", "skip mode should preserve original");
}

// ────────────────────────────────────────────────────────────────────
// 3. Import entities + relation using seeded relation type
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_import_with_relations() {
    let db = setup_test_db().await;
    let pool = db.pool();

    let payload = ImportPayload {
        conflict_mode: ConflictMode::Skip,
        entities: vec![
            make_entity("dm_rel_user", "alice", "Alice", vec![]),
            make_entity("dm_rel_role", "editor", "Editor Role", vec![]),
        ],
        relations: vec![
            // "has_role" is seeded by setup_test_db()
            make_relation("has_role", "dm_rel_user:alice", "dm_rel_role:editor"),
        ],
    };

    let result = import::import_data(pool, &payload).await.expect("import failed");

    assert_eq!(result.created, 2, "should create 2 entities");
    assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

    // Verify the relation exists in DB
    let rel_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM relations r \
         JOIN entities src ON r.source_id = src.id \
         JOIN entities tgt ON r.target_id = tgt.id \
         WHERE src.name = 'alice' AND tgt.name = 'editor'",
    )
    .fetch_one(pool)
    .await
    .expect("relation count query failed");
    assert_eq!(rel_count.0, 1, "should have 1 has_role relation");
}

// ────────────────────────────────────────────────────────────────────
// 4. Export entities — round-trip import then export
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_export_entities() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Import some test entities
    let payload = ImportPayload {
        conflict_mode: ConflictMode::Skip,
        entities: vec![
            make_entity(
                "dm_export",
                "export_one",
                "Export One",
                vec![("tag", "first")],
            ),
            make_entity(
                "dm_export",
                "export_two",
                "Export Two",
                vec![("tag", "second")],
            ),
        ],
        relations: vec![],
    };
    import::import_data(pool, &payload).await.expect("import failed");

    // Export all entities (no type filter)
    let exported = export::export_entities(pool, None).await.expect("export failed");

    // Find our test entities in the export
    let dm_entities: Vec<_> = exported
        .entities
        .iter()
        .filter(|e| e.entity_type == "dm_export")
        .collect();

    assert_eq!(dm_entities.len(), 2, "should export both dm_export entities");

    // Verify properties are included
    let one = dm_entities.iter().find(|e| e.name == "export_one").expect("export_one missing");
    assert_eq!(one.label, "Export One");
    assert_eq!(one.properties.get("tag").map(String::as_str), Some("first"));

    let two = dm_entities.iter().find(|e| e.name == "export_two").expect("export_two missing");
    assert_eq!(two.properties.get("tag").map(String::as_str), Some("second"));
}

// ────────────────────────────────────────────────────────────────────
// 5. Export filtered by entity type
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_export_filtered_by_type() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Import entities of two different types
    let payload = ImportPayload {
        conflict_mode: ConflictMode::Skip,
        entities: vec![
            make_entity("dm_type_a", "a_item", "A Item", vec![]),
            make_entity("dm_type_b", "b_item", "B Item", vec![]),
        ],
        relations: vec![],
    };
    import::import_data(pool, &payload).await.expect("import failed");

    // Export only type "dm_type_a"
    let filter = vec!["dm_type_a".to_string()];
    let exported = export::export_entities(pool, Some(&filter))
        .await
        .expect("filtered export failed");

    // All exported entities should be of type dm_type_a
    let type_a: Vec<_> = exported
        .entities
        .iter()
        .filter(|e| e.entity_type == "dm_type_a")
        .collect();
    let type_b: Vec<_> = exported
        .entities
        .iter()
        .filter(|e| e.entity_type == "dm_type_b")
        .collect();

    assert_eq!(type_a.len(), 1, "should include dm_type_a entity");
    assert_eq!(type_a[0].name, "a_item");
    assert!(type_b.is_empty(), "should NOT include dm_type_b entities");
}
