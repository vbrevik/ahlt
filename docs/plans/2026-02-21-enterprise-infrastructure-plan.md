# Enterprise Infrastructure Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Migrate im-ctrl from SQLite + local dev to PostgreSQL 17 + Neo4j + Docker Compose + GitLab CI + Kubernetes + nginx Ingress.

**Architecture:** Incremental 5-phase approach. Phase 1a (PostgreSQL) is the largest — touching ~65 model files, ~53 handler files, and ~21 test files. All subsequent phases are additive (new files, no rewrites).

**Tech Stack:** Rust/Actix-web, sqlx (postgres), Neo4j (bolt via `neo4rs`), Docker Compose, GitLab CE, k3s (Rancher Desktop), nginx Ingress, Helm 3.

**Design doc:** `docs/plans/2026-02-21-enterprise-infrastructure-design.md`

---

## Codebase Inventory (Migration Scope)

Before starting, understand the blast radius:

**Core infrastructure (4 files):**
- `src/db.rs` — pool init, migrations, seeding
- `src/errors.rs` — `AppError::Db(rusqlite::Error)`, `AppError::Pool(r2d2::Error)`, From impls
- `src/lib.rs` — module declarations
- `src/main.rs` — startup, pool creation, env config

**Model files (65 files across 17 modules):**
- `src/models/entity.rs` — core EAV entity CRUD
- `src/models/relation.rs` — core EAV relation CRUD
- `src/models/setting.rs` — app settings
- `src/models/audit.rs` — audit logging
- `src/models/nav_item.rs` — navigation menu
- `src/models/permission.rs` — permission queries
- `src/models/mod.rs` — module declarations
- `src/models/user/` — types.rs, queries.rs, filter.rs, mod.rs
- `src/models/role/` — types.rs, queries.rs, builder.rs, mod.rs
- `src/models/ontology/` — schema.rs, instance.rs, entities.rs, mod.rs
- `src/models/tor/` — types.rs, queries.rs, dependencies.rs, calendar.rs, mod.rs
- `src/models/meeting/` — types.rs, queries.rs, mod.rs
- `src/models/agenda_point/` — types.rs, queries.rs, mod.rs
- `src/models/minutes/` — types.rs, queries.rs, mod.rs
- `src/models/coa/` — types.rs, queries.rs, sections.rs, mod.rs
- `src/models/opinion/` — types.rs, queries.rs, mod.rs
- `src/models/suggestion/` — types.rs, queries.rs, mod.rs
- `src/models/proposal/` — types.rs, queries.rs, mod.rs
- `src/models/workflow/` — types.rs, queries.rs, mod.rs
- `src/models/protocol/` — types.rs, queries.rs, mod.rs
- `src/models/document/` — types.rs, queries.rs, mod.rs
- `src/models/presentation_template/` — types.rs, queries.rs, mod.rs
- `src/models/data_manager/` — types.rs, import.rs, export.rs, jsonld.rs, mod.rs
- `src/models/table_filter/` — builder.rs, columns.rs, mod.rs

**Handler files (53 files across 14 modules):**
- `src/handlers/mod.rs` + dashboard.rs, auth_handlers.rs, account_handlers.rs
- `src/handlers/user_handlers/` — mod.rs, list.rs, crud.rs
- `src/handlers/role_handlers/` — mod.rs, list.rs, crud.rs, assignment.rs, helpers.rs
- `src/handlers/tor_handlers/` — mod.rs, crud.rs, list.rs, members.rs, dependencies.rs, calendar.rs, presentation.rs, protocol.rs
- `src/handlers/meeting_handlers/` — mod.rs, list.rs, crud.rs, export.rs
- `src/handlers/minutes_handlers/` — mod.rs, crud.rs
- `src/handlers/document_handlers/` — mod.rs, list.rs, crud.rs
- `src/handlers/warning_handlers/` — mod.rs, list.rs, detail.rs, actions.rs, ws.rs
- `src/handlers/governance_handlers/` — mod.rs, map.rs
- `src/handlers/api_v1/` — mod.rs, entities.rs, users.rs
- Individual: agenda_handlers.rs, coa_handlers.rs, opinion_handlers.rs, suggestion_handlers.rs, proposal_handlers.rs, workflow_handlers.rs, workflow_builder_handlers.rs, role_builder_handlers.rs, menu_builder_handlers.rs, settings_handlers.rs, ontology_handlers.rs, audit_handlers.rs, data_handlers.rs, queue_handlers.rs

**Other Rust files:**
- `src/auth/` — session.rs, password.rs, csrf.rs, abac.rs, mod.rs
- `src/warnings/` — generators, scheduler, queries
- `src/templates_structs.rs` — template context types

**Test files (21 files):**
- `tests/common/mod.rs` — test infrastructure
- 20 test files: user_test, auth_test, tor_test, meeting_test, governance_test, warning_test, warnings_test, permission_test, proposal_test, role_builder_test, role_builder_model_test, workflow_integration_test, workflow_builder_test, minutes_test, abac_test, opinion_relation_test, calendar_confirmation_e2e, phase2a_integration_test, phase2b_e2e_test, mod.rs

---

## Current Patterns → Target Patterns

### Model function signature

```rust
// BEFORE (rusqlite)
pub fn find_by_id(conn: &Connection, id: i64) -> rusqlite::Result<Option<Entity>> {
    let mut stmt = conn.prepare("SELECT ... FROM entities WHERE id = ?1")?;
    let mut rows = stmt.query_map(params![id], row_to_entity)?;
    match rows.next() { Some(row) => Ok(Some(row?)), None => Ok(None) }
}

// AFTER (sqlx)
pub async fn find_by_id(pool: &PgPool, id: i64) -> Result<Option<Entity>, sqlx::Error> {
    sqlx::query_as!(Entity, "SELECT ... FROM entities WHERE id = $1", id)
        .fetch_optional(pool)
        .await
}
```

### Handler pattern

```rust
// BEFORE
pub async fn handler(pool: web::Data<DbPool>, session: Session) -> Result<HttpResponse, AppError> {
    let conn = pool.get()?;
    let users = user::find_all(&conn)?;
    // ...
}

// AFTER
pub async fn handler(pool: web::Data<PgPool>, session: Session) -> Result<HttpResponse, AppError> {
    let users = user::find_all(&pool).await?;
    // ...
}
```

### SQL dialect changes

| SQLite | PostgreSQL 17 |
|---|---|
| `?1, ?2, ?3` | `$1, $2, $3` |
| `INTEGER PRIMARY KEY AUTOINCREMENT` | `BIGINT GENERATED ALWAYS AS IDENTITY` |
| `TEXT NOT NULL DEFAULT (strftime(...))` | `TIMESTAMPTZ NOT NULL DEFAULT NOW()` |
| `PRAGMA foreign_keys=ON` | Always on (schema constraint) |
| `PRAGMA journal_mode=WAL` | Not needed (MVCC built-in) |
| `INSERT OR IGNORE` | `INSERT ... ON CONFLICT DO NOTHING` |
| `GROUP_CONCAT(col, ',')` | `STRING_AGG(col, ',')` |
| `row.get::<_, i64>("is_active")? != 0` | `row.get::<_, bool>("is_active")` (native bool) |
| Dynamic params `Box<dyn ToSql>` | sqlx bind with `query_builder` or string formatting |

### Test infrastructure

```rust
// BEFORE
pub fn setup_test_db() -> (TempDir, Connection) {
    let dir = TempDir::new().expect("...");
    let conn = Connection::open(dir.path().join("test.db")).expect("...");
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    conn.execute_batch(MIGRATIONS).unwrap();
    seed_base_entities(&conn).unwrap();
    (dir, conn)
}

// AFTER (using sqlx test utilities)
pub async fn setup_test_db() -> PgPool {
    let db_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or("postgresql://ahlt:test@localhost:5432/ahlt_test".to_string());
    let pool = PgPool::connect(&db_url).await.expect("...");
    sqlx::migrate!().run(&pool).await.expect("...");
    seed_base_entities(&pool).await.expect("...");
    pool
}
```

---

## PHASE 1a: PostgreSQL 17 Migration

### Task 1: Local Postgres + Cargo Dependencies

**Prompt Contract:**
- **GOAL:** Stand up local Postgres 17 for development and update Cargo.toml dependencies.
- **CONSTRAINTS:** Postgres via Homebrew (or Docker). Keep both rusqlite and sqlx temporarily so the project compiles during incremental migration. Do not delete rusqlite yet.
- **FORMAT:** Updated Cargo.toml, working `psql` connection, three databases created.
- **FAILURE CONDITIONS:** `cargo check` fails. Cannot connect to Postgres.

**Files:**
- Modify: `Cargo.toml`

**Steps:**

1. Install Postgres 17 locally:
   ```bash
   brew install postgresql@17
   brew services start postgresql@17
   createdb ahlt_dev
   createdb ahlt_staging
   createdb ahlt_test
   createuser -s ahlt
   psql -c "ALTER USER ahlt PASSWORD 'secret';"
   ```

2. Add sqlx dependencies to Cargo.toml:
   ```toml
   sqlx = { version = "0.8", features = ["postgres", "runtime-tokio", "macros"] }
   neo4rs = "0.8"  # For Phase 1b
   ```
   Keep existing rusqlite/r2d2 deps for now.

3. Run: `cargo check` — must pass.

4. Commit: `feat(deps): add sqlx and neo4rs dependencies for postgres migration`

---

### Task 2: PostgreSQL Schema Migration

**Prompt Contract:**
- **GOAL:** Convert SQLite schema.sql to PostgreSQL migration files using sqlx-cli.
- **CONSTRAINTS:** Keep the EAV structure identical. Use `BIGINT GENERATED ALWAYS AS IDENTITY`. Use `TIMESTAMPTZ`. Maintain all indexes and constraints.
- **FORMAT:** `migrations/001_initial.up.sql` that creates the same tables in Postgres dialect.
- **FAILURE CONDITIONS:** `sqlx migrate run` fails. Schema differs structurally from SQLite version.

**Files:**
- Create: `migrations/001_initial.up.sql`
- Create: `migrations/001_initial.down.sql`

**Steps:**

1. Install sqlx-cli:
   ```bash
   cargo install sqlx-cli --features postgres
   ```

2. Create the migration:
   ```bash
   sqlx migrate add initial
   ```

3. Write `migrations/001_initial.up.sql`:
   ```sql
   CREATE TABLE entities (
       id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
       entity_type TEXT NOT NULL,
       name        TEXT NOT NULL,
       label       TEXT NOT NULL,
       sort_order  INTEGER NOT NULL DEFAULT 0,
       is_active   BOOLEAN NOT NULL DEFAULT TRUE,
       created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
       updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
       UNIQUE(entity_type, name)
   );

   CREATE TABLE entity_properties (
       entity_id BIGINT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
       key       TEXT NOT NULL,
       value     TEXT NOT NULL,
       PRIMARY KEY (entity_id, key)
   );

   CREATE TABLE relations (
       id               BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
       relation_type_id BIGINT NOT NULL REFERENCES entities(id),
       source_id        BIGINT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
       target_id        BIGINT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
       created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
       UNIQUE(relation_type_id, source_id, target_id)
   );

   CREATE TABLE relation_properties (
       relation_id BIGINT NOT NULL REFERENCES relations(id) ON DELETE CASCADE,
       key         TEXT NOT NULL,
       value       TEXT NOT NULL,
       PRIMARY KEY (relation_id, key)
   );

   CREATE INDEX idx_entities_type ON entities(entity_type);
   CREATE INDEX idx_relations_source ON relations(source_id, relation_type_id);
   CREATE INDEX idx_relations_target ON relations(target_id, relation_type_id);
   CREATE INDEX idx_properties_entity ON entity_properties(entity_id);
   CREATE INDEX idx_properties_entity_key ON entity_properties(entity_id, key);
   ```

4. Write `migrations/001_initial.down.sql`:
   ```sql
   DROP TABLE IF EXISTS relation_properties;
   DROP TABLE IF EXISTS relations;
   DROP TABLE IF EXISTS entity_properties;
   DROP TABLE IF EXISTS entities;
   ```

5. Run migration:
   ```bash
   DATABASE_URL=postgresql://ahlt:secret@localhost/ahlt_dev sqlx migrate run
   ```

6. Verify:
   ```bash
   psql -U ahlt ahlt_dev -c "\dt"
   ```

7. Commit: `feat(schema): add PostgreSQL migration from SQLite schema`

---

### Task 3: Core Infrastructure — db.rs + errors.rs

**Prompt Contract:**
- **GOAL:** Rewrite `db.rs` to use `sqlx::PgPool` and `errors.rs` to use `sqlx::Error`. Remove rusqlite/r2d2 pool types.
- **CONSTRAINTS:** All other files will break after this — that's expected. This task only makes `db.rs` and `errors.rs` correct. The `DbPool` type alias changes globally.
- **FORMAT:** `db.rs` exports `PgPool`-based pool init + async seed functions. `errors.rs` has `AppError::Db(sqlx::Error)`.
- **FAILURE CONDITIONS:** `db.rs` or `errors.rs` don't compile in isolation.

**Files:**
- Rewrite: `src/db.rs`
- Rewrite: `src/errors.rs`

**Steps:**

1. Rewrite `src/errors.rs`:
   - `AppError::Db(sqlx::Error)` replaces `AppError::Db(rusqlite::Error)`
   - Remove `AppError::Pool(r2d2::Error)` — sqlx doesn't use r2d2
   - Update `From<sqlx::Error>` impl
   - Remove `From<r2d2::Error>` impl

2. Rewrite `src/db.rs`:
   - `pub type DbPool = sqlx::PgPool;`
   - `pub async fn init_pool(database_url: &str) -> PgPool` using `PgPoolOptions`
   - `pub async fn run_migrations(pool: &PgPool)` using `sqlx::migrate!()`
   - `pub async fn seed_ontology(pool: &PgPool, admin_password_hash: &str)`
   - `pub async fn seed_staging(pool: &PgPool, admin_password_hash: &str)`
   - Seed functions use `sqlx::query!()` instead of rusqlite params
   - `import_seed()` becomes async, takes `&PgPool`

3. Do NOT run `cargo check` yet — expect 100+ errors from downstream files.

4. Commit: `refactor(core): rewrite db.rs and errors.rs for sqlx/postgres`

---

### Task 4: Core Models — entity.rs + relation.rs

**Prompt Contract:**
- **GOAL:** Migrate the two foundational model files to async sqlx queries. Every other model depends on these.
- **CONSTRAINTS:** Keep all existing function signatures (name, params) but change `&Connection` → `&PgPool`, return `Result<T, sqlx::Error>`, and make async. Convert all SQL to Postgres dialect.
- **FORMAT:** Both files compile with sqlx. All functions are async. SQL uses `$1` params.
- **FAILURE CONDITIONS:** Function signatures change in a way that would require different call patterns from handlers. Missing a function that other models depend on.

**Files:**
- Rewrite: `src/models/entity.rs`
- Rewrite: `src/models/relation.rs`

**Steps:**

1. Migrate `entity.rs`:
   - All `fn(conn: &Connection, ...)` → `async fn(pool: &PgPool, ...)`
   - `rusqlite::Result<T>` → `Result<T, sqlx::Error>`
   - `conn.prepare()` + `query_map()` → `sqlx::query_as!()` + `.fetch_all(pool).await`
   - `params![x]` → positional `$1` in SQL string
   - `row.get::<_, i64>("is_active")? != 0` → native `bool` column
   - Derive `sqlx::FromRow` on `Entity` struct

2. Migrate `relation.rs` — same pattern.

3. Commit: `refactor(models): migrate entity.rs and relation.rs to sqlx/postgres`

---

### Task 5: Remaining Model Modules (Batch Migration)

**Prompt Contract:**
- **GOAL:** Migrate all remaining model files to async sqlx. This is the largest single task.
- **CONSTRAINTS:** Follow exact same transformation pattern from Task 4. Convert one module at a time. Each module has types.rs (structs only — may need no changes beyond adding `sqlx::FromRow`) and queries.rs (all SQL changes). Do not change business logic.
- **FORMAT:** Every model file compiles. All SQL uses Postgres dialect. All functions are async.
- **FAILURE CONDITIONS:** Any query produces different results than the SQLite version. Missing a `GROUP_CONCAT` → `STRING_AGG` conversion. Missing an `INSERT OR IGNORE` → `ON CONFLICT DO NOTHING` conversion.

**Files:** All files listed in the inventory under "Model files" except entity.rs and relation.rs.

**Migration order (respecting dependencies):**

1. `src/models/permission.rs` — simple, few queries
2. `src/models/setting.rs` — simple key-value
3. `src/models/audit.rs` — standalone
4. `src/models/nav_item.rs` — standalone
5. `src/models/user/` — types.rs (add FromRow), queries.rs, filter.rs
6. `src/models/role/` — types.rs, queries.rs, builder.rs
7. `src/models/ontology/` — schema.rs, instance.rs, entities.rs
8. `src/models/workflow/` — types.rs, queries.rs
9. `src/models/tor/` — types.rs, queries.rs, dependencies.rs, calendar.rs
10. `src/models/meeting/` — types.rs, queries.rs
11. `src/models/agenda_point/` — types.rs, queries.rs
12. `src/models/minutes/` — types.rs, queries.rs
13. `src/models/coa/` — types.rs, queries.rs, sections.rs
14. `src/models/opinion/` — types.rs, queries.rs
15. `src/models/suggestion/` — types.rs, queries.rs
16. `src/models/proposal/` — types.rs, queries.rs
17. `src/models/protocol/` — types.rs, queries.rs
18. `src/models/document/` — types.rs, queries.rs
19. `src/models/presentation_template/` — types.rs, queries.rs
20. `src/models/data_manager/` — types.rs, import.rs, export.rs, jsonld.rs
21. `src/models/table_filter/` — builder.rs, columns.rs

**Key transformations to watch for:**
- `GROUP_CONCAT(DISTINCT col)` → `STRING_AGG(DISTINCT col, ',')`
- `INSERT OR IGNORE` → `INSERT ... ON CONFLICT DO NOTHING`
- `strftime('%Y-%m-%dT%H:%M:%S','now')` → `NOW()`
- `Box<dyn rusqlite::types::ToSql>` dynamic params → sqlx `QueryBuilder` for dynamic queries
- `LIKE ?1` → `LIKE $1` (parameter numbering)
- Boolean: `i64 != 0` → native `bool`

**Commit after each module group:** e.g. `refactor(models): migrate user module to sqlx/postgres`

---

### Task 6: Auth + Warnings Modules

**Prompt Contract:**
- **GOAL:** Migrate auth (session, password, CSRF, ABAC) and warnings (generators, scheduler, queries) to use PgPool.
- **CONSTRAINTS:** Session helpers that take `&Connection` must take `&PgPool`. Warning scheduler must work with async pool. ABAC queries use recursive patterns — verify they work in Postgres.
- **FORMAT:** All auth/ and warnings/ files compile with sqlx.
- **FAILURE CONDITIONS:** ABAC graph traversal returns different results. Warning scheduler panics.

**Files:**
- Modify: `src/auth/session.rs`, `src/auth/password.rs`, `src/auth/abac.rs`
- Modify: `src/warnings/` (all files)

**Steps:**

1. `auth/session.rs` — `get_user_id`, `get_permissions` etc. may use DB queries → make async
2. `auth/abac.rs` — `has_resource_capability`, `load_tor_capabilities` use complex JOIN chains → convert to Postgres, keep recursive CTE option
3. `warnings/` — generators and scheduler use `conn` → change to pool, make async
4. Commit: `refactor(auth,warnings): migrate to sqlx/postgres`

---

### Task 7: All Handlers

**Prompt Contract:**
- **GOAL:** Update every handler to use `web::Data<PgPool>` and `.await` model calls.
- **CONSTRAINTS:** Remove all `let conn = pool.get()?;` lines. Replace with direct pool reference to model functions. Add `.await` to every model call. Remove any `web::block()` wrappers. Keep handler business logic unchanged.
- **FORMAT:** Every handler compiles. No rusqlite imports remain.
- **FAILURE CONDITIONS:** A handler missing `.await` on a model call. A handler still using `pool.get()`.

**Files:** All 53 handler files listed in the inventory.

**Transformation pattern for every handler:**
```rust
// BEFORE
pub async fn handler(pool: web::Data<DbPool>, session: Session) -> Result<HttpResponse, AppError> {
    let conn = pool.get()?;
    let data = some_model::find(&conn, id)?;
    // ...
}

// AFTER
pub async fn handler(pool: web::Data<PgPool>, session: Session) -> Result<HttpResponse, AppError> {
    let data = some_model::find(&pool, id).await?;
    // ...
}
```

**Also update:**
- `src/templates_structs.rs` — if `PageContext::build` takes `&Connection`, change to `&PgPool` + async
- `src/main.rs` — async pool init, async seed calls, `DATABASE_URL` env var

**Commit after each handler module:** e.g. `refactor(handlers): migrate user_handlers to async sqlx`

---

### Task 8: Test Infrastructure + All Tests

**Prompt Contract:**
- **GOAL:** Migrate test infrastructure to use Postgres test database. Update all 21 test files.
- **CONSTRAINTS:** Use a dedicated `ahlt_test` database. Each test gets a clean schema via `sqlx::migrate!()`. Tests must remain parallelizable — use unique schema names or transaction rollbacks per test.
- **FORMAT:** `cargo test` passes with all tests using Postgres.
- **FAILURE CONDITIONS:** Any test fails that passed before. Tests interfere with each other. Test database not cleaned up.

**Files:**
- Rewrite: `tests/common/mod.rs`
- Modify: All 20 test files

**Steps:**

1. Rewrite `tests/common/mod.rs`:
   ```rust
   use sqlx::PgPool;

   pub async fn setup_test_db() -> PgPool {
       let url = std::env::var("TEST_DATABASE_URL")
           .unwrap_or("postgresql://ahlt:test@localhost/ahlt_test".into());
       let pool = PgPool::connect(&url).await.expect("...");
       sqlx::migrate!().run(&pool).await.expect("...");
       seed_base_entities(&pool).await.expect("...");
       pool
   }
   ```

2. Update each test file:
   - `#[actix_rt::test]` → `#[tokio::test]` (or keep actix_rt)
   - `let (_dir, conn) = setup_test_db()` → `let pool = setup_test_db().await`
   - All model calls: `model::func(&conn, ...)` → `model::func(&pool, ...).await`

3. Run: `TEST_DATABASE_URL=postgresql://ahlt:test@localhost/ahlt_test cargo test`

4. Commit: `refactor(tests): migrate all tests to sqlx/postgres`

---

### Task 9: Cleanup — Remove SQLite Dependencies

**Prompt Contract:**
- **GOAL:** Remove all rusqlite, r2d2, r2d2_sqlite dependencies. Delete src/schema.sql. Clean up any remaining SQLite references.
- **CONSTRAINTS:** `cargo check` must pass with zero warnings about unused imports. `cargo clippy` clean.
- **FORMAT:** No rusqlite/r2d2 in Cargo.toml or anywhere in src/.
- **FAILURE CONDITIONS:** Any file still imports rusqlite. Cargo.toml still lists SQLite deps.

**Files:**
- Modify: `Cargo.toml` (remove rusqlite, r2d2, r2d2_sqlite from both deps and dev-deps)
- Delete: `src/schema.sql` (replaced by `migrations/`)
- Modify: `src/db.rs` (remove `MIGRATIONS` const that included schema.sql)
- Modify: `.env.example` (add `DATABASE_URL`)

**Steps:**

1. Remove from Cargo.toml: `rusqlite`, `r2d2`, `r2d2_sqlite`
2. Delete `src/schema.sql`
3. Grep for any remaining `rusqlite` or `r2d2` imports: `grep -r "rusqlite\|r2d2" src/`
4. Update `.env.example` with `DATABASE_URL=postgresql://ahlt:secret@localhost/ahlt_dev`
5. Run: `cargo clippy -- -D warnings`
6. Run: `cargo test`
7. Commit: `refactor(cleanup): remove SQLite dependencies, migration complete`

---

### Task 10: Update Dockerfile for Postgres

**Prompt Contract:**
- **GOAL:** Update the Dockerfile to work with sqlx (no embedded SQLite, needs DATABASE_URL at compile time for sqlx macros, connects to external Postgres at runtime).
- **CONSTRAINTS:** Multi-stage build stays. No SQLite libraries needed. If using sqlx compile-time checking, need `DATABASE_URL` at build time or use `sqlx::query()` (runtime-checked) instead.
- **FORMAT:** `docker build` succeeds. Container starts and connects to external Postgres.
- **FAILURE CONDITIONS:** Build fails due to missing DATABASE_URL. Container can't reach Postgres.

**Files:**
- Modify: `Dockerfile`
- Create: `.sqlx/` directory with offline query data (if using compile-time checked queries)

**Steps:**

1. If using `sqlx::query!()` macros (compile-time checked):
   ```bash
   DATABASE_URL=postgresql://ahlt:secret@localhost/ahlt_dev cargo sqlx prepare
   ```
   This generates `.sqlx/` directory — commit it.

2. Update Dockerfile:
   - Remove `COPY src/schema.sql` line
   - Add `COPY .sqlx/ .sqlx/` for offline mode
   - Set `SQLX_OFFLINE=true` during build
   - Remove `mkdir -p data/dev data/staging` (no local DB dirs needed)
   - Add `DATABASE_URL` as required env var

3. Commit: `build(docker): update Dockerfile for PostgreSQL + sqlx`

---

## PHASE 1b: Neo4j Graph Projection

### Task 11: Neo4j Docker + Rust Client

**Prompt Contract:**
- **GOAL:** Set up Neo4j Community in Docker and create a Rust `graph_sync` module that can mirror EAV data to Neo4j.
- **CONSTRAINTS:** Neo4j is read-only projection. Postgres is master. Use `neo4rs` crate (already added in Task 1). Best-effort sync — if Neo4j is down, app continues working.
- **FORMAT:** `graph_sync` module with `sync_entity()`, `sync_relation()`, `delete_entity()`, `delete_relation()`, `full_resync()`.
- **FAILURE CONDITIONS:** Sync blocks the request thread. Neo4j being unavailable crashes the app.

**Files:**
- Create: `src/models/graph_sync/mod.rs`
- Create: `src/models/graph_sync/sync.rs`
- Create: `src/models/graph_sync/queries.rs` (Cypher query helpers)
- Modify: `src/models/mod.rs` (add `pub mod graph_sync`)
- Modify: `src/lib.rs` (if needed)
- Modify: `src/main.rs` (Neo4j connection init)

**Steps:**

1. Start Neo4j Community locally:
   ```bash
   docker run -d --name neo4j \
     -p 7474:7474 -p 7687:7687 \
     -e NEO4J_AUTH=neo4j/secret \
     neo4j:5-community
   ```

2. Create `graph_sync` module:
   - `init(uri: &str, user: &str, password: &str) -> Result<Graph, Error>`
   - `sync_entity(graph: &Graph, entity: &Entity, properties: &HashMap<String, String>)` — MERGE node by id
   - `sync_relation(graph: &Graph, relation_type: &str, source_id: i64, target_id: i64)` — MERGE relationship
   - `delete_entity(graph: &Graph, entity_id: i64)` — DETACH DELETE
   - `full_resync(graph: &Graph, pool: &PgPool)` — read all from Postgres, write all to Neo4j

3. Add sync calls to entity.rs and relation.rs create/update/delete functions (fire-and-forget via `tokio::spawn`)

4. Add `NEO4J_URI`, `NEO4J_USER`, `NEO4J_PASSWORD` to `.env.example`

5. Write tests for sync module

6. Commit: `feat(neo4j): add graph_sync module for EAV projection to Neo4j`

---

### Task 12: Migrate Graph Queries to Cypher

**Prompt Contract:**
- **GOAL:** Move ABAC capability lookup and governance map queries from Postgres recursive CTEs to Neo4j Cypher.
- **CONSTRAINTS:** Fall back to Postgres if Neo4j is unavailable. Keep existing Postgres queries as backup.
- **FORMAT:** ABAC check and governance map API use Neo4j when available.
- **FAILURE CONDITIONS:** ABAC returns different results from Neo4j vs Postgres. Governance map renders differently.

**Files:**
- Modify: `src/auth/abac.rs` (add Neo4j path with Postgres fallback)
- Modify: `src/handlers/governance_handlers/map.rs` (Neo4j graph query)
- Create: `src/models/graph_sync/queries.rs` (Cypher query functions)

**Steps:**

1. ABAC Cypher:
   ```cypher
   MATCH (u:user {id: $uid})-[:fills_position]->(f:tor_function)-[:belongs_to_tor]->(t:tor {id: $tid})
   WHERE f.can_edit = 'true'
   RETURN COUNT(f) > 0 AS has_capability
   ```

2. Governance map Cypher:
   ```cypher
   MATCH path = (e)-[r]->(t)
   WHERE e.entity_type IN ['tor', 'tor_function', 'user']
   RETURN e, r, t
   ```

3. Add integration tests comparing Neo4j results to Postgres results

4. Commit: `feat(neo4j): migrate ABAC and governance map to Cypher queries`

---

## PHASE 2: Docker Compose Multi-Environment

### Task 13: Base Docker Compose + Environment Files

**Prompt Contract:**
- **GOAL:** Create base docker-compose.yml with Postgres 17 + Neo4j, plus per-environment override files and a Makefile.
- **CONSTRAINTS:** Single Postgres container with 3 databases. Ports: dev=8080, staging=8081, prod=8082. All secrets in `.env.*` files (gitignored).
- **FORMAT:** `make dev` starts dev environment. `make all` starts all three.
- **FAILURE CONDITIONS:** Port conflicts. Database init scripts fail. App can't reach Postgres from inside container.

**Files:**
- Create: `docker-compose.yml`
- Create: `docker-compose.dev.yml`
- Create: `docker-compose.staging.yml`
- Create: `docker-compose.prod.yml`
- Create: `docker/postgres/init-databases.sh` (creates 3 DBs on first run)
- Create: `.env.dev`, `.env.staging`, `.env.prod` (from .env.example, gitignored)
- Create: `Makefile`
- Modify: `.gitignore` (add `.env.dev`, `.env.staging`, `.env.prod`)

**Steps:**

1. Write `docker-compose.yml` (base services: postgres, neo4j, shared network)
2. Write `docker/postgres/init-databases.sh`:
   ```bash
   #!/bin/bash
   set -e
   psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" <<-EOSQL
       CREATE DATABASE ahlt_dev;
       CREATE DATABASE ahlt_staging;
       CREATE DATABASE ahlt_prod;
   EOSQL
   ```
3. Write per-env compose overrides
4. Write Makefile with `dev`, `staging`, `prod`, `all`, `down`, `logs-dev` targets
5. Test: `make dev` → verify app starts + connects to Postgres
6. Commit: `feat(docker): add multi-environment Docker Compose setup`

---

## PHASE 3: Self-hosted GitLab + CI Pipeline

### Task 14: GitLab CE on Mac M2

**Prompt Contract:**
- **GOAL:** Run GitLab CE locally in Docker with container registry enabled.
- **CONSTRAINTS:** Ports: 8929 (web), 2222 (SSH), 5050 (registry). Data persisted to ~/gitlab-data/. ARM64 image.
- **FORMAT:** GitLab accessible at http://gitlab.local:8929. Registry functional at registry.gitlab.local:5050.
- **FAILURE CONDITIONS:** GitLab doesn't start (RAM issue). Registry pushes fail.

**Files:**
- Create: `infra/gitlab/docker-compose.gitlab.yml`
- Create: `infra/gitlab/gitlab.rb` (config overrides)
- Document: `docs/plans/2026-02-21-enterprise-infrastructure-plan.md` (this file, steps section)

**Steps:**

1. Add `/etc/hosts` entries: `127.0.0.1 gitlab.local registry.gitlab.local`
2. Write `infra/gitlab/docker-compose.gitlab.yml`
3. Start: `docker compose -f infra/gitlab/docker-compose.gitlab.yml up -d`
4. Wait ~3 min for GitLab to initialize
5. Set root password via `docker exec -it gitlab gitlab-rails console` or web UI
6. Enable container registry in `gitlab.rb`
7. Test registry: `docker login registry.gitlab.local:5050`
8. Commit: `feat(infra): add GitLab CE Docker Compose configuration`

---

### Task 15: GitLab Runner + CI Pipeline

**Prompt Contract:**
- **GOAL:** Register a GitLab Runner and create `.gitlab-ci.yml` with test/lint/build/deploy stages.
- **CONSTRAINTS:** Runner uses Docker executor with host socket. Pipeline builds app image and pushes to local registry. Deploy stages use Helm (placeholder until Phase 4).
- **FORMAT:** Push to main triggers full pipeline. Tags trigger prod deploy with manual gate.
- **FAILURE CONDITIONS:** Runner can't build Docker images. Registry push fails from CI.

**Files:**
- Create: `infra/gitlab/docker-compose.runner.yml`
- Create: `.gitlab-ci.yml`

**Steps:**

1. Register runner with GitLab instance
2. Write `.gitlab-ci.yml` with 5 stages: test, lint, build, deploy-staging, deploy-prod
3. Push a test commit to verify pipeline runs
4. Commit: `feat(ci): add GitLab CI pipeline with test/lint/build/deploy stages`

---

### Task 16: Repository Migration

**Prompt Contract:**
- **GOAL:** Import the GitHub repository into local GitLab and switch the local remote.
- **CONSTRAINTS:** Keep GitHub as optional mirror. Don't lose any history.
- **FORMAT:** `git remote -v` shows GitLab as origin. All history intact.
- **FAILURE CONDITIONS:** Commits lost. Branch structure different.

**Steps:**

1. Import from GitHub via GitLab UI
2. `git remote set-url origin http://gitlab.local:8929/group/im-ctrl.git`
3. `git push -u origin main`
4. Verify: `git log --oneline -5` matches on both remotes

---

## PHASE 4: Kubernetes with Rancher Desktop

### Task 17: Rancher Desktop + Shared Infrastructure

**Prompt Contract:**
- **GOAL:** Install Rancher Desktop, create namespaces, deploy Postgres 17 + Neo4j to `shared-infra` namespace.
- **CONSTRAINTS:** Use Helm for Postgres (Bitnami chart) and Neo4j. PVCs for data persistence. k3s must trust local GitLab registry.
- **FORMAT:** `kubectl get pods -n shared-infra` shows healthy Postgres + Neo4j.
- **FAILURE CONDITIONS:** PVCs not bound. Postgres not accepting connections from other namespaces.

**Files:**
- Create: `helm/infra/postgres-values.yaml`
- Create: `helm/infra/neo4j-values.yaml`
- Create: `infra/k3s/registries.yaml`

**Steps:**

1. Install Rancher Desktop: `brew install --cask rancher`
2. Configure k3s registry trust for `registry.gitlab.local:5050`
3. Create namespaces: `ahlt-dev`, `ahlt-staging`, `ahlt-prod`, `shared-infra`
4. Deploy Postgres: `helm install postgres bitnami/postgresql -n shared-infra -f helm/infra/postgres-values.yaml`
5. Deploy Neo4j: `helm install neo4j neo4j/neo4j -n shared-infra -f helm/infra/neo4j-values.yaml`
6. Verify: `kubectl exec -n shared-infra deploy/postgres -- psql -U ahlt -c "\l"`
7. Commit: `feat(k8s): add shared infrastructure Helm configs`

---

### Task 18: Application Helm Chart

**Prompt Contract:**
- **GOAL:** Create the Helm chart for the ahlt application with per-environment values files.
- **CONSTRAINTS:** Single chart, three values files. Image from GitLab registry. ConfigMap for non-sensitive vars, Secret for DATABASE_URL/SESSION_KEY/NEO4J_PASSWORD.
- **FORMAT:** `helm install ahlt-dev ./helm/ahlt -n ahlt-dev -f helm/ahlt/values-dev.yaml` succeeds.
- **FAILURE CONDITIONS:** Pod crashes due to missing env vars. Can't pull image from registry.

**Files:**
- Create: `helm/ahlt/Chart.yaml`
- Create: `helm/ahlt/templates/deployment.yaml`
- Create: `helm/ahlt/templates/service.yaml`
- Create: `helm/ahlt/templates/configmap.yaml`
- Create: `helm/ahlt/templates/secret.yaml`
- Create: `helm/ahlt/templates/ingress.yaml` (placeholder)
- Create: `helm/ahlt/values.yaml`
- Create: `helm/ahlt/values-dev.yaml`
- Create: `helm/ahlt/values-staging.yaml`
- Create: `helm/ahlt/values-prod.yaml`

**Steps:**

1. Write chart templates
2. Write values files for each environment
3. Deploy dev: `helm install ahlt-dev ./helm/ahlt -n ahlt-dev -f helm/ahlt/values-dev.yaml`
4. Verify: `kubectl logs -n ahlt-dev deploy/ahlt-dev`
5. Deploy staging and prod
6. Commit: `feat(k8s): add application Helm chart with per-environment values`

---

### Task 19: Update CI Pipeline for K8s Deploys

**Prompt Contract:**
- **GOAL:** Update `.gitlab-ci.yml` deploy stages to use `helm upgrade` instead of Docker Compose.
- **CONSTRAINTS:** Runner needs KUBECONFIG access. Deploy-staging is automatic on main. Deploy-prod is manual on tags.
- **FORMAT:** Push to main → staging pod updates automatically.
- **FAILURE CONDITIONS:** Runner can't reach k3s API. Helm upgrade fails.

**Files:**
- Modify: `.gitlab-ci.yml`

**Steps:**

1. Add KUBECONFIG as GitLab CI variable
2. Update deploy-staging and deploy-prod stages
3. Test with a push to main
4. Commit: `feat(ci): update pipeline to deploy via Helm to k3s`

---

## PHASE 5: nginx Ingress + Subdomain Routing

### Task 20: nginx Ingress Controller

**Prompt Contract:**
- **GOAL:** Install nginx Ingress Controller in k3s, replacing k3s default Traefik.
- **CONSTRAINTS:** Disable Traefik first. nginx binds ports 80 and 443 on the host.
- **FORMAT:** `kubectl get pods -n ingress-nginx` shows running controller.
- **FAILURE CONDITIONS:** Port 80/443 already in use. Traefik still running.

**Steps:**

1. Disable Traefik in Rancher Desktop settings
2. Install nginx Ingress:
   ```bash
   helm install nginx-ingress ingress-nginx/ingress-nginx -n ingress-nginx --create-namespace
   ```
3. Verify: `curl -H "Host: test.local" http://localhost` returns 404 (expected — no ingress rules yet)
4. Commit: `feat(k8s): install nginx Ingress Controller`

---

### Task 21: TLS Certificates + Subdomain Routing

**Prompt Contract:**
- **GOAL:** Generate locally-trusted TLS certs with mkcert, create Ingress resources for all environments, update /etc/hosts.
- **CONSTRAINTS:** Subdomains: dev.local, staging.local, app.local, gitlab.local. mkcert for local trust. Helm ingress template activated via values files.
- **FORMAT:** `curl https://dev.local` returns the app. Browser shows trusted HTTPS.
- **FAILURE CONDITIONS:** Certificate warnings in browser. Wrong environment served for a subdomain.

**Files:**
- Modify: `helm/ahlt/values-dev.yaml` (enable ingress, set host)
- Modify: `helm/ahlt/values-staging.yaml`
- Modify: `helm/ahlt/values-prod.yaml`
- Modify: `helm/ahlt/templates/ingress.yaml` (activate)

**Steps:**

1. Install mkcert: `brew install mkcert && mkcert -install`
2. Generate certs: `mkcert dev.local staging.local app.local gitlab.local`
3. Create TLS secrets in each namespace
4. Add `/etc/hosts` entries
5. Update values files to enable ingress with correct hosts
6. Helm upgrade all three environments
7. Test: `curl https://dev.local`, `curl https://staging.local`, `curl https://app.local`
8. Commit: `feat(k8s): enable subdomain routing with TLS via nginx Ingress`

---

## Post-Migration Verification Checklist

After all 21 tasks are complete:

- [ ] `cargo test` passes (all ~169 tests)
- [ ] `cargo clippy -- -D warnings` clean
- [ ] `docker build .` succeeds
- [ ] `make dev` starts dev environment via Compose
- [ ] GitLab pipeline runs test/lint/build on push
- [ ] `helm install` deploys to all three k3s namespaces
- [ ] `https://dev.local` serves the app
- [ ] `https://staging.local` serves staging with seed data
- [ ] `https://app.local` serves production
- [ ] Neo4j reflects entities from Postgres
- [ ] ABAC checks work via Neo4j with Postgres fallback
- [ ] No rusqlite/r2d2/SQLite references remain in codebase
