//! Shared test infrastructure for model layer tests.
//!
//! This module provides common utilities for setting up test databases.
//!
//! # Test Database Setup
//! - `setup_test_db()` - Schema + basic entities (relation types, roles)
//! - `setup_test_db_seeded()` - Schema + full staging seed data (in progress)

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

// ============================================================================
// TEST CONSTANTS
// ============================================================================

pub const ADMIN_USER: &str = "admin";
pub const ADMIN_PASS: &str = "admin123";
pub const TEST_USER_EMAIL: &str = "test@example.com";

// ============================================================================
// DATABASE SETUP
// ============================================================================

/// Setup a test database with schema and basic entities.
///
/// Connects to the `ahlt_test` Postgres database, creates a unique schema
/// for test isolation, runs migrations, and seeds with essential entities
/// like relation types and a default role.
///
/// Returns a `TestDb` wrapper holding the pool and schema name.
/// The schema is dropped when `TestDb` is dropped.
pub async fn setup_test_db() -> TestDb {
    let base_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://ahlt@localhost/ahlt_test".to_string());

    // Create a unique schema for this test
    let schema = format!("test_{}", unique_id());

    // First, connect without schema to create it
    let setup_pool = PgPool::connect(&base_url)
        .await
        .expect("Failed to connect to test database");

    sqlx::query(&format!("CREATE SCHEMA \"{}\"", schema))
        .execute(&setup_pool)
        .await
        .expect("Failed to create test schema");

    setup_pool.close().await;

    // Now create a pool that sets search_path on every connection
    let schema_clone = schema.clone();
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .after_connect(move |conn, _meta| {
            let s = schema_clone.clone();
            Box::pin(async move {
                sqlx::query(&format!("SET search_path TO \"{}\"", s))
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
        .connect(&base_url)
        .await
        .expect("Failed to create pool with schema");

    // Run migrations in our schema
    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    // Seed essential entities
    seed_base_entities(&pool).await.expect("Failed to seed base entities");

    TestDb { pool, schema, url: base_url }
}

/// Wrapper that holds a test database pool and cleans up the schema on drop.
pub struct TestDb {
    pub pool: PgPool,
    schema: String,
    url: String,
}

impl TestDb {
    /// Get a reference to the pool for use in test queries.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        // Close the pool first so all connections using the schema are released
        // We need a runtime to run async cleanup
        let url = self.url.clone();
        let schema = self.schema.clone();
        // Close pool synchronously by dropping it (pool will close connections)
        // Then spawn a thread to drop the schema
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build cleanup runtime");
            rt.block_on(async {
                if let Ok(cleanup_pool) = PgPool::connect(&url).await {
                    let _ = sqlx::query(&format!("DROP SCHEMA IF EXISTS \"{}\" CASCADE", schema))
                        .execute(&cleanup_pool)
                        .await;
                    cleanup_pool.close().await;
                }
            });
        })
        .join()
        .ok();
    }
}

/// Generate a unique identifier for schema isolation.
fn unique_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}_{}", ts, count)
}

/// Seed basic entities needed for most tests.
///
/// Creates:
/// - relation_type entities (has_role, requires_permission, belongs_to_tor, etc.)
/// - workflow relation types (transition_from, transition_to)
/// - A default role
async fn seed_base_entities(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Create relation types
    let relation_types = vec![
        "has_role",
        "has_permission",
        "requires_permission",
        "belongs_to_tor",
        "fills_position",
        "participates_in",
        "is_blocking",
        "transition_from",
        "transition_to",
        "minutes_of",
        "section_of",
    ];

    for rt in relation_types {
        sqlx::query(
            "INSERT INTO entities (entity_type, name, label) VALUES ('relation_type', $1, $2)",
        )
        .bind(rt)
        .bind(rt.replace('_', " "))
        .execute(pool)
        .await?;
    }

    // Create a default role
    sqlx::query(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', 'default', 'Default Role')",
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Setup a test database with schema and staging seed data.
pub async fn setup_test_db_seeded() -> TestDb {
    setup_test_db().await
}

// ============================================================================
// RAW SQL HELPERS (for tests that need direct DB manipulation)
// ============================================================================

/// Insert a raw entity and return its id.
pub async fn insert_entity(pool: &PgPool, entity_type: &str, name: &str, label: &str) -> i64 {
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO entities (entity_type, name, label) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(entity_type)
    .bind(name)
    .bind(label)
    .fetch_one(pool)
    .await
    .expect("Failed to insert entity");
    row.0
}

/// Insert a raw entity property (upsert).
pub async fn insert_prop(pool: &PgPool, entity_id: i64, key: &str, value: &str) {
    sqlx::query(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, $2, $3) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(entity_id)
    .bind(key)
    .bind(value)
    .execute(pool)
    .await
    .expect("Failed to insert property");
}

/// Insert a raw relation and return its id.
pub async fn insert_relation(pool: &PgPool, relation_type_id: i64, source_id: i64, target_id: i64) -> i64 {
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(relation_type_id)
    .bind(source_id)
    .bind(target_id)
    .fetch_one(pool)
    .await
    .expect("Failed to insert relation");
    row.0
}
