# Database Environment Separation — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Separate dev, staging, and test databases via `APP_ENV` env var, eliminate test schema duplication, and fix orphan WAL/SHM files.

**Architecture:** Extract schema SQL to a shared file. `APP_ENV` selects the data directory (`data/dev/`, `data/staging/`). Tests use `tempfile::TempDir` for auto-cleanup. Staging gets a rich seed function.

**Tech Stack:** Rust, SQLite, tempfile crate, include_str!() for schema sharing

---

### Task 1: Extract Schema to Shared SQL File

**Files:**
- Create: `src/schema.sql`
- Modify: `src/db.rs:7-48`

**Step 1: Create the schema SQL file**

Extract the `MIGRATIONS` const content from `db.rs` into `src/schema.sql`. The file should contain only the raw SQL (no Rust string quotes):

```sql
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

CREATE TABLE IF NOT EXISTS relation_properties (
    relation_id INTEGER NOT NULL,
    key         TEXT NOT NULL,
    value       TEXT NOT NULL,
    PRIMARY KEY (relation_id, key),
    FOREIGN KEY (relation_id) REFERENCES relations(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(entity_type);
CREATE INDEX IF NOT EXISTS idx_relations_source ON relations(source_id, relation_type_id);
CREATE INDEX IF NOT EXISTS idx_relations_target ON relations(target_id, relation_type_id);
CREATE INDEX IF NOT EXISTS idx_properties_entity ON entity_properties(entity_id);
```

**Step 2: Update db.rs to use include_str!()**

Replace the `MIGRATIONS` const in `db.rs:7-48` with:

```rust
pub const MIGRATIONS: &str = include_str!("schema.sql");
```

Note: `MIGRATIONS` changes from `const` (private) to `pub const`. The `pub` doesn't matter for the binary crate itself but documents intent.

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: Success (no functional change, same SQL)

**Step 4: Run existing tests to verify no regression**

Run: `cargo test`
Expected: All Phase 2a tests still pass (they use their own schema copy for now)

**Step 5: Commit**

```bash
git add src/schema.sql src/db.rs
git commit -m "refactor: extract schema SQL to shared file"
```

---

### Task 2: Add tempfile Dev-Dependency

**Files:**
- Modify: `Cargo.toml:27-31`

**Step 1: Add tempfile to dev-dependencies**

Add `tempfile = "3"` to the `[dev-dependencies]` section:

```toml
[dev-dependencies]
actix-rt = "2.11"
serde_urlencoded = "0.7"
regex = "1"
tempfile = "3"
rusqlite = { version = "0.32", features = ["bundled"] }
```

Note: `rusqlite` is also added to dev-dependencies so tests can open SQLite connections directly. It's already a regular dependency, but integration tests need it explicitly as a dev-dependency to import it.

**Step 2: Verify it compiles**

Run: `cargo check --tests`
Expected: Success, tempfile downloaded

**Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "chore: add tempfile and rusqlite to dev-dependencies"
```

---

### Task 3: Add APP_ENV Configuration to main.rs

**Files:**
- Modify: `src/main.rs:13-26`

**Step 1: Update main() to read APP_ENV and resolve data directory**

Replace lines 13-26 in `main.rs` with:

```rust
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    // Determine environment and data directory
    let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "dev".to_string());
    let data_dir = format!("data/{}", app_env);
    log::info!("Environment: {} | Data directory: {}", app_env, data_dir);

    // Ensure data directory exists
    std::fs::create_dir_all(&data_dir).expect("Failed to create data directory");

    // Initialize database
    let db_path = format!("{}/app.db", data_dir);
    let pool = db::init_pool(&db_path);
    db::run_migrations(&pool);

    // Seed data based on environment
    let admin_hash = auth::password::hash_password("admin123")
        .expect("Failed to hash default password");
    match app_env.as_str() {
        "staging" => db::seed_staging(&pool, &admin_hash),
        _ => db::seed_ontology(&pool, &admin_hash),
    }
```

**Step 2: Update audit path default**

In `src/audit/mod.rs:64`, the fallback `"data/audit/"` is hardcoded. This will be addressed in Task 5. For now, the audit settings in the DB seed already point to `"data/audit/"` which will be updated in the staging seed.

**Step 3: Verify it compiles**

This will fail because `db::seed_staging` doesn't exist yet. That's expected — we'll add it in Task 4.

---

### Task 4: Add seed_staging() Function

**Files:**
- Modify: `src/db.rs` (add function after `seed_ontology`)

**Step 1: Add seed_staging function**

Add after the `seed_ontology` function (after line 529):

```rust
/// Extended seed for staging environment with realistic test data.
/// Calls seed_ontology() first, then adds additional users, roles, and sample data.
/// Safe to call repeatedly — checks for existing staging data before inserting.
pub fn seed_staging(pool: &DbPool, admin_password_hash: &str) {
    // First, run the base seed
    seed_ontology(pool, admin_password_hash);

    let conn = pool.get().expect("Failed to get DB connection for staging seed");

    // Check if staging data already exists
    let staging_marker: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entities WHERE entity_type='user' AND name='alice'",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    if staging_marker > 0 {
        log::info!("Staging data already seeded, skipping");
        return;
    }

    log::info!("Seeding staging data...");

    // Look up relation type IDs
    let has_role_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='relation_type' AND name='has_role'",
        [], |row| row.get(0),
    ).unwrap();
    let has_perm_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='relation_type' AND name='has_permission'",
        [], |row| row.get(0),
    ).unwrap();

    // --- Additional roles ---
    let editor_role_id = insert_entity(&conn, "role", "editor", "Editor", 3);
    insert_prop(&conn, editor_role_id, "description", "Can create and edit content");

    let viewer_role_id = insert_entity(&conn, "role", "viewer", "Viewer", 4);
    insert_prop(&conn, viewer_role_id, "description", "Read-only access");

    let manager_role_id = insert_entity(&conn, "role", "manager", "Manager", 5);
    insert_prop(&conn, manager_role_id, "description", "Can manage governance workflows");

    // --- Editor permissions: dashboard, users (list/create/edit), suggestions, proposals ---
    let editor_perms = [
        "dashboard.view", "users.list", "users.create", "users.edit",
        "suggestion.view", "suggestion.create", "proposal.view", "proposal.create",
        "proposal.edit", "proposal.submit", "settings.manage",
    ];
    for perm_code in &editor_perms {
        let perm_id: i64 = conn.query_row(
            "SELECT id FROM entities WHERE entity_type='permission' AND name=?1",
            [perm_code], |row| row.get(0),
        ).unwrap();
        insert_relation(&conn, has_perm_id, editor_role_id, perm_id);
    }

    // --- Viewer permissions: dashboard, users.list only ---
    let viewer_perms = ["dashboard.view", "users.list"];
    for perm_code in &viewer_perms {
        let perm_id: i64 = conn.query_row(
            "SELECT id FROM entities WHERE entity_type='permission' AND name=?1",
            [perm_code], |row| row.get(0),
        ).unwrap();
        insert_relation(&conn, has_perm_id, viewer_role_id, perm_id);
    }

    // --- Manager permissions: dashboard, users, governance, workflow ---
    let manager_perms = [
        "dashboard.view", "users.list", "users.create", "users.edit",
        "tor.list", "tor.create", "tor.edit", "tor.manage_members",
        "suggestion.view", "suggestion.create", "suggestion.review",
        "proposal.view", "proposal.create", "proposal.edit", "proposal.submit",
        "proposal.review", "proposal.approve",
        "agenda.view", "agenda.create", "agenda.queue", "agenda.manage",
        "agenda.participate", "agenda.decide",
        "coa.create", "coa.edit",
    ];
    for perm_code in &manager_perms {
        let perm_id: i64 = conn.query_row(
            "SELECT id FROM entities WHERE entity_type='permission' AND name=?1",
            [perm_code], |row| row.get(0),
        ).unwrap();
        insert_relation(&conn, has_perm_id, manager_role_id, perm_id);
    }

    // --- Additional users (all use same password hash as admin for easy testing) ---
    let alice_id = insert_entity(&conn, "user", "alice", "Alice Editor", 0);
    insert_prop(&conn, alice_id, "password", admin_password_hash);
    insert_prop(&conn, alice_id, "email", "alice@example.com");
    insert_relation(&conn, has_role_id, alice_id, editor_role_id);

    let bob_id = insert_entity(&conn, "user", "bob", "Bob Viewer", 0);
    insert_prop(&conn, bob_id, "password", admin_password_hash);
    insert_prop(&conn, bob_id, "email", "bob@example.com");
    insert_relation(&conn, has_role_id, bob_id, viewer_role_id);

    let charlie_id = insert_entity(&conn, "user", "charlie", "Charlie Manager", 0);
    insert_prop(&conn, charlie_id, "password", admin_password_hash);
    insert_prop(&conn, charlie_id, "email", "charlie@example.com");
    insert_relation(&conn, has_role_id, charlie_id, manager_role_id);

    let diana_id = insert_entity(&conn, "user", "diana", "Diana Admin", 0);
    insert_prop(&conn, diana_id, "password", admin_password_hash);
    insert_prop(&conn, diana_id, "email", "diana@example.com");
    let admin_role_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='role' AND name='admin'",
        [], |row| row.get(0),
    ).unwrap();
    insert_relation(&conn, has_role_id, diana_id, admin_role_id);

    log::info!("Staging data seeded: 3 additional roles (editor, viewer, manager), 4 additional users (alice, bob, charlie, diana)");
}
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: Success

**Step 3: Commit Tasks 3+4 together**

```bash
git add src/main.rs src/db.rs
git commit -m "feat: add APP_ENV config and staging seed data"
```

---

### Task 5: Update Audit Path for Environment Awareness

**Files:**
- Modify: `src/main.rs` (pass data_dir to audit cleanup)
- Modify: `src/db.rs` (update audit seed path)

**Step 1: Update audit seed path in seed_ontology**

In `src/db.rs`, the audit settings seed hardcodes `"data/audit/"` on line 310 and `"data/audit"` on line 510. These need to remain as defaults since the seed runs before any environment context. The audit system reads the path from the DB at runtime, so this is OK — it will resolve to the correct directory.

However, the `seed_ontology` function creates the audit directory at `data/audit` (line 510-525). Update this to use the data directory:

Change `src/db.rs` line 509-510 from:
```rust
    // Create audit directory with secure permissions
    let audit_path = "data/audit";
```
to:
```rust
    // Create audit directory with secure permissions
    // Note: audit path is configurable via audit.log_path setting
    // Default seed value is "data/audit/" but actual path is read from DB at runtime
    let audit_path = "data/audit";
```

Actually, the simplest fix: **don't create the audit directory in the seed**. The audit module already creates it on-demand in `get_log_path()` (line 67: `fs::create_dir_all(&log_path)?`). Remove the directory creation from the seed function entirely (lines 509-525).

**Step 2: Update audit.log_path seed default for staging**

In the staging seed (Task 4), the audit settings are inherited from `seed_ontology`. No change needed — the staging environment will use whatever `audit.log_path` is configured in the DB.

**Step 3: Update the seed's audit.log_path default value**

Change `src/db.rs` line 310 from:
```rust
    insert_prop(&conn, audit_log_path_id, "value", "data/audit/");
```
to use a relative path that will work for any environment:
```rust
    insert_prop(&conn, audit_log_path_id, "value", "data/audit/");
```

Actually, keep this as-is. The audit path is a user-configurable setting. When running staging, users can change it via the Settings UI. The default `data/audit/` works fine — audit logs are a cross-environment concern anyway.

**Step 4: Remove hardcoded audit directory creation from seed**

Delete lines 509-525 from `seed_ontology()` in `src/db.rs`:
```rust
    // Create audit directory with secure permissions
    let audit_path = "data/audit";
    if !std::path::Path::new(audit_path).exists() {
        std::fs::create_dir_all(audit_path)
            ...
    }
```

The audit module's `get_log_path()` already handles directory creation. This removes the coupling between the seed function and the filesystem.

**Step 5: Verify and commit**

Run: `cargo check`

```bash
git add src/db.rs
git commit -m "refactor: remove hardcoded audit dir creation from seed"
```

---

### Task 6: Rewrite Phase 2a Tests

**Files:**
- Modify: `tests/phase2a_integration_test.rs`

**Step 1: Rewrite test file to use shared schema and TempDir**

Replace the entire file with:

```rust
use std::collections::HashSet;
use regex::Regex;
use tempfile::TempDir;

// ============================================================================
// SHARED TEST INFRASTRUCTURE
// ============================================================================

/// Real schema from src/schema.sql — single source of truth
const MIGRATIONS: &str = include_str!("../src/schema.sql");

fn setup_test_db() -> (TempDir, rusqlite::Connection) {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = dir.path().join("test.db");
    let conn = rusqlite::Connection::open(&db_path).expect("Failed to open test DB");
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;")
        .expect("Failed to set pragmas");
    conn.execute_batch(MIGRATIONS)
        .expect("Failed to run migrations");
    (dir, conn)
}

fn insert_entity(
    conn: &rusqlite::Connection,
    entity_type: &str,
    name: &str,
    label: &str,
) -> i64 {
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES (?, ?, ?)",
        [entity_type, name, label],
    ).expect("Failed to insert entity");
    conn.last_insert_rowid()
}

fn insert_prop(conn: &rusqlite::Connection, entity_id: i64, key: &str, value: &str) {
    conn.execute(
        "INSERT OR REPLACE INTO entity_properties (entity_id, key, value) VALUES (?1, ?2, ?3)",
        rusqlite::params![entity_id, key, value],
    ).expect("Failed to insert property");
}

fn insert_relation(
    conn: &rusqlite::Connection,
    relation_type_id: i64,
    source_id: i64,
    target_id: i64,
) -> i64 {
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
        rusqlite::params![relation_type_id, source_id, target_id],
    ).expect("Failed to insert relation");
    conn.last_insert_rowid()
}

fn get_permissions_for_user(conn: &rusqlite::Connection, user_id: i64) -> HashSet<String> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT p.name FROM entities p
         WHERE p.entity_type = 'permission'
         AND EXISTS (
             SELECT 1 FROM relations r1
             WHERE r1.relation_type_id = (
                 SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission'
             )
             AND r1.target_id = p.id
             AND r1.source_id IN (
                 SELECT r2.target_id FROM relations r2
                 WHERE r2.relation_type_id = (
                     SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role'
                 )
                 AND r2.source_id = ?1
             )
         )"
    ).expect("Failed to prepare permissions query");

    stmt.query_map([user_id], |row| row.get(0))
        .expect("Failed to query permissions")
        .filter_map(Result::ok)
        .collect()
}

fn extract_csrf_token(html: &str) -> String {
    let re = Regex::new(r#"name="csrf_token"\s+value="([^"]+)""#)
        .expect("Failed to compile regex");
    re.captures(html)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "invalid_token".to_string())
}

// ============================================================================
// INFRASTRUCTURE TESTS
// ============================================================================

#[test]
fn test_infrastructure_compiled() {
    // Verify test infrastructure compiles and TempDir works
    let (_dir, conn) = setup_test_db();
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM entities", [], |row| row.get(0)).unwrap();
    assert_eq!(count, 0, "Fresh database should have zero entities");
}

#[test]
fn test_csrf_extraction() {
    let html = r#"<form><input type="hidden" name="csrf_token" value="test_token_12345" /></form>"#;
    assert_eq!(extract_csrf_token(html), "test_token_12345");
}

#[test]
fn test_csrf_extraction_missing() {
    let html = r#"<form><input type="text" name="username" /></form>"#;
    assert_eq!(extract_csrf_token(html), "invalid_token");
}

// ============================================================================
// AUTHENTICATION TESTS
// ============================================================================

#[test]
fn test_auth_user_lookup() {
    let (_dir, conn) = setup_test_db();

    let user_id = insert_entity(&conn, "user", "testuser", "Test User");
    insert_prop(&conn, user_id, "password", "hashed_password_123");

    let (found_id, found_name): (i64, String) = conn.query_row(
        "SELECT id, name FROM entities WHERE entity_type = 'user' AND name = 'testuser'",
        [], |row| Ok((row.get(0)?, row.get(1)?)),
    ).expect("User should be found");

    assert_eq!(found_id, user_id);
    assert_eq!(found_name, "testuser");

    let password: String = conn.query_row(
        "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = 'password'",
        [user_id], |row| row.get(0),
    ).expect("Password should be retrievable");
    assert_eq!(password, "hashed_password_123");
}

#[test]
fn test_auth_nonexistent_user() {
    let (_dir, conn) = setup_test_db();

    let found: Option<i64> = conn.query_row(
        "SELECT id FROM entities WHERE entity_type = 'user' AND name = 'nonexistent'",
        [], |row| row.get(0),
    ).ok();
    assert!(found.is_none());
}

#[test]
fn test_auth_permission_assignment() {
    let (_dir, conn) = setup_test_db();

    let has_role_id = insert_entity(&conn, "relation_type", "has_role", "Has Role");
    let has_permission_id = insert_entity(&conn, "relation_type", "has_permission", "Has Permission");

    let user_id = insert_entity(&conn, "user", "alice", "Alice");
    let admin_role_id = insert_entity(&conn, "role", "admin", "Administrator");
    let perm_id = insert_entity(&conn, "permission", "users.create", "Create Users");

    insert_relation(&conn, has_role_id, user_id, admin_role_id);
    insert_relation(&conn, has_permission_id, admin_role_id, perm_id);

    let has_perm: bool = conn.query_row(
        "SELECT COUNT(*) FROM entities p
         WHERE p.entity_type = 'permission' AND p.name = 'users.create'
         AND EXISTS (
             SELECT 1 FROM relations r1
             WHERE r1.relation_type_id = ?1 AND r1.target_id = p.id
             AND r1.source_id IN (
                 SELECT r2.target_id FROM relations r2
                 WHERE r2.relation_type_id = ?2 AND r2.source_id = ?3
             )
         )",
        [has_permission_id, has_role_id, user_id],
        |row| row.get::<_, i64>(0).map(|c| c > 0),
    ).unwrap();

    assert!(has_perm, "User should have users.create through role");
}

// ============================================================================
// USER MANAGEMENT TESTS
// ============================================================================

#[test]
fn test_user_create_and_retrieve() {
    let (_dir, conn) = setup_test_db();

    let user_id = insert_entity(&conn, "user", "newuser", "New User");
    insert_prop(&conn, user_id, "email", "newuser@example.com");
    insert_prop(&conn, user_id, "password", "hashed_pass");

    let (id, name, label): (i64, String, String) = conn.query_row(
        "SELECT id, name, label FROM entities WHERE entity_type = 'user' AND id = ?1",
        [user_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).expect("User should be retrievable");

    assert_eq!(id, user_id);
    assert_eq!(name, "newuser");
    assert_eq!(label, "New User");

    let email: String = conn.query_row(
        "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = 'email'",
        [user_id], |row| row.get(0),
    ).unwrap();
    assert_eq!(email, "newuser@example.com");
}

#[test]
fn test_user_update_properties() {
    let (_dir, conn) = setup_test_db();

    let user_id = insert_entity(&conn, "user", "updatetest", "Update Test");
    insert_prop(&conn, user_id, "email", "old@example.com");
    insert_prop(&conn, user_id, "email", "new@example.com");

    let email: String = conn.query_row(
        "SELECT value FROM entity_properties WHERE entity_id = ?1 AND key = 'email'",
        [user_id], |row| row.get(0),
    ).unwrap();
    assert_eq!(email, "new@example.com");
}

// ============================================================================
// PERMISSION ENFORCEMENT TESTS
// ============================================================================

#[test]
fn test_permission_admin_has_all() {
    let (_dir, conn) = setup_test_db();

    let has_role_id = insert_entity(&conn, "relation_type", "has_role", "Has Role");
    let has_perm_id = insert_entity(&conn, "relation_type", "has_permission", "Has Permission");

    let admin_role_id = insert_entity(&conn, "role", "admin", "Administrator");

    let p1 = insert_entity(&conn, "permission", "users.list", "List Users");
    let p2 = insert_entity(&conn, "permission", "users.create", "Create Users");
    let p3 = insert_entity(&conn, "permission", "roles.manage", "Manage Roles");

    insert_relation(&conn, has_perm_id, admin_role_id, p1);
    insert_relation(&conn, has_perm_id, admin_role_id, p2);
    insert_relation(&conn, has_perm_id, admin_role_id, p3);

    let admin_id = insert_entity(&conn, "user", "admin", "Administrator");
    insert_relation(&conn, has_role_id, admin_id, admin_role_id);

    let perms = get_permissions_for_user(&conn, admin_id);
    assert_eq!(perms.len(), 3);
    assert!(perms.contains("users.list"));
    assert!(perms.contains("users.create"));
    assert!(perms.contains("roles.manage"));
}

#[test]
fn test_permission_viewer_limited() {
    let (_dir, conn) = setup_test_db();

    let has_role_id = insert_entity(&conn, "relation_type", "has_role", "Has Role");
    let has_perm_id = insert_entity(&conn, "relation_type", "has_permission", "Has Permission");

    let viewer_role_id = insert_entity(&conn, "role", "viewer", "Viewer");

    let p_list = insert_entity(&conn, "permission", "users.list", "List Users");
    let _p_create = insert_entity(&conn, "permission", "users.create", "Create Users");

    insert_relation(&conn, has_perm_id, viewer_role_id, p_list);
    // Note: users.create NOT granted to viewer

    let viewer_id = insert_entity(&conn, "user", "bob", "Bob Viewer");
    insert_relation(&conn, has_role_id, viewer_id, viewer_role_id);

    let perms = get_permissions_for_user(&conn, viewer_id);
    assert_eq!(perms.len(), 1);
    assert!(perms.contains("users.list"));
    assert!(!perms.contains("users.create"));
}
```

**Step 2: Run tests to verify all pass**

Run: `cargo test --test phase2a_integration_test`
Expected: 10 passed, 0 failed

**Step 3: Commit**

```bash
git add tests/phase2a_integration_test.rs
git commit -m "refactor: Phase 2a tests use shared schema and TempDir"
```

---

### Task 7: Rewrite Phase 2b Tests

**Files:**
- Modify: `tests/phase2b_e2e_test.rs`

**Step 1: Read the current Phase 2b test file to understand all tests**

Read `tests/phase2b_e2e_test.rs` fully — it has 12 tests with complex setup. The rewrite needs to:
- Replace `init_test_db()` with the `setup_test_db()` pattern using TempDir
- Use `include_str!("../src/schema.sql")` for the real schema
- Remove all `test_db_path()` and `cleanup_test_db()` functions
- Ensure `(_dir, conn)` pattern so TempDir lives as long as the connection

**Step 2: Apply the same pattern as Phase 2a**

Replace the init/cleanup infrastructure at the top of the file. Each test function that creates a DB should use `setup_test_db()`.

**Step 3: Run tests**

Run: `cargo test --test phase2b_e2e_test`
Expected: All 12 tests pass (including the 8 that were previously failing due to missing schema)

**Step 4: Commit**

```bash
git add tests/phase2b_e2e_test.rs
git commit -m "refactor: Phase 2b tests use shared schema and TempDir"
```

---

### Task 8: Cleanup — Gitignore, Delete test_data, Move DB

**Files:**
- Modify: `.gitignore`
- Delete: `test_data/` directory
- Move: `data/app.db` → `data/dev/app.db`

**Step 1: Add test_data/ to .gitignore**

Add `test_data/` to `.gitignore` (after the `*.db` line):

```
/target
/data
*.db
.env
*.png
!docs/screenshots/*.png
.playwright-mcp/
.claude/
.serena/
.worktrees/
test_data/
```

**Step 2: Delete test_data/ directory**

```bash
rm -rf test_data/
```

**Step 3: Move existing dev database**

```bash
mkdir -p data/dev
# Only move if data/app.db exists (it may not on fresh clones)
[ -f data/app.db ] && mv data/app.db data/dev/app.db
[ -f data/app.db-shm ] && mv data/app.db-shm data/dev/app.db-shm
[ -f data/app.db-wal ] && mv data/app.db-wal data/dev/app.db-wal
# Move audit directory if it exists
[ -d data/audit ] && mv data/audit data/dev/audit
```

**Step 4: Verify the app starts with default env**

Run: `cargo run` (briefly, then Ctrl+C)
Expected: Creates `data/dev/app.db`, seeds ontology, starts server

**Step 5: Verify staging env works**

Run: `APP_ENV=staging cargo run` (briefly, then Ctrl+C)
Expected: Creates `data/staging/app.db`, seeds ontology + staging data

**Step 6: Verify all tests pass**

Run: `cargo test`
Expected: All tests pass, no files created in `test_data/`

**Step 7: Commit**

```bash
git add .gitignore
git commit -m "feat: database environment separation complete

- APP_ENV=dev|staging selects data directory
- Tests use TempDir for auto-cleanup
- Staging seed with rich demo data
- Schema shared via include_str!()"
```

---

### Task 9: Final Verification

**Step 1: Run full test suite**

Run: `cargo test`
Expected: All tests pass

**Step 2: Verify git status is clean**

Run: `git status`
Expected: No untracked test_data files, no orphan WAL/SHM files

**Step 3: Verify dev environment**

Run: `cargo run`, visit http://localhost:8080, login as admin/admin123
Expected: Normal operation, single admin user

**Step 4: Verify staging environment**

Run: `APP_ENV=staging cargo run`, visit http://localhost:8080
Expected: Login as admin, alice, bob, charlie, or diana (all password: admin123)
Verify: Multiple users visible, different roles assigned

**Step 5: Check data directory structure**

```bash
ls -la data/dev/
ls -la data/staging/
```
Expected: Separate app.db files in each directory
