use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;

use crate::models::data_manager::{import::import_data, types::ImportPayload};

pub type DbPool = Pool<SqliteConnectionManager>;

pub const MIGRATIONS: &str = include_str!("schema.sql");

const ONTOLOGY_SEED: &str = include_str!("../data/seed/ontology.json");
const STAGING_SEED: &str = include_str!("../data/seed/staging.json");

pub fn init_pool(database_url: &str) -> DbPool {
    let manager = SqliteConnectionManager::file(database_url).with_init(|conn| {
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        Ok(())
    });
    Pool::builder()
        .max_size(8)
        .build(manager)
        .expect("Failed to create DB pool")
}

pub fn run_migrations(pool: &DbPool) {
    let conn = pool.get().expect("Failed to get DB connection for migrations");
    conn.execute_batch(MIGRATIONS)
        .expect("Failed to run migrations");
    log::info!("Database migrations complete");
}

/// Import a JSON seed file using the data manager. Returns (created, skipped) counts.
fn import_seed(conn: &rusqlite::Connection, json: &str, label: &str) -> (usize, usize) {
    let payload: ImportPayload =
        serde_json::from_str(json).unwrap_or_else(|e| panic!("Bad {} seed JSON: {}", label, e));
    match import_data(conn, &payload) {
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
fn set_user_password(conn: &rusqlite::Connection, username: &str, hash: &str) {
    // Upsert: delete old password property if it exists, then insert new one
    let user_id: Result<i64, _> = conn.query_row(
        "SELECT id FROM entities WHERE entity_type = 'user' AND name = ?1",
        params![username],
        |row| row.get(0),
    );
    if let Ok(id) = user_id {
        conn.execute(
            "DELETE FROM entity_properties WHERE entity_id = ?1 AND key = 'password'",
            params![id],
        )
        .ok();
        conn.execute(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'password', ?2)",
            params![id, hash],
        )
        .ok();
    }
}

/// Seed base ontology data (relation types, roles, permissions, nav items, settings, workflows).
pub fn seed_ontology(pool: &DbPool, admin_password_hash: &str) {
    let conn = pool.get().expect("Failed to get DB connection for seeding");

    // Skip if database already has data (same idempotency as before)
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM entities", [], |row| row.get(0))
        .unwrap_or(0);
    if count > 0 {
        log::info!("Database already seeded ({} entities), skipping ontology seed", count);
        return;
    }

    import_seed(&conn, ONTOLOGY_SEED, "ontology");
    set_user_password(&conn, "admin", admin_password_hash);
    log::info!("Base ontology seed complete");
}

/// Seed ontology + staging demo data (sample users, ToRs, workflows).
pub fn seed_staging(pool: &DbPool, admin_password_hash: &str) {
    seed_ontology(pool, admin_password_hash);

    let conn = pool.get().expect("Failed to get DB connection for staging seed");

    // Skip if staging data already present
    let has_staging: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM entities WHERE entity_type = 'user' AND name = 'alice'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);
    if has_staging {
        log::info!("Staging data already present, skipping");
        return;
    }

    let (created, _) = import_seed(&conn, STAGING_SEED, "staging");

    // Set passwords for demo users
    if created > 0 {
        let demo_users = ["alice", "bob", "charlie", "diana"];
        for username in &demo_users {
            set_user_password(&conn, username, admin_password_hash);
        }
    }

    log::info!("Staging seed complete");
}