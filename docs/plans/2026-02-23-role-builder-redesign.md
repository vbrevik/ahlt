# Role Builder Redesign — Accordion-Based UX

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Redesign the role builder from a cramped 3-column flat-list layout to a 2-column accordion-based layout with permission descriptions and progressive disclosure.

**Architecture:** Pure frontend/template redesign. Add `description` EAV property to permission entities, thread it through the model and template layers, rewrite the builder template to use collapsible accordion groups, replace the CSS with a 2-column layout (main + sticky preview). No backend logic changes — form submission, preview API, validation, and audit logging remain identical.

**Tech Stack:** Askama templates, CSS custom properties (warm earth-tone design system), vanilla JS (no innerHTML — use `createElement`/`textContent`)

---

## Design Reference

See the design decisions section below, then skip to Task 1 for implementation.

### Layout Change
- **Before**: `[240px details] [flex permissions] [200px preview]` (3-column)
- **After**: `[2fr details+permissions] [1fr sticky preview]` (2-column)

### Accordion Behavior
- Groups start collapsed: chevron + group name + checked count badge (`3/4`) + "Select All" toggle
- Click header → CSS max-height transition expands group
- Multiple groups can be open simultaneously
- Groups with checked permissions show accent tint on header
- Expanded rows show: checkbox, label (bold), description (muted), code (mono pill)

### Responsive Breakpoints
- `>1100px`: 2-column (main + sticky preview)
- `768–1100px`: Single column, preview below
- `<768px`: Full stack, accordion max-height removed

---

## Task 1: Add description field to PermissionCheckbox struct

**Files:**
- Modify: `src/models/role/types.rs:34-40`

**Step 1: Add description field to PermissionCheckbox**

In `src/models/role/types.rs`, add `pub description: String` to the `PermissionCheckbox` struct:

```rust
pub struct PermissionCheckbox {
    pub id: i64,
    pub code: String,
    pub label: String,
    pub group_name: String,
    pub description: String,
    pub checked: bool,
}
```

**Step 2: Run cargo check to see what breaks**

Run: `cargo check 2>&1 | head -30`
Expected: Compile errors in `role_builder_handlers.rs` and `queries.rs` where PermissionCheckbox is constructed without the new field.

---

## Task 2: Update queries to include permission description

**Files:**
- Modify: `src/models/role/queries.rs:64-95`
- Modify: `src/models/permission.rs:5-11`

**Step 1: Add description to PermissionCheckboxRow in queries.rs**

In `src/models/role/queries.rs`, update the `PermissionCheckboxRow` struct (line ~64):

```rust
#[derive(sqlx::FromRow)]
struct PermissionCheckboxRow {
    id: i64,
    code: String,
    label: String,
    group_name: String,
    description: String,
    checked: i32,
}
```

**Step 2: Update the SQL query in find_permission_checkboxes**

Add a LEFT JOIN for the description property. The query (line ~75) becomes:

```sql
SELECT p.id, p.name AS code, p.label,
       COALESCE(pg.value, '') AS group_name,
       COALESCE(pd.value, '') AS description,
       CASE WHEN r.id IS NOT NULL THEN 1 ELSE 0 END AS checked
FROM entities p
LEFT JOIN entity_properties pg ON p.id = pg.entity_id AND pg.key = 'group_name'
LEFT JOIN entity_properties pd ON p.id = pd.entity_id AND pd.key = 'description'
LEFT JOIN relations r ON r.source_id = $1 AND r.target_id = p.id
    AND r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission')
WHERE p.entity_type = 'permission'
ORDER BY group_name, p.name
```

**Step 3: Update the row-to-PermissionCheckbox mapping**

In the `.map()` closure (line ~90), add `description: row.description`:

```rust
let perms = rows.into_iter().map(|row| PermissionCheckbox {
    id: row.id,
    code: row.code,
    label: row.label,
    group_name: row.group_name,
    description: row.description,
    checked: row.checked == 1,
}).collect();
```

**Step 4: Add description to PermissionInfo in permission.rs**

In `src/models/permission.rs`, add `description` to the struct and query:

```rust
#[derive(sqlx::FromRow)]
pub struct PermissionInfo {
    pub id: i64,
    pub code: String,
    pub label: String,
    pub group_name: String,
    pub description: String,
}
```

Update the query in `find_all_with_groups`:

```sql
SELECT e.id, e.name AS code, e.label,
       COALESCE(ep.value, 'Other') AS group_name,
       COALESCE(ed.value, '') AS description
FROM entities e
LEFT JOIN entity_properties ep ON e.id = ep.entity_id AND ep.key = 'group_name'
LEFT JOIN entity_properties ed ON e.id = ed.entity_id AND ed.key = 'description'
WHERE e.entity_type = 'permission' AND e.is_active = true
ORDER BY group_name, e.name
```

---

## Task 3: Update handler to pass description through

**Files:**
- Modify: `src/handlers/role_builder_handlers.rs:30-36`

**Step 1: Add description field in wizard_form's PermissionCheckbox construction**

In `wizard_form` (line ~30), add `description: perm.description` to the PermissionCheckbox:

```rust
crate::models::role::PermissionCheckbox {
    id: perm.id,
    code: perm.code,
    label: perm.label,
    group_name: perm.group_name,
    description: perm.description,
    checked: false,
}
```

**Step 2: Verify the edit_form handler works automatically**

The `edit_form` handler uses `find_permission_checkboxes()` which already returns `PermissionCheckbox` — the description field flows through automatically via Task 2 changes.

**Step 3: Run cargo check**

Run: `cargo check 2>&1 | tail -5`
Expected: No errors. The model→handler→template pipeline is now complete for the description field.

**Step 4: Commit data model changes**

```bash
git add src/models/role/types.rs src/models/role/queries.rs src/models/permission.rs src/handlers/role_builder_handlers.rs
git commit -m "feat(role-builder): add description field to permission model pipeline"
```

---

## Task 4: Add description property to seed data

**Files:**
- Modify: `data/seed/ontology.json` (all 46 permission entries)

**Step 1: Add `description` property to every permission entity**

For each permission entity in `data/seed/ontology.json`, add a `"description"` key to the `"properties"` object. The descriptions should concisely explain what the permission grants:

| Permission | Description |
|-----------|-------------|
| `dashboard.view` | Access the main dashboard with stats and activity |
| `users.list` | View the list of all user accounts |
| `users.create` | Create new user accounts |
| `users.edit` | Edit existing user profiles and account details |
| `users.delete` | Permanently remove user accounts |
| `roles.manage` | Create, edit, and delete roles and their permissions |
| `roles.assign` | Assign and remove roles from users |
| `settings.manage` | View and modify application settings |
| `audit.view` | View the audit log of all system actions |
| `tor.list` | View list of all Terms of Reference |
| `tor.create` | Create new Terms of Reference |
| `tor.edit` | Modify ToR structure, functions, and settings |
| `tor.manage_members` | Add and remove members from ToR functions |
| `suggestion.view` | View suggestions submitted to your ToRs |
| `suggestion.create` | Submit new suggestions to ToRs you belong to |
| `suggestion.review` | Accept or reject submitted suggestions |
| `proposal.view` | View proposals in your ToRs |
| `proposal.create` | Draft new proposals from accepted suggestions |
| `proposal.submit` | Submit draft proposals for formal review |
| `proposal.edit` | Modify proposals still in draft status |
| `proposal.review` | Move proposals to under-review status |
| `proposal.approve` | Cast final approval or rejection on proposals |
| `agenda.view` | View meeting agendas |
| `agenda.create` | Add new agenda points to meetings |
| `agenda.queue` | Queue approved proposals onto meeting agendas |
| `agenda.manage` | Change agenda point status and ordering |
| `agenda.participate` | Participate in meetings (vote, discuss) |
| `agenda.decide` | Record final decisions on agenda points |
| `coa.create` | Create new courses of action from decisions |
| `coa.edit` | Modify existing courses of action |
| `workflow.manage` | Configure workflow statuses and transitions |
| `warnings.view` | View system warnings and notifications |
| `minutes.generate` | Generate meeting minutes from agenda data |
| `minutes.edit` | Edit generated meeting minutes |
| `minutes.approve` | Approve meeting minutes as official record |
| `minutes.view` | View meeting minutes |
| `meetings.view` | View meeting details and schedules |
| `document.list` | View the document library |
| `document.create` | Upload or create new documents |
| `document.view` | Open and read document details |
| `document.edit` | Modify existing documents |
| `document.delete` | Permanently remove documents |
| `entities.list` | List entities via the REST API |
| `entities.create` | Create entities via the REST API |
| `entities.edit` | Modify entities via the REST API |
| `entities.delete` | Remove entities via the REST API |

Example JSON entry after change:

```json
{
  "entity_type": "permission",
  "name": "users.create",
  "label": "Create Users",
  "sort_order": 0,
  "properties": {
    "group_name": "Users",
    "description": "Create new user accounts"
  }
}
```

**Step 2: Commit seed data**

```bash
git add -f data/seed/ontology.json
git commit -m "feat(role-builder): add description property to all permission entities"
```

**Note:** Seed uses `ConflictMode::Skip` — existing DBs won't pick up new properties. Drop and recreate the database to see descriptions: `dropdb ahlt_staging && createdb ahlt_staging && APP_ENV=staging cargo run`

---

## Task 5: Rewrite the CSS for 2-column accordion layout

**Files:**
- Rewrite: `static/css/pages/role-builder.css`

**Step 1: Replace role-builder.css entirely**

Replace `static/css/pages/role-builder.css` with the new 2-column accordion layout CSS. Key changes:

- `.rb-layout`: `display: grid; grid-template-columns: 2fr 1fr; gap: 1.5rem; align-items: start;`
- `.rb-main`: Stacks role details panel + permissions accordion vertically
- `.rb-preview`: `position: sticky; top: 1rem;` for the preview sidebar
- `.rb-details`: Card with inline name/label fields (2-col subgrid)
- `.rb-accordion`: Container for collapsible permission groups
- `.rb-group__header`: Clickable, with chevron rotation on expand, accent tint when group has selections
- `.rb-group__body`: `max-height: 0; overflow: hidden; transition: max-height 300ms ease;` collapsed by default
- `.rb-group--expanded .rb-group__body`: `max-height` set dynamically by JS
- `.rb-group--has-selected .rb-group__header`: `background: color-mix(in srgb, var(--accent) 6%, var(--surface));`
- `.rb-perm`: Flexbox row with checkbox, label+description stack, code pill
- `.rb-perm__desc`: `font-size: 0.72rem; color: var(--text-muted); margin-top: 0.125rem;`
- Responsive: `@media (max-width: 1100px)` → single column; `@media (max-width: 768px)` → full stack
- Preserve existing design tokens: `var(--surface)`, `var(--border)`, `var(--accent)`, `var(--text-muted)`, `var(--duration)`, `var(--ease)`, etc.
- Chevron: CSS triangle via `border` or a simple `>` rotated with `transform: rotate(90deg)` on expanded

Refer to the existing `role-builder.css` for the design token usage patterns, but the layout and component structure is entirely new.

**Step 2: Verify CSS file parses**

Open the file in the browser dev tools or just check no syntax errors visually.

**Step 3: Commit CSS**

```bash
git add static/css/pages/role-builder.css
git commit -m "feat(role-builder): replace CSS with 2-column accordion layout"
```

---

## Task 6: Rewrite the builder template

**Files:**
- Rewrite: `templates/roles/builder.html`

**Step 1: Rewrite the template**

Key structural changes from the current template:

1. **Page header**: Keep the title but move submit button to bottom of form
2. **2-column grid**: `.rb-layout` > `.rb-main` + `.rb-preview`
3. **Role details**: `.rb-details` card with inline name/label (side by side), description textarea below
4. **Permissions accordion**: `.rb-accordion` with `.rb-group` for each group
5. **Each group header**: Chevron + group name + count badge + select-all toggle
6. **Each permission row**: Checkbox + label/description stack + code pill
7. **Submit at bottom**: Below the accordion, within `.rb-main`
8. **Preview sidebar**: Same as current but in the `.rb-preview` column

Template structure (Askama):

```
{% extends "base.html" %}
{% block title %}...{% endblock %}
{% block nav %}{% include "partials/nav.html" %}{% endblock %}
{% block sidebar %}{% include "partials/sidebar.html" %}{% endblock %}

{% block content %}
  flash message (if any)

  <div class="page-header">
    <h1>New/Edit Role</h1>
    <a href="/roles" class="btn btn-sm">Back to Roles</a>
  </div>

  <form id="role-builder-form" ...>
    <input type="hidden" csrf_token>
    <input type="hidden" permission_ids>
    <input type="hidden" role_id (if editing)>

    <div class="rb-layout">
      <div class="rb-main">
        <!-- Role Details card -->
        <div class="rb-details">
          <div class="rb-details__row">
            <div class="form-group">Name input</div>
            <div class="form-group">Label input</div>
          </div>
          <div class="form-group">Description textarea</div>
        </div>

        <!-- Permissions Accordion -->
        <div class="rb-accordion">
          <div class="rb-accordion__header">
            <h2>Permissions</h2>
            <span class="rb-perm-count" id="perm-count"></span>
          </div>

          {% for group in permission_groups %}
          <div class="rb-group" data-group="{{ group.group_name }}">
            <button type="button" class="rb-group__header">
              <span class="rb-group__chevron"></span>
              <span class="rb-group__name">{{ group.group_name }}</span>
              <span class="rb-group__badge">0/{{ group.permissions.len() }}</span>
              <label class="rb-group__select-all" onclick="event.stopPropagation()">
                <input type="checkbox" class="checkbox-input select-all-checkbox" data-group="{{ group.group_name }}">
                <span class="checkbox-mark"></span>
                All
              </label>
            </button>
            <div class="rb-group__body">
              {% for perm in group.permissions %}
              <label class="rb-perm">
                <span class="rb-perm__check">
                  <input type="checkbox" class="checkbox-input permission-item"
                         data-group="{{ group.group_name }}"
                         value="{{ perm.id }}"
                         {% if perm.checked %}checked{% endif %}>
                  <span class="checkbox-mark"></span>
                </span>
                <span class="rb-perm__info">
                  <span class="rb-perm__label">{{ perm.label }}</span>
                  {% if !perm.description.is_empty() %}
                  <span class="rb-perm__desc">{{ perm.description }}</span>
                  {% endif %}
                </span>
                <code class="rb-perm__code">{{ perm.code }}</code>
              </label>
              {% endfor %}
            </div>
          </div>
          {% endfor %}
        </div>

        <!-- Actions -->
        <div class="rb-actions">
          {% if let Some(r) = role %}
          <form method="post" action="/roles/builder/{{ r.id }}/delete" ...>
            <button type="submit" class="btn btn-danger btn-sm">Delete Role</button>
          </form>
          {% endif %}
          <div class="rb-actions__right">
            <a href="/roles" class="btn btn-sm">Cancel</a>
            <button type="submit" form="role-builder-form" class="btn btn-primary" id="submit-btn" disabled>
              {% if role.is_some() %}Update Role{% else %}Create Role{% endif %}
            </button>
          </div>
        </div>
      </div>

      <!-- Preview Sidebar -->
      <aside class="rb-preview">
        <h2 class="rb-preview__heading">Menu Preview</h2>
        <div class="rb-mock-sidebar" id="mock-sidebar">
          <div id="preview-empty">Select permissions to preview menu access</div>
          <div id="preview-loading" style="display:none">Updating...</div>
        </div>
        <p class="rb-preview-summary" id="preview-summary"></p>
      </aside>
    </div>
  </form>

  <script>
    // Accordion toggle logic
    // Select All logic
    // Permission change handler (updates count, badge, hidden input, preview)
    // Preview fetch logic (same as current, using el() helper)
    // Initialize on load
  </script>
{% endblock %}
```

**Critical JS behaviors to implement:**

1. **Accordion toggle**: Click `.rb-group__header` → toggle `.rb-group--expanded`, set `max-height` to `scrollHeight` or `0`
2. **Badge update**: On permission change, update each group's badge text (`3/4`)
3. **Header tint**: Add/remove `.rb-group--has-selected` based on group having any checked items
4. **Select all**: Same as current, but also update badge + tint
5. **Permission count**: Same as current
6. **Hidden input**: Same as current (JSON array of IDs)
7. **Submit enable/disable**: Same as current
8. **Preview fetch**: Same as current (debounced, using `el()` helper, no innerHTML)
9. **Init**: On load, sync select-all state, update badges, expand groups that have checked permissions (edit mode)

**Security**: No `innerHTML`. Use `createElement`/`textContent`/`appendChild` for all DOM construction. Use `el(tag, cls, text)` helper.

**Step 2: Run cargo build to verify template compiles**

Run: `cargo build 2>&1 | tail -5`
Expected: Compiles successfully. Askama templates compile at build time.

**Step 3: Commit template**

```bash
git add templates/roles/builder.html
git commit -m "feat(role-builder): rewrite template with accordion layout and descriptions"
```

---

## Task 7: Verify and test

**Step 1: Run the full test suite**

Run: `cargo test 2>&1 | tail -10`
Expected: All ~200 tests pass. The role builder tests in `tests/role_builder_test.rs` test model-level behavior (permission checkboxes, uniqueness) — they should still pass because we only added a field to the struct.

**Step 2: Manual testing (requires running server)**

Start the server: `APP_ENV=staging cargo run`

Then verify in browser:
1. Navigate to `/roles/builder` — should show 2-column layout with accordion
2. Click accordion groups — should expand/collapse smoothly
3. Check permissions — badges should update, accent tint should appear
4. Check "Select All" — should toggle all in group
5. Navigate to `/roles/builder/{id}/edit` — should show pre-checked permissions with groups expanded
6. Submit create/update — should work as before
7. Preview panel — should update live as permissions change

**Step 3: Final commit with all changes**

If any fixups were needed during testing, commit them:

```bash
git add -A
git commit -m "fix(role-builder): post-testing adjustments"
```

---

## Summary of All Files

| File | Action | Description |
|------|--------|-------------|
| `src/models/role/types.rs` | Modify | Add `description: String` to `PermissionCheckbox` |
| `src/models/role/queries.rs` | Modify | Add description LEFT JOIN + field mapping |
| `src/models/permission.rs` | Modify | Add description to `PermissionInfo` + query |
| `src/handlers/role_builder_handlers.rs` | Modify | Pass `description` in wizard_form constructor |
| `data/seed/ontology.json` | Modify | Add `description` property to all 46 permissions |
| `static/css/pages/role-builder.css` | Rewrite | 2-column grid, accordion styles, animations |
| `templates/roles/builder.html` | Rewrite | Accordion groups, descriptions, 2-column layout |
