# Point Paper Detail Page — Design

**Date:** 2026-02-21
**Scope:** `templates/agenda/detail.html`, `static/css/style.css`, `src/models/opinion/types.rs`, `src/handlers/agenda_handlers.rs`, `src/handlers/opinion_handlers.rs`
**Bonus fix:** `templates/agenda/decision_form.html` broken URLs

---

## Goal

Transform the agenda point detail page from a generic flat-stack entity view into a purpose-built "point paper" — a formal briefing document for governance decision-making. Serves both roles equally: decision makers scanning options and managers recording data.

---

## Layout

**Two-column `.detail-grid`** (`grid-template-columns: 1fr 320px`).

### Left column — document content

1. Flash alert (if any)
2. Page header — `<h1>` title + "Back to Workflow" button
3. Description prose block — if non-empty: white surface card with a `4px solid var(--accent)` left border. **Not** a detail row — this is body content.
4. COA comparison grid — `display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 1rem`
5. Opinions section — collapsible groups using `<details>`/`<summary>`

### Right column — sticky sidebar

Single `.point-paper-sidebar` card. `position: sticky; top: 80px; align-self: start`.

Top half: compact metadata rows:
- Status badge
- Type badge
- Scheduled Date
- Time Allocation
- Presenter (only if non-empty)
- Priority (only if non-normal)
- Pre-Read link (only if non-empty)

Divider line.

Bottom half: action buttons:
- Workflow transition forms (for `agenda.manage`)
- "Finalize Decision" button (if `agenda.decide` + status != `decided`)
- "Manage COAs" link (if `agenda.manage` + type == `decision`)
- "Record Opinion" link (if `agenda.participate` + type == `decision`)

---

## COA Comparison Cards

CSS class: `.coa-comparison-card`

```
┌─────────────────────────────────────┐
│ Adopt Azure Cloud Platform          │  ← display font
│ ████████░░░░░░░░  3 prefer          │  ← 4px bar + count badge
│ [Simple]                            │  ← type chip
│                                     │
│ Migrate primary IT infrastructure   │  ← body text, muted
│ to Microsoft Azure...               │
│                            View →   │  ← only if agenda.manage
└─────────────────────────────────────┘
```

**Decided state:** If agenda point status is `decided` and this COA matches the decision (COA id = `agenda_point.selected_coa_id` — see note below), add `border-left: 3px solid var(--success)` + a "✓ Selected" chip.

**Preference bar:** A `4px` height bar (`--pref-w` custom property set via inline style). Server-side computed `preference_pct: u32` on `OpinionSummary`. In the template, match COA by `summary.coa_id == coa.id` via nested loop.

**Note on decided state:** `AgendaPointDetail` struct needs a `selected_coa_id: Option<i64>` field to support the "selected" card highlight. This is a small model extension (check if field already exists; if not, query from entity_properties or decision relation).

---

## Opinions — Collapsible Groups

```html
<details class="opinion-group">
  <summary class="opinion-group-header">
    Adopt Azure Cloud Platform
    <span class="badge badge-info">3 prefer</span>
    <span class="chevron">▾</span>
  </summary>
  <ul class="opinion-list">
    <li>
      <strong>Alice Chen</strong>
      <em class="opinion-commentary">Azure AD integration is critical for us.</em>
      <small class="opinion-date">2026-03-10</small>
    </li>
  </ul>
</details>
```

- Default: collapsed (`<details>` without `open`)
- Each member: name (bold) · commentary (italic muted) · date (small, right-aligned)
- Empty state: "No opinions recorded yet." muted hint (outside the details element)

---

## CSS Changes

New classes to add to `static/css/style.css`:

```css
/* --- Point Paper Layout --- */
.point-paper-sidebar { /* sticky sidebar card */ }
.point-paper-meta-row { /* compact metadata row */ }
.point-paper-divider { /* horizontal rule between meta and actions */ }
.point-paper-actions { /* action buttons section in sidebar */ }

/* --- COA Comparison Grid --- */
.coa-grid { /* grid container */ }
.coa-comparison-card { /* individual COA card */ }
.coa-comparison-card--selected { /* green border state */ }
.coa-pref-bar { /* preference bar container */ }
.coa-pref-fill { /* filled portion, width set via custom property */ }
.coa-pref-count { /* "N prefer" label */ }

/* --- Description Block --- */
.point-paper-desc { /* accent left border prose block */ }

/* --- Opinions --- */
.opinion-group { /* details element */ }
.opinion-group-header { /* summary element */ }
.opinion-list { /* list of individual opinions */ }
.opinion-item { /* single opinion row */ }
```

~60 lines total.

---

## Data Model Changes

### `src/models/opinion/types.rs`
Add field to `OpinionSummary`:
```rust
pub preference_pct: u32,  // 0–100, computed from total opinions
```

### `src/handlers/agenda_handlers.rs`
After building `opinions` vec, compute total and set percentages:
```rust
let total_pct: i32 = opinions.iter().map(|s| s.preference_count).sum();
for summary in &mut opinions {
    summary.preference_pct = if total_pct > 0 {
        (summary.preference_count * 100 / total_pct) as u32
    } else { 0 };
}
```

### `src/handlers/opinion_handlers.rs`
Two `OpinionSummary {}` construction sites: add `preference_pct: 0` (these views don't need the bar; the decision_form computes percentages separately if needed).

### `AgendaPointDetail` — check for `selected_coa_id`
Check `src/models/agenda_point/types.rs` for `selected_coa_id: Option<i64>`. If absent, check if the query already fetches it. Add if missing (query from entity_properties where key = 'selected_coa_id').

---

## Bonus Fix — `templates/agenda/decision_form.html`

Three broken URLs still using `/agenda-points/` prefix (lines 17, 82, 104). Fix to `/workflow/agenda/` pattern matching the actual routes.

---

## Implementation Order

1. CSS classes (`style.css`) — no Rust, no compile step
2. `OpinionSummary.preference_pct` struct change + handler updates
3. Check `AgendaPointDetail.selected_coa_id` — add if missing
4. Template redesign (`agenda/detail.html`)
5. Fix `decision_form.html` broken URLs

Each task uses a `/prompt-contracts` spec before touching code.
