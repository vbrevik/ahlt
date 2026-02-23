# TD.2 Template Partial Extraction — Design

**Date**: 2026-02-23
**Status**: Approved
**Approach**: JS partial per template + shared utility deduplication

## Problem

6 templates exceed the 300-line threshold, primarily due to inline `<script>` blocks. The 7th (`data_manager_js.html`) is already a pure-JS partial and needs no work.

| Template | Total | JS Lines | HTML Lines | JS % |
|----------|-------|----------|------------|------|
| tor/outlook.html | 586 | 541 | 45 | 92% |
| ontology/graph.html | 499 | 432 | 67 | 87% |
| governance/map.html | 378 | 300 | 78 | 79% |
| minutes/view.html | 399 | 175 | 224 | 44% |
| meetings/detail.html | 398 | 67 | 331 | 17% |
| roles/assignment.html | 397 | 79+147css | 171 | 57% |

## Extraction Map

Each template's `<script>` block moves to a same-directory partial following the existing `data_manager_js.html` convention. Parent templates replace the block with `{% include %}`.

| Template | Partial Created | Parent Result |
|----------|----------------|---------------|
| tor/outlook.html | tor/partials/outlook_js.html | ~45 lines |
| ontology/graph.html | ontology/partials/graph_js.html | ~67 lines |
| governance/map.html | governance/partials/map_js.html | ~78 lines |
| minutes/view.html | minutes/partials/view_js.html | ~224 lines |
| meetings/detail.html | meetings/partials/detail_js.html | ~331 lines |
| roles/assignment.html | roles/partials/assignment_js.html + CSS to external file | ~171 lines |

## Shared Utility Deduplication

### 1. Graph Toolkit (`partials/graph_toolkit_js.html`)

Shared by ontology/graph.html and governance/map.html. Extracts:
- Zoom/fit bounding-box math (~25 lines, near-identical in both)
- Toolbar event wiring: fit(F), zoom-in(+), zoom-out(-), reset(0) (~20 lines)
- Keyboard shortcuts for the same 4 actions (~15 lines)

Each graph template calls the toolkit with its own config (SVG element, node data, zoom behavior). Template-specific partials provide rendering logic (force-directed vs dagre).

### 2. Dynamic Table Editor (`partials/dynamic_table_js.html`)

Shared by minutes/view.html (Attendance + Action Items = 2 instances) and meetings/detail.html (Roll Call = 1 instance). Extracts the repeated lifecycle:
- JSON data loading from `<script type="application/json">`
- Row factory pattern (create DOM row from data object)
- Add/delete row button handlers
- Serialize-to-hidden-input on form submit
- Permission-based read-only mode detection

Each usage passes a config object: `{ tableId, dataId, fields, canEdit }`.

### 3. Assignment CSS → External File

Move 147-line inline `<style>` block from roles/assignment.html to `static/css/pages/role-assignment.css`. Add to `index.css`, rebuild with PostCSS.

## Verification

**Build check**: `cargo check` after each extraction — Askama compiles templates at build time, catching broken includes and missing struct fields.

**Askama constraint**: Included partials share parent struct scope. JS partials use Askama variables like `{{ tor_id }}`, `{{ api_url|safe }}` — these continue to work without Rust struct changes.

**Ordering**: Extract first (pure move, commit per template), then deduplicate shared code (commit per shared utility). This keeps extraction commits clean and independently shippable.

**Behavioral check**: Run with `APP_ENV=staging` and verify calendar views, graph interactions, table editing, role assignment tabs all work correctly.
