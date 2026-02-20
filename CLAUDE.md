# im-ctrl — Rust Web Application

Ontology-based entity management system built with Actix-web, Askama templates, and SQLite.

**Crate name**: `ahlt` — used in `use ahlt::models::user` imports and test output
**Documentation**: All project documentation must be stored in the `docs/` folder.

## Quick Start

```bash
cargo run                  # Build and run
cargo watch -x run         # Dev with auto-reload
APP_ENV=staging cargo run  # Run with staging data (ToR, governance, meetings)
cargo clippy               # Linter
```

**Access**: http://localhost:8080
**Default login**: admin / (password set during initial setup)

## Architecture

### Stack
- **Web framework**: Actix-web 4
- **Templates**: Askama 0.14
- **Database**: SQLite (rusqlite 0.32 + r2d2_sqlite 0.25)
- **Auth**: argon2 0.5, actix-session 0.10
- **Serialization**: serde + serde_json

### Directory Structure

```
src/
├── main.rs              # App config, routing, middleware
├── lib.rs               # Library crate root (pub mod declarations)
├── db.rs                # Database pool initialization + seed data
├── errors.rs            # AppError enum, render() helper
├── schema.sql           # Embedded SQLite schema
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
│   └── data_manager/    # JSON import/export
└── handlers/            # HTTP request handlers
    ├── mod.rs           # Handler module declarations
    ├── user_handlers/   # User CRUD (list, crud)
    ├── role_handlers/   # Role CRUD (helpers, list, crud)
    ├── tor_handlers/, governance_handlers/
    ├── workflow_handlers.rs, suggestion_handlers.rs, proposal_handlers.rs, ...
    └── ...              # auth, account, settings, audit, dashboard, etc.

templates/               # Askama HTML templates
static/                  # CSS (BEM naming), fonts, client-side JS
data/                    # SQLite databases (per APP_ENV)
data/seed/               # JSON seed fixtures (ontology.json, staging.json)
docs/plans/              # Design & implementation documentation
```

### Key Patterns

**AppError Pattern** — All handlers return `Result<HttpResponse, AppError>`:

```rust
pub async fn handler(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "permission.code")?;
    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/path")?;
    // ... business logic ...
    render(tmpl)
}
```

**AppError variants**: `Db`, `Pool`, `Template`, `Hash`, `NotFound`, `PermissionDenied`, `Session`, `Csrf`

**Session Helpers** (`src/auth/session.rs`):
`require_permission()`, `get_user_id()`, `get_username()`, `get_permissions()`

**Template Rendering**: Use `render(tmpl)` helper — converts `askama::Error` to `AppError` automatically.

**EAV Ontology Pattern** — Everything is an entity with properties and relations:

```sql
entities (id, entity_type, name, created_at)
entity_properties (entity_id, key, value)  -- Flexible schema
relations (id, relation_type_id, from_entity_id, to_entity_id)
```

### Database

**Location**: `data/{APP_ENV}/app.db` (default: `data/dev/app.db`, staging: `data/staging/app.db`)

**Pragmas** (set per-connection via r2d2 init):
```sql
PRAGMA foreign_keys = ON;  -- Required for CASCADE deletes
PRAGMA journal_mode = WAL; -- Write-Ahead Logging for concurrency
```

**Constraints**: Foreign keys CASCADE on entity delete. UNIQUE on usernames, role names. Autoincrement IDs shared across all entity types.

**Graph Panel CSS** — Wrap in `.graph-panel` > `.graph-panel-header` + `.graph-container`. Lives in `style.css` — do not re-define in template `<style>` blocks.

## Testing

Tests use TempDir isolation — safe to run in parallel. Crate name in test imports: `ahlt`.

```bash
cargo test                          # All tests (~141 across 18 files)
cargo test user_test                # Single test file
cargo test -- --nocapture           # With stdout
cargo test --test meeting_test      # Integration test by file
```

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
- **`relation::create()` takes name, not ID**: `relation::create(&conn, "relation_name", src, dst)`
- **No `innerHTML`**: Security hook rejects it — use `createElement`/`textContent`/`appendChild`
- **Seed changes need DB delete**: Seed skips non-empty DB — delete `data/{env}/app.db` to pick up fixture changes
- **Full gotchas**: `.claude/rules/gotchas.md`

## Troubleshooting

- **Build errors after git pull**: `cargo clean && cargo build`
- **Database locked**: Check for zombie connections. WAL mode helps but doesn't eliminate all locking.
- **Template not found**: Askama compiles templates. Run `cargo clean` after adding new templates.
- **Session cookie issues**: Clear cookies in browser dev tools → Application → Cookies.

## Verification Commands

```bash
cargo check 2>&1 | tail -10          # Quick build check
cargo build 2>&1 | tail -1           # Verify "Finished"
git log --oneline -20                # Recent commits
```
