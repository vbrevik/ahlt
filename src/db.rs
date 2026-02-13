use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;

pub type DbPool = Pool<SqliteConnectionManager>;

const MIGRATIONS: &str = "
CREATE TABLE IF NOT EXISTS entities (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_type TEXT NOT NULL,
    name        TEXT NOT NULL,
    label       TEXT NOT NULL,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    is_active   INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S','now')),
    UNIQUE(entity_type, name)
);

CREATE TABLE IF NOT EXISTS entity_properties (
    entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    key       TEXT NOT NULL,
    value     TEXT NOT NULL,
    PRIMARY KEY (entity_id, key)
);

CREATE TABLE IF NOT EXISTS relations (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    relation_type_id INTEGER NOT NULL REFERENCES entities(id),
    source_id        INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    target_id        INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    created_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S','now')),
    UNIQUE(relation_type_id, source_id, target_id)
);

CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(entity_type);
CREATE INDEX IF NOT EXISTS idx_relations_source ON relations(source_id, relation_type_id);
CREATE INDEX IF NOT EXISTS idx_relations_target ON relations(target_id, relation_type_id);
CREATE INDEX IF NOT EXISTS idx_properties_entity ON entity_properties(entity_id);
";

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

/// Helper: insert entity and return its id.
fn insert_entity(conn: &rusqlite::Connection, entity_type: &str, name: &str, label: &str, sort_order: i64) -> i64 {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label, sort_order) VALUES (?1, ?2, ?3, ?4)",
        params![entity_type, name, label, sort_order],
    ).unwrap();
    conn.last_insert_rowid()
}

/// Helper: insert property.
fn insert_prop(conn: &rusqlite::Connection, entity_id: i64, key: &str, value: &str) {
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
        params![entity_id, key, value],
    ).unwrap();
}

/// Helper: insert relation.
fn insert_relation(conn: &rusqlite::Connection, rel_type_id: i64, source_id: i64, target_id: i64) {
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
        params![rel_type_id, source_id, target_id],
    ).unwrap();
}

/// Seed the full ontology: relation types, roles, permissions, role-permission relations,
/// and default admin user. Only runs if no entities exist yet.
pub fn seed_ontology(pool: &DbPool, admin_password_hash: &str) {
    let conn = pool.get().expect("Failed to get DB connection for seeding");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM entities", [], |row| row.get(0))
        .unwrap_or(0);

    if count > 0 {
        return;
    }

    log::info!("Seeding ontology...");

    // --- Relation types ---
    let has_role_id = insert_entity(&conn, "relation_type", "has_role", "Has Role", 0);
    let has_perm_id = insert_entity(&conn, "relation_type", "has_permission", "Has Permission", 0);

    // --- Roles ---
    let admin_role_id = insert_entity(&conn, "role", "admin", "Administrator", 1);
    insert_prop(&conn, admin_role_id, "description", "Full system access");

    let user_role_id = insert_entity(&conn, "role", "user", "User", 2);
    insert_prop(&conn, user_role_id, "description", "Standard user access");
    insert_prop(&conn, user_role_id, "is_default", "1");

    // --- Permissions ---
    let perms = [
        ("dashboard.view", "View Dashboard", "Dashboard"),
        ("users.list", "List Users", "Users"),
        ("users.create", "Create Users", "Users"),
        ("users.edit", "Edit Users", "Users"),
        ("users.delete", "Delete Users", "Users"),
        ("roles.manage", "Manage Roles", "Roles"),
        ("settings.manage", "Manage Settings", "Settings"),
    ];

    let mut perm_ids: Vec<(i64, &str)> = Vec::new();
    for (code, label, group) in &perms {
        let id = insert_entity(&conn, "permission", code, label, 0);
        insert_prop(&conn, id, "group_name", group);
        perm_ids.push((id, code));
    }

    // --- Role-permission relations ---
    // Admin gets all permissions
    for (perm_id, _) in &perm_ids {
        insert_relation(&conn, has_perm_id, admin_role_id, *perm_id);
    }

    // User gets dashboard.view + users.list
    let basic_perms = ["dashboard.view", "users.list"];
    for (perm_id, code) in &perm_ids {
        if basic_perms.contains(code) {
            insert_relation(&conn, has_perm_id, user_role_id, *perm_id);
        }
    }

    // --- Default admin user ---
    let admin_user_id = insert_entity(&conn, "user", "admin", "Administrator", 0);
    insert_prop(&conn, admin_user_id, "password", admin_password_hash);
    insert_prop(&conn, admin_user_id, "email", "admin@example.com");
    insert_relation(&conn, has_role_id, admin_user_id, admin_role_id);

    log::info!("Seeded ontology: 2 relation types, 2 roles, {} permissions, 1 admin user", perms.len());
    log::info!("Default admin created — username: admin, password: admin123");
}
