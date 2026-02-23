# TD.2 Template Partial Extraction — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extract inline `<script>` blocks from 6 oversized templates into same-directory partials, then deduplicate shared graph toolkit and dynamic table editor patterns.

**Architecture:** Phase 1 does pure cut-and-paste extraction (one commit per template, zero logic changes). Phase 2 extracts shared utilities. Each phase is independently shippable. Verification is `cargo check` (Askama compiles templates at build time).

**Tech Stack:** Askama 0.14 templates, inline JavaScript, PostCSS for CSS extraction.

**Design doc:** `docs/plans/2026-02-23-template-partial-extraction-design.md`

---

## Phase 1: Pure Extraction (one commit per template)

### Task 1: Extract tor/outlook.html JS

**Files:**
- Modify: `templates/tor/outlook.html` (lines 44–585 are `<script>...</script>`)
- Create: `templates/tor/partials/outlook_js.html`

**Step 1: Create the JS partial**

Copy lines 44–585 from `templates/tor/outlook.html` into `templates/tor/partials/outlook_js.html`. The file should contain the entire `<script>...</script>` block including the opening and closing tags.

**Step 2: Replace the script block in the parent**

In `templates/tor/outlook.html`, replace lines 44–585 (the entire `<script>...</script>` block) with:

```html
{% include "tor/partials/outlook_js.html" %}
```

The parent should now be ~45 lines.

**Step 3: Verify**

```bash
cargo check 2>&1 | tail -5
```

Expected: `Finished` (no errors). Askama validates all includes at compile time.

**Step 4: Commit**

```bash
git add templates/tor/outlook.html templates/tor/partials/outlook_js.html
git commit -m "refactor(tor): extract outlook calendar JS to partial (541 lines)"
```

---

### Task 2: Extract ontology/graph.html JS

**Files:**
- Modify: `templates/ontology/graph.html` (lines 66–498 are `<script>...</script>`, line 65 is CDN import)
- Create: `templates/ontology/partials/graph_js.html`

**Step 1: Create the JS partial**

Copy lines 65–498 from `templates/ontology/graph.html` into `templates/ontology/partials/graph_js.html`. Include BOTH the D3 CDN `<script src="...">` tag (line 65) and the inline `<script>...</script>` block (lines 66–498).

**Step 2: Replace in parent**

In `templates/ontology/graph.html`, replace lines 65–498 with:

```html
{% include "ontology/partials/graph_js.html" %}
```

Parent should now be ~65 lines.

**Step 3: Verify**

```bash
cargo check 2>&1 | tail -5
```

**Step 4: Commit**

```bash
git add templates/ontology/graph.html templates/ontology/partials/graph_js.html
git commit -m "refactor(ontology): extract graph JS to partial (433 lines)"
```

---

### Task 3: Extract governance/map.html JS

**Files:**
- Modify: `templates/governance/map.html` (lines 75–377 are CDN imports + `<script>...</script>`)
- Create: `templates/governance/partials/map_js.html` (directory needs creating)

**Step 1: Create partials directory**

```bash
mkdir -p templates/governance/partials
```

**Step 2: Create the JS partial**

Copy lines 75–377 from `templates/governance/map.html` into `templates/governance/partials/map_js.html`. Include the D3 + dagre CDN `<script src="...">` tags (lines 75–76) and the inline `<script>...</script>` block (lines 77–377).

**Step 3: Replace in parent**

In `templates/governance/map.html`, replace lines 75–377 with:

```html
{% include "governance/partials/map_js.html" %}
```

Parent should now be ~76 lines.

**Step 4: Verify**

```bash
cargo check 2>&1 | tail -5
```

**Step 5: Commit**

```bash
git add templates/governance/map.html templates/governance/partials/map_js.html
git commit -m "refactor(governance): extract map JS to partial (302 lines)"
```

---

### Task 4: Extract minutes/view.html JS

**Files:**
- Modify: `templates/minutes/view.html` (lines 223–399 are `<script>...</script>`)
- Create: `templates/minutes/partials/view_js.html` (directory needs creating)

**Step 1: Create partials directory**

```bash
mkdir -p templates/minutes/partials
```

**Step 2: Create the JS partial**

Copy lines 223–399 from `templates/minutes/view.html` into `templates/minutes/partials/view_js.html`. Include the `<script>...</script>` tags.

**Step 3: Replace in parent**

Replace lines 223–399 with:

```html
{% include "minutes/partials/view_js.html" %}
```

Parent should now be ~223 lines.

**Step 4: Verify**

```bash
cargo check 2>&1 | tail -5
```

**Step 5: Commit**

```bash
git add templates/minutes/view.html templates/minutes/partials/view_js.html
git commit -m "refactor(minutes): extract view JS to partial (176 lines)"
```

---

### Task 5: Extract meetings/detail.html JS

**Files:**
- Modify: `templates/meetings/detail.html` (lines 330–397 are `<script>...</script>`)
- Create: `templates/meetings/partials/detail_js.html` (directory needs creating)

**Step 1: Create partials directory**

```bash
mkdir -p templates/meetings/partials
```

**Step 2: Create the JS partial**

Copy lines 330–397 from `templates/meetings/detail.html` into `templates/meetings/partials/detail_js.html`. Include the `<script>...</script>` tags.

**Step 3: Replace in parent**

Replace lines 330–397 with:

```html
{% include "meetings/partials/detail_js.html" %}
```

Parent should now be ~331 lines. This is still above 300, but the remaining HTML is all semantic markup (meeting info card, agenda points, protocol steps, minutes, roll call) — no further extraction makes sense.

**Step 4: Verify**

```bash
cargo check 2>&1 | tail -5
```

**Step 5: Commit**

```bash
git add templates/meetings/detail.html templates/meetings/partials/detail_js.html
git commit -m "refactor(meetings): extract detail JS to partial (67 lines)"
```

---

### Task 6: Extract roles/assignment.html JS + CSS

**Files:**
- Modify: `templates/roles/assignment.html` (lines 167–313 are `<style>`, lines 316–395 are `<script>`)
- Create: `templates/roles/partials/assignment_js.html` (directory needs creating)
- Create: `static/css/pages/role-assignment.css`
- Modify: `static/css/index.css` (add import)

**Step 1: Create partials directory**

```bash
mkdir -p templates/roles/partials
```

**Step 2: Extract JS to partial**

Copy lines 316–395 from `templates/roles/assignment.html` into `templates/roles/partials/assignment_js.html`. Include the `<script>...</script>` tags.

Replace lines 316–395 in the parent with:

```html
{% include "roles/partials/assignment_js.html" %}
```

**Step 3: Extract CSS to external file**

Copy lines 168–312 (the CSS rules inside the `<style>` tags, NOT the tags themselves) from `templates/roles/assignment.html` into `static/css/pages/role-assignment.css`.

Then remove the entire `<style>...</style>` block (lines 167–313) from the parent template.

**Step 4: Add CSS import to index.css**

In `static/css/index.css`, add after the `role-permissions.css` import:

```css
@import "pages/role-assignment.css";
```

**Step 5: Rebuild CSS**

```bash
npm run css:build
```

**Step 6: Verify**

```bash
cargo check 2>&1 | tail -5
```

Parent should now be ~171 lines.

**Step 7: Commit**

```bash
git add templates/roles/assignment.html templates/roles/partials/assignment_js.html static/css/pages/role-assignment.css static/css/index.css
git commit -m "refactor(roles): extract assignment JS + CSS to partial and external file"
```

---

## Phase 2: Shared Utility Deduplication

### Task 7: Extract graph toolkit shared partial

**Files:**
- Create: `templates/partials/graph_toolkit_js.html`
- Modify: `templates/ontology/partials/graph_js.html`
- Modify: `templates/governance/partials/map_js.html`

**Context:** Both graph JS partials have near-identical code for:
- Zoom/fit bounding-box calculation (~25 lines)
- Toolbar button event wiring (fit, zoom-in, zoom-out, reset) (~20 lines)
- Keyboard shortcuts (F, +, -, 0) (~15 lines)

**Step 1: Read both graph partials to identify the shared code**

Read `templates/ontology/partials/graph_js.html` and `templates/governance/partials/map_js.html`. Identify the zoom/fit function, toolbar handlers, and keyboard shortcut blocks in each.

**Step 2: Create the shared toolkit**

Create `templates/partials/graph_toolkit_js.html` containing a `<script>` block that defines:

```javascript
function graphToolkit(config) {
  // config: { svg, zoomBehavior, getNodePositions, fitPadding }

  function fitToView() {
    // Bounding box calculation from node positions
    // Scale + translate to fit viewport with padding
    // Apply via zoomBehavior.transform
  }

  function setupToolbar(toolbarContainer) {
    // Wire fit, zoom-in, zoom-out, reset buttons
  }

  function setupKeyboardShortcuts() {
    // F = fit, + = zoom in, - = zoom out, 0 = reset
  }

  return { fitToView, setupToolbar, setupKeyboardShortcuts };
}
```

**Step 3: Update both graph partials to use the toolkit**

In each graph JS partial:
1. Add `{% include "partials/graph_toolkit_js.html" %}` BEFORE the main `<script>` block
2. Replace the duplicated zoom/fit function with a call to `graphToolkit({...})`
3. Replace toolbar event handlers with `toolkit.setupToolbar(toolbar)`
4. Replace keyboard shortcuts with `toolkit.setupKeyboardShortcuts()`

**Step 4: Verify**

```bash
cargo check 2>&1 | tail -5
```

**Step 5: Commit**

```bash
git add templates/partials/graph_toolkit_js.html templates/ontology/partials/graph_js.html templates/governance/partials/map_js.html
git commit -m "refactor: extract shared graph toolkit (zoom, fit, toolbar, shortcuts)"
```

---

### Task 8: Extract dynamic table editor shared partial

**Files:**
- Create: `templates/partials/dynamic_table_js.html`
- Modify: `templates/minutes/partials/view_js.html`
- Modify: `templates/meetings/partials/detail_js.html`

**Context:** Three table-editing modules (Attendance, Action Items, Roll Call) share identical lifecycle:
1. Load JSON from `<script type="application/json" id="...">`
2. Build DOM row from data object with field-specific rendering
3. Add-row button creates empty row
4. Delete-row button removes row from DOM
5. On form submit, serialize all rows to JSON into hidden `<input>`
6. Permission detection via `!!document.getElementById('add-btn-id')`

**Step 1: Read both partials to map the shared pattern**

Read `templates/minutes/partials/view_js.html` and `templates/meetings/partials/detail_js.html`. Map the common lifecycle code.

**Step 2: Create the shared module**

Create `templates/partials/dynamic_table_js.html` containing:

```javascript
function createDynamicTable(config) {
  // config: {
  //   tableBodyId: 'attendance-body',
  //   dataId: 'attendance-data',
  //   addBtnId: 'add-attendance-btn',
  //   saveBtnId: 'save-attendance-btn',
  //   hiddenInputId: 'structured_attendance',
  //   canEditDetectorId: 'add-attendance-btn',
  //   fields: [
  //     { name: 'name', type: 'text', label: 'Name' },
  //     { name: 'status', type: 'select', options: ['Present','Absent','Excused'] },
  //   ],
  //   makeRow: function(item, canEdit) { /* field-specific rendering */ }
  // }

  const data = JSON.parse(
    document.getElementById(config.dataId)?.textContent || '[]'
  );
  const canEdit = !!document.getElementById(config.canEditDetectorId);
  const tbody = document.getElementById(config.tableBodyId);

  // Load existing data
  data.forEach(item => tbody.appendChild(config.makeRow(item, canEdit)));

  // Add row handler
  if (canEdit) {
    const addBtn = document.getElementById(config.addBtnId);
    if (addBtn) addBtn.addEventListener('click', () => {
      tbody.appendChild(config.makeRow({}, canEdit));
    });
  }

  // Serialize on save
  const saveBtn = document.getElementById(config.saveBtnId);
  if (saveBtn) {
    saveBtn.closest('form').addEventListener('submit', () => {
      const rows = [...tbody.querySelectorAll('tr')];
      const items = rows.map(tr => {
        const obj = {};
        config.fields.forEach(f => {
          const el = tr.querySelector(`[data-field="${f.name}"]`);
          if (el) obj[f.name] = el.value || el.textContent;
        });
        return obj;
      }).filter(obj => Object.values(obj).some(v => v));
      document.getElementById(config.hiddenInputId).value = JSON.stringify(items);
    });
  }
}
```

**Step 3: Update minutes JS partial**

Replace the duplicated Attendance and Action Items modules with calls to `createDynamicTable({...})`, keeping only the field-specific `makeRow` function for each.

**Step 4: Update meetings JS partial**

Replace the Roll Call module with a `createDynamicTable({...})` call.

**Step 5: Verify**

```bash
cargo check 2>&1 | tail -5
```

**Step 6: Commit**

```bash
git add templates/partials/dynamic_table_js.html templates/minutes/partials/view_js.html templates/meetings/partials/detail_js.html
git commit -m "refactor: extract shared dynamic table editor (attendance, action items, roll call)"
```

---

## Phase 3: Finalize

### Task 9: Verify and update backlog

**Step 1: Check all templates are under threshold**

```bash
for f in templates/tor/outlook.html templates/ontology/graph.html templates/governance/map.html templates/minutes/view.html templates/meetings/detail.html templates/roles/assignment.html; do
  echo "$(wc -l < "$f") $f"
done
```

Expected: All under 335 lines (meetings/detail.html may be ~331 due to rich HTML).

**Step 2: Verify CSS build**

```bash
npm run css:build && npm run css:verify
```

**Step 3: Update backlog**

Mark TD.2 as complete in `docs/BACKLOG.md`.

**Step 4: Commit**

```bash
git add docs/BACKLOG.md
git commit -m "docs: mark TD.2 template partial extraction as complete"
```
