# Menu Builder (Permission Matrix) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a visual "Menu Builder" page that shows a permission matrix — roles as columns, pages/actions as rows — allowing admins to configure what each role can see and do across the entire application from one place.

**Architecture:** A new `/menu-builder` page under the Admin module, gated by `roles.manage` permission. The page renders a matrix table where rows are permissions grouped by page section (Dashboard, Users, Roles, Settings, Audit) and columns are roles. Each cell is a checkbox that toggles a `has_permission` relation. The POST handler diffs submitted checkboxes against current state to add/remove relations. No schema changes — this is a visual layer over the existing EAV permission system.

**Tech Stack:** Rust (Actix-web 4, Askama 0.14), SQLite (rusqlite), existing EAV model (entities + entity_properties + relations tables)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│ GET /menu-builder                                       │
│                                                         │
│ 1. Query all roles (columns)                            │
│ 2. Query all permissions + group_name (rows)            │
│ 3. Query all has_permission relations (checkbox state)  │
│ 4. Pre-compute MatrixCell grid for Askama template      │
│ 5. Render matrix with checkboxes                        │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│ POST /menu-builder                                      │
│                                                         │
│ 1. Parse body: checkboxes named `perm_{role_id}_{pid}`  │
│ 2. Load current has_permission relations                │
│ 3. Diff: submitted vs current                           │
│ 4. INSERT new grants, DELETE revoked grants              │
│ 5. Audit log changes                                    │
│ 6. Redirect with flash message                          │
└─────────────────────────────────────────────────────────┘
```

### Data Flow

```
Database:
  entities (role, permission)
  entity_properties (group_name on permissions)
  relations (has_permission: role → permission)
       ↓
Query Functions (src/models/permission.rs):
  find_all_with_groups() → Vec<PermissionInfo>
  find_all_role_grants() → HashSet<(role_id, perm_id)>
       ↓
Handler (src/handlers/menu_builder_handlers.rs):
  Builds MatrixData { roles, page_groups }
       ↓
Template (templates/menu_builder.html):
  Renders matrix table with checkbox per cell
       ↓
POST Handler:
  Parses perm_{rid}_{pid} checkboxes → diffs → updates relations
```

### Template Matrix Rendering Strategy

Askama can't call `.contains()` on vectors, so we pre-compute the matrix:

```rust
struct MatrixCell {
    role_id: i64,
    permission_id: i64,
    checked: bool,
}

struct PermissionRow {
    permission_id: i64,
    code: String,
    label: String,
    cells: Vec<MatrixCell>, // One per role, same order as roles vec
}

struct PageGroup {
    group_name: String,
    permissions: Vec<PermissionRow>,
}
```

Template just does `{% if cell.checked %}checked{% endif %}` — no method calls needed.

### Form Encoding Strategy

Each checkbox has a unique name: `perm_{role_id}_{permission_id}` with value `"1"`.
Since all names are unique, `serde_urlencoded` works fine (no duplicate key issue).
Parse manually by iterating form pairs and matching the `perm_` prefix pattern.

---

## Task 1: Add permission matrix query functions

**Files:**
- Modify: `src/models/permission.rs`

**Step 1: Add the PermissionInfo struct and find_all_with_groups query**

Add to `src/models/permission.rs`:

```rust
use std::collections::HashSet;

/// Permission info for the matrix display
pub struct PermissionInfo {
    pub id: i64,
    pub code: String,
    pub label: String,
    pub group_name: String,
}

/// Get all permissions with their group_name property, ordered by group then name.
pub fn find_all_with_groups(conn: &Connection) -> rusqlite::Result<Vec<PermissionInfo>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label, COALESCE(ep.value, 'Other') AS group_name \
         FROM entities e \
         LEFT JOIN entity_properties ep ON e.id = ep.entity_id AND ep.key = 'group_name' \
         WHERE e.entity_type = 'permission' AND e.is_active = 1 \
         ORDER BY group_name, e.name"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(PermissionInfo {
            id: row.get(0)?,
            code: row.get(1)?,
            label: row.get(2)?,
            group_name: row.get(3)?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Get all (role_id, permission_id) pairs that have has_permission relations.
pub fn find_all_role_grants(conn: &Connection) -> rusqlite::Result<HashSet<(i64, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT r.source_id, r.target_id \
         FROM relations r \
         WHERE r.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission')"
    )?;
    let pairs = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
    })?.collect::<Result<HashSet<_>, _>>()?;
    Ok(pairs)
}

/// Add a has_permission relation between a role and permission.
pub fn grant_permission(conn: &Connection, role_id: i64, permission_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id) \
         VALUES ((SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission'), ?1, ?2)",
        params![role_id, permission_id],
    )?;
    Ok(())
}

/// Remove a has_permission relation between a role and permission.
pub fn revoke_permission(conn: &Connection, role_id: i64, permission_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM relations WHERE relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission') \
         AND source_id = ?1 AND target_id = ?2",
        params![role_id, permission_id],
    )?;
    Ok(())
}
```

**Step 2: Verify it compiles**

Run: `cargo check 2>&1 | tail -5`
Expected: No errors related to permission.rs

**Step 3: Commit**

```bash
git add src/models/permission.rs
git commit -m "feat(menu-builder): add permission matrix query functions"
```

---

## Task 2: Add template types for the matrix

**Files:**
- Modify: `src/templates_structs.rs`

**Step 1: Add matrix display types and MenuBuilderTemplate**

Add these types and the template struct to `src/templates_structs.rs`:

```rust
// --- Menu Builder types ---

/// A single cell in the permission matrix (one role × one permission)
pub struct MatrixCell {
    pub role_id: i64,
    pub permission_id: i64,
    pub checked: bool,
}

/// One row in the matrix (one permission, with cells for each role)
pub struct PermissionRow {
    pub permission_id: i64,
    pub code: String,
    pub label: String,
    pub cells: Vec<MatrixCell>,
}

/// A group of permission rows under a page section header
pub struct PageGroup {
    pub group_name: String,
    pub permissions: Vec<PermissionRow>,
}

/// Column header data for a role
pub struct RoleColumn {
    pub id: i64,
    pub name: String,
    pub label: String,
}
```

And the template struct:

```rust
#[derive(Template)]
#[template(path = "menu_builder.html")]
pub struct MenuBuilderTemplate {
    pub ctx: PageContext,
    pub roles: Vec<RoleColumn>,
    pub page_groups: Vec<PageGroup>,
    pub col_count: usize,
}
```

Note: `col_count` = `roles.len() + 1` (pre-computed because Askama may not support arithmetic in `colspan`).

**Step 2: Verify it compiles**

Run: `cargo check 2>&1 | tail -5`
Expected: May warn about unused types (that's OK, template file doesn't exist yet)

**Step 3: Commit**

```bash
git add src/templates_structs.rs
git commit -m "feat(menu-builder): add matrix display types and template struct"
```

---

## Task 3: Create the GET handler

**Files:**
- Create: `src/handlers/menu_builder_handlers.rs`
- Modify: `src/handlers/mod.rs`

**Step 1: Create the handler file**

Create `src/handlers/menu_builder_handlers.rs`:

```rust
use actix_session::Session;
use actix_web::{web, HttpResponse};

use crate::auth::session::require_permission;
use crate::db::DbPool;
use crate::errors::{render, AppError};
use crate::models::permission;
use crate::models::role;
use crate::templates_structs::{
    MenuBuilderTemplate, PageContext, RoleColumn, PageGroup, PermissionRow, MatrixCell,
};

/// GET /menu-builder — render the permission matrix
pub async fn index(
    session: Session,
    pool: web::Data<DbPool>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.manage")?;
    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/menu-builder")?;

    // Load all roles (columns)
    let all_roles = role::find_all_display(&conn)?;
    let roles: Vec<RoleColumn> = all_roles
        .iter()
        .map(|r| RoleColumn {
            id: r.id,
            name: r.name.clone(),
            label: r.label.clone(),
        })
        .collect();

    // Load all permissions with group_name (rows)
    let all_perms = permission::find_all_with_groups(&conn)?;

    // Load current grants (role_id, permission_id) pairs
    let grants = permission::find_all_role_grants(&conn)?;

    // Group permissions by group_name and build matrix cells
    let mut groups: Vec<PageGroup> = Vec::new();
    let mut current_group: Option<String> = None;
    let mut current_perms: Vec<PermissionRow> = Vec::new();

    for perm in &all_perms {
        if current_group.as_deref() != Some(&perm.group_name) {
            // Flush previous group
            if let Some(gn) = current_group.take() {
                groups.push(PageGroup {
                    group_name: gn,
                    permissions: std::mem::take(&mut current_perms),
                });
            }
            current_group = Some(perm.group_name.clone());
        }

        let cells: Vec<MatrixCell> = roles
            .iter()
            .map(|r| MatrixCell {
                role_id: r.id,
                permission_id: perm.id,
                checked: grants.contains(&(r.id, perm.id)),
            })
            .collect();

        current_perms.push(PermissionRow {
            permission_id: perm.id,
            code: perm.code.clone(),
            label: perm.label.clone(),
            cells,
        });
    }

    // Flush last group
    if let Some(gn) = current_group {
        groups.push(PageGroup {
            group_name: gn,
            permissions: current_perms,
        });
    }

    let col_count = roles.len() + 1;

    render(MenuBuilderTemplate {
        ctx,
        roles,
        page_groups: groups,
        col_count,
    })
}
```

**Step 2: Register the module in handlers/mod.rs**

Add to `src/handlers/mod.rs`:

```rust
pub mod menu_builder_handlers;
```

**Step 3: Check compilation**

Run: `cargo check 2>&1 | tail -10`

Note: This will fail because:
1. `role::find_all_display` may not exist (need to check — it might have been removed as dead code)
2. Template file doesn't exist yet

If `find_all_display` was removed, add a simple `find_all_for_matrix` query to `src/models/role/queries.rs`:

```rust
/// Get all roles for the permission matrix (id, name, label).
pub fn find_all_for_matrix(conn: &Connection) -> rusqlite::Result<Vec<(i64, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label FROM entities e \
         WHERE e.entity_type = 'role' AND e.is_active = 1 \
         ORDER BY e.sort_order, e.id"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })?.collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
```

Then update the handler to use `role::find_all_for_matrix` and map tuples to RoleColumn.

**Step 4: Commit**

```bash
git add src/handlers/menu_builder_handlers.rs src/handlers/mod.rs
git commit -m "feat(menu-builder): add GET handler for permission matrix"
```

---

## Task 4: Create the template

**Files:**
- Create: `templates/menu_builder.html`

**Step 1: Create the template file**

Create `templates/menu_builder.html`:

```html
{% extends "base.html" %}

{% block title %}Menu Builder — {{ ctx.app_name }}{% endblock %}

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
    <h1>Menu Builder</h1>
</div>

<p class="page-description">Configure what each role can see and do. Check a box to grant a permission to a role. Changes affect sidebar visibility and page element visibility (buttons, actions).</p>

<form method="post" action="/menu-builder" class="matrix-form">
    <input type="hidden" name="csrf_token" value="{{ ctx.csrf_token }}">

    <div class="matrix-wrapper">
        <table class="matrix-table">
            <thead>
                <tr>
                    <th class="matrix-label-col">Permission</th>
                    {% for role in roles %}
                    <th class="matrix-role-col">
                        <div class="role-header">
                            <span class="role-name">{{ role.label }}</span>
                            <code class="role-code">{{ role.name }}</code>
                        </div>
                    </th>
                    {% endfor %}
                </tr>
            </thead>
            <tbody>
                {% for group in page_groups %}
                <tr class="matrix-group-row">
                    <th colspan="{{ col_count }}" class="matrix-group-header">{{ group.group_name }}</th>
                </tr>
                {% for perm in group.permissions %}
                <tr class="matrix-perm-row">
                    <td class="matrix-perm-label">
                        <span class="perm-label">{{ perm.label }}</span>
                        <code class="perm-code">{{ perm.code }}</code>
                    </td>
                    {% for cell in perm.cells %}
                    <td class="matrix-cell">
                        <label class="matrix-checkbox">
                            <input type="checkbox"
                                   name="perm_{{ cell.role_id }}_{{ cell.permission_id }}"
                                   value="1"
                                   {% if cell.checked %}checked{% endif %}>
                            <span class="checkmark"></span>
                        </label>
                    </td>
                    {% endfor %}
                </tr>
                {% endfor %}
                {% endfor %}
            </tbody>
        </table>
    </div>

    <div class="form-actions matrix-actions">
        <button type="submit" class="btn btn-primary">Save Changes</button>
        <a href="/menu-builder" class="btn">Reset</a>
    </div>
</form>

<script>
(function() {
    // Track changes to show unsaved indicator
    const form = document.querySelector('.matrix-form');
    const checkboxes = form.querySelectorAll('input[type="checkbox"]');
    const saveBtn = form.querySelector('.btn-primary');
    let hasChanges = false;

    // Store initial state
    const initialState = {};
    checkboxes.forEach(cb => {
        initialState[cb.name] = cb.checked;
    });

    // Detect changes
    checkboxes.forEach(cb => {
        cb.addEventListener('change', () => {
            hasChanges = false;
            checkboxes.forEach(c => {
                if (c.checked !== initialState[c.name]) {
                    hasChanges = true;
                }
            });
            saveBtn.textContent = hasChanges ? 'Save Changes *' : 'Save Changes';
            saveBtn.classList.toggle('has-changes', hasChanges);
        });
    });

    // Warn on navigate away with unsaved changes
    window.addEventListener('beforeunload', (e) => {
        if (hasChanges) {
            e.preventDefault();
            e.returnValue = '';
        }
    });

    // Column toggle: click role header to toggle all in column
    const roleHeaders = document.querySelectorAll('.role-header');
    roleHeaders.forEach((header, colIndex) => {
        header.style.cursor = 'pointer';
        header.title = 'Click to toggle all permissions for this role';
        header.addEventListener('click', () => {
            const colCheckboxes = [];
            document.querySelectorAll('.matrix-perm-row').forEach(row => {
                const cells = row.querySelectorAll('.matrix-cell input[type="checkbox"]');
                if (cells[colIndex]) colCheckboxes.push(cells[colIndex]);
            });
            const allChecked = colCheckboxes.every(cb => cb.checked);
            colCheckboxes.forEach(cb => {
                cb.checked = !allChecked;
                cb.dispatchEvent(new Event('change'));
            });
        });
    });
})();
</script>
{% endblock %}
```

**Step 2: Verify compilation**

Run: `cargo check 2>&1 | tail -5`
Expected: Clean compilation (template is now referenced by the struct)

**Step 3: Commit**

```bash
git add templates/menu_builder.html
git commit -m "feat(menu-builder): add permission matrix template with change tracking"
```

---

## Task 5: Create the POST handler (save)

**Files:**
- Modify: `src/handlers/menu_builder_handlers.rs`

**Step 1: Add the POST handler**

Add to `src/handlers/menu_builder_handlers.rs`:

```rust
use std::collections::HashSet;
use crate::auth::csrf;

/// POST /menu-builder — save permission matrix changes
pub async fn save(
    session: Session,
    pool: web::Data<DbPool>,
    body: String,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "roles.manage")?;
    let conn = pool.get()?;

    // Validate CSRF
    let params: Vec<(String, String)> = serde_urlencoded::from_str(&body).unwrap_or_default();
    let csrf_token = params.iter()
        .find(|(k, _)| k == "csrf_token")
        .map(|(_, v)| v.clone())
        .unwrap_or_default();
    csrf::validate_csrf(&session, &csrf_token)?;

    // Parse submitted checkbox names: perm_{role_id}_{permission_id}
    let submitted: HashSet<(i64, i64)> = params.iter()
        .filter_map(|(key, _)| {
            let rest = key.strip_prefix("perm_")?;
            let mut parts = rest.splitn(2, '_');
            let role_id = parts.next()?.parse::<i64>().ok()?;
            let perm_id = parts.next()?.parse::<i64>().ok()?;
            Some((role_id, perm_id))
        })
        .collect();

    // Load current grants
    let current = permission::find_all_role_grants(&conn)?;

    // Compute diff
    let to_grant: Vec<_> = submitted.difference(&current).collect();
    let to_revoke: Vec<_> = current.difference(&submitted).collect();

    let changes = to_grant.len() + to_revoke.len();

    // Apply changes
    for (role_id, perm_id) in &to_grant {
        permission::grant_permission(&conn, *role_id, *perm_id)?;
    }
    for (role_id, perm_id) in &to_revoke {
        permission::revoke_permission(&conn, *role_id, *perm_id)?;
    }

    // Audit log
    if changes > 0 {
        let username = crate::auth::session::get_username(&session)
            .unwrap_or_else(|_| "unknown".to_string());
        let summary = format!(
            "{} granted {} permissions, revoked {} permissions",
            username, to_grant.len(), to_revoke.len()
        );
        crate::audit::log_action(
            &conn,
            crate::auth::session::get_user_id(&session),
            "permissions_updated",
            "permission",
            None,
            &summary,
        );
    }

    // Flash message and redirect
    let msg = if changes > 0 {
        format!("Permissions updated ({} granted, {} revoked)", to_grant.len(), to_revoke.len())
    } else {
        "No changes made".to_string()
    };
    session.insert("flash", &msg)
        .map_err(|e| AppError::Session(e.to_string()))?;

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/menu-builder"))
        .finish())
}
```

**Step 2: Verify compilation**

Run: `cargo check 2>&1 | tail -10`

Note: The `audit::log_action` call signature needs to match the existing audit module. Check `src/audit/mod.rs` for the exact signature and adjust accordingly. The function might take different parameters — adapt the call to match.

**Step 3: Commit**

```bash
git add src/handlers/menu_builder_handlers.rs
git commit -m "feat(menu-builder): add POST handler for saving permission matrix"
```

---

## Task 6: Register routes and nav item

**Files:**
- Modify: `src/main.rs`
- Modify: `src/db.rs`

**Step 1: Add routes to main.rs**

In `src/main.rs`, add the menu-builder routes inside the protected scope, after the roles routes:

```rust
// Menu Builder
.route("/menu-builder", web::get().to(handlers::menu_builder_handlers::index))
.route("/menu-builder", web::post().to(handlers::menu_builder_handlers::save))
```

**Step 2: Add nav item to seed_ontology in db.rs**

In `src/db.rs`, inside the `seed_ontology` function, after the audit nav item section, add:

```rust
// Admin → Menu Builder: sidebar child
let nav_admin_menu_builder_id = insert_entity(&conn, "nav_item", "admin.menu_builder", "Menu Builder", 6);
insert_prop(&conn, nav_admin_menu_builder_id, "url", "/menu-builder");
insert_prop(&conn, nav_admin_menu_builder_id, "parent", "admin");

// Menu Builder requires roles.manage permission
insert_relation(&conn, requires_permission_rel_type_id, nav_admin_menu_builder_id, roles_manage_perm_id);
```

Also update the log message entity count at the end:
```rust
log::info!("Seeded ontology: 3 relation types, 2 roles, {} permissions, 8 nav items, 5 settings, 1 admin user", perms.len());
```

**Step 3: For existing databases, add nav item via SQL migration**

Since `seed_ontology` only runs on empty databases, create a manual SQL statement to add the nav item for existing databases. Add a migration function or run this SQL once:

```sql
-- Add Menu Builder nav item (only for existing databases)
INSERT OR IGNORE INTO entities (entity_type, name, label, sort_order, is_active)
VALUES ('nav_item', 'admin.menu_builder', 'Menu Builder', 6, 1);

INSERT OR REPLACE INTO entity_properties (entity_id, key, value)
SELECT e.id, 'url', '/menu-builder'
FROM entities e WHERE e.entity_type = 'nav_item' AND e.name = 'admin.menu_builder';

INSERT OR REPLACE INTO entity_properties (entity_id, key, value)
SELECT e.id, 'parent', 'admin'
FROM entities e WHERE e.entity_type = 'nav_item' AND e.name = 'admin.menu_builder';

INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id)
SELECT
    (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'requires_permission'),
    nav.id,
    perm.id
FROM entities nav, entities perm
WHERE nav.entity_type = 'nav_item' AND nav.name = 'admin.menu_builder'
  AND perm.entity_type = 'permission' AND perm.name = 'roles.manage';
```

The simplest approach: run this SQL against the existing database after building. Add it to `seed_test_data.sql` or run it manually.

**Step 4: Verify compilation**

Run: `cargo check 2>&1 | tail -5`
Expected: Clean compilation

**Step 5: Commit**

```bash
git add src/main.rs src/db.rs
git commit -m "feat(menu-builder): register routes and nav item"
```

---

## Task 7: Add CSS styles

**Files:**
- Modify: `static/css/style.css`

**Step 1: Add matrix-specific CSS**

Append to `static/css/style.css`:

```css
/* ── Menu Builder (Permission Matrix) ────────────────── */

.page-description {
    color: var(--text-muted, #6b7280);
    margin-bottom: 1.5rem;
    font-size: 0.9rem;
}

.matrix-wrapper {
    overflow-x: auto;
    margin-bottom: 1.5rem;
    border: 1px solid var(--border-color, #e5e7eb);
    border-radius: var(--radius-md, 8px);
}

.matrix-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.875rem;
}

.matrix-table th,
.matrix-table td {
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid var(--border-color, #e5e7eb);
    text-align: center;
}

.matrix-label-col {
    text-align: left !important;
    min-width: 220px;
    position: sticky;
    left: 0;
    background: var(--surface-bg, #fff);
    z-index: 1;
}

.matrix-role-col {
    min-width: 100px;
}

.role-header {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.15rem;
}

.role-header .role-name {
    font-weight: 600;
    font-size: 0.85rem;
}

.role-header .role-code {
    font-size: 0.7rem;
    color: var(--text-muted, #6b7280);
    background: var(--surface-alt, #f3f4f6);
    padding: 0.1rem 0.35rem;
    border-radius: 3px;
}

.matrix-group-row {
    background: var(--surface-alt, #f9fafb);
}

.matrix-group-header {
    text-align: left !important;
    font-weight: 700;
    font-size: 0.8rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted, #6b7280);
    padding: 0.75rem 0.75rem 0.5rem !important;
    border-bottom: 2px solid var(--border-color, #e5e7eb) !important;
}

.matrix-perm-row:hover {
    background: var(--surface-hover, #f9fafb);
}

.matrix-perm-label {
    text-align: left !important;
    position: sticky;
    left: 0;
    background: inherit;
    z-index: 1;
}

.matrix-perm-label .perm-label {
    display: block;
    font-weight: 500;
}

.matrix-perm-label .perm-code {
    display: block;
    font-size: 0.7rem;
    color: var(--text-muted, #6b7280);
    margin-top: 0.1rem;
}

.matrix-cell {
    padding: 0.5rem !important;
}

.matrix-checkbox {
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
}

.matrix-checkbox input[type="checkbox"] {
    width: 18px;
    height: 18px;
    accent-color: var(--accent, #b45309);
    cursor: pointer;
}

.matrix-actions {
    padding-top: 1rem;
    border-top: 1px solid var(--border-color, #e5e7eb);
}

.btn.has-changes {
    background: var(--accent, #b45309);
    animation: pulse-subtle 1.5s ease-in-out infinite;
}

@keyframes pulse-subtle {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.85; }
}
```

**Step 2: Commit**

```bash
git add static/css/style.css
git commit -m "feat(menu-builder): add permission matrix CSS styles"
```

---

## Task 8: Fix compilation issues and wire everything together

**Files:**
- Potentially modify: `src/handlers/menu_builder_handlers.rs`, `src/models/role/queries.rs`, `src/models/role/mod.rs`

**Step 1: Run full build**

Run: `cargo build 2>&1 | tail -20`

**Step 2: Fix any compilation errors**

Common issues to expect and fix:

1. **`role::find_all_display` not found** — This function may have been removed as dead code. If so, either:
   - Add `find_all_for_matrix()` to `src/models/role/queries.rs` and re-export from `src/models/role/mod.rs`
   - Or use the existing `find_all_list()` and map to RoleColumn

2. **`audit::log_action` signature mismatch** — Check `src/audit/mod.rs` for the exact function signature. The `user_id` parameter might be `Option<i64>` or `i64`. Adapt the handler call.

3. **Import issues** — Ensure `std::collections::HashSet` is imported in the handler and permission module.

4. **Askama template errors** — Ensure all struct fields used in the template are accessible. All fields in PageGroup, PermissionRow, MatrixCell, and RoleColumn must be `pub`.

**Step 3: Iterate until clean build**

Run: `cargo build 2>&1 | tail -1`
Expected: `Finished` with no errors

**Step 4: Commit**

```bash
git add -A
git commit -m "fix(menu-builder): resolve compilation issues"
```

---

## Task 9: Add nav item to existing database

**Files:**
- Modify: `seed_test_data.sql` (append)

**Step 1: Append nav item SQL to seed_test_data.sql**

Add at the end of `seed_test_data.sql`:

```sql
-- Add Menu Builder nav item
INSERT OR IGNORE INTO entities (entity_type, name, label, sort_order, is_active)
VALUES ('nav_item', 'admin.menu_builder', 'Menu Builder', 6, 1);

INSERT OR REPLACE INTO entity_properties (entity_id, key, value)
SELECT e.id, 'url', '/menu-builder'
FROM entities e WHERE e.entity_type = 'nav_item' AND e.name = 'admin.menu_builder';

INSERT OR REPLACE INTO entity_properties (entity_id, key, value)
SELECT e.id, 'parent', 'admin'
FROM entities e WHERE e.entity_type = 'nav_item' AND e.name = 'admin.menu_builder';

INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id)
SELECT
    (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'requires_permission'),
    nav.id,
    perm.id
FROM entities nav, entities perm
WHERE nav.entity_type = 'nav_item' AND nav.name = 'admin.menu_builder'
  AND perm.entity_type = 'permission' AND perm.name = 'roles.manage';
```

**Step 2: Apply to existing database**

Run: `sqlite3 data/app.db < seed_test_data.sql`

**Step 3: Verify**

Run: `sqlite3 data/app.db "SELECT e.name, e.label FROM entities WHERE e.entity_type = 'nav_item' ORDER BY e.sort_order;"`
Expected: Should include `admin.menu_builder | Menu Builder`

**Step 4: Commit**

```bash
git add seed_test_data.sql
git commit -m "feat(menu-builder): add nav item to seed data"
```

---

## Task 10: Manual verification

**Step 1: Start the server**

Run: `cargo run`

**Step 2: Login as admin**

Navigate to http://localhost:8080/login
Login: admin / admin123

**Step 3: Navigate to Menu Builder**

Click Admin → Menu Builder in sidebar
Expected: Permission matrix table with all roles as columns and all permissions grouped by page section as rows

**Step 4: Verify checkbox state**

- Admin role should have ALL checkboxes checked
- User role should have only `dashboard.view` and `users.list` checked
- Editor role should have `users.list`, `users.create`, `users.edit`, `settings.manage` checked
- Viewer should have `dashboard.view`, `users.list` checked

**Step 5: Test toggling**

- Uncheck one permission for the Viewer role (e.g., `users.list`)
- Click "Save Changes"
- Expected: Flash message "Permissions updated (0 granted, 1 revoked)"
- Verify the checkbox is now unchecked after page reload
- Re-check the permission and save again to restore

**Step 6: Test with restricted user**

- Login as bob (Viewer) — should NOT see Menu Builder in sidebar
- Try navigating directly to /menu-builder — should get 403 Forbidden
- Login as alice (Editor) — should NOT see Menu Builder (editor doesn't have roles.manage)

**Step 7: Verify sidebar visibility changes**

- As admin, uncheck `users.list` for the Editor role
- Login as alice (Editor) — Users should no longer appear in sidebar
- Re-grant `users.list` to Editor to restore

**Step 8: Take screenshot for verification**

Use Playwright to capture the permission matrix page.

**Step 9: Commit final state**

```bash
git add -A
git commit -m "feat(menu-builder): complete permission matrix feature"
```

---

## Future Enhancements (Not in Scope)

These are potential follow-ups identified during planning:

1. **Split `roles.manage` into granular permissions** (`roles.list`, `roles.create`, `roles.edit`, `roles.delete`) — allows per-action control on the Roles page, matching the Users page pattern

2. **Nav item drag-and-drop reordering** — allow admins to reorder sidebar items and move items between modules

3. **Custom permission creation** — UI to define new permissions and associate them with page elements

4. **"Select All" row toggles** — click a permission label to toggle it across all roles

5. **Role comparison view** — side-by-side comparison of two roles' permissions

6. **Permission impact preview** — before saving, show what sidebar/page changes will result

7. **Protected admin role** — prevent accidentally revoking critical permissions from the admin role (currently no guard)
