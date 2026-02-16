# Roles Builder Design

**Date:** 2026-02-16
**Status:** Approved

## Goal

Create a standalone "roles builder" that guides administrators through creating new roles from scratch with a clear understanding of how those roles relate to menu access. The builder shows which menu items will become accessible based on selected permissions.

## Background

Currently, the system has:
- Role CRUD at `/roles` with permission checkboxes
- Menu builder at `/menu-builder` showing a permission matrix (roles × permissions)
- Implicit relationship: roles grant permissions → permissions control menu item visibility

**Problem:** Admins creating roles don't see the connection between permission selection and menu access until after creation.

**Solution:** A guided wizard that shows the permission → menu access mapping in real-time.

## Architecture

### Approach

**Standalone Roles Builder with Menu Preview** (3-step wizard)

**Route Structure:**
- `GET /roles/builder` - Main wizard page
- `POST /roles/builder/preview` - AJAX endpoint for menu preview
- `POST /roles/builder/create` - Final role creation

### Three-Step Workflow

**Step 1: Role Details**
- Form fields: name (unique), label (display name), description (optional)
- Validation: name must be alphanumeric+underscore, label required
- CSRF protection via hidden token

**Step 2: Permission Selection**
- Load permissions grouped by `group_name` (same as menu builder)
- Checkbox grid: one section per group
- "Select All" toggle per group for convenience
- Selected permissions tracked via form state

**Step 3: Menu Preview**
- AJAX call to `/roles/builder/preview` with selected permission IDs
- Backend queries: which nav_items this role would grant access to
- Display: hierarchical menu tree showing accessible items
- Visual indicator: "This role grants access to N menu items"

**Final Action:**
- "Create Role" button POSTs to `/roles/builder/create`
- Transactional creation: entity + properties + permission relations
- Redirect to `/roles/{id}` on success
- Audit log: "created role via builder"

### Database Operations

```sql
-- Insert role entity
INSERT INTO entities (entity_type, name, label)
VALUES ('role', ?, ?)

-- Insert description property
INSERT INTO entity_properties (entity_id, key, value)
VALUES (?, 'description', ?)

-- Insert permission relations
INSERT INTO relations (relation_type_id, source_id, target_id)
  SELECT rt.id, ?, ?
  FROM entities rt
  WHERE rt.name = 'has_permission'
```

## Components

### Handler Structure

**File:** `src/handlers/role_builder_handlers.rs`

```rust
pub async fn wizard_form(
    pool: web::Data<DbPool>,
    session: Session,
) -> Result<HttpResponse, AppError>
// GET /roles/builder
// Renders RoleBuilderTemplate with permission groups

pub async fn preview_menu(
    pool: web::Data<DbPool>,
    session: Session,
    body: web::Json<PreviewRequest>,
) -> Result<HttpResponse, AppError>
// POST /roles/builder/preview (AJAX)
// Returns JSON: { accessible_items: Vec<NavItemPreview> }

pub async fn create_role(
    pool: web::Data<DbPool>,
    session: Session,
    form: web::Form<RoleBuilderForm>,
) -> Result<HttpResponse, AppError>
// POST /roles/builder/create
// Transaction: entity + properties + relations
```

### Model Queries

**File:** `src/models/role/builder.rs` (new)

```rust
pub fn find_accessible_nav_items(
    conn: &Connection,
    permission_ids: &[i64]
) -> rusqlite::Result<Vec<NavItemPreview>>
// Query nav_items where ANY required permission is in the set

pub struct NavItemPreview {
    pub id: i64,
    pub label: String,
    pub path: String,
    pub module_name: String,
}
```

### Templates

**File:** `templates/roles/builder.html`

Components:
- Step indicator (1→2→3 progress bar)
- Step 1: Role details form (name, label, description)
- Step 2: Permission checkboxes (grouped, collapsible sections)
- Step 3: Menu preview (populated via AJAX, hidden until permissions selected)
- JavaScript: step navigation, AJAX preview, form submission

**Template Context:**

```rust
pub struct RoleBuilderTemplate {
    pub ctx: PageContext,
    pub permission_groups: Vec<PermissionGroup>,
    pub csrf_token: String,
}

pub struct PermissionGroup {
    pub group_name: String,
    pub permissions: Vec<PermissionCheckbox>,
}
```

### Static Assets

**File:** `static/css/role-builder.css`

Styles for:
- Step indicator styling
- Permission group accordion
- Menu preview tree

## Data Flow

### Page Load (GET /roles/builder)

```
1. require_permission(&session, "admin.roles") → AppError::PermissionDenied?
2. pool.get() → Connection
3. PageContext::build(&session, &conn, "/roles/builder")
4. permission::find_all_with_groups(&conn) → Vec<PermissionGroup>
5. csrf::generate_token(&session) → String
6. render(RoleBuilderTemplate { ctx, permission_groups, csrf_token })
```

### Menu Preview (POST /roles/builder/preview - AJAX)

```
1. Parse JSON body: { permission_ids: [1, 5, 12, ...] }
2. role::builder::find_accessible_nav_items(&conn, &permission_ids)
   ├── Query: nav_items with permissions in the set
   └── Build tree structure (modules → items)
3. Return JSON: { items: Vec<NavItemPreview>, count: usize }
4. Frontend updates preview section with tree
```

### Role Creation (POST /roles/builder/create)

```
1. require_permission(&session, "admin.roles")
2. csrf::validate_csrf(&session, &form_token)
3. Validate: name unique, label non-empty
4. Transaction BEGIN
   ├── Insert entity (type='role', name, label)
   ├── Insert property (description)
   ├── Insert relations (has_permission × N)
   └── audit::log(user_id, "role.created_via_builder", "role", role_id, details)
5. COMMIT
6. Redirect → /roles/{role_id}
```

### Error Paths

- Missing permission → 403 error page
- Invalid CSRF → AppError::Csrf
- Duplicate role name → AppError::Db + flash message "Role name already exists"
- DB transaction failure → Rollback + AppError::Db
- AJAX preview failure → 500 JSON response: { error: "message" }

## Error Handling

### Validation Rules

```rust
// Role name validation
fn validate_role_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Role name required".into());
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err("Role name must be alphanumeric + underscore".into());
    }
    if name.len() > 50 {
        return Err("Role name too long (max 50)".into());
    }
    Ok(())
}

// Role label validation
fn validate_role_label(label: &str) -> Result<(), String> {
    if label.trim().is_empty() {
        return Err("Role label required".into());
    }
    if label.len() > 100 {
        return Err("Role label too long (max 100)".into());
    }
    Ok(())
}

// Uniqueness check
fn ensure_unique_role_name(conn: &Connection, name: &str) -> Result<(), AppError> {
    let exists = conn.query_row(
        "SELECT 1 FROM entities WHERE entity_type='role' AND name=?1",
        params![name],
        |_| Ok(true)
    ).unwrap_or(false);

    if exists {
        Err(AppError::NotFound) // reuse for "already exists" with custom message
    } else {
        Ok(())
    }
}
```

### User-Facing Error Messages

- Empty permission selection → Flash: "Please select at least one permission"
- Duplicate role name → Flash: "Role 'admin_custom' already exists"
- Transaction failure → Flash: "Failed to create role. Please try again."
- Network error on preview → Toast: "Failed to load menu preview"

### Rollback Strategy

- All DB operations in a transaction
- On error, automatic rollback (no partial role creation)
- Audit log only written on successful commit

## Testing

### Integration Tests

**File:** `tests/role_builder_test.rs`

```rust
#[test]
fn test_create_role_via_builder() {
    // Create role with permissions, verify entity + properties + relations
}

#[test]
fn test_role_name_uniqueness() {
    // Attempt duplicate role name, expect error
}

#[test]
fn test_menu_preview_calculation() {
    // Given permission IDs, verify correct nav_items returned
}

#[test]
fn test_no_permissions_selected() {
    // Create role with zero permissions (valid but not useful)
}

#[test]
fn test_builder_requires_admin_permission() {
    // Non-admin user, expect 403
}
```

### Manual Testing Checklist

- [ ] Create role with description, verify saved
- [ ] Create role without description (optional field)
- [ ] Select permissions across multiple groups
- [ ] Preview shows correct menu items
- [ ] "Select All" toggle works per group
- [ ] Step navigation (1→2→3→back)
- [ ] CSRF protection (tamper with token, expect error)
- [ ] Duplicate name shows error flash
- [ ] Created role appears in /roles list
- [ ] Audit log entry created

## Trade-offs

### Why This Approach?

**Chosen:** Standalone wizard with menu preview

**Rejected alternatives:**
1. **Enhanced menu builder** - Would overload already complex UI
2. **Dual interface** - Three ways to manage roles would confuse users

**Key benefits:**
- Clear guided workflow from scratch
- Educational - shows permission → menu access mapping
- Doesn't break existing CRUD or menu builder
- Single-purpose interface (easier to understand)

### Known Limitations

- Requires JavaScript for menu preview (graceful degradation: show all permissions without preview)
- Additional route compared to enhancing existing `/roles/new`
- Menu preview shows accessible items but doesn't explain permission logic

## Future Enhancements

- **Permission recommendations:** Suggest common permission sets (e.g., "Read-only analyst", "Content editor")
- **Role templates:** Clone existing roles as starting point
- **Permission search:** Filter permissions by keyword in Step 2
- **Diff view:** Compare permission sets between roles
- **Bulk user assignment:** Assign users to new role immediately after creation

## Success Metrics

- Admins can create roles without referring to documentation
- Menu preview reduces "why can't my users see X?" support tickets
- Role creation time decreases (fewer permission selection mistakes)
- Audit logs show consistent role naming conventions (validation helps)
