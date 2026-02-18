# Data Manager — Implementation Plan

**Date**: 2026-02-18
**Design**: [2026-02-18-data-manager-design.md](2026-02-18-data-manager-design.md)
**Tasks**: 8 prompt contracts, ordered by dependency

---

## Task 1: Module Scaffolding & Types

GOAL: Create the `data_manager` module with all shared types for import/export payloads.
After this task, `cargo check` passes and all types used by later tasks exist.
Success = types compile, module is declared in `src/models/mod.rs` and `src/lib.rs`.

CONSTRAINTS:
- Follow existing module pattern (see `src/models/tor/` for reference)
- Use `serde::{Serialize, Deserialize}` on all types
- No business logic in this task — types and module wiring only
- `conflict_mode` is an enum: `Skip`, `Upsert`, `Fail` with serde rename to lowercase

FORMAT:
1. `src/models/data_manager/mod.rs` — re-exports submodules
2. `src/models/data_manager/types.rs` — all shared types:
   - `ImportPayload { conflict_mode, entities, relations }`
   - `EntityImport { entity_type, name, label, sort_order, properties: HashMap<String,String> }`
   - `RelationImport { relation_type, source: String ("type:name"), target: String }`
   - `ConflictMode` enum
   - `ImportResult { created, updated, skipped, errors: Vec<ImportError> }`
   - `ImportError { item: serde_json::Value, reason: String }`
   - `ExportPayload { entities: Vec<EntityExport>, relations: Vec<RelationExport> }`
   - `EntityExport` and `RelationExport` (mirror of import types with `id` included)
3. `src/models/mod.rs` — add `pub mod data_manager`

FAILURE CONDITIONS:
- Any type missing `Serialize` or `Deserialize`
- `conflict_mode` serializes as "Skip" instead of "skip"
- Module not reachable as `ahlt::models::data_manager::types::*`
- `cargo check` fails

---

## Task 2: Export Logic (JSON + SQL)

GOAL: Implement export functions that query the full entity graph and serialize to native JSON and SQL formats.
Success = calling `export_json(&conn, None)` returns a valid `ExportPayload` with all entities, properties, and relations. `export_sql(&conn, None)` returns a String of valid INSERT statements.

CONSTRAINTS:
- Query entities, entity_properties, and relations tables directly
- Optional `types` filter (Vec<String>) limits to specific entity_types
- SQL output uses `INSERT OR IGNORE` for safe re-execution
- SQL output escapes single quotes in values
- Relations resolve `relation_type_id`, `source_id`, `target_id` to `type:name` strings via entity lookups
- No new dependencies

FORMAT:
1. `src/models/data_manager/export.rs`:
   - `pub fn export_entities(conn: &Connection, types: Option<&[String]>) -> Result<ExportPayload, rusqlite::Error>`
   - `pub fn export_sql(conn: &Connection, types: Option<&[String]>) -> Result<String, rusqlite::Error>`
2. Update `src/models/data_manager/mod.rs` with `pub mod export`

FAILURE CONDITIONS:
- Properties not grouped by entity (N+1 query pattern)
- Relations reference numeric IDs instead of `type:name` strings
- SQL output would fail on values containing single quotes
- Missing entity_type filter support
- `cargo check` fails

---

## Task 3: JSON-LD Conversion

GOAL: Bidirectional conversion between EAV data and JSON-LD `@graph` format.
Success = round-trip: `export_jsonld(conn)` produces valid JSON-LD, and `parse_jsonld(jsonld_value)` converts it back to an `ImportPayload` that, if imported, would recreate the same data.

CONSTRAINTS:
- IRI scheme: `ahlt:{entity_type}/{name}` — e.g. `ahlt:tor/budget_committee`
- `@context` built dynamically from property keys and relation types in the DB
- `@type` maps to `ahlt:{EntityType}` (PascalCase of entity_type, e.g. `tor` -> `ahlt:Tor`)
- Entity properties become literal-valued predicates
- Relations become IRI-valued predicates linking two `@id` nodes
- Use `serde_json::Value` for JSON-LD manipulation — no external JSON-LD crate
- `ahlt:conflict_mode` in JSON-LD root maps to `ConflictMode`

FORMAT:
1. `src/models/data_manager/jsonld.rs`:
   - `pub fn export_jsonld(conn: &Connection, types: Option<&[String]>) -> Result<serde_json::Value, rusqlite::Error>`
   - `pub fn build_context(conn: &Connection) -> Result<serde_json::Value, rusqlite::Error>`
   - `pub fn parse_jsonld(value: &serde_json::Value) -> Result<ImportPayload, String>`
   - `fn entity_type_to_class(entity_type: &str) -> String` (e.g. "tor" -> "Tor")
   - `fn iri_to_type_name(iri: &str) -> Result<(String, String), String>` (e.g. "ahlt:tor/x" -> ("tor","x"))
2. Update `src/models/data_manager/mod.rs` with `pub mod jsonld`

FAILURE CONDITIONS:
- IRI scheme inconsistent between export and parse (round-trip breaks)
- `@context` hardcoded instead of built from DB
- Relation predicates indistinguishable from property predicates in the output
- PascalCase conversion wrong for multi-word types like `tor_function` (should be `TorFunction`)
- `cargo check` fails

---

## Task 4: Import Logic

GOAL: Implement import function that takes an `ImportPayload`, applies conflict resolution, and inserts/updates entities and relations in the database.
Success = importing a payload with `conflict_mode: skip` into a populated DB creates only new items and returns accurate counts. `upsert` updates existing properties. `fail` returns errors for duplicates without modifying anything.

CONSTRAINTS:
- Wrap entire import in a single SQL transaction (rollback on `fail` mode errors)
- Process entities first, then relations (relations reference entities by `type:name`)
- For `upsert`: update `label`, `sort_order`, and all properties; delete properties not in import payload
- For `skip`: check existence by `entity_type + name`, skip if exists
- For `fail`: check existence, return error for each duplicate, rollback entire transaction
- Relations resolved by looking up `type:name` -> entity ID after entity phase
- Return `ImportResult` with accurate counts and error details (including original item JSON)

FORMAT:
1. `src/models/data_manager/import.rs`:
   - `pub fn import_data(conn: &Connection, payload: &ImportPayload) -> Result<ImportResult, AppError>`
   - `fn resolve_entity_ref(conn: &Connection, ref_str: &str) -> Result<i64, String>` (parses "type:name", looks up ID)
   - `fn upsert_entity(conn: &Connection, entity: &EntityImport) -> Result<UpsertOutcome, String>`
   - `fn insert_or_skip_entity(conn: &Connection, entity: &EntityImport) -> Result<SkipOutcome, String>`
2. Update `src/models/data_manager/mod.rs` with `pub mod import`

FAILURE CONDITIONS:
- No transaction wrapping (partial imports on failure)
- `upsert` leaves stale properties that were removed from the import payload
- `fail` mode modifies the database before returning errors
- Relations inserted before all entities are resolved (ordering bug)
- Numeric IDs used in relation source/target instead of `type:name` resolution
- `cargo check` fails

---

## Task 5: HTTP Handlers

GOAL: Wire up three Actix-web handler functions for import, export, and schema endpoints.
Success = `curl -X POST /api/data/import -d @seed.json` returns an ImportResult JSON. `curl /api/data/export?format=jsonld` returns valid JSON-LD. `curl /api/data/schema` returns the @context.

CONSTRAINTS:
- All endpoints require `settings.manage` permission via `require_permission()`
- POST import requires CSRF validation via `csrf::validate_csrf()`
- Import auto-detects JSON-LD vs native by checking for `@context` key in the body
- Export `format` query param: `json` (default), `jsonld`, `sql`
- Export `types` query param: optional comma-separated entity types
- Return `Result<HttpResponse, AppError>` on all handlers
- Content-Type: `application/json` for JSON/JSON-LD, `text/plain` for SQL

FORMAT:
1. `src/handlers/data_handlers.rs`:
   - `pub async fn import_data(pool, session, body: web::Json<serde_json::Value>) -> Result<HttpResponse, AppError>`
   - `pub async fn export_data(pool, session, query: web::Query<ExportQuery>) -> Result<HttpResponse, AppError>`
   - `pub async fn schema(pool, session) -> Result<HttpResponse, AppError>`
   - `ExportQuery { format: Option<String>, types: Option<String> }`
2. `src/handlers/mod.rs` — add `pub mod data_handlers`
3. `src/main.rs` — register routes:
   - `POST /api/data/import`
   - `GET /api/data/export`
   - `GET /api/data/schema`
   - `GET /data-manager` (page handler, separate from API)

FAILURE CONDITIONS:
- Missing permission check on any endpoint
- Missing CSRF on POST
- Import silently accepts invalid JSON (no error response)
- SQL export returns `application/json` content type
- Route registration order conflicts with existing routes
- `cargo check` fails

---

## Task 6: Admin UI Template

GOAL: Build the Data Manager admin page with import and export panels.
Success = navigating to `/data-manager` shows a page with file upload, conflict mode selector, entity type filter, format selector, and export/import buttons. The page uses the existing design system.

CONSTRAINTS:
- Template extends the existing base layout (same pattern as other admin pages)
- Use existing CSS classes from the design system (cards, form controls, buttons, tables)
- Page handler passes: `PageContext`, list of entity types (for export filter), current CSRF token
- No external JS libraries — vanilla JS with fetch API
- File upload via hidden `<input type="file">` with drag-and-drop zone
- Accessible: proper labels, ARIA attributes on interactive elements

FORMAT:
1. `templates/admin/data_manager.html` — full page template with:
   - Page header ("Data Manager")
   - Import panel: drop zone, conflict mode `<select>`, import button, result area
   - Export panel: entity type checkboxes, format radios, export button
   - Error table area (hidden by default, shown after import with errors)
2. `src/handlers/data_handlers.rs` — add page handler:
   - `pub async fn data_manager_page(pool, session) -> Result<HttpResponse, AppError>`
3. Template struct in `src/templates_structs.rs` (or inline in handler)

FAILURE CONDITIONS:
- Template doesn't extend base layout (missing nav, session context)
- Hardcoded entity types instead of dynamic from handler
- Missing CSRF token in import request
- Drop zone doesn't provide visual feedback on dragover
- No loading state during import/export operations
- `cargo check` fails

---

## Task 7: Client-Side Error Mitigation JS

GOAL: Implement the JavaScript that handles import results, displays errors, and supports inline editing + retry of failed items.
Success = after an import with 3 errors, user can edit one item inline, skip another, force-upsert the third, and retry — the retry sends only the modified subset and the error table updates.

CONSTRAINTS:
- Vanilla JS only (no frameworks, no build step)
- JS lives in a `<script>` block at the bottom of the template (consistent with existing patterns)
- Failed items held in JS array state — retry sends subset to `/api/data/import`
- Inline editor uses a `<textarea>` pre-filled with JSON of the failing item
- "Force upsert" sends the single item with `conflict_mode: "upsert"`
- Result summary updates after each retry (cumulative counts)

FORMAT:
1. Embedded in `templates/admin/data_manager.html` `<script>` block:
   - `handleImport()` — reads file, sends to API, displays results
   - `handleExport()` — builds query params, triggers download
   - `displayErrors(errors)` — renders error table with per-item actions
   - `retryItem(index)` — sends single edited item back to API
   - `retryAll()` — sends all remaining failed items
   - `skipItem(index)` — removes item from error list
   - `skipAll()` — clears error list

FAILURE CONDITIONS:
- Full page reload on import/export (should be fetch + DOM update)
- Error table doesn't update after retry (stale state)
- Inline editor doesn't validate JSON before sending
- "Force upsert" sends all items instead of just the one
- Missing CSRF token on retry requests
- Export doesn't trigger file download (just displays in page)

---

## Task 8: Navigation, Seed & Wiring

GOAL: Add the Data Manager nav item to the admin sidebar and ensure the full feature is accessible end-to-end.
Success = after deleting the dev DB and restarting, "Data Manager" appears in the Admin sidebar for admin users, links to `/data-manager`, and the page loads correctly.

CONSTRAINTS:
- Nav item: `admin.data_manager`, label "Data Manager", parent `admin`, URL `/data-manager`
- Requires `settings.manage` permission (reuse existing permission, don't create a new one)
- Add to `seed_ontology()` alongside other admin nav items
- Register all routes in `main.rs` in correct order (specific before parameterized)
- No new permissions needed — reuse `settings.manage`

FORMAT:
1. `src/db.rs` — in `seed_ontology()`:
   - Insert nav item entity `admin.data_manager`
   - Insert `url` and `parent` properties
   - Insert `requires_permission` relation to `settings.manage`
2. `src/main.rs` — register routes:
   - `GET /data-manager` -> `data_handlers::data_manager_page`
   - `POST /api/data/import` -> `data_handlers::import_data`
   - `GET /api/data/export` -> `data_handlers::export_data`
   - `GET /api/data/schema` -> `data_handlers::schema`

FAILURE CONDITIONS:
- Nav item not visible to admin users (missing permission relation)
- Nav item visible to non-admin users (wrong permission)
- Routes registered after parameterized routes (swallowed by `/{id}`)
- Page 404s after fresh DB seed
- `cargo check` fails

---

## Dependency Order

```
Task 1 (types) ─────────────────────────────────────┐
    ├── Task 2 (export) ──┐                          │
    ├── Task 3 (json-ld) ─┼── Task 5 (handlers) ─── Task 8 (nav + wiring)
    └── Task 4 (import) ──┘       │
                                  └── Task 6 (template) ── Task 7 (JS)
```

Tasks 2, 3, 4 can run in parallel after Task 1.
Tasks 6 and 7 can be combined if preferred.
Task 8 is final integration.
