mod common;
use common::*;

async fn seed_warning_types(pool: &sqlx::PgPool) {
    // Seed the relation types needed by the warning system
    insert_entity(pool, "relation_type", "targets_user", "Targets User").await;
    insert_entity(pool, "relation_type", "for_warning", "For Warning").await;
    insert_entity(pool, "relation_type", "for_user", "For User").await;
    insert_entity(pool, "relation_type", "on_receipt", "On Receipt").await;
    insert_entity(pool, "relation_type", "forwarded_to_user", "Forwarded To User").await;
}

async fn seed_users(pool: &sqlx::PgPool) -> (i64, i64) {
    let user1 = insert_entity(pool, "user", "alice", "Alice").await;
    insert_prop(pool, user1, "email", "alice@test.com").await;
    let user2 = insert_entity(pool, "user", "bob", "Bob").await;
    insert_prop(pool, user2, "email", "bob@test.com").await;
    (user1, user2)
}

// --- Tests ---

#[tokio::test]
async fn test_create_warning_and_receipts() {
    let db = setup_test_db().await;
    let pool = db.pool();
    seed_warning_types(pool).await;
    let (user1, user2) = seed_users(pool).await;

    let warning_id = ahlt::warnings::create_warning(
        pool, "high", "security", "test.action", "Test warning message", "details", "system",
    ).await.expect("Failed to create warning");

    // Verify warning entity exists
    let (entity_type,): (String,) = sqlx::query_as(
        "SELECT entity_type FROM entities WHERE id = $1",
    )
    .bind(warning_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(entity_type, "warning");

    // Verify properties
    let (severity,): (String,) = sqlx::query_as(
        "SELECT value FROM entity_properties WHERE entity_id = $1 AND key = 'severity'",
    )
    .bind(warning_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(severity, "high");

    // Create receipts for both users
    let receipt_ids = ahlt::warnings::create_receipts(pool, warning_id, &[user1, user2])
        .await
        .expect("Failed to create receipts");
    assert_eq!(receipt_ids.len(), 2);

    // Verify receipt entities exist
    for receipt_id in &receipt_ids {
        let (rt,): (String,) = sqlx::query_as(
            "SELECT entity_type FROM entities WHERE id = $1",
        )
        .bind(receipt_id)
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(rt, "warning_receipt");
    }
}

#[tokio::test]
async fn test_count_unread() {
    let db = setup_test_db().await;
    let pool = db.pool();
    seed_warning_types(pool).await;
    let (user1, user2) = seed_users(pool).await;

    // Initially zero
    assert_eq!(ahlt::warnings::queries::count_unread(pool, user1).await, 0);

    // Create a warning with receipt for user1
    let w1 = ahlt::warnings::create_warning(
        pool, "info", "system", "test.1", "Warning 1", "", "system",
    ).await.unwrap();
    ahlt::warnings::create_receipts(pool, w1, &[user1]).await.unwrap();

    assert_eq!(ahlt::warnings::queries::count_unread(pool, user1).await, 1);
    assert_eq!(ahlt::warnings::queries::count_unread(pool, user2).await, 0);

    // Create another warning for both
    let w2 = ahlt::warnings::create_warning(
        pool, "medium", "governance", "test.2", "Warning 2", "", "system",
    ).await.unwrap();
    ahlt::warnings::create_receipts(pool, w2, &[user1, user2]).await.unwrap();

    assert_eq!(ahlt::warnings::queries::count_unread(pool, user1).await, 2);
    assert_eq!(ahlt::warnings::queries::count_unread(pool, user2).await, 1);
}

#[tokio::test]
async fn test_mark_read_updates_receipt() {
    let db = setup_test_db().await;
    let pool = db.pool();
    seed_warning_types(pool).await;
    let (user1, _user2) = seed_users(pool).await;

    let w = ahlt::warnings::create_warning(
        pool, "low", "system", "test.read", "Read test", "", "system",
    ).await.unwrap();
    ahlt::warnings::create_receipts(pool, w, &[user1]).await.unwrap();

    assert_eq!(ahlt::warnings::queries::count_unread(pool, user1).await, 1);

    // Mark as read
    let receipt_id = ahlt::warnings::queries::find_receipt_for_user(pool, w, user1)
        .await.unwrap().unwrap();
    ahlt::warnings::update_receipt_status(pool, receipt_id, "read", user1).await.unwrap();

    assert_eq!(ahlt::warnings::queries::count_unread(pool, user1).await, 0);

    // Verify status property
    let (status,): (String,) = sqlx::query_as(
        "SELECT value FROM entity_properties WHERE entity_id = $1 AND key = 'status'",
    )
    .bind(receipt_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(status, "read");
}

#[tokio::test]
async fn test_warning_deduplication() {
    let db = setup_test_db().await;
    let pool = db.pool();
    seed_warning_types(pool).await;

    let source_action = "scheduled.test_dedup";
    let dedup_key = "test_dedup";

    // First check — should not exist
    assert!(!ahlt::warnings::warning_exists(pool, source_action, dedup_key).await);

    // Create warning — dedup_key must appear in details for warning_exists LIKE check
    ahlt::warnings::create_warning(
        pool, "info", "system", source_action, "First warning", dedup_key, "system",
    ).await.unwrap();

    // Now should exist
    assert!(ahlt::warnings::warning_exists(pool, source_action, dedup_key).await);
}

#[tokio::test]
async fn test_find_for_user_pagination() {
    let db = setup_test_db().await;
    let pool = db.pool();
    seed_warning_types(pool).await;
    let (user1, _) = seed_users(pool).await;

    // Create 5 warnings
    for i in 0..5 {
        let w = ahlt::warnings::create_warning(
            pool, "info", "system", &format!("test.page.{}", i),
            &format!("Warning {}", i), "", "system",
        ).await.unwrap();
        ahlt::warnings::create_receipts(pool, w, &[user1]).await.unwrap();
    }

    // Get page 1 with per_page=2
    let page = ahlt::warnings::queries::find_for_user(
        pool, user1, 1, 2, None, None, false, false,
    ).await.unwrap();
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.total_count, 5);
    assert_eq!(page.total_pages, 3);

    // Get page 3 (last page, 1 item)
    let page3 = ahlt::warnings::queries::find_for_user(
        pool, user1, 3, 2, None, None, false, false,
    ).await.unwrap();
    assert_eq!(page3.items.len(), 1);
}

#[tokio::test]
async fn test_warning_detail() {
    let db = setup_test_db().await;
    let pool = db.pool();
    seed_warning_types(pool).await;
    let (user1, _) = seed_users(pool).await;

    let w = ahlt::warnings::create_warning(
        pool, "critical", "security", "test.detail",
        "Critical security issue", "{\"ip\":\"1.2.3.4\"}", "system",
    ).await.unwrap();
    ahlt::warnings::create_receipts(pool, w, &[user1]).await.unwrap();

    let detail = ahlt::warnings::queries::get_warning_detail(pool, w)
        .await.unwrap().expect("Should find warning detail");

    assert_eq!(detail.severity, "critical");
    assert_eq!(detail.category, "security");
    assert_eq!(detail.message, "Critical security issue");
    assert_eq!(detail.source_action, "test.detail");
    assert!(detail.details.contains("1.2.3.4"));

    // Verify recipients
    let recipients = ahlt::warnings::queries::get_recipients(pool, w).await.unwrap();
    assert_eq!(recipients.len(), 1);
    assert_eq!(recipients[0].username, "alice");
    assert_eq!(recipients[0].status, "unread");
}

#[tokio::test]
async fn test_event_timeline() {
    let db = setup_test_db().await;
    let pool = db.pool();
    seed_warning_types(pool).await;
    let (user1, _) = seed_users(pool).await;

    let w = ahlt::warnings::create_warning(
        pool, "low", "system", "test.timeline", "Timeline test", "", "system",
    ).await.unwrap();
    ahlt::warnings::create_receipts(pool, w, &[user1]).await.unwrap();

    let receipt_id = ahlt::warnings::queries::find_receipt_for_user(pool, w, user1)
        .await.unwrap().unwrap();

    // Receipt creation should have generated a "created" event
    let timeline = ahlt::warnings::queries::get_receipt_timeline(pool, receipt_id).await.unwrap();
    assert!(!timeline.is_empty());
    assert_eq!(timeline[0].action, "created");

    // Update status and check new event
    ahlt::warnings::update_receipt_status(pool, receipt_id, "read", user1).await.unwrap();
    let timeline = ahlt::warnings::queries::get_receipt_timeline(pool, receipt_id).await.unwrap();
    assert!(timeline.len() >= 2);
}

#[tokio::test]
async fn test_tor_vacancy_generator_creates_warning() {
    let db = setup_test_db().await;
    let pool = db.pool();
    seed_warning_types(pool).await;

    let conn_map = ahlt::handlers::warning_handlers::ws::new_connection_map();

    // Create an admin user with tor.manage_members permission
    let admin = insert_entity(pool, "user", "admin", "Admin").await;
    let role = insert_entity(pool, "role", "admin_role", "Admin Role").await;
    let perm = insert_entity(pool, "permission", "tor.manage_members", "Manage ToR Members").await;
    // has_role: admin -> admin_role
    let (has_role_rt,): (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE name = 'has_role'",
    ).fetch_one(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES ($1, $2, $3)",
    )
    .bind(has_role_rt).bind(admin).bind(role)
    .execute(pool).await.unwrap();
    // has_permission: admin_role -> tor.manage_members
    let (has_perm_rt,): (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE name = 'has_permission'",
    ).fetch_one(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES ($1, $2, $3)",
    )
    .bind(has_perm_rt).bind(role).bind(perm)
    .execute(pool).await.unwrap();

    // Create a ToR with status=active
    let tor = insert_entity(pool, "tor", "test_tor", "Test Committee").await;
    insert_prop(pool, tor, "status", "active").await;

    // Create a mandatory position linked to the ToR (vacant -- no fills_position)
    let pos = insert_entity(pool, "tor_function", "chair", "Chair").await;
    insert_prop(pool, pos, "membership_type", "mandatory").await;
    let (belongs_to_rt,): (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE name = 'belongs_to_tor'",
    ).fetch_one(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES ($1, $2, $3)",
    )
    .bind(belongs_to_rt).bind(pos).bind(tor)
    .execute(pool).await.unwrap();

    // Run the generator
    ahlt::warnings::generators::check_tor_vacancies(pool, &conn_map).await;

    // Verify a warning was created
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM entities e
         JOIN entity_properties sa ON sa.entity_id = e.id AND sa.key = 'source_action'
         WHERE e.entity_type = 'warning' AND sa.value = 'scheduled.tor_vacancy'",
    ).fetch_one(pool).await.unwrap();
    assert_eq!(count, 1, "Expected one vacancy warning");

    // Verify the warning message references the position
    let (message,): (String,) = sqlx::query_as(
        "SELECT ep.value FROM entities e
         JOIN entity_properties ep ON ep.entity_id = e.id AND ep.key = 'message'
         JOIN entity_properties sa ON sa.entity_id = e.id AND sa.key = 'source_action'
         WHERE e.entity_type = 'warning' AND sa.value = 'scheduled.tor_vacancy'",
    ).fetch_one(pool).await.unwrap();
    assert!(message.contains("Chair"), "Warning should mention the vacant position");
    assert!(message.contains("Test Committee"), "Warning should mention the ToR");

    // Verify receipt was created for admin
    let (receipt_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM entities WHERE entity_type = 'warning_receipt'",
    ).fetch_one(pool).await.unwrap();
    assert_eq!(receipt_count, 1, "Expected one receipt for the admin user");

    // Run again -- dedup should prevent a second warning
    ahlt::warnings::generators::check_tor_vacancies(pool, &conn_map).await;
    let (count2,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM entities e
         JOIN entity_properties sa ON sa.entity_id = e.id AND sa.key = 'source_action'
         WHERE e.entity_type = 'warning' AND sa.value = 'scheduled.tor_vacancy'",
    ).fetch_one(pool).await.unwrap();
    assert_eq!(count2, 1, "Dedup should prevent second warning");
}

#[tokio::test]
async fn test_tor_vacancy_auto_resolves_when_filled() {
    let db = setup_test_db().await;
    let pool = db.pool();
    seed_warning_types(pool).await;

    let conn_map = ahlt::handlers::warning_handlers::ws::new_connection_map();

    // Minimal setup: admin with permission
    let admin = insert_entity(pool, "user", "admin", "Admin").await;
    let role = insert_entity(pool, "role", "admin_role", "Admin Role").await;
    let perm = insert_entity(pool, "permission", "tor.manage_members", "Manage Members").await;
    let (has_role_rt,): (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE name = 'has_role'",
    ).fetch_one(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES ($1, $2, $3)",
    )
    .bind(has_role_rt).bind(admin).bind(role)
    .execute(pool).await.unwrap();
    let (has_perm_rt,): (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE name = 'has_permission'",
    ).fetch_one(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES ($1, $2, $3)",
    )
    .bind(has_perm_rt).bind(role).bind(perm)
    .execute(pool).await.unwrap();

    // Create ToR + vacant mandatory position
    let tor = insert_entity(pool, "tor", "test_tor", "Test Committee").await;
    insert_prop(pool, tor, "status", "active").await;
    let pos = insert_entity(pool, "tor_function", "chair", "Chair").await;
    insert_prop(pool, pos, "membership_type", "mandatory").await;
    let (belongs_to_rt,): (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE name = 'belongs_to_tor'",
    ).fetch_one(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES ($1, $2, $3)",
    )
    .bind(belongs_to_rt).bind(pos).bind(tor)
    .execute(pool).await.unwrap();

    // Run generator -- should create warning
    ahlt::warnings::generators::check_tor_vacancies(pool, &conn_map).await;
    let (status,): (String,) = sqlx::query_as(
        "SELECT ep.value FROM entities e
         JOIN entity_properties ep ON ep.entity_id = e.id AND ep.key = 'status'
         JOIN entity_properties sa ON sa.entity_id = e.id AND sa.key = 'source_action'
         WHERE e.entity_type = 'warning' AND sa.value = 'scheduled.tor_vacancy'",
    ).fetch_one(pool).await.unwrap();
    assert_eq!(status, "active");

    // Fill the position: create fills_position relation
    let (fills_rt,): (i64,) = sqlx::query_as(
        "SELECT id FROM entities WHERE name = 'fills_position'",
    ).fetch_one(pool).await.unwrap();
    let filler = insert_entity(pool, "user", "bob", "Bob").await;
    sqlx::query(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES ($1, $2, $3)",
    )
    .bind(fills_rt).bind(filler).bind(pos)
    .execute(pool).await.unwrap();

    // Run generator again -- should auto-resolve the warning
    ahlt::warnings::generators::check_tor_vacancies(pool, &conn_map).await;
    let (status2,): (String,) = sqlx::query_as(
        "SELECT ep.value FROM entities e
         JOIN entity_properties ep ON ep.entity_id = e.id AND ep.key = 'status'
         JOIN entity_properties sa ON sa.entity_id = e.id AND sa.key = 'source_action'
         WHERE e.entity_type = 'warning' AND sa.value = 'scheduled.tor_vacancy'",
    ).fetch_one(pool).await.unwrap();
    assert_eq!(status2, "resolved", "Warning should auto-resolve when vacancy is filled");
}
