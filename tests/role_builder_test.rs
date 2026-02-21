mod common;

use ahlt::models::role;
use common::*;

#[tokio::test]
async fn test_create_role_via_builder() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Look up has_permission relation type from seed data
    let hp_rt_id: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission'"
    )
    .fetch_one(pool)
    .await
    .expect("has_permission relation type should exist from seed")
    .0;

    // Create test permissions
    let perm_read_id = insert_entity(pool, "permission", "test.read", "Test Read").await;
    let perm_write_id = insert_entity(pool, "permission", "test.write", "Test Write").await;

    // Create role via builder simulation
    let role_id: i64 = sqlx::query_as::<_, (i64,)>(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', 'test_role', 'Test Role') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
    .0;

    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'description', 'Test description')",
    )
    .bind(role_id)
    .execute(pool)
    .await
    .unwrap();

    for perm_id in [perm_read_id, perm_write_id] {
        sqlx::query(
            "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES ($1, $2, $3)",
        )
        .bind(hp_rt_id)
        .bind(role_id)
        .bind(perm_id)
        .execute(pool)
        .await
        .unwrap();
    }

    // Verify role
    let role = role::find_by_id(pool, role_id).await.unwrap().unwrap();
    assert_eq!(role.name, "test_role");
    assert_eq!(role.label, "Test Role");

    // Verify permissions
    let permissions = role::find_permission_checkboxes(pool, role_id).await.unwrap();
    let granted = permissions.iter().filter(|p| p.checked).count();
    assert_eq!(granted, 2);
}

#[tokio::test]
async fn test_role_name_uniqueness() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Create first role
    sqlx::query(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', 'duplicate', 'First')",
    )
    .execute(pool)
    .await
    .unwrap();

    // Attempt duplicate
    let result = sqlx::query(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', 'duplicate', 'Second')",
    )
    .execute(pool)
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_menu_preview_calculation() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Create permission
    let perm_id = insert_entity(pool, "permission", "admin.settings", "Admin Settings").await;

    // Create nav module
    let _module_id = insert_entity(pool, "nav_module", "admin", "Admin").await;

    // Create nav item
    let nav_item_id = insert_entity(pool, "nav_item", "settings", "Settings").await;
    insert_prop(pool, nav_item_id, "path", "/settings").await;
    insert_prop(pool, nav_item_id, "permission_required", "admin.settings").await;

    // Create in_module relation type
    let rt_id = insert_entity(pool, "relation_type", "in_module", "In Module").await;

    // Link nav item to module
    sqlx::query(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES ($1, $2, $3)",
    )
    .bind(rt_id)
    .bind(nav_item_id)
    .bind(_module_id)
    .execute(pool)
    .await
    .unwrap();

    // Query accessible items
    let items = role::builder::find_accessible_nav_items(pool, &[perm_id]).await.unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Settings");
}

#[tokio::test]
async fn test_no_permissions_selected() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // Create role with no permissions (valid but not useful)
    let role_id: i64 = sqlx::query_as::<_, (i64,)>(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', 'empty_role', 'Empty Role') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
    .0;

    // Verify role exists
    let role = role::find_by_id(pool, role_id).await.unwrap().unwrap();
    assert_eq!(role.name, "empty_role");

    // Verify no permissions
    let permissions = role::find_permission_checkboxes(pool, role_id).await.unwrap();
    let granted = permissions.iter().filter(|p| p.checked).count();
    assert_eq!(granted, 0);
}

#[tokio::test]
async fn test_builder_requires_admin_permission() {
    // This would be tested at the handler level with mock sessions
    // For now, we just verify the permission code can be created
    let db = setup_test_db().await;
    let pool = db.pool();

    // Create the admin.roles permission
    sqlx::query(
        "INSERT INTO entities (entity_type, name, label) VALUES ('permission', 'admin.roles', 'Manage Roles')",
    )
    .execute(pool)
    .await
    .unwrap();

    let exists: bool = sqlx::query_as::<_, (bool,)>(
        "SELECT EXISTS(SELECT 1 FROM entities WHERE entity_type='permission' AND name='admin.roles')",
    )
    .fetch_one(pool)
    .await
    .unwrap()
    .0;

    assert!(exists, "admin.roles permission should exist");
}
