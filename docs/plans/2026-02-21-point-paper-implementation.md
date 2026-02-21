# Point Paper Detail Page — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Redesign `templates/agenda/detail.html` into a two-column "point paper" layout with a sticky sidebar, COA comparison grid with preference bars, and collapsible opinions — using the existing design system.

**Architecture:** Four independent changes: (1) CSS additions to `style.css`, (2) `preference_pct` field added to `OpinionSummary` struct + 3 handler updates, (3) full template redesign, (4) broken URL fix in `decision_form.html`. No schema changes. No new files. No JS.

**Tech Stack:** Rust (actix-web, sqlx), Askama 0.14, vanilla CSS (custom properties, CSS grid).

---

## Pre-flight check

Before starting, verify the server builds:

```bash
cargo check 2>&1 | tail -5
```

Expected: `Finished` with no errors.

---

## Task 1: Add CSS classes for the point paper layout

**Prompt Contract:**
- GOAL: Add all new CSS classes for the point paper redesign in one commit — no Rust changes yet
- CONSTRAINTS: Add to end of `static/css/style.css` only. No existing rules modified. No inline styles in CSS file. Dark mode: all custom properties already handle this automatically via `:root.dark` — no extra dark selectors needed.
- FORMAT: One `/* === Point Paper === */` block, then alphabetical-ish grouping by component
- FAILURE CONDITIONS: Any existing test fails; any existing page visually breaks; `cargo check` fails

**Files:**
- Modify: `static/css/style.css` (append at end)

**Step 1: Append the CSS block**

Open `static/css/style.css` and append at the very end:

```css
/* === Point Paper (Agenda Point Detail) === */

/* Layout */
.point-paper-grid {
    display: grid;
    grid-template-columns: 1fr 300px;
    gap: 2rem;
    align-items: start;
}

/* Description prose block — left accent bar */
.point-paper-desc {
    background: var(--surface);
    border: 1px solid var(--border);
    border-left: 4px solid var(--accent);
    border-radius: var(--radius);
    padding: 1.25rem 1.5rem;
    margin-bottom: 1.5rem;
    color: var(--text);
    line-height: 1.7;
}

/* Sidebar — unified metadata + actions card */
.point-paper-sidebar {
    position: sticky;
    top: 80px;
    align-self: start;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
    box-shadow: var(--shadow-sm);
}

.point-paper-meta {
    padding: 1.25rem;
}

.point-paper-meta-row {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    padding: 0.5rem 0;
    border-bottom: 1px solid var(--border);
    font-size: 0.875rem;
    gap: 0.5rem;
}

.point-paper-meta-row:last-child {
    border-bottom: none;
}

.point-paper-meta-label {
    color: var(--text-muted);
    font-weight: 600;
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    flex-shrink: 0;
}

.point-paper-meta-value {
    color: var(--text);
    text-align: right;
}

.point-paper-divider {
    height: 1px;
    background: var(--border);
    margin: 0;
}

.point-paper-actions {
    padding: 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 0.625rem;
}

.point-paper-actions-label {
    font-size: 0.6875rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--text-muted);
    margin-bottom: 0.25rem;
}

/* COA comparison grid */
.coa-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 1rem;
    margin-bottom: 0.5rem;
}

.coa-comparison-card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 1.25rem;
    box-shadow: var(--shadow-xs);
    display: flex;
    flex-direction: column;
    gap: 0.625rem;
    transition: box-shadow var(--duration) var(--ease);
}

.coa-comparison-card:hover {
    box-shadow: var(--shadow-sm);
}

.coa-comparison-card-title {
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 1rem;
    letter-spacing: -0.01em;
    color: var(--text);
    line-height: 1.3;
}

/* Preference bar */
.coa-pref {
    display: flex;
    align-items: center;
    gap: 0.625rem;
}

.coa-pref-bar-track {
    flex: 1;
    height: 4px;
    background: var(--bg-subtle);
    border-radius: var(--radius-full);
    overflow: hidden;
}

.coa-pref-bar-fill {
    height: 100%;
    background: var(--accent);
    border-radius: var(--radius-full);
    transition: width var(--duration-slow) var(--ease-out);
}

.coa-pref-count {
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--text-secondary);
    white-space: nowrap;
}

.coa-pref-empty {
    font-size: 0.75rem;
    color: var(--text-muted);
}

.coa-card-desc {
    font-size: 0.875rem;
    color: var(--text-secondary);
    line-height: 1.55;
    flex: 1;
}

.coa-card-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: 0.25rem;
}

.coa-card-link {
    font-size: 0.8125rem;
    font-weight: 500;
    color: var(--accent);
    text-decoration: none;
    transition: color var(--duration) var(--ease);
}

.coa-card-link:hover {
    color: var(--accent-hover);
}

/* Opinions — collapsible groups */
.opinion-group {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
    margin-bottom: 0.75rem;
}

.opinion-group-summary {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.875rem 1.25rem;
    cursor: pointer;
    list-style: none;
    font-weight: 600;
    font-size: 0.9375rem;
    color: var(--text);
    gap: 0.75rem;
    transition: background var(--duration) var(--ease);
}

.opinion-group-summary::-webkit-details-marker { display: none; }

.opinion-group-summary:hover {
    background: var(--bg-subtle);
}

.opinion-group-summary-left {
    display: flex;
    align-items: center;
    gap: 0.625rem;
}

.opinion-chevron {
    font-size: 0.75rem;
    color: var(--text-muted);
    transition: transform var(--duration) var(--ease);
}

details[open] .opinion-chevron {
    transform: rotate(180deg);
}

.opinion-group-body {
    border-top: 1px solid var(--border);
    padding: 0.5rem 0;
}

.opinion-item {
    display: grid;
    grid-template-columns: 160px 1fr auto;
    align-items: baseline;
    gap: 0.75rem;
    padding: 0.5rem 1.25rem;
    font-size: 0.875rem;
}

.opinion-item:hover {
    background: var(--bg-subtle);
}

.opinion-member {
    font-weight: 600;
    color: var(--text);
}

.opinion-commentary {
    color: var(--text-secondary);
    font-style: italic;
}

.opinion-date {
    color: var(--text-muted);
    font-size: 0.8125rem;
    white-space: nowrap;
}
```

**Step 2: Verify no visual breakage**

```bash
cargo check 2>&1 | tail -3
```

Expected: `Finished` — CSS changes don't affect Rust compilation but confirms nothing else broke.

**Step 3: Commit**

```bash
git add static/css/style.css
git commit -m "style: add point paper CSS classes for agenda detail redesign"
```

---

## Task 2: Add `preference_pct` to `OpinionSummary` and update handlers

**Prompt Contract:**
- GOAL: Add `preference_pct: u32` to `OpinionSummary`; compute it in `agenda_handlers.rs` after building the summaries vec; add `preference_pct: 0` in the two `opinion_handlers.rs` construction sites
- CONSTRAINTS: `opinion_handlers.rs` construction sites get `preference_pct: 0` (they serve the decision form which doesn't use the bar). `agenda_handlers.rs` computes total from `opinions.iter().map(|s| s.preference_count).sum::<i32>()` then sets `pct = count * 100 / total` (guard for total == 0). Askama auto-handles the new field — no template change yet.
- FORMAT: One commit per file (types → agenda handler → opinion handler), or one combined commit
- FAILURE CONDITIONS: `cargo check` fails; any test fails

**Files:**
- Modify: `src/models/opinion/types.rs`
- Modify: `src/handlers/agenda_handlers.rs`
- Modify: `src/handlers/opinion_handlers.rs`

**Step 1: Add field to `OpinionSummary`**

In `src/models/opinion/types.rs`, find `OpinionSummary`:

```rust
pub struct OpinionSummary {
    pub coa_id: i64,
    pub coa_title: String,
    pub preference_count: i32,
    pub opinions: Vec<OpinionListItem>,
}
```

Add `preference_pct` field:

```rust
pub struct OpinionSummary {
    pub coa_id: i64,
    pub coa_title: String,
    pub preference_count: i32,
    pub preference_pct: u32,   // ← add this
    pub opinions: Vec<OpinionListItem>,
}
```

**Step 2: Verify it fails to compile (3 construction sites need updating)**

```bash
cargo check 2>&1 | grep "missing field"
```

Expected: 3 errors mentioning `missing field preference_pct`.

**Step 3: Update `agenda_handlers.rs` — compute percentages**

Find the section in `src/handlers/agenda_handlers.rs` that builds the `opinions` vec (around line 147–160). After the loop, add percentage computation.

Current code (the loop ends at line ~160):
```rust
// ... for loop building opinions vec ...

// Get user permissions for workflow transitions
```

Replace with (insert between loop end and the permissions line):

```rust
            // Compute preference percentages for the preference bar
            let total_prefs: i32 = opinions.iter().map(|s| s.preference_count).sum();
            for summary in &mut opinions {
                summary.preference_pct = if total_prefs > 0 {
                    (summary.preference_count * 100 / total_prefs) as u32
                } else {
                    0
                };
            }
```

Also update the `OpinionSummary {}` construction (line ~154) to include the new field:

```rust
                opinions.push(crate::models::opinion::OpinionSummary {
                    coa_id: coa_detail.id,
                    coa_title: coa_detail.title.clone(),
                    preference_count: coa_opinions.len() as i32,
                    preference_pct: 0,  // ← set to 0 initially; updated in post-loop below
                    opinions: coa_opinions,
                });
```

**Step 4: Update `opinion_handlers.rs` — two construction sites**

Find both `OpinionSummary {` blocks in `src/handlers/opinion_handlers.rs` (around lines 201 and 276).

For both, add `preference_pct: 0`:

```rust
        opinions.push(opinion::OpinionSummary {
            coa_id: coa.id,
            coa_title: coa.title.clone(),
            preference_count: count,
            preference_pct: 0,   // ← add this line
            opinions: items,
        });
```

**Step 5: Verify it compiles**

```bash
cargo check 2>&1 | tail -5
```

Expected: `Finished` with no errors.

**Step 6: Run the test suite**

```bash
cargo test 2>&1 | tail -5
```

Expected: all tests pass.

**Step 7: Commit**

```bash
git add src/models/opinion/types.rs src/handlers/agenda_handlers.rs src/handlers/opinion_handlers.rs
git commit -m "feat(opinion): add preference_pct field for COA bar chart in point paper"
```

---

## Task 3: Redesign `templates/agenda/detail.html`

**Prompt Contract:**
- GOAL: Replace the flat-stack template with the two-column point paper layout. All data already available in template context. No Rust changes.
- CONSTRAINTS: Askama 0.14 — no `&&`, no `||`, no array indexing, no `ref` in `if let`. Nested `{% if %}` for compound conditions. Use `<details>`/`<summary>` for collapsible opinions (no JS). The `coa-pref-bar-fill` width = `style="width: {{ summary.preference_pct }}%"` via inline style (percentage, not a complex expression — Askama handles integer display). Cross-reference COA ↔ opinion summary using `{% for summary in opinions %}{% if summary.coa_id == coa.id %}`.
- FORMAT: Full file replacement. All existing functionality preserved (transitions, decide link, manage COAs link, record opinion link).
- FAILURE CONDITIONS: Any URL broken; any permission check removed; `cargo check` fails; template fails to compile

**Files:**
- Modify: `templates/agenda/detail.html` (full replacement)

**Step 1: Replace the template**

Write the following content to `templates/agenda/detail.html`:

```html
{% extends "base.html" %}

{% block title %}{{ agenda_point.title }} — {{ ctx.app_name }}{% endblock %}

{% block nav %}
{% include "partials/nav.html" %}
{% endblock %}

{% block sidebar %}
{% include "partials/sidebar.html" %}
{% endblock %}

{% block content %}
{% if let Some(msg) = ctx.flash %}
<div class="alert alert-success">{{ msg }}</div>
{% endif %}

<div class="page-header">
    <h1>{{ agenda_point.title }}</h1>
    <div class="page-actions">
        <a href="/tor/{{ tor_id }}/workflow?tab=agenda_points" class="btn btn-sm">Back to Workflow</a>
    </div>
</div>

<div class="point-paper-grid">

<!-- ── Left column: document content ── -->
<div class="point-paper-body">

    {% if !agenda_point.description.is_empty() %}
    <div class="point-paper-desc">{{ agenda_point.description }}</div>
    {% endif %}

    <!-- COA comparison grid -->
    {% if !coas.is_empty() %}
    <section class="section">
        <div class="section-header">
            <h2>Courses of Action</h2>
        </div>
        <div class="coa-grid">
            {% for coa in coas %}
            <div class="coa-comparison-card">
                <div class="coa-comparison-card-title">{{ coa.title }}</div>

                <!-- Preference bar: find matching opinion summary by coa_id -->
                {% for summary in opinions %}
                {% if summary.coa_id == coa.id %}
                <div class="coa-pref">
                    <div class="coa-pref-bar-track">
                        <div class="coa-pref-bar-fill" style="width: {{ summary.preference_pct }}%"></div>
                    </div>
                    {% if summary.preference_count == 1 %}
                    <span class="coa-pref-count">1 prefer</span>
                    {% else %}
                    <span class="coa-pref-count">{{ summary.preference_count }} prefer</span>
                    {% endif %}
                </div>
                {% endif %}
                {% endfor %}

                <div class="coa-card-footer">
                    <span class="badge badge-muted">
                        {% if coa.coa_type.as_str() == "complex" %}Complex{% else %}Simple{% endif %}
                    </span>
                    {% if ctx.permissions.has("agenda.manage") %}
                    <a href="/tor/{{ tor_id }}/workflow/agenda/{{ agenda_point.id }}/coa/{{ coa.id }}/edit" class="coa-card-link">View details →</a>
                    {% endif %}
                </div>

                {% if !coa.description.is_empty() %}
                <p class="coa-card-desc">{{ coa.description }}</p>
                {% endif %}
            </div>
            {% endfor %}
        </div>
    </section>
    {% endif %}

    <!-- Opinions — collapsible groups -->
    {% if agenda_point.item_type.as_str() == "decision" %}
    <section class="section">
        <div class="section-header">
            <h2>Member Opinions</h2>
        </div>

        {% if opinions.is_empty() %}
        <p class="empty-hint">No opinions recorded yet.</p>
        {% else %}
        {% for summary in opinions %}
        <details class="opinion-group">
            <summary class="opinion-group-summary">
                <span class="opinion-group-summary-left">
                    <span class="opinion-chevron">▾</span>
                    {{ summary.coa_title }}
                </span>
                {% if summary.preference_count == 1 %}
                <span class="badge badge-info">1 member</span>
                {% else %}
                <span class="badge badge-info">{{ summary.preference_count }} members</span>
                {% endif %}
            </summary>
            <div class="opinion-group-body">
                {% for opinion in summary.opinions %}
                <div class="opinion-item">
                    <span class="opinion-member">{{ opinion.recorded_by_name }}</span>
                    {% if !opinion.commentary.is_empty() %}
                    <span class="opinion-commentary">{{ opinion.commentary }}</span>
                    {% else %}
                    <span class="opinion-commentary" style="opacity:0.4">—</span>
                    {% endif %}
                    <span class="opinion-date">{{ opinion.created_date }}</span>
                </div>
                {% endfor %}
            </div>
        </details>
        {% endfor %}
        {% endif %}
    </section>
    {% endif %}

</div><!-- end .point-paper-body -->

<!-- ── Right column: sticky sidebar ── -->
<div class="point-paper-sidebar">
    <div class="point-paper-meta">
        <div class="point-paper-meta-row">
            <span class="point-paper-meta-label">Status</span>
            <span class="point-paper-meta-value">
                {% if agenda_point.status.as_str() == "draft" %}
                <span class="badge badge-muted">Draft</span>
                {% else if agenda_point.status.as_str() == "scheduled" %}
                <span class="badge badge-info">Scheduled</span>
                {% else if agenda_point.status.as_str() == "presented" %}
                <span class="badge badge-primary">Presented</span>
                {% else if agenda_point.status.as_str() == "decided" %}
                <span class="badge badge-success">Decided</span>
                {% else %}
                <span class="badge badge-muted">{{ agenda_point.status }}</span>
                {% endif %}
            </span>
        </div>

        <div class="point-paper-meta-row">
            <span class="point-paper-meta-label">Type</span>
            <span class="point-paper-meta-value">
                {% if agenda_point.item_type.as_str() == "decision" %}
                <span class="badge badge-warning">Decision</span>
                {% else %}
                <span class="badge badge-info">Informative</span>
                {% endif %}
            </span>
        </div>

        <div class="point-paper-meta-row">
            <span class="point-paper-meta-label">Date</span>
            <span class="point-paper-meta-value">{{ agenda_point.scheduled_date }}</span>
        </div>

        <div class="point-paper-meta-row">
            <span class="point-paper-meta-label">Time</span>
            <span class="point-paper-meta-value">{{ agenda_point.time_allocation_minutes }} min</span>
        </div>

        {% if !agenda_point.presenter.is_empty() %}
        <div class="point-paper-meta-row">
            <span class="point-paper-meta-label">Presenter</span>
            <span class="point-paper-meta-value">{{ agenda_point.presenter }}</span>
        </div>
        {% endif %}

        {% if !agenda_point.priority.is_empty() %}
        {% if agenda_point.priority.as_str() != "normal" %}
        <div class="point-paper-meta-row">
            <span class="point-paper-meta-label">Priority</span>
            <span class="point-paper-meta-value">
                {% if agenda_point.priority.as_str() == "urgent" %}
                <span class="badge badge-danger">Urgent</span>
                {% else %}
                <span class="badge badge-warning">High</span>
                {% endif %}
            </span>
        </div>
        {% endif %}
        {% endif %}

        {% if !agenda_point.pre_read_url.is_empty() %}
        <div class="point-paper-meta-row">
            <span class="point-paper-meta-label">Pre-Read</span>
            <span class="point-paper-meta-value">
                <a href="{{ agenda_point.pre_read_url }}" target="_blank" rel="noopener">Open →</a>
            </span>
        </div>
        {% endif %}
    </div>

    <!-- Actions section -->
    <div class="point-paper-divider"></div>
    <div class="point-paper-actions">
        <div class="point-paper-actions-label">Actions</div>

        {% if !available_transitions.is_empty() %}
        {% if ctx.permissions.has("agenda.manage") %}
        {% for transition in available_transitions %}
        <form method="post" action="/tor/{{ tor_id }}/workflow/agenda/{{ agenda_point.id }}/transition">
            <input type="hidden" name="csrf_token" value="{{ ctx.csrf_token }}">
            <input type="hidden" name="to_status" value="{{ transition.to_status_code }}">
            <button type="submit" class="btn btn-sm btn-secondary" style="width:100%">
                {{ transition.transition_label }}
            </button>
        </form>
        {% endfor %}
        {% endif %}
        {% endif %}

        {% if agenda_point.item_type.as_str() == "decision" %}
        {% if ctx.permissions.has("agenda.manage") %}
        <a href="/tor/{{ tor_id }}/workflow/agenda/{{ agenda_point.id }}/coa/new" class="btn btn-sm" style="width:100%;text-align:center">Manage COAs</a>
        {% endif %}
        {% if ctx.permissions.has("agenda.participate") %}
        <a href="/tor/{{ tor_id }}/workflow/agenda/{{ agenda_point.id }}/input" class="btn btn-sm btn-primary" style="width:100%;text-align:center">Record Opinion</a>
        {% endif %}
        {% if ctx.permissions.has("agenda.decide") %}
        {% if agenda_point.status.as_str() != "decided" %}
        <a href="/tor/{{ tor_id }}/workflow/agenda/{{ agenda_point.id }}/decide" class="btn btn-sm btn-primary" style="width:100%;text-align:center">Finalize Decision</a>
        {% endif %}
        {% endif %}
        {% endif %}

        {% if available_transitions.is_empty() %}
        {% if agenda_point.status.as_str() == "decided" %}
        <p class="empty-hint" style="font-size:0.8125rem;margin:0">Decision recorded.</p>
        {% else %}
        <p class="empty-hint" style="font-size:0.8125rem;margin:0">No actions available.</p>
        {% endif %}
        {% endif %}
    </div>
</div><!-- end .point-paper-sidebar -->

</div><!-- end .point-paper-grid -->
{% endblock %}
```

**Step 2: Verify it compiles**

```bash
cargo check 2>&1 | tail -5
```

Expected: `Finished` — Askama compiles templates at build time.

**Step 3: Run tests**

```bash
cargo test 2>&1 | tail -5
```

Expected: all tests pass.

**Step 4: Visual smoke test (optional but recommended)**

```bash
APP_ENV=staging cargo run &
sleep 3
curl -s http://localhost:8080 -o /dev/null -w "%{http_code}"
```

Expected: `200` (redirect to login). Log in as admin / admin123 and navigate to an IGB agenda point.

**Step 5: Commit**

```bash
git add templates/agenda/detail.html
git commit -m "feat(template): redesign agenda detail as two-column point paper layout"
```

---

## Task 4: Fix broken URLs in `templates/agenda/decision_form.html`

**Prompt Contract:**
- GOAL: Fix three broken `/agenda-points/` URL prefixes in `decision_form.html` to use `/workflow/agenda/`
- CONSTRAINTS: Askama template changes only. No Rust changes. Three locations (line 17, line 82, line 104).
- FORMAT: One Edit per broken URL, then cargo check, then commit
- FAILURE CONDITIONS: Any URL still uses `/agenda-points/`; `cargo check` fails

**Files:**
- Modify: `templates/agenda/decision_form.html`

**Step 1: Fix line 17 — Back link**

Find:
```html
<a href="/tor/{{ tor_id }}/agenda-points/{{ agenda_point.id }}" class="btn btn-sm">Back to Agenda Point</a>
```

Replace with:
```html
<a href="/tor/{{ tor_id }}/workflow/agenda/{{ agenda_point.id }}" class="btn btn-sm">Back to Agenda Point</a>
```

**Step 2: Fix line 82 — Form action**

Find:
```html
<form method="post" action="/tor/{{ tor_id }}/agenda-points/{{ agenda_point.id }}/decision" class="form-card">
```

Replace with:
```html
<form method="post" action="/tor/{{ tor_id }}/workflow/agenda/{{ agenda_point.id }}/decide" class="form-card">
```

**Step 3: Fix line 104 — Cancel link**

Find:
```html
<a href="/tor/{{ tor_id }}/agenda-points/{{ agenda_point.id }}" class="btn">Cancel</a>
```

Replace with:
```html
<a href="/tor/{{ tor_id }}/workflow/agenda/{{ agenda_point.id }}" class="btn">Cancel</a>
```

**Step 4: Verify no remaining broken URLs**

```bash
grep -n "agenda-points" templates/agenda/decision_form.html
```

Expected: no output.

**Step 5: Compile check**

```bash
cargo check 2>&1 | tail -3
```

Expected: `Finished`.

**Step 6: Commit**

```bash
git add templates/agenda/decision_form.html
git commit -m "fix(template): correct broken URLs in decision_form.html"
```

---

## Final verification

```bash
cargo test 2>&1 | tail -5
git log --oneline -5
```

Expected: all tests pass, 4 new commits visible.

---

## Scope notes

**Not included in this plan (future work):**
- "✓ Selected" highlight on the winning COA card — requires `selected_coa_id` on `AgendaPointDetail` (model query change, separate task)
- `decision_form.html` opinion percentage bars — the decision form has its own opinion rendering; bar would need same `preference_pct` computation
- Mobile responsive breakpoint for `.point-paper-grid` — add `@media (max-width: 768px) { .point-paper-grid { grid-template-columns: 1fr; } }` as a quick follow-up
