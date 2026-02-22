# ToR Context Bar — Design Document

**Date**: 2026-02-22
**Feature**: P7 — Workflow sidebar nav highlighting / ToR contextual navigation
**Status**: Approved

---

## Problem

ToR-scoped pages (workflow, meetings, templates) have no contextual indication of:
- Which ToR the user is currently viewing
- How to navigate between sections of the same ToR

The global nav system uses longest-prefix matching. Handlers for `/tor/{id}/workflow/queue` pass `/workflow` as the active path to avoid triggering the `governance.tor` nav item (which would incorrectly highlight "Terms of Reference" in the sidebar). This means the sidebar correctly highlights the top-level **Workflow** module, but there is no finer-grained navigation for the ToR's sub-sections.

---

## Solution

A **ToR context bar** — a persistent banner immediately below the main header — showing:
- The ToR name (left)
- Four tab pills for the ToR's main sections (right)

This follows the GitHub repository nav pattern: the global nav stays stable, and a secondary contextual bar provides orientation within a resource.

---

## Visual Design

```
┌─────────────────────────────────────────────────────────┐
│ [Global nav: Dashboard | Governance | Users | ...]       │
├─────────────────────────────────────────────────────────┤
│ ToR: NATO Standing Committee        [Overview] [Workflow] [Meetings] [Templates] │
├─────────────────────────────────────────────────────────┤
│ [Sidebar]  │  [Page content]                            │
```

- Full-width bar, muted background (`--color-muted-bg`), 1px bottom border
- ToR name on the left (bold, truncated with ellipsis on small screens)
- Four tab pills on the right; active tab underlined with accent color
- Bar only renders when `tor_context` is present in `PageContext`

---

## Architecture

### 1. Entity helper — `entity::find_label_by_id`

New function in `src/models/entity.rs`:

```rust
pub async fn find_label_by_id(pool: &PgPool, id: i64) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT label FROM entities WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
}
```

Also add `tor_name: String` to the `Entity` struct (convenience field populated by `find_by_id`; already in the SELECT, just formally named for IDE discoverability). Actually, `Entity` already has a `label` field — no struct change needed.

### 2. `TorContext` struct

In `src/templates_structs.rs`:

```rust
pub struct TorContext {
    pub tor_id: i64,
    pub tor_name: String,
    pub active_section: String, // "overview" | "workflow" | "meetings" | "templates"
}
```

### 3. `PageContext` extension

Add field and builder method to `PageContext`:

```rust
pub struct PageContext {
    // ... existing fields ...
    pub tor_context: Option<TorContext>,
}

impl PageContext {
    pub fn with_tor(mut self, tor_id: i64, name: &str, section: &str) -> Self {
        self.tor_context = Some(TorContext {
            tor_id,
            tor_name: name.to_string(),
            active_section: section.to_string(),
        });
        self
    }
}
```

### 4. New route: `GET /tor/{id}/meetings`

A dedicated meeting list handler for a specific ToR, returning a lightweight list of all meetings belonging to that ToR ordered by date descending.

**Handler**: `src/handlers/meeting_handlers/list.rs` — new `list_for_tor()` function
**Template**: `templates/meetings/tor_list.html`
**Route**: registered in `src/main.rs`

### 5. Template partial: `templates/partials/tor_context_bar.html`

```html
{% if let Some(tc) = ctx.tor_context %}
<div class="tor-context-bar">
  <span class="tor-context-name">{{ tc.tor_name }}</span>
  <nav class="tor-context-tabs">
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

### 6. Base template inclusion

In `templates/base.html`, immediately after the `{% block nav %}` block and before `{% block sidebar %}`:

```html
{% include "partials/tor_context_bar.html" %}
```

### 7. CSS

New rules in `static/css/style.css`:

```css
.tor-context-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.5rem 1.5rem;
    background: var(--color-muted-bg);
    border-bottom: 1px solid var(--color-border);
    font-size: 0.875rem;
}

.tor-context-name {
    font-weight: 600;
    color: var(--color-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 40%;
}

.tor-context-tabs {
    display: flex;
    gap: 0.25rem;
}

.tor-tab {
    padding: 0.25rem 0.75rem;
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
    text-decoration: none;
    font-weight: 500;
}

.tor-tab:hover {
    background: var(--color-hover-bg);
    color: var(--color-text);
}

.tor-tab.active {
    color: var(--color-accent);
    border-bottom: 2px solid var(--color-accent);
    border-radius: 0;
}
```

### 8. Handler updates

All GET handlers under `/tor/{id}/` must call `.with_tor()`. Sections:

| Handler file | Section value |
|---|---|
| `tor_handlers/detail.rs` | `"overview"` |
| `tor_handlers/protocol.rs` (if exists) | `"overview"` |
| `workflow_handlers.rs` (view) | `"workflow"` |
| `queue_handlers.rs` (view, schedule, unqueue GET) | `"workflow"` |
| `agenda_handlers.rs` (all GETs) | `"workflow"` |
| `suggestion_handlers.rs` (GETs with tor_id) | `"workflow"` |
| `proposal_handlers.rs` (GETs with tor_id) | `"workflow"` |
| `meeting_handlers/list.rs` (list_for_tor) | `"meetings"` |
| `meeting_handlers/crud.rs` (detail, edit) | `"meetings"` |
| Template handlers (if any) | `"templates"` |

**Pattern**:
```rust
let tor_name = entity::find_label_by_id(&pool, tor_id).await?
    .unwrap_or_else(|| "Unknown ToR".to_string());
let ctx = PageContext::build(&session, &pool, "/workflow").await?
    .with_tor(tor_id, &tor_name, "workflow");
```

---

## Data Flow

```
HTTP GET /tor/42/workflow/queue
  → queue_handlers::view()
  → entity::find_label_by_id(&pool, 42) → "NATO Standing Committee"
  → PageContext::build(..., "/workflow").await?
      .with_tor(42, "NATO Standing Committee", "workflow")
  → Template renders tor_context_bar.html with active_section = "workflow"
  → Context bar shows with "Workflow" tab active
```

---

## Testing

- No new integration tests required — this is pure rendering logic
- Verify visually via Playwright or manual browser testing on `APP_ENV=staging`
- Confirm the context bar appears on: ToR detail, workflow view, queue, agenda detail, meeting detail
- Confirm it does NOT appear on: user list, role list, dashboard (no `tor_context` set)

---

## Scope Boundaries

**In scope:**
- Entity helper function
- `TorContext` + `PageContext.with_tor()`
- ToR context bar partial + CSS
- New `/tor/{id}/meetings` route + template
- Updating all ToR-scoped GET handlers

**Out of scope:**
- Changing the global nav highlighting logic
- Adding more tabs (e.g., Members — already on ToR detail page)
- Mobile-responsive collapse (future improvement)
- ABAC-based tab visibility (all tabs always shown; permission errors handled on navigation)
