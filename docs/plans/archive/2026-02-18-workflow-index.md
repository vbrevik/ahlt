# Workflow Index Page — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement `GET /workflow` as a standalone cross-ToR workflow landing page with 3 tabs (Suggestions / Proposals / Agenda Points) showing items from all ToRs the user is a member of (or all ToRs for `workflow.manage` holders).

**Architecture:** Three new `find_all_cross_tor(conn, user_id: Option<i64>)` query functions (one per model), a new `WorkflowIndexTemplate` struct, a new `index` handler, a new template, and a single route registration before the existing `/tor/{id}/workflow` route.

**Tech Stack:** Rust / Actix-web 4 / Askama 0.14 / rusqlite — same as all existing handlers.

---

### Task 1: CrossTorSuggestionItem type + cross-ToR query

**Files:**
- Modify: `src/models/suggestion/types.rs`
- Modify: `src/models/suggestion/queries.rs`

**Prompt Contract:**

GOAL: Add `CrossTorSuggestionItem` struct to `src/models/suggestion/types.rs` and `find_all_cross_tor` function to `src/models/suggestion/queries.rs`. Running `cargo check` must produce zero errors.

CONSTRAINTS:
- Do NOT modify any existing types or functions — only add new ones
- `user_id: Option<i64>` — `None` = global (no membership filter), `Some(id)` = filter to ToRs user fills a position in
- Use `WHERE EXISTS` subquery for the membership filter to avoid duplicate rows from multiple positions
- Membership chain: `fills_position` (user→tor_function) then `belongs_to_tor` (tor_function→tor)
- All existing `suggested_to` JOIN and LEFT JOIN patterns from `find_all_for_tor` must be preserved verbatim
- `make_preview` is a private helper in queries.rs — call it the same way as the existing function

FORMAT:
```rust
// In types.rs — add after SuggestionListItem:
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossTorSuggestionItem {
    pub tor_id: i64,
    pub tor_name: String,
    pub id: i64,
    pub description: String,
    pub description_preview: String,
    pub submitted_by_id: i64,
    pub submitted_by_name: String,
    pub submitted_date: String,
    pub status: String,
    pub rejection_reason: Option<String>,
    pub spawned_proposal_id: Option<i64>,
}

// In queries.rs — add after find_all_for_tor:
pub fn find_all_cross_tor(conn: &Connection, user_id: Option<i64>) -> Result<Vec<CrossTorSuggestionItem>, AppError> {
    // Build SQL: base query JOINs suggested_to → tor. When user_id is Some,
    // appends WHERE EXISTS (fills_position → belongs_to_tor membership check).
    // SELECT tor.id AS tor_id, tor.label AS tor_name, e.id, <all existing columns>
    // FROM entities e
    // JOIN relations r ON e.id = r.source_id
    // JOIN entities rt ON r.relation_type_id = rt.id AND rt.name = 'suggested_to'
    // JOIN entities tor ON tor.id = r.target_id AND tor.entity_type = 'tor'
    // <all existing LEFT JOINs for properties>
    // WHERE e.entity_type = 'suggestion'
    // [AND EXISTS (SELECT 1 FROM relations r_fills
    //              JOIN relations r_tor ON r_fills.target_id = r_tor.source_id
    //              WHERE r_fills.source_id = ?user_id
    //                AND r_tor.target_id = tor.id
    //                AND r_fills.relation_type_id = (fills_position type subselect)
    //                AND r_tor.relation_type_id = (belongs_to_tor type subselect))]
    // ORDER BY tor.label ASC, submitted_date DESC
}
```

FAILURE CONDITIONS:
- Modifies or removes any existing type or function
- Uses JOIN (instead of WHERE EXISTS) for membership filter — causes duplicate rows
- Missing `tor_id` or `tor_name` columns in SELECT
- `user_id = None` branch does not return ALL suggestions across all ToRs
- `cargo check` produces any errors

**Steps:**
1. Add `CrossTorSuggestionItem` struct to `src/models/suggestion/types.rs` after `SuggestionListItem`
2. Add `find_all_cross_tor` to `src/models/suggestion/queries.rs` after `find_all_for_tor`
3. Run `cargo check 2>&1 | tail -5` — must output `Finished`
4. Commit: `git commit -m "feat(workflow): add CrossTorSuggestionItem and find_all_cross_tor query"`

---

### Task 2: CrossTorProposalItem type + cross-ToR query

**Files:**
- Modify: `src/models/proposal/types.rs`
- Modify: `src/models/proposal/queries.rs`

**Prompt Contract:**

GOAL: Add `CrossTorProposalItem` struct to `src/models/proposal/types.rs` and `find_all_cross_tor` function to `src/models/proposal/queries.rs`. Running `cargo check` must produce zero errors.

CONSTRAINTS:
- Same `Option<i64>` / `WHERE EXISTS` membership pattern as Task 1
- Proposals link to ToR via `submitted_to` relation (source=proposal, target=tor)
- All existing property LEFT JOINs from `find_all_for_tor` (title, date, status, submitted_by, rejection_reason, spawns_proposal reverse) must be preserved
- Do NOT modify any existing types or functions

FORMAT:
```rust
// In types.rs — add after ProposalListItem:
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossTorProposalItem {
    pub tor_id: i64,
    pub tor_name: String,
    pub id: i64,
    pub title: String,
    pub submitted_by_id: i64,
    pub submitted_by_name: String,
    pub submitted_date: String,
    pub status: String,
    pub rejection_reason: Option<String>,
    pub related_suggestion_id: Option<i64>,
}

// In queries.rs: same structure as Task 1 but with submitted_to relation
// and proposal-specific columns
```

FAILURE CONDITIONS:
- Uses `member_of` instead of `fills_position` membership chain
- Missing `tor_id` or `tor_name` in output
- Modifies any existing function
- `cargo check` errors

**Steps:**
1. Add `CrossTorProposalItem` to `src/models/proposal/types.rs`
2. Add `find_all_cross_tor` to `src/models/proposal/queries.rs`
3. Run `cargo check 2>&1 | tail -5` — must output `Finished`
4. Commit: `git commit -m "feat(workflow): add CrossTorProposalItem and find_all_cross_tor query"`

---

### Task 3: CrossTorAgendaItem type + cross-ToR query

**Files:**
- Modify: `src/models/agenda_point/types.rs`
- Modify: `src/models/agenda_point/queries.rs`

**Prompt Contract:**

GOAL: Add `CrossTorAgendaItem` struct and `find_all_cross_tor` function to the agenda_point model. Running `cargo check` must produce zero errors.

CONSTRAINTS:
- Same `Option<i64>` / `WHERE EXISTS` membership pattern as Tasks 1 and 2
- Agenda points link to ToR via `belongs_to_tor` relation (source=agenda_point, target=tor)
- `AgendaPointListItem` already has `tor_id: i64` but not `tor_name` — `CrossTorAgendaItem` adds `tor_name: String`
- The cross-ToR query can JOIN `tor.label` directly since we're already JOINing tor; no need for a separate `tor_id` entity_property lookup
- Do NOT modify any existing types or functions

FORMAT:
```rust
// In types.rs — add after AgendaPointListItem:
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossTorAgendaItem {
    pub tor_id: i64,
    pub tor_name: String,
    pub id: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub scheduled_date: String,
    pub item_type: String,
}
```

FAILURE CONDITIONS:
- Modifies `AgendaPointListItem` instead of creating new type
- `cargo check` errors
- Missing `tor_name` field

**Steps:**
1. Add `CrossTorAgendaItem` to `src/models/agenda_point/types.rs`
2. Add `find_all_cross_tor` to `src/models/agenda_point/queries.rs`
3. Run `cargo check 2>&1 | tail -5` — must output `Finished`
4. Commit: `git commit -m "feat(workflow): add CrossTorAgendaItem and find_all_cross_tor query"`

---

### Task 4: WorkflowIndexTemplate + index handler

**Files:**
- Modify: `src/templates_structs.rs`
- Modify: `src/handlers/workflow_handlers.rs`

**Prompt Contract:**

GOAL: Add `WorkflowIndexTemplate` to `src/templates_structs.rs` and `index` handler function to `src/handlers/workflow_handlers.rs`. Running `cargo check` must produce zero errors.

CONSTRAINTS:
- Template struct must use `#[template(path = "workflow/index.html")]`
- Handler permission check: `require_permission(&session, "suggestion.view")?`
- Admin/reviewer check: `permissions.has("workflow.manage")` — if true call `find_all_cross_tor(conn, None)`, else call `find_all_cross_tor(conn, Some(user_id))`
- `active_tab` defaults to `"suggestions"` when `?tab=` param is absent
- `PageContext::build` path argument must be `"/workflow"` (matches nav item URL, highlights Governance→Item Workflow in sidebar)
- Import the three new cross-ToR types at the top of `templates_structs.rs`
- Do NOT modify the existing `WorkflowTemplate` struct or `view` handler

FORMAT:
```rust
// In templates_structs.rs — new imports alongside existing:
use crate::models::suggestion::CrossTorSuggestionItem;
use crate::models::proposal::CrossTorProposalItem;
use crate::models::agenda_point::CrossTorAgendaItem;

// New struct after WorkflowTemplate:
#[derive(Template)]
#[template(path = "workflow/index.html")]
pub struct WorkflowIndexTemplate {
    pub ctx: PageContext,
    pub active_tab: String,
    pub suggestions: Vec<CrossTorSuggestionItem>,
    pub proposals: Vec<CrossTorProposalItem>,
    pub agenda_points: Vec<CrossTorAgendaItem>,
}

// In workflow_handlers.rs — new handler:
pub async fn index(
    pool: web::Data<DbPool>,
    session: Session,
    query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "suggestion.view")?;
    let conn = pool.get()?;
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".into()))?;
    let permissions = get_permissions(&session)?;
    let active_tab = query.get("tab").cloned().unwrap_or_else(|| "suggestions".to_string());

    let filter_id = if permissions.has("workflow.manage") { None } else { Some(user_id) };

    let suggestions = suggestion::find_all_cross_tor(&conn, filter_id)?;
    let proposals = proposal::find_all_cross_tor(&conn, filter_id)?;
    let agenda_points = agenda_point::find_all_cross_tor(&conn, filter_id)?;

    let ctx = PageContext::build(&session, &conn, "/workflow")?;
    render(WorkflowIndexTemplate { ctx, active_tab, suggestions, proposals, agenda_points })
}
```

FAILURE CONDITIONS:
- Modifies existing `WorkflowTemplate` or `view` handler
- Missing `use crate::auth::session::get_permissions` import in handler file
- `filter_id` logic is inverted (admin gets filtered, members get global)
- `cargo check` errors

**Steps:**
1. Add imports + `WorkflowIndexTemplate` to `src/templates_structs.rs`
2. Add `index` handler to `src/handlers/workflow_handlers.rs` (add any needed imports)
3. Create placeholder `templates/workflow/index.html` with just `{% extends "base.html" %}{% block content %}TODO{% endblock %}` so Askama can find it at compile time
4. Run `cargo check 2>&1 | tail -5` — must output `Finished`
5. Commit: `git commit -m "feat(workflow): add WorkflowIndexTemplate and index handler"`

---

### Task 5: workflow/index.html template

**Files:**
- Modify: `templates/workflow/index.html` (replace placeholder from Task 4)

**Prompt Contract:**

GOAL: Replace the placeholder `templates/workflow/index.html` with a complete 3-tab template showing cross-ToR suggestions, proposals, and agenda points. Running `cargo check` must produce zero errors.

CONSTRAINTS:
- Must extend `base.html` and include `partials/nav.html` + `partials/sidebar.html` (identical block structure to `templates/workflow/view.html`)
- Tab links: `?tab=suggestions`, `?tab=proposals`, `?tab=agenda_points` — all relative to `/workflow`
- Each table must have a **ToR** column (first column) as a link to `/tor/{item.tor_id}/workflow?tab=<tabname>` — so clicking the ToR name navigates to the per-ToR view for that tab
- No create/action buttons — this is a read-only overview; actions happen in the per-ToR view
- Empty state per tab (e.g. "No suggestions across your Terms of Reference")
- Status badges must be identical to `workflow/view.html` — copy the `{% if ... %}` badge blocks verbatim
- Askama 0.14 constraint: NO `&&` in `{% if %}` — use nested `{% if %}` blocks for compound conditions
- Askama 0.14 constraint: `{% if let Some(x) = val %}` not `{% if let Some(ref x) = val %}`
- Use `item.tor_name` and `item.tor_id` (fields on `CrossTor*` types)

FORMAT (table structure per tab):
```html
<!-- Suggestions tab table columns: ToR | ID | Description | Submitted By | Date | Status -->
<!-- Proposals tab table columns:   ToR | ID | Title | Submitted By | Date | Status -->
<!-- Agenda Points tab table cols:  ToR | ID | Title | Type | Scheduled Date | Status -->
<!-- Each ToR cell: <a href="/tor/{{ s.tor_id }}/workflow?tab=suggestions">{{ s.tor_name }}</a> -->
```

FAILURE CONDITIONS:
- Any Askama compile error (run `cargo check`)
- Uses `&&` in `{% if %}` condition
- Uses `{% if let Some(ref x) = val %}`
- Missing ToR column in any of the 3 tabs
- ToR cell does not link to the per-ToR workflow view

**Steps:**
1. Replace `templates/workflow/index.html` with the full template
2. Run `cargo check 2>&1 | tail -5` — must output `Finished`
3. Run `cargo build 2>&1 | tail -3` — must output `Finished`
4. Commit: `git commit -m "feat(workflow): workflow index template with cross-ToR tabs"`

---

### Task 6: Register GET /workflow route

**Files:**
- Modify: `src/main.rs`

**Prompt Contract:**

GOAL: Register `.route("/workflow", web::get().to(handlers::workflow_handlers::index))` in `main.rs` so that `GET /workflow` returns 200. Must be registered **before** any `/tor/{id}/workflow` route to prevent path-param capture.

CONSTRAINTS:
- Add the route immediately before the `/tor` scope block (or at least before the line `.route("/tor/{id}/workflow", ...)`)
- Do NOT reorder any existing routes relative to each other
- No changes outside route registration

FORMAT:
```rust
// In main.rs, add before the .service(web::scope("/tor") block:
.route("/workflow", web::get().to(handlers::workflow_handlers::index))
```

FAILURE CONDITIONS:
- Route registered after `/tor/{id}/workflow` — would never be reachable since there's no `{id}` collision, but wrong placement signals misunderstanding
- Any other routes reordered
- `cargo check` errors

**Steps:**
1. Add the route in `src/main.rs` before `/tor/{id}/workflow`
2. Run `cargo check 2>&1 | tail -5` — must output `Finished`
3. Run `cargo test 2>&1 | tail -5` — all tests must pass
4. Start server (`cargo run`) and navigate to `http://localhost:8080/workflow` — must return 200 with the 3-tab page
5. Commit: `git commit -m "feat(workflow): register GET /workflow route — fixes 404"`

---

## Verification

After all tasks, confirm:
- `cargo test` — all 47+ tests pass
- `GET /workflow` — 200, shows 3 tabs
- `GET /workflow?tab=proposals` — proposals tab active
- Nav item "Item Workflow" in Governance sidebar highlighted when on `/workflow`
- User with only `suggestion.view` sees only their ToRs' items
- User with `workflow.manage` sees all items
