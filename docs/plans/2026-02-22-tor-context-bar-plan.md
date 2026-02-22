# ToR Context Bar — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a persistent ToR context bar below the header on all `/tor/{id}/...` pages, showing the ToR name and four section tabs (Overview | Workflow | Meetings | Templates) so users always know which ToR they're viewing.

**Architecture:** Add `TorContext { tor_id, tor_name, active_section }` to `PageContext` via an optional field and a `.with_tor()` builder. A new Askama partial renders the bar when the field is present. All GET handlers under `/tor/{id}/` call `.with_tor()` after building `PageContext`. A new `/tor/{id}/meetings` route lists that ToR's meetings.

**Tech Stack:** Actix-web 4, Askama 0.14, PostgreSQL/sqlx 0.8, CSS custom properties

---

## Task 1: Add `TorContext` + `PageContext::with_tor()`

**GOAL:** `PageContext` has an optional `tor_context: Option<TorContext>` field and a `with_tor(tor_id, name, section)` builder method. After this task, `cargo build` passes.

**CONSTRAINTS:**
- `TorContext` is a plain struct (no `#[derive(Template)]`) — it's a data carrier
- `with_tor()` takes `&str` args (not `String`) for ergonomics, stores as `String` internally
- `PageContext::build()` signature is UNCHANGED — no new DB calls inside it
- No new dependencies

**FORMAT:**

Modify `src/templates_structs.rs` — insert after line 40 (the `warning_count` field):

```rust
    pub tor_context: Option<TorContext>,
```

Change the `Ok(Self { ... })` line (currently line 56) to:

```rust
        Ok(Self { username, avatar_initial, permissions, flash, nav_modules, sidebar_items, app_name, csrf_token, warning_count, tor_context: None })
```

Add after the closing `}` of `impl PageContext` (after line 57):

```rust
pub struct TorContext {
    pub tor_id: i64,
    pub tor_name: String,
    pub active_section: String,
}
```

Add this method inside `impl PageContext` (before the closing `}` at line 57):

```rust
    /// Attach ToR context for pages nested under /tor/{id}/...
    pub fn with_tor(mut self, tor_id: i64, name: &str, section: &str) -> Self {
        self.tor_context = Some(TorContext {
            tor_id,
            tor_name: name.to_string(),
            active_section: section.to_string(),
        });
        self
    }
```

**FAILURE CONDITIONS:**
- `PageContext::build()` signature changes
- `tor_context` field has a default value other than `None`
- `TorContext` derives `Template` or `Serialize`

**Verify:**

```bash
cargo check 2>&1 | tail -5
```
Expected: no errors (warnings OK).

**Commit:**

```bash
git add src/templates_structs.rs
git commit -m "feat(tor-context): add TorContext struct and PageContext::with_tor() builder"
```

---

## Task 2: Create `tor_context_bar.html` partial + include in `base.html`

**GOAL:** A new partial renders a context bar when `ctx.tor_context` is set. `base.html` includes it. After this task, all pages that don't call `.with_tor()` are unaffected (bar hidden). `cargo build` passes.

**CONSTRAINTS:**
- Askama: no `&&` in `{% if %}` — use nested ifs
- Askama: use `.as_str()` for string comparisons in templates
- The partial MUST use `ctx` as its variable (base template scope)
- Do NOT add any JavaScript — the bar is static HTML

**FORMAT:**

Create `templates/partials/tor_context_bar.html`:

```html
{% if let Some(tc) = ctx.tor_context %}
<div class="tor-context-bar">
    <span class="tor-context-name">{{ tc.tor_name }}</span>
    <nav class="tor-context-tabs" aria-label="ToR sections">
        <a href="/tor/{{ tc.tor_id }}"
           class="tor-tab{% if tc.active_section.as_str() == "overview" %} active{% endif %}">Overview</a>
        <a href="/tor/{{ tc.tor_id }}/workflow"
           class="tor-tab{% if tc.active_section.as_str() == "workflow" %} active{% endif %}">Workflow</a>
        <a href="/tor/{{ tc.tor_id }}/meetings"
           class="tor-tab{% if tc.active_section.as_str() == "meetings" %} active{% endif %}">Meetings</a>
        <a href="/tor/{{ tc.tor_id }}/templates"
           class="tor-tab{% if tc.active_section.as_str() == "templates" %} active{% endif %}">Templates</a>
    </nav>
</div>
{% endif %}
```

Modify `templates/base.html` — after line 30 (`{% block nav %}{% endblock %}`), insert:

```html
    {% include "partials/tor_context_bar.html" %}
```

The full lines 29-32 should become:

```html
    {% block nav %}{% endblock %}
    {% include "partials/tor_context_bar.html" %}
    <div class="app-body">
```

**FAILURE CONDITIONS:**
- Template uses `&&` in Askama `{% if %}`
- Bar renders on pages that don't call `.with_tor()`
- JavaScript added to the partial

**Verify:**

```bash
cargo build 2>&1 | tail -3
```
Expected: `Finished` line, no errors.

**Commit:**

```bash
git add templates/partials/tor_context_bar.html templates/base.html
git commit -m "feat(tor-context): add tor_context_bar partial and include in base.html"
```

---

## Task 3: CSS for `.tor-context-bar`

**GOAL:** The context bar has correct styles: flex layout, muted background, ToR name left, tabs right. After this task, loading any page with `.with_tor()` shows a styled bar.

**CONSTRAINTS:**
- Use existing CSS custom properties (`--color-*`, `--text-*`, `--border-*`) — check existing usage in style.css with `grep "var(--" static/css/style.css | head -20` to find the right variable names
- No hardcoded color values
- Append to end of `static/css/style.css`

**FORMAT:**

Append to `static/css/style.css`:

```css
/* ── ToR Context Bar ────────────────────────────────────── */
.tor-context-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.5rem 1.5rem;
    background: var(--surface-secondary);
    border-bottom: 1px solid var(--border);
    font-size: 0.875rem;
}

.tor-context-name {
    font-weight: 600;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 50%;
    font-size: 0.9375rem;
}

.tor-context-tabs {
    display: flex;
    gap: 0.125rem;
}

.tor-tab {
    padding: 0.3rem 0.875rem;
    border-radius: var(--radius-sm, 4px);
    color: var(--text-muted);
    text-decoration: none;
    font-weight: 500;
    transition: background 0.15s, color 0.15s;
}

.tor-tab:hover {
    background: var(--surface-hover, rgba(0,0,0,0.04));
    color: var(--text);
}

.tor-tab.active {
    color: var(--accent, #b45309);
    border-bottom: 2px solid var(--accent, #b45309);
    border-radius: 0;
    padding-bottom: calc(0.3rem - 2px);
}
```

**IMPORTANT**: Before appending, run `grep -n "var(--surface-secondary\|var(--surface-hover\|var(--accent" static/css/style.css | head -5` to verify those variable names exist. If they don't match, use the closest equivalents from the existing codebase.

**FAILURE CONDITIONS:**
- Hardcoded hex colors (unless as fallback in `var(--x, #fallback)`)
- Breaks dark mode (custom properties handle this automatically if they're defined in `:root` with dark overrides)

**Verify:**

```bash
cargo build 2>&1 | tail -3
```
Then open the app, navigate to any ToR detail page — confirm bar is not yet visible (no `.with_tor()` calls yet).

**Commit:**

```bash
git add static/css/style.css
git commit -m "feat(tor-context): add CSS for tor-context-bar"
```

---

## Task 4: New `/tor/{id}/meetings` route + handler + template

**GOAL:** `GET /tor/{id}/meetings` renders a list of all meetings for that ToR, with the context bar showing "Meetings" tab active. `cargo build` passes.

**CONSTRAINTS:**
- Uses existing `meeting::find_by_tor(&pool, tor_id)` — do NOT add new model queries
- Requires `meetings.view` permission
- Uses `tor::require_tor_membership()` for access control
- New template struct name: `TorMeetingsListTemplate`
- Template path: `templates/meetings/tor_list.html`
- Route must be registered BEFORE any catch-all in the `.service()` block in `main.rs`

**FORMAT:**

Step 1: Add template struct to `src/templates_structs.rs` — insert after the existing `MeetingsListTemplate` block (around line 518):

```rust
#[derive(Template)]
#[template(path = "meetings/tor_list.html")]
pub struct TorMeetingsListTemplate {
    pub ctx: PageContext,
    pub tor_id: i64,
    pub tor_name: String,
    pub upcoming: Vec<MeetingListItem>,
    pub past: Vec<MeetingListItem>,
}
```

Step 2: Add to imports in `src/templates_structs.rs` (update the line `use crate::templates_structs::{MeetingsListTemplate, PageContext};` in any file as needed):

The struct is in `templates_structs.rs` — it imports are already there via `MeetingListItem`.

Step 3: Create `templates/meetings/tor_list.html`:

```html
{% extends "base.html" %}

{% block title %}Meetings — {{ tor_name }} — {{ ctx.app_name }}{% endblock %}

{% block nav %}
{% include "partials/nav.html" %}
{% endblock %}

{% block sidebar %}
{% include "partials/sidebar.html" %}
{% endblock %}

{% block content %}
<div class="page-header">
    <h1>Meetings — {{ tor_name }}</h1>
    <div class="page-actions">
        <a href="/tor/{{ tor_id }}" class="btn btn-sm">Back to ToR</a>
    </div>
</div>

{% if !upcoming.is_empty() %}
<section class="section">
    <div class="section-header">
        <h2>Upcoming ({{ upcoming.len() }})</h2>
    </div>
    <table class="table">
        <thead>
            <tr>
                <th>Date</th>
                <th>Status</th>
                <th>Location</th>
                <th>Actions</th>
            </tr>
        </thead>
        <tbody>
        {% for m in upcoming %}
            <tr>
                <td>{{ m.meeting_date }}</td>
                <td><span class="badge badge-info">{{ m.status }}</span></td>
                <td>{{ m.location }}</td>
                <td class="actions">
                    <a href="/tor/{{ tor_id }}/meetings/{{ m.id }}" class="btn btn-sm">View</a>
                </td>
            </tr>
        {% endfor %}
        </tbody>
    </table>
</section>
{% endif %}

{% if !past.is_empty() %}
<section class="section">
    <div class="section-header">
        <h2>Past ({{ past.len() }})</h2>
    </div>
    <table class="table">
        <thead>
            <tr>
                <th>Date</th>
                <th>Status</th>
                <th>Location</th>
                <th>Actions</th>
            </tr>
        </thead>
        <tbody>
        {% for m in past %}
            <tr>
                <td>{{ m.meeting_date }}</td>
                <td><span class="badge badge-muted">{{ m.status }}</span></td>
                <td>{{ m.location }}</td>
                <td class="actions">
                    <a href="/tor/{{ tor_id }}/meetings/{{ m.id }}" class="btn btn-sm">View</a>
                </td>
            </tr>
        {% endfor %}
        </tbody>
    </table>
</section>
{% endif %}

{% if upcoming.is_empty() %}
{% if past.is_empty() %}
<div class="empty-state">
    <p class="empty-state-title">No meetings yet</p>
    <p>Meetings are scheduled from the ToR detail page.</p>
</div>
{% endif %}
{% endif %}
{% endblock %}
```

Step 4: Add handler to `src/handlers/meeting_handlers/list.rs` — append after the existing `list()` function:

```rust
/// GET /tor/{id}/meetings — list meetings for a specific ToR.
pub async fn list_for_tor(
    pool: web::Data<PgPool>,
    session: Session,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    use crate::models::tor;
    use crate::auth::session::get_user_id;
    use crate::templates_structs::TorMeetingsListTemplate;

    require_permission(&session, "meetings.view")?;
    let tor_id = path.into_inner();
    let user_id = get_user_id(&session).ok_or(AppError::Session("User not logged in".to_string()))?;
    tor::require_tor_membership(&pool, user_id, tor_id).await?;

    let tor_name = tor::get_tor_name(&pool, tor_id).await?;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let upcoming = meeting::find_upcoming_for_tor(&pool, tor_id, &today).await
        .unwrap_or_default();
    let past = meeting::find_past_for_tor(&pool, tor_id, &today).await
        .unwrap_or_default();

    let ctx = PageContext::build(&session, &pool, "/meetings").await?
        .with_tor(tor_id, &tor_name, "meetings");

    render(TorMeetingsListTemplate { ctx, tor_id, tor_name, upcoming, past })
}
```

**IMPORTANT:** The meeting model may not have `find_upcoming_for_tor` and `find_past_for_tor`. Check first:

```bash
grep -n "pub async fn find" src/models/meeting/queries.rs
```

If `find_by_tor` exists (it does — line 101), use it for both upcoming and past:

```rust
    let all_meetings = meeting::find_by_tor(&pool, tor_id).await.unwrap_or_default();
    let upcoming: Vec<_> = all_meetings.iter()
        .filter(|m| m.meeting_date >= today)
        .cloned()
        .collect();
    let past: Vec<_> = all_meetings.iter()
        .filter(|m| m.meeting_date < today)
        .cloned()
        .collect();
```

Step 5: Add route to `src/main.rs` — insert after line 260 (`/meetings`, web::get()):

```rust
.route("/tor/{id}/meetings", web::get().to(handlers::meeting_handlers::list_for_tor))
```

Step 6: Add `list_for_tor` to the `mod.rs` exports. Check `src/handlers/meeting_handlers/mod.rs`:

```bash
cat src/handlers/meeting_handlers/mod.rs
```

Ensure `list_for_tor` is accessible via `handlers::meeting_handlers::list_for_tor`.

**FAILURE CONDITIONS:**
- Template references fields not in `TorMeetingsListTemplate`
- `find_by_tor` called with wrong argument types
- Route registered after a conflicting catch-all
- Missing `chrono` import in the handler (already used elsewhere — check `use chrono` in existing handler)

**Verify:**

```bash
cargo build 2>&1 | tail -5
```
Expected: `Finished` with no errors.

**Commit:**

```bash
git add src/templates_structs.rs templates/meetings/tor_list.html \
        src/handlers/meeting_handlers/list.rs src/handlers/meeting_handlers/mod.rs \
        src/main.rs
git commit -m "feat(tor-context): add /tor/{id}/meetings list page with context bar"
```

---

## Task 5: Update ToR overview and templates handlers

**GOAL:** ToR detail page (`GET /tor/{id}`) shows context bar with "Overview" tab active. Presentation templates page shows "Templates" tab active.

**CONSTRAINTS:**
- `tor_handlers/crud.rs::detail` has `tor: TorDetail` — use `tor.label` (not a new DB call)
- `with_tor()` call goes AFTER `PageContext::build()`, chained
- Do NOT change any template struct fields — only the handler's `ctx` construction

**FORMAT:**

In `src/handlers/tor_handlers/crud.rs`, find the `detail` handler (around line 81). It builds ctx and constructs `TorDetailTemplate`. The `tor.label` field holds the display name.

Look for:
```rust
let ctx = PageContext::build(&session, &pool, "/tor").await?;
let tmpl = TorDetailTemplate {
    ctx,
    tor,
    ...
};
```

Change to:
```rust
let ctx = PageContext::build(&session, &pool, "/tor").await?
    .with_tor(tor_id, &tor.label, "overview");
let tmpl = TorDetailTemplate {
    ctx,
    tor,
    ...
};
```

**Note:** The `detail` handler may extract `tor_id` from path as `let id = path.into_inner()` — use that variable. If `tor_id` isn't a named variable, check the handler and use whatever holds the ToR's ID.

In `src/handlers/tor_handlers/presentation.rs`, find the GET handler (around line 37). It already has `tor_id`. Look for how it gets the tor name — there's likely a `tor_label` field. Use that.

Pattern to add:
```rust
let ctx = PageContext::build(&session, &pool, "/tor").await?
    .with_tor(tor_id, &tor_label, "templates");
```

**FAILURE CONDITIONS:**
- `tor_id` variable name doesn't match what's in scope — always read the handler first
- New DB call added when tor name is already available in scope

**Verify:**

```bash
cargo build 2>&1 | tail -3
```

**Commit:**

```bash
git add src/handlers/tor_handlers/crud.rs src/handlers/tor_handlers/presentation.rs
git commit -m "feat(tor-context): activate context bar on tor detail and templates pages"
```

---

## Task 6: Update workflow + queue handlers

**GOAL:** `GET /tor/{id}/workflow`, `GET /tor/{id}/workflow/queue`, and `GET /tor/{id}/workflow/queue/schedule-form` show context bar with "Workflow" tab active.

**CONSTRAINTS:**
- `workflow_handlers::view` already has `tor_name` variable — reuse it, don't call `get_tor_name` again
- Queue handlers need `tor::get_tor_name(&pool, tor_id).await?` — `tor_name` is not yet in scope
- Import `crate::models::tor` if not already imported in `queue_handlers.rs`

**FORMAT:**

In `src/handlers/workflow_handlers.rs`, function `view` (line 13). It already calls `tor::get_tor_name` and stores result in `tor_name`. Just chain `.with_tor()`:

```rust
let ctx = PageContext::build(&session, &pool, "/workflow").await?
    .with_tor(tor_id, &tor_name, "workflow");
```

In `src/handlers/queue_handlers.rs`, find the GET handlers (`view_queue` and `schedule_form`). For each:

Step 1: Read the handler to see if `tor_name` is already fetched.
Step 2: If not, add before the `PageContext::build` call:
```rust
let tor_name = tor::get_tor_name(&pool, tor_id).await?;
```
Step 3: Chain `.with_tor()`:
```rust
let ctx = PageContext::build(&session, &pool, "/workflow").await?
    .with_tor(tor_id, &tor_name, "workflow");
```

**FAILURE CONDITIONS:**
- Calls `tor::get_tor_name` twice in a handler where `tor_name` is already available
- POST handlers updated (only GET handlers need the context bar)

**Verify:**

```bash
cargo build 2>&1 | tail -3
```
Visit `APP_ENV=staging` → ToR → Workflow. Confirm context bar shows "Workflow" active.

**Commit:**

```bash
git add src/handlers/workflow_handlers.rs src/handlers/queue_handlers.rs
git commit -m "feat(tor-context): context bar active on workflow and queue pages"
```

---

## Task 7: Update agenda, COA, and opinion GET handlers

**GOAL:** All GET handlers under `/tor/{id}/workflow/agenda/...` and `/tor/{id}/workflow/agenda/{id}/coa/...` and `/tor/{id}/workflow/agenda/{id}/input` show context bar with "Workflow" tab active.

**CONSTRAINTS:**
- Only modify GET handlers (skip POST handlers entirely)
- Each handler needs `tor_name` in scope before building `PageContext`
- Use `tor::get_tor_name(&pool, tor_id).await?` — it returns `Result<String, AppError>`
- `tor_id` comes from the path parameter (first element of path tuple)
- Do NOT change handler signatures or template structs

**AFFECTED HANDLERS:**

`src/handlers/agenda_handlers.rs`:
- `new_form` (GET /tor/{id}/workflow/agenda/new)
- `detail` (GET /tor/{id}/workflow/agenda/{id})
- `edit_form` (GET /tor/{id}/workflow/agenda/{id}/edit)

`src/handlers/coa_handlers.rs`:
- `new_form` (GET /tor/{id}/workflow/agenda/{id}/coa/new)
- `edit_form` (GET /tor/{id}/workflow/agenda/{id}/coa/{id}/edit)
- `detail` if it exists (GET /tor/{id}/workflow/agenda/{id}/coa/{id}) — check routes first

`src/handlers/opinion_handlers.rs`:
- `form` (GET /tor/{id}/workflow/agenda/{id}/input)
- `decision_form` (GET /tor/{id}/workflow/agenda/{id}/decide)

**FORMAT:** For each handler above, add before `PageContext::build`:

```rust
let tor_name = tor::get_tor_name(&pool, tor_id).await?;
```

Then chain:

```rust
let ctx = PageContext::build(&session, &pool, "/workflow").await?
    .with_tor(tor_id, &tor_name, "workflow");
```

**Verify:**

```bash
cargo build 2>&1 | tail -3
```

**Commit:**

```bash
git add src/handlers/agenda_handlers.rs src/handlers/coa_handlers.rs \
        src/handlers/opinion_handlers.rs
git commit -m "feat(tor-context): context bar on agenda, COA, and opinion pages"
```

---

## Task 8: Update proposal and suggestion GET handlers

**GOAL:** ToR-scoped proposal and suggestion pages show context bar with "Workflow" tab active.

**CONSTRAINTS:**
- Only ToR-scoped GETs: `proposal::detail`, `proposal::new_form`, `proposal::edit_form`, `suggestion::new_form`
- Cross-ToR views (`/workflow` index) must NOT get `.with_tor()` — no `tor_id` in scope there

**AFFECTED HANDLERS:**

`src/handlers/proposal_handlers.rs`:
- `detail` (line 19) — has `tor_id`
- `new_form` (line 46) — has `tor_id`
- `edit_form` (line 140) — has `tor_id`

`src/handlers/suggestion_handlers.rs`:
- `new_form` — has `tor_id` via `GET /tor/{id}/suggestions/new`

**FORMAT:** Same pattern as Task 7. For each GET handler:

```rust
let tor_name = tor::get_tor_name(&pool, tor_id).await?;
let ctx = PageContext::build(&session, &pool, "/workflow").await?
    .with_tor(tor_id, &tor_name, "workflow");
```

**FAILURE CONDITIONS:**
- `WorkflowIndexTemplate` or cross-ToR handlers get `.with_tor()` added
- `suggestion_handlers::new_form` uses wrong `tor_id` variable name

**Verify:**

```bash
cargo build 2>&1 | tail -3
```

**Commit:**

```bash
git add src/handlers/proposal_handlers.rs src/handlers/suggestion_handlers.rs
git commit -m "feat(tor-context): context bar on proposal and suggestion pages"
```

---

## Task 9: Update meeting detail handler

**GOAL:** `GET /tor/{id}/meetings/{mid}` (meeting detail page) shows context bar with "Meetings" tab active.

**CONSTRAINTS:**
- `meeting_handlers/crud.rs::detail` handler — already has `tor_id` from path
- The meeting detail may already have `tor_name` from the meeting model or the `ConfirmForm` struct — check first
- If `tor_name` is not available, fetch with `tor::get_tor_name(&pool, tor_id).await?`

**FORMAT:**

Read `src/handlers/meeting_handlers/crud.rs` starting at line 60 to find the detail handler. It extracts `(tor_id, mid)` from path. Look for where `ctx` is built and add `.with_tor()`:

```rust
let tor_name = tor::get_tor_name(&pool, tor_id).await?;
let ctx = PageContext::build(&session, &pool, "/meetings").await?
    .with_tor(tor_id, &tor_name, "meetings");
```

**Note:** Check what import `crate::models::tor` is available in this file. Add `use crate::models::tor;` to imports if missing.

**FAILURE CONDITIONS:**
- Changes POST handlers in meeting_handlers
- New DB call made when tor_name already in scope from the meeting's data

**Verify:**

```bash
cargo build 2>&1 | tail -3
```

Then run the app and navigate: `/tor/1/meetings/1` → confirm context bar shows "Meetings" tab active.

**Commit:**

```bash
git add src/handlers/meeting_handlers/crud.rs
git commit -m "feat(tor-context): context bar active on meeting detail page"
```

---

## Task 10: Final build + visual verification

**GOAL:** All tasks complete, app builds clean, context bar appears on all ToR-scoped pages.

**Steps:**

```bash
cargo build 2>&1 | grep -E "error|warning.*unused" | head -20
cargo test 2>&1 | tail -5
```

Visual checklist (run `APP_ENV=staging cargo run`, then open browser):

| URL | Expected active tab |
|-----|---------------------|
| `/tor/{id}` | Overview |
| `/tor/{id}/workflow` | Workflow |
| `/tor/{id}/workflow/queue` | Workflow |
| `/tor/{id}/workflow/agenda/{id}` | Workflow |
| `/tor/{id}/proposals/{id}` | Workflow |
| `/tor/{id}/meetings` | Meetings |
| `/tor/{id}/meetings/{mid}` | Meetings |
| `/tor/{id}/templates` | Templates |
| `/workflow` (cross-ToR) | no bar |
| `/meetings` (global) | no bar |
| `/dashboard` | no bar |

**Commit (if any cleanup needed):**

```bash
git add -p  # stage only what changed
git commit -m "fix(tor-context): cleanup after visual verification"
```

---

## Implementation Notes

- `tor::get_tor_name` is in `src/models/tor/queries.rs:395` — returns `Result<String, AppError>`
- `meeting::find_by_tor` is in `src/models/meeting/queries.rs:101`
- `TorMeetingsListTemplate` needs `chrono` for date comparison — `chrono` is already a dependency
- The `MeetingListItem` struct must have a `meeting_date: String` field — verify before using it in the template
- If a handler has `let path: web::Path<(i64, i64)>` and calls `path.into_inner()` consuming it, make sure `tor_id` is captured before the call
