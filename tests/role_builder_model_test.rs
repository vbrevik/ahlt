mod common;

use ahlt::models::role::builder::find_accessible_nav_items;
use common::*;

#[tokio::test]
async fn test_find_accessible_nav_items() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // requires_permission relation type is seeded by common; look up its id
    let rp_rt_id: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'requires_permission'",
    )
    .fetch_one(pool)
    .await
    .unwrap()
    .0;

    // Create test permission
    let perm_id = insert_entity(pool, "permission", "test.view", "Test View").await;

    // Create parent module (top-level nav_item, no parent)
    let module_id: i64 = sqlx::query_as::<_, (i64,)>(
        "INSERT INTO entities (entity_type, name, label, is_active, sort_order) \
         VALUES ('nav_item', 'test_module', 'Test Module', true, 1) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
    .0;
    insert_prop(pool, module_id, "url", "/test-module").await;

    // Create child nav item with parent property
    let child_id: i64 = sqlx::query_as::<_, (i64,)>(
        "INSERT INTO entities (entity_type, name, label, is_active, sort_order) \
         VALUES ('nav_item', 'test_page', 'Test Page', true, 2) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
    .0;
    insert_prop(pool, child_id, "url", "/test").await;
    insert_prop(pool, child_id, "parent", "test_module").await;

    // Link child nav item to permission via requires_permission relation
    insert_relation(pool, rp_rt_id, child_id, perm_id).await;

    // Query with permission ID
    let items = find_accessible_nav_items(pool, &[perm_id]).await.unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Test Page");
    assert_eq!(items[0].path, "/test");
    assert_eq!(items[0].module_name, "Test Module");
}

#[tokio::test]
async fn test_standalone_module_appears_when_permitted() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // requires_permission relation type is seeded by common; look up its id
    let rp_rt_id: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'requires_permission'",
    )
    .fetch_one(pool)
    .await
    .unwrap()
    .0;

    // Create permission
    let perm_id = insert_entity(pool, "permission", "dashboard.view", "View Dashboard").await;

    // Create standalone module (no children) with permission
    let module_id: i64 = sqlx::query_as::<_, (i64,)>(
        "INSERT INTO entities (entity_type, name, label, is_active, sort_order) \
         VALUES ('nav_item', 'dashboard', 'Dashboard', true, 1) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
    .0;
    insert_prop(pool, module_id, "url", "/dashboard").await;

    insert_relation(pool, rp_rt_id, module_id, perm_id).await;

    let items = find_accessible_nav_items(pool, &[perm_id]).await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Dashboard");
}

#[tokio::test]
async fn test_unpermitted_items_excluded() {
    let db = setup_test_db().await;
    let pool = db.pool();

    // requires_permission relation type is seeded by common; look up its id
    let rp_rt_id: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'requires_permission'",
    )
    .fetch_one(pool)
    .await
    .unwrap()
    .0;

    // Create two permissions
    let perm_users = insert_entity(pool, "permission", "users.list", "List Users").await;
    let perm_roles = insert_entity(pool, "permission", "roles.manage", "Manage Roles").await;

    // Parent module
    let module_id: i64 = sqlx::query_as::<_, (i64,)>(
        "INSERT INTO entities (entity_type, name, label, is_active, sort_order) \
         VALUES ('nav_item', 'admin', 'Admin', true, 1) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
    .0;

    // Child requiring users.list
    let users_id: i64 = sqlx::query_as::<_, (i64,)>(
        "INSERT INTO entities (entity_type, name, label, is_active, sort_order) \
         VALUES ('nav_item', 'admin.users', 'Users', true, 2) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
    .0;
    insert_prop(pool, users_id, "parent", "admin").await;
    insert_relation(pool, rp_rt_id, users_id, perm_users).await;

    // Child requiring roles.manage
    let roles_id: i64 = sqlx::query_as::<_, (i64,)>(
        "INSERT INTO entities (entity_type, name, label, is_active, sort_order) \
         VALUES ('nav_item', 'admin.roles', 'Roles', true, 3) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
    .0;
    insert_prop(pool, roles_id, "parent", "admin").await;
    insert_relation(pool, rp_rt_id, roles_id, perm_roles).await;

    // Only grant users.list -- should see Users but not Roles
    let items = find_accessible_nav_items(pool, &[perm_users]).await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Users");

    // Suppress unused variable warning
    let _ = module_id;
}
