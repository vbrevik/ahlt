use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::models::data_manager::{import::import_data, types::ImportPayload};

pub type DbPool = PgPool;

const ONTOLOGY_SEED: &str = include_str!("../data/seed/ontology.json");
const STAGING_SEED: &str = include_str!("../data/seed/staging.json");

pub async fn init_pool(database_url: &str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(8)
        .connect(database_url)
        .await
        .expect("Failed to create DB pool")
}

pub async fn run_migrations(pool: &PgPool) {
    sqlx::migrate!()
        .run(pool)
        .await
        .expect("Failed to run migrations");
    log::info!("Database migrations complete");
}

/// Import a JSON seed file using the data manager. Returns (created, skipped) counts.
async fn import_seed(pool: &PgPool, json: &str, label: &str) -> (usize, usize) {
    let payload: ImportPayload =
        serde_json::from_str(json).unwrap_or_else(|e| panic!("Bad {} seed JSON: {}", label, e));
    match import_data(pool, &payload).await {
        Ok(result) => {
            if !result.errors.is_empty() {
                for err in &result.errors {
                    log::warn!("Seed {}: {}", label, err.reason);
                }
            }
            log::info!(
                "Seed {}: created={}, skipped={}, errors={}",
                label,
                result.created,
                result.skipped,
                result.errors.len()
            );
            (result.created, result.skipped)
        }
        Err(e) => {
            log::error!("Seed {} import failed: {}", label, e);
            (0, 0)
        }
    }
}

/// Set the password hash for a user entity (by name).
async fn set_user_password(pool: &PgPool, username: &str, hash: &str) {
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM entities WHERE entity_type = 'user' AND name = $1",
    )
    .bind(username)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    if let Some((id,)) = row {
        let _ = sqlx::query(
            "DELETE FROM entity_properties WHERE entity_id = $1 AND key = 'password'",
        )
        .bind(id)
        .execute(pool)
        .await;

        let _ = sqlx::query(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES ($1, 'password', $2)",
        )
        .bind(id)
        .bind(hash)
        .execute(pool)
        .await;
    }
}

/// Seed base ontology data (relation types, roles, permissions, nav items, settings, workflows).
pub async fn seed_ontology(pool: &PgPool, admin_password_hash: &str) {
    // Skip if database already has data
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM entities")
        .fetch_one(pool)
        .await
        .unwrap_or((0,));
    if count.0 > 0 {
        log::info!(
            "Database already seeded ({} entities), skipping ontology seed",
            count.0
        );
        return;
    }

    import_seed(pool, ONTOLOGY_SEED, "ontology").await;
    set_user_password(pool, "admin", admin_password_hash).await;
    log::info!("Base ontology seed complete");
}

/// Seed ontology + staging demo data (sample users, ToRs, workflows).
pub async fn seed_staging(pool: &PgPool, admin_password_hash: &str) {
    seed_ontology(pool, admin_password_hash).await;

    // Skip if staging data already present
    let has_staging: (bool,) = sqlx::query_as(
        "SELECT COUNT(*) > 0 FROM entities WHERE entity_type = 'user' AND name = 'alice'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or((false,));

    if has_staging.0 {
        log::info!("Staging data already present, skipping");
        return;
    }

    let (created, _) = import_seed(pool, STAGING_SEED, "staging").await;

    // Set passwords for demo users
    if created > 0 {
        let demo_users = ["alice", "bob", "charlie", "diana"];
        for username in &demo_users {
            set_user_password(pool, username, admin_password_hash).await;
        }
    }

    log::info!("Staging seed complete");
}
