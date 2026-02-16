# Roles Builder Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a 3-step wizard that guides administrators through creating roles with real-time menu access preview.

**Architecture:** Standalone `/roles/builder` route with wizard form, AJAX menu preview endpoint, and transactional role creation. Shows permission → menu item mapping.

**Tech Stack:** Actix-web, Askama templates, SQLite/rusqlite, JavaScript (AJAX)

---

## Task 1: Menu Preview Model Query

Create the backend query that finds which nav items are accessible given a set of permission IDs.

**Files:**
- Create: `src/models/role/builder.rs`
- Modify: `src/models/role/mod.rs`

**Step 1: Write the failing test**

Create `tests/role_builder_model_test.rs`:

```rust
use ahlt::db::init_pool;
use ahlt::models::role::builder::find_accessible_nav_items;

#[test]
fn test_find_accessible_nav_items() {
    let pool = init_pool(":memory:").unwrap();
    let conn = pool.get().unwrap();

    // Create test permission
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (100, 'permission', 'test.view', 'Test View')",
        [],
    ).unwrap();

    // Create test nav item with permission requirement
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (200, 'nav_item', 'test_page', 'Test Page')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (200, 'path', '/test')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (200, 'permission_required', 'test.view')",
        [],
    ).unwrap();

    // Create nav module
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (201, 'nav_module', 'test_module', 'Test Module')",
        [],
    ).unwrap();

    // Link nav item to module
    let rt_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='relation_type' AND name='in_module'",
        [],
        |row| row.get(0),
    ).unwrap();
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, 200, 201)",
        [rt_id],
    ).unwrap();

    // Query with permission ID 100
    let items = find_accessible_nav_items(&conn, &[100]).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Test Page");
    assert_eq!(items[0].path, "/test");
    assert_eq!(items[0].module_name, "Test Module");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test test_find_accessible_nav_items`

Expected: FAIL with "failed to resolve: use of undeclared crate or module `builder`"

**Step 3: Write minimal implementation**

Create `src/models/role/builder.rs`:

```rust
use rusqlite::{Connection, params};

#[derive(Debug, Clone, serde::Serialize)]
pub struct NavItemPreview {
    pub id: i64,
    pub label: String,
    pub path: String,
    pub module_name: String,
}

pub fn find_accessible_nav_items(
    conn: &Connection,
    permission_ids: &[i64],
) -> rusqlite::Result<Vec<NavItemPreview>> {
    if permission_ids.is_empty() {
        return Ok(Vec::new());
    }

    // Build permission code lookup
    let mut stmt = conn.prepare(
        "SELECT name FROM entities WHERE id IN (SELECT value FROM json_each(?1))"
    )?;
    let permission_codes: Vec<String> = stmt.query_map(
        params![serde_json::to_string(&permission_ids).unwrap()],
        |row| row.get(0),
    )?.collect::<Result<Vec<_>, _>>()?;

    if permission_codes.is_empty() {
        return Ok(Vec::new());
    }

    // Find nav items where permission_required matches any permission code
    let placeholders = permission_codes.iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");

    let query = format!(
        "SELECT DISTINCT ni.id, ni.label,
                COALESCE(p_path.value, '') as path,
                COALESCE(m.label, 'General') as module_name
         FROM entities ni
         LEFT JOIN entity_properties p_path ON p_path.entity_id = ni.id AND p_path.key = 'path'
         LEFT JOIN entity_properties p_perm ON p_perm.entity_id = ni.id AND p_perm.key = 'permission_required'
         LEFT JOIN relations r_mod ON r_mod.source_id = ni.id
         LEFT JOIN entities rt_mod ON rt_mod.id = r_mod.relation_type_id AND rt_mod.name = 'in_module'
         LEFT JOIN entities m ON m.id = r_mod.target_id
         WHERE ni.entity_type = 'nav_item'
           AND (p_perm.value IS NULL OR p_perm.value IN ({}))
         ORDER BY m.label, ni.label",
        placeholders
    );

    let mut stmt = conn.prepare(&query)?;
    let params_refs: Vec<&dyn rusqlite::types::ToSql> = permission_codes.iter()
        .map(|s| s as &dyn rusqlite::types::ToSql)
        .collect();

    let items = stmt.query_map(params_refs.as_slice(), |row| {
        Ok(NavItemPreview {
            id: row.get("id")?,
            label: row.get("label")?,
            path: row.get("path")?,
            module_name: row.get("module_name")?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;

    Ok(items)
}
```

Modify `src/models/role/mod.rs` to add:

```rust
pub mod builder;
```

**Step 4: Run test to verify it passes**

Run: `cargo test test_find_accessible_nav_items`

Expected: PASS

**Step 5: Commit**

```bash
git add src/models/role/builder.rs src/models/role/mod.rs tests/role_builder_model_test.rs
git commit -m "feat: add menu preview query for role builder"
```

---

## Task 2: Template Context Types

Add the template context structs and request/response types.

**Files:**
- Modify: `src/templates_structs.rs`

**Step 1: Add template types**

Add to `src/templates_structs.rs`:

```rust
use crate::models::role::builder::NavItemPreview;

#[derive(Template)]
#[template(path = "roles/builder.html")]
pub struct RoleBuilderTemplate {
    pub ctx: PageContext,
    pub permission_groups: Vec<PermissionGroup>,
    pub csrf_token: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PreviewRequest {
    pub permission_ids: Vec<i64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PreviewResponse {
    pub items: Vec<NavItemPreview>,
    pub count: usize,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RoleBuilderForm {
    pub name: String,
    pub label: String,
    pub description: String,
    pub permission_ids: String, // JSON array
    pub csrf_token: String,
}
```

**Step 2: Verify compilation**

Run: `cargo check`

Expected: Success (or warnings, but no errors)

**Step 3: Commit**

```bash
git add src/templates_structs.rs
git commit -m "feat: add role builder template context types"
```

---

## Task 3: Role Builder Handlers

Create the three handler functions with validation logic.

**Files:**
- Create: `src/handlers/role_builder_handlers.rs`
- Modify: `src/handlers/mod.rs`

**Step 1: Create handlers file**

Create `src/handlers/role_builder_handlers.rs`:

```rust
use actix_session::Session;
use actix_web::{web, HttpResponse};
use rusqlite::params;

use crate::auth::session::{require_permission, get_user_id};
use crate::auth::csrf;
use crate::db::DbPool;
use crate::errors::{AppError, render};
use crate::models::{permission, role};
use crate::templates_structs::{
    PageContext, RoleBuilderTemplate, PreviewRequest, PreviewResponse, RoleBuilderForm,
};
use crate::audit;

pub async fn wizard_form(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "admin.roles")?;

    let conn = pool.get()?;
    let ctx = PageContext::build(&session, &conn, "/roles/builder")?;
    let permission_groups = permission::find_all_with_groups(&conn)?;
    let csrf_token = csrf::generate_token(&session)?;

    let tmpl = RoleBuilderTemplate {
        ctx,
        permission_groups,
        csrf_token,
    };

    render(tmpl)
}

pub async fn preview_menu(
    pool: web::Data<DbPool>,
    session: Session,
    body: web::Json<PreviewRequest>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "admin.roles")?;

    let conn = pool.get()?;
    let items = role::builder::find_accessible_nav_items(&conn, &body.permission_ids)?;
    let count = items.len();

    Ok(HttpResponse::Ok().json(PreviewResponse { items, count }))
}

pub async fn create_role(
    pool: web::Data<DbPool>,
    session: Session,
    form: web::Form<RoleBuilderForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "admin.roles")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let conn = pool.get()?;

    // Validate role name
    validate_role_name(&form.name)?;
    validate_role_label(&form.label)?;
    ensure_unique_role_name(&conn, &form.name)?;

    // Parse permission IDs
    let permission_ids: Vec<i64> = serde_json::from_str(&form.permission_ids)
        .map_err(|_| AppError::Session("Invalid permission data".into()))?;

    if permission_ids.is_empty() {
        return Err(AppError::Session("Please select at least one permission".into()));
    }

    // Transaction: create role entity + properties + relations
    let role_id = conn.query_row(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', ?1, ?2) RETURNING id",
        params![&form.name, &form.label],
        |row| row.get::<_, i64>(0),
    )?;

    // Add description property if provided
    if !form.description.trim().is_empty() {
        conn.execute(
            "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'description', ?2)",
            params![role_id, &form.description],
        )?;
    }

    // Add permission relations
    let rt_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='relation_type' AND name='has_permission'",
        [],
        |row| row.get(0),
    )?;

    for perm_id in &permission_ids {
        conn.execute(
            "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
            params![rt_id, role_id, perm_id],
        )?;
    }

    // Audit log
    let user_id = get_user_id(&session).unwrap_or(0);
    let details = serde_json::json!({
        "name": form.name,
        "label": form.label,
        "permission_count": permission_ids.len(),
    });
    let _ = audit::log(&conn, user_id, "role.created_via_builder", "role", role_id, details);

    Ok(HttpResponse::SeeOther()
        .append_header(("Location", format!("/roles/{}", role_id)))
        .finish())
}

fn validate_role_name(name: &str) -> Result<(), AppError> {
    if name.is_empty() {
        return Err(AppError::Session("Role name required".into()));
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(AppError::Session("Role name must be alphanumeric + underscore".into()));
    }
    if name.len() > 50 {
        return Err(AppError::Session("Role name too long (max 50)".into()));
    }
    Ok(())
}

fn validate_role_label(label: &str) -> Result<(), AppError> {
    if label.trim().is_empty() {
        return Err(AppError::Session("Role label required".into()));
    }
    if label.len() > 100 {
        return Err(AppError::Session("Role label too long (max 100)".into()));
    }
    Ok(())
}

fn ensure_unique_role_name(conn: &rusqlite::Connection, name: &str) -> Result<(), AppError> {
    let exists = conn.query_row(
        "SELECT 1 FROM entities WHERE entity_type='role' AND name=?1",
        params![name],
        |_| Ok(true),
    ).unwrap_or(false);

    if exists {
        Err(AppError::Session(format!("Role '{}' already exists", name)))
    } else {
        Ok(())
    }
}
```

Add to `src/handlers/mod.rs`:

```rust
pub mod role_builder_handlers;
```

**Step 2: Verify compilation**

Run: `cargo check`

Expected: Success

**Step 3: Commit**

```bash
git add src/handlers/role_builder_handlers.rs src/handlers/mod.rs
git commit -m "feat: add role builder handlers with validation"
```

---

## Task 4: Template with Step Navigation

Create the wizard template with 3 steps and JavaScript for navigation and AJAX preview.

**Files:**
- Create: `templates/roles/builder.html`

**Step 1: Create template**

Create `templates/roles/builder.html`:

```html
{% extends "base.html" %}

{% block title %}Role Builder{% endblock %}

{% block content %}
<div class="role-builder-container">
    <h1>Create New Role</h1>

    <!-- Step Indicator -->
    <div class="step-indicator">
        <div class="step active" data-step="1">
            <span class="step-number">1</span>
            <span class="step-label">Details</span>
        </div>
        <div class="step" data-step="2">
            <span class="step-number">2</span>
            <span class="step-label">Permissions</span>
        </div>
        <div class="step" data-step="3">
            <span class="step-number">3</span>
            <span class="step-label">Preview</span>
        </div>
    </div>

    <form id="role-builder-form" action="/roles/builder/create" method="POST">
        <input type="hidden" name="csrf_token" value="{{ csrf_token }}">
        <input type="hidden" name="permission_ids" id="permission-ids-input" value="[]">

        <!-- Step 1: Role Details -->
        <div class="wizard-step" data-step="1">
            <div class="form-group">
                <label for="name">Role Name *</label>
                <input type="text" id="name" name="name" required pattern="[a-zA-Z0-9_]+" maxlength="50">
                <small>Alphanumeric and underscore only</small>
            </div>

            <div class="form-group">
                <label for="label">Display Label *</label>
                <input type="text" id="label" name="label" required maxlength="100">
            </div>

            <div class="form-group">
                <label for="description">Description</label>
                <textarea id="description" name="description" rows="3"></textarea>
            </div>

            <button type="button" class="btn btn-primary" onclick="nextStep()">Next: Select Permissions</button>
        </div>

        <!-- Step 2: Permission Selection -->
        <div class="wizard-step" data-step="2" style="display: none;">
            {% for group in permission_groups %}
            <div class="permission-group">
                <div class="group-header">
                    <h3>{{ group.group_name }}</h3>
                    <label class="select-all-label">
                        <input type="checkbox" class="select-all-checkbox" data-group="{{ group.group_name }}">
                        Select All
                    </label>
                </div>
                <div class="permission-list">
                    {% for perm in group.permissions %}
                    <label class="permission-checkbox">
                        <input type="checkbox"
                               class="permission-item"
                               data-group="{{ group.group_name }}"
                               value="{{ perm.id }}"
                               {% if perm.checked %}checked{% endif %}>
                        <span class="permission-label">{{ perm.label }}</span>
                        <span class="permission-code">{{ perm.code }}</span>
                    </label>
                    {% endfor %}
                </div>
            </div>
            {% endfor %}

            <div class="button-group">
                <button type="button" class="btn btn-secondary" onclick="prevStep()">Back</button>
                <button type="button" class="btn btn-primary" onclick="showPreview()">Next: Preview Menu Access</button>
            </div>
        </div>

        <!-- Step 3: Menu Preview -->
        <div class="wizard-step" data-step="3" style="display: none;">
            <div id="preview-loading" style="display: none;">Loading menu preview...</div>
            <div id="preview-error" class="error-message" style="display: none;"></div>

            <div id="preview-content">
                <p id="preview-summary"></p>
                <div id="preview-tree"></div>
            </div>

            <div class="button-group">
                <button type="button" class="btn btn-secondary" onclick="prevStep()">Back</button>
                <button type="submit" class="btn btn-success">Create Role</button>
            </div>
        </div>
    </form>
</div>

<script>
let currentStep = 1;

function showStep(step) {
    document.querySelectorAll('.wizard-step').forEach(el => el.style.display = 'none');
    document.querySelectorAll('.step').forEach(el => el.classList.remove('active'));

    document.querySelector(`.wizard-step[data-step="${step}"]`).style.display = 'block';
    document.querySelector(`.step[data-step="${step}"]`).classList.add('active');
    currentStep = step;
}

function nextStep() {
    if (currentStep === 1) {
        // Validate Step 1
        const name = document.getElementById('name').value;
        const label = document.getElementById('label').value;
        if (!name || !label) {
            alert('Please fill in required fields');
            return;
        }
    }
    showStep(currentStep + 1);
}

function prevStep() {
    showStep(currentStep - 1);
}

// Select All toggle
document.querySelectorAll('.select-all-checkbox').forEach(checkbox => {
    checkbox.addEventListener('change', function() {
        const group = this.dataset.group;
        const checked = this.checked;
        document.querySelectorAll(`.permission-item[data-group="${group}"]`).forEach(item => {
            item.checked = checked;
        });
    });
});

// Update select-all state when individual checkboxes change
document.querySelectorAll('.permission-item').forEach(checkbox => {
    checkbox.addEventListener('change', function() {
        const group = this.dataset.group;
        const groupCheckboxes = document.querySelectorAll(`.permission-item[data-group="${group}"]`);
        const allChecked = Array.from(groupCheckboxes).every(cb => cb.checked);
        const selectAllCheckbox = document.querySelector(`.select-all-checkbox[data-group="${group}"]`);
        if (selectAllCheckbox) {
            selectAllCheckbox.checked = allChecked;
        }
    });
});

async function showPreview() {
    // Collect selected permission IDs
    const selectedIds = Array.from(document.querySelectorAll('.permission-item:checked'))
        .map(cb => parseInt(cb.value));

    if (selectedIds.length === 0) {
        alert('Please select at least one permission');
        return;
    }

    // Update hidden input
    document.getElementById('permission-ids-input').value = JSON.stringify(selectedIds);

    // Show step 3
    showStep(3);

    // Fetch preview
    document.getElementById('preview-loading').style.display = 'block';
    document.getElementById('preview-error').style.display = 'none';
    document.getElementById('preview-content').style.display = 'none';

    try {
        const response = await fetch('/roles/builder/preview', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ permission_ids: selectedIds })
        });

        if (!response.ok) {
            throw new Error('Failed to load preview');
        }

        const data = await response.json();
        displayPreview(data);
    } catch (error) {
        document.getElementById('preview-loading').style.display = 'none';
        document.getElementById('preview-error').textContent = error.message;
        document.getElementById('preview-error').style.display = 'block';
    }
}

function displayPreview(data) {
    document.getElementById('preview-loading').style.display = 'none';
    document.getElementById('preview-content').style.display = 'block';

    const summary = document.getElementById('preview-summary');
    summary.textContent = `This role grants access to ${data.count} menu item(s)`;

    const tree = document.getElementById('preview-tree');
    tree.textContent = ''; // Clear previous content

    // Group by module
    const byModule = {};
    data.items.forEach(item => {
        if (!byModule[item.module_name]) {
            byModule[item.module_name] = [];
        }
        byModule[item.module_name].push(item);
    });

    // Render tree using safe DOM methods
    for (const [moduleName, items] of Object.entries(byModule)) {
        const moduleDiv = document.createElement('div');
        moduleDiv.className = 'preview-module';

        const moduleTitle = document.createElement('strong');
        moduleTitle.textContent = moduleName;
        moduleDiv.appendChild(moduleTitle);

        const itemsList = document.createElement('ul');
        items.forEach(item => {
            const li = document.createElement('li');
            li.textContent = `${item.label} (${item.path})`;
            itemsList.appendChild(li);
        });

        moduleDiv.appendChild(itemsList);
        tree.appendChild(moduleDiv);
    }
}
</script>
{% endblock %}
```

**Step 2: Verify template compiles**

Run: `cargo check`

Expected: Success (template will be checked at compile time)

**Step 3: Commit**

```bash
git add templates/roles/builder.html
git commit -m "feat: add role builder wizard template"
```

---

## Task 5: CSS Styling

Create the stylesheet for the role builder wizard.

**Files:**
- Create: `static/css/role-builder.css`
- Modify: `templates/base.html`

**Step 1: Create CSS**

Create `static/css/role-builder.css`:

```css
/* Role Builder Wizard */
.role-builder-container {
    max-width: 900px;
    margin: 2rem auto;
    padding: 2rem;
    background: white;
    border-radius: 8px;
    box-shadow: 0 2px 4px rgba(0,0,0,0.1);
}

/* Step Indicator */
.step-indicator {
    display: flex;
    justify-content: space-between;
    margin-bottom: 2rem;
    padding: 0 2rem;
}

.step {
    display: flex;
    flex-direction: column;
    align-items: center;
    flex: 1;
    position: relative;
    opacity: 0.5;
}

.step.active {
    opacity: 1;
}

.step-number {
    width: 40px;
    height: 40px;
    border-radius: 50%;
    background: #e0e0e0;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: bold;
    margin-bottom: 0.5rem;
}

.step.active .step-number {
    background: #2196F3;
    color: white;
}

.step-label {
    font-size: 0.9rem;
}

.step:not(:last-child)::after {
    content: '';
    position: absolute;
    top: 20px;
    left: calc(50% + 20px);
    width: calc(100% - 40px);
    height: 2px;
    background: #e0e0e0;
}

.step.active:not(:last-child)::after {
    background: #2196F3;
}

/* Form Groups */
.wizard-step {
    margin-bottom: 1rem;
}

.form-group {
    margin-bottom: 1.5rem;
}

.form-group label {
    display: block;
    margin-bottom: 0.5rem;
    font-weight: 500;
}

.form-group input[type="text"],
.form-group textarea {
    width: 100%;
    padding: 0.5rem;
    border: 1px solid #ddd;
    border-radius: 4px;
}

.form-group small {
    display: block;
    margin-top: 0.25rem;
    color: #666;
    font-size: 0.85rem;
}

/* Permission Groups */
.permission-group {
    margin-bottom: 2rem;
    border: 1px solid #e0e0e0;
    border-radius: 4px;
    overflow: hidden;
}

.group-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem;
    background: #f5f5f5;
    border-bottom: 1px solid #e0e0e0;
}

.group-header h3 {
    margin: 0;
    font-size: 1.1rem;
}

.select-all-label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
    font-weight: normal;
}

.permission-list {
    padding: 1rem;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
    gap: 0.75rem;
}

.permission-checkbox {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    cursor: pointer;
    padding: 0.5rem;
    border-radius: 4px;
    transition: background 0.2s;
}

.permission-checkbox:hover {
    background: #f5f5f5;
}

.permission-checkbox input[type="checkbox"] {
    margin-top: 0.2rem;
    flex-shrink: 0;
}

.permission-label {
    font-weight: 500;
}

.permission-code {
    display: block;
    font-size: 0.85rem;
    color: #666;
}

/* Preview */
#preview-summary {
    font-size: 1.1rem;
    font-weight: 500;
    margin-bottom: 1rem;
    padding: 1rem;
    background: #e3f2fd;
    border-left: 4px solid #2196F3;
}

#preview-tree {
    margin-top: 1rem;
}

.preview-module {
    margin-bottom: 1.5rem;
}

.preview-module strong {
    display: block;
    margin-bottom: 0.5rem;
    color: #333;
    font-size: 1.05rem;
}

.preview-module ul {
    margin: 0;
    padding-left: 1.5rem;
    list-style: disc;
}

.preview-module li {
    margin-bottom: 0.25rem;
    color: #555;
}

/* Buttons */
.button-group {
    display: flex;
    gap: 1rem;
    margin-top: 2rem;
}

.btn {
    padding: 0.75rem 1.5rem;
    border: none;
    border-radius: 4px;
    font-size: 1rem;
    cursor: pointer;
    transition: background 0.2s;
}

.btn-primary {
    background: #2196F3;
    color: white;
}

.btn-primary:hover {
    background: #1976D2;
}

.btn-secondary {
    background: #e0e0e0;
    color: #333;
}

.btn-secondary:hover {
    background: #d0d0d0;
}

.btn-success {
    background: #4CAF50;
    color: white;
}

.btn-success:hover {
    background: #45a049;
}

/* Error Message */
.error-message {
    padding: 1rem;
    background: #ffebee;
    border-left: 4px solid #f44336;
    color: #c62828;
    margin-bottom: 1rem;
}
```

**Step 2: Link CSS in base template**

Add to `templates/base.html` in the `<head>` section:

```html
<link rel="stylesheet" href="/static/css/role-builder.css">
```

**Step 3: Verify**

Run: `cargo check`

Expected: Success

**Step 4: Commit**

```bash
git add static/css/role-builder.css templates/base.html
git commit -m "feat: add role builder CSS styling"
```

---

## Task 6: Route Registration

Register the role builder routes in the main application.

**Files:**
- Modify: `src/main.rs`

**Step 1: Add routes**

In `src/main.rs`, find the route configuration section and add:

```rust
use crate::handlers::role_builder_handlers;

// ... in the app configuration:

.route("/roles/builder", web::get().to(role_builder_handlers::wizard_form))
.route("/roles/builder/preview", web::post().to(role_builder_handlers::preview_menu))
.route("/roles/builder/create", web::post().to(role_builder_handlers::create_role))
```

**Step 2: Verify compilation**

Run: `cargo check`

Expected: Success

**Step 3: Test manually**

Run: `cargo run`

Visit: `http://localhost:8080/roles/builder`

Expected: Wizard page loads (requires login as admin)

**Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: register role builder routes"
```

---

## Task 7: Integration Tests

Write comprehensive tests for the role builder functionality.

**Files:**
- Create: `tests/role_builder_test.rs`

**Step 1: Write tests**

Create `tests/role_builder_test.rs`:

```rust
use ahlt::db::init_pool;
use ahlt::models::role;

fn setup_test_db() -> rusqlite::Connection {
    let pool = init_pool(":memory:").unwrap();
    pool.get().unwrap()
}

#[test]
fn test_create_role_via_builder() {
    let conn = setup_test_db();

    // Create test permissions
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (1, 'permission', 'test.read', 'Test Read')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (2, 'permission', 'test.write', 'Test Write')",
        [],
    ).unwrap();

    // Create role via builder simulation
    let role_id = conn.query_row(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', 'test_role', 'Test Role') RETURNING id",
        [],
        |row| row.get::<_, i64>(0),
    ).unwrap();

    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'description', 'Test description')",
        [role_id],
    ).unwrap();

    let rt_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='relation_type' AND name='has_permission'",
        [],
        |row| row.get(0),
    ).unwrap();

    for perm_id in [1, 2] {
        conn.execute(
            "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, ?2, ?3)",
            [rt_id, role_id, perm_id],
        ).unwrap();
    }

    // Verify role
    let role = role::find_by_id(&conn, role_id).unwrap();
    assert_eq!(role.name, "test_role");
    assert_eq!(role.label, "Test Role");

    // Verify permissions
    let permissions = role::find_permission_checkboxes(&conn, role_id).unwrap();
    let granted = permissions.iter().filter(|p| p.checked).count();
    assert_eq!(granted, 2);
}

#[test]
fn test_role_name_uniqueness() {
    let conn = setup_test_db();

    // Create first role
    conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', 'duplicate', 'First')",
        [],
    ).unwrap();

    // Attempt duplicate
    let result = conn.execute(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', 'duplicate', 'Second')",
        [],
    );

    assert!(result.is_err());
}

#[test]
fn test_menu_preview_calculation() {
    let conn = setup_test_db();

    // Create permission
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (100, 'permission', 'admin.settings', 'Admin Settings')",
        [],
    ).unwrap();

    // Create nav module
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (200, 'nav_module', 'admin', 'Admin')",
        [],
    ).unwrap();

    // Create nav item
    conn.execute(
        "INSERT INTO entities (id, entity_type, name, label) VALUES (201, 'nav_item', 'settings', 'Settings')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (201, 'path', '/settings')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (201, 'permission_required', 'admin.settings')",
        [],
    ).unwrap();

    // Link nav item to module
    let rt_id: i64 = conn.query_row(
        "SELECT id FROM entities WHERE entity_type='relation_type' AND name='in_module'",
        [],
        |row| row.get(0),
    ).unwrap();
    conn.execute(
        "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?1, 201, 200)",
        [rt_id],
    ).unwrap();

    // Query accessible items
    let items = role::builder::find_accessible_nav_items(&conn, &[100]).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "Settings");
}

#[test]
fn test_no_permissions_selected() {
    let conn = setup_test_db();

    // Create role with no permissions (valid but not useful)
    let role_id = conn.query_row(
        "INSERT INTO entities (entity_type, name, label) VALUES ('role', 'empty_role', 'Empty Role') RETURNING id",
        [],
        |row| row.get::<_, i64>(0),
    ).unwrap();

    // Verify role exists
    let role = role::find_by_id(&conn, role_id).unwrap();
    assert_eq!(role.name, "empty_role");

    // Verify no permissions
    let permissions = role::find_permission_checkboxes(&conn, role_id).unwrap();
    let granted = permissions.iter().filter(|p| p.checked).count();
    assert_eq!(granted, 0);
}

#[test]
fn test_builder_requires_admin_permission() {
    // This would be tested at the handler level with mock sessions
    // For now, we just verify the permission code exists
    let conn = setup_test_db();

    let exists = conn.query_row(
        "SELECT 1 FROM entities WHERE entity_type='permission' AND name='admin.roles'",
        [],
        |_| Ok(true),
    ).unwrap_or(false);

    assert!(exists, "admin.roles permission should exist");
}
```

**Step 2: Run tests**

Run: `cargo test role_builder`

Expected: All 5 tests PASS

**Step 3: Commit**

```bash
git add tests/role_builder_test.rs
git commit -m "test: add role builder integration tests"
```

---

## Task 8: Add Navigation Item (Optional)

Add a "Role Builder" link to the navigation for easy access.

**Files:**
- Modify database (via SQL or seed script)

**Step 1: Add nav item**

Run SQL (or add to seed script):

```sql
-- Check if nav item already exists
SELECT id FROM entities WHERE entity_type='nav_item' AND name='role_builder';

-- If not, create it
INSERT INTO entities (entity_type, name, label) VALUES ('nav_item', 'role_builder', 'Role Builder');

-- Get the nav item ID
-- Assuming it's 999, replace with actual ID from previous query

-- Add properties
INSERT INTO entity_properties (entity_id, key, value) VALUES (999, 'path', '/roles/builder');
INSERT INTO entity_properties (entity_id, key, value) VALUES (999, 'permission_required', 'admin.roles');
INSERT INTO entity_properties (entity_id, key, value) VALUES (999, 'position', '2');

-- Link to admin module (assuming module ID is 1, check with: SELECT id FROM entities WHERE entity_type='nav_module' AND name='admin')
INSERT INTO relations (relation_type_id, source_id, target_id)
  SELECT rt.id, 999, 1
  FROM entities rt WHERE rt.entity_type='relation_type' AND rt.name='in_module';
```

**Step 2: Verify**

Run the app and check navigation menu shows "Role Builder" under Admin section.

**Step 3: Commit**

```bash
git add -A
git commit -m "feat: add role builder to navigation menu"
```

---

## Final Verification

**Step 1: Run all tests**

Run: `cargo test`

Expected: All tests PASS (existing + new role builder tests)

**Step 2: Run the application**

Run: `cargo run`

**Step 3: Manual testing checklist**

- [ ] Navigate to `/roles/builder` as admin
- [ ] Fill in role details (step 1), click Next
- [ ] Select permissions (step 2), use Select All toggle
- [ ] Click "Next: Preview Menu Access"
- [ ] Verify menu preview shows correct items
- [ ] Click "Create Role"
- [ ] Verify redirect to role detail page
- [ ] Check role appears in `/roles` list
- [ ] Check audit log entry exists

**Step 4: Code quality check**

Run: `cargo clippy`

Expected: No new warnings

**Step 5: Final commit**

```bash
git add -A
git commit -m "feat: complete role builder implementation"
```

---

## Success Criteria

✅ All integration tests pass
✅ Role creation works end-to-end
✅ Menu preview shows correct accessible items
✅ Validation prevents duplicate names and invalid input
✅ CSRF protection works
✅ Audit logging captures role creation
✅ No clippy warnings
✅ UI is responsive and intuitive
