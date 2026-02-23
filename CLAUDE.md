# im-ctrl — Rust Web Application

Ontology-based entity management system built with Actix-web, Askama templates, and PostgreSQL.

**Crate name**: `ahlt` — used in `use ahlt::models::user` imports and test output
**Documentation**: All project documentation must be stored in the `docs/` folder.

## Quick Start

```bash
# Start infrastructure (PostgreSQL + Neo4j)
make infra                 # or: docker compose up -d postgres neo4j

# Run the application
DATABASE_URL=postgresql://ahlt@localhost/ahlt_dev cargo run
APP_ENV=staging DATABASE_URL=postgresql://ahlt@localhost/ahlt_staging cargo run
cargo clippy               # Linter
```

**Access**: http://localhost:8080
**Default login**: admin / (password set during initial setup)

### Docker Compose (full stack)

```bash
make dev                   # App + Postgres + Neo4j on port 8080
make staging               # Staging environment on port 8081
make prod                  # Production environment on port 8082
make down                  # Stop all environments
```

## Architecture

### Stack
- **Web framework**: Actix-web 4
- **Templates**: Askama 0.14
- **Database**: PostgreSQL 17 (sqlx 0.8, async)
- **Graph DB**: Neo4j 5 Community (neo4rs 0.8, optional)
- **Auth**: argon2 0.5, actix-session 0.10
- **Serialization**: serde + serde_json

### Directory Structure

```
src/
├── main.rs              # App config, routing, middleware
├── lib.rs               # Library crate root (pub mod declarations)
├── db.rs                # Database pool initialization + seed data
├── errors.rs            # AppError enum, render() helper
├── templates_structs.rs # Template context types
├── auth/                # Authentication (login, session helpers, CSRF)
├── audit/               # Audit logging subsystem
├── warnings/            # Warning system (generators, scheduler, queries)
├── models/              # Database models & queries
│   ├── entity.rs        # Core EAV entity CRUD
│   ├── relation.rs      # Core EAV relation CRUD
│   ├── nav_item.rs      # Navigation menu building
│   ├── user/            # User types + queries
│   ├── role/            # Role types + queries
│   ├── ontology/        # EAV ontology (schema, instance, entities)
│   ├── workflow/        # Workflow engine (types, queries)
│   ├── suggestion/      # Suggestion pipeline
│   ├── proposal/        # Proposal pipeline
│   ├── tor/             # Terms of Reference
│   ├── agenda_point/    # Meeting agenda points
│   ├── minutes/         # Meeting minutes
│   ├── document/        # Document entity type
│   ├── graph_sync/      # Neo4j graph projection (optional)
│   └── data_manager/    # JSON import/export
└── handlers/            # HTTP request handlers
    ├── mod.rs           # Handler module declarations
    ├── user_handlers/   # User CRUD (list, create, update, delete, crud)
    ├── role_handlers/   # Role CRUD (helpers, list, crud, builder)
    ├── tor_handlers/, governance_handlers/
    ├── meeting_handlers/, minutes_handlers/
    ├── proposal_handlers/ # Proposal CRUD (crud.rs) + workflow (workflow.rs)
    ├── document_handlers/ # Document CRUD (crud.rs, list.rs)
    ├── api_v1/          # REST API (entities, users, tors, proposals, warnings)
    ├── workflow_handlers.rs, suggestion_handlers.rs, ...
    └── ...              # auth, account, settings, audit, dashboard, etc.

migrations/              # PostgreSQL schema migrations (sqlx)
templates/               # Askama HTML templates
static/                  # CSS (modular, BEM naming), fonts, client-side JS
static/css/              # PostCSS modular build: index.css → base/, components/, layout/, pages/, utilities/
data/seed/               # JSON seed fixtures (ontology.json, staging.json)
docs/plans/              # Design & implementation documentation
docker-compose.yml       # Base services (Postgres + Neo4j)
docker-compose.{dev,staging,prod}.yml  # Per-environment overrides
helm/                    # Kubernetes Helm charts
infra/                   # GitLab CE + Runner configs
```

### Key Patterns

**AppError Pattern** — All handlers return `Result<HttpResponse, AppError>`:

```rust
pub async fn handler(
    pool: web::Data<PgPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "permission.code")?;
    let ctx = PageContext::build(&session, &pool, "/path").await?;
    // ... business logic using &pool with .await ...
    render(tmpl)
}
```

**AppError variants**: `Db`, `Template`, `Hash`, `NotFound`, `PermissionDenied`, `Session`, `Csrf`

**Session Helpers** (`src/auth/session.rs`):
`require_permission()`, `get_user_id()`, `get_username()`, `get_permissions()`

**Template Rendering**: Use `render(tmpl)` helper — converts `askama::Error` to `AppError` automatically.

**EAV Ontology Pattern** — Everything is an entity with properties and relations:

```sql
entities (id, entity_type, name, label, created_at, updated_at)
entity_properties (entity_id, key, value)  -- Flexible schema
relations (id, relation_type_id, source_id, target_id, created_at)
```

### Database

**PostgreSQL** via `DATABASE_URL` env var (e.g., `postgresql://ahlt@localhost/ahlt_dev`).

**Migrations**: `migrations/` directory, run automatically by sqlx on startup.

**Multi-environment databases**: `ahlt_dev`, `ahlt_staging`, `ahlt_prod`, `ahlt_test` — created by `docker/postgres/init-databases.sh`.

**Constraints**: Foreign keys CASCADE on entity delete. UNIQUE on `(entity_type, name)`. BIGINT GENERATED ALWAYS AS IDENTITY for IDs.

**SQL dialect notes** (vs SQLite):
- Parameters: `$1`, `$2` (not `?1`, `?2`)
- Aggregation: `STRING_AGG(col, ',')` (not `GROUP_CONCAT`)
- Timestamps: `NOW()` (not `strftime`), cast with `::TEXT` when selecting into String fields
- Upsert: `ON CONFLICT(cols) DO UPDATE SET ...` (not `INSERT OR REPLACE`)
- GROUP BY strictness: All non-aggregated columns must be in GROUP BY or wrapped in `MAX()`/aggregate

### Neo4j (optional)

Read-only graph projection of EAV data. Set `NEO4J_URI`, `NEO4J_USER`, `NEO4J_PASSWORD` env vars to enable.

- `graph_sync` module: fire-and-forget sync via `tokio::spawn`, ABAC Cypher queries, governance map visualization
- Falls back to PostgreSQL queries when Neo4j is unavailable
- `GraphPool = Option<Arc<Graph>>` — `None` means disabled

**Graph Panel CSS** — Wrap in `.graph-panel` > `.graph-panel-header` + `.graph-container`. Lives in `style.css` — do not re-define in template `<style>` blocks.

## Testing

Tests use PostgreSQL schema isolation (unique schema per test via `search_path`) — safe to run in parallel. Requires `ahlt_test` database. Crate name in test imports: `ahlt`.

```bash
cargo test                          # All tests (~221 across 26 files, 8 ignored)
cargo test user_test                # Single test file
cargo test -- --nocapture           # With stdout
cargo test --test meeting_test      # Integration test by file
cargo test --test graph_sync_test -- --ignored --test-threads=1  # Neo4j tests (requires running Neo4j)
```

**Test database**: `TEST_DATABASE_URL` env var (default: `postgresql://ahlt@localhost/ahlt_test`). Each test creates a unique schema, runs migrations, seeds base entities, and drops the schema on cleanup.

### Playwright E2E Tests

Browser-based integration tests live in `scripts/`. They require a running server with staging seed data.

**One-time setup** (Node.js required):

```bash
mkdir -p /tmp/pw-test
cd /tmp/pw-test
npm init -y
npm install @playwright/test
npx playwright install chromium
```

**Running tests** (server must be running first):

```bash
# Terminal 1 — start server with staging data
APP_ENV=staging cargo run

# Terminal 2 — run tests
cd /tmp/pw-test
node /Users/vidarbrevik/projects/im-ctrl/scripts/users-table.test.mjs
```

**Credentials**: `admin` / `admin123` · **Base URL**: `http://localhost:8080`

**Test files**:

| File | Coverage |
|------|----------|
| `scripts/users-table.test.mjs` | Users table: filter builder, sorting, column picker, per-page, CSV export, URL state (46 tests) |

**Key gotcha**: Askama's `{{ variable }}` auto-HTML-escapes content. JSON embedded in `<script type="application/json">` blocks **must** use `{{ variable|safe }}` or `JSON.parse()` will fail on `&#34;` instead of `"`.

## Critical Rules

- **No `&&` in Askama**: `{% if a %}{% if b %}...{% endif %}{% endif %}` — nested, not `{% if a && b %}`
- **Route order**: `/users/new` BEFORE `/users/{id}` or path param swallows "new"
- **`relation::create()` takes name, not ID**: `relation::create(&pool, "relation_name", src, dst).await`
- **No `innerHTML`**: Security hook rejects it — use `createElement`/`textContent`/`appendChild`
- **All model calls are async**: Every `model::function(&pool, ...)` must have `.await`
- **Cast timestamps in SELECT**: Use `created_at::TEXT` when selecting into `String` fields
- **Template partials**: Large templates are split into `{page}/partials/*.html` — edit partials, not the parent template
- **Seed changes need DB drop**: Seed skips non-empty DB — drop and recreate the database to pick up fixture changes
- **Full gotchas**: `.claude/rules/gotchas.md`

## Troubleshooting

- **Build errors after git pull**: `cargo clean && cargo build`
- **Connection refused**: Ensure PostgreSQL is running (`make infra` or `docker compose up -d postgres`)
- **Migration errors**: Check `migrations/` directory. sqlx runs migrations automatically on startup.
- **Template not found**: Askama compiles templates. Run `cargo clean` after adding new templates.
- **Session cookie issues**: Clear cookies in browser dev tools → Application → Cookies.
- **Neo4j optional**: App runs fine without Neo4j. Graph features fall back to PostgreSQL queries.

## Verification Commands

```bash
cargo check 2>&1 | tail -10          # Quick build check
cargo build 2>&1 | tail -1           # Verify "Finished"
git log --oneline -20                # Recent commits
```
