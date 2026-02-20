# Users / Roles / Role Builder Separation Design

**Date**: 2026-02-20
**Status**: Implemented

## Problem

Users, Roles, and Role Builder have overlapping concerns:
- Users page embeds role assignment in the create/edit form
- Two ways to create a role (legacy `/roles/new` + Role Builder)
- No dedicated role assignment view
- These three areas may be operated by completely different people

## Design: Three Distinct Pages

Each page is independently operable with its own permission requirement and no cross-dependencies.

---

### Page 1: Users (`/users`) — User Account Management

**Operator**: HR / onboarding person
**Permission**: `users.manage`

**What it does**:
- List, create, edit, delete user accounts
- Fields: username, password, email, display_name
- Features: filter, sort, pagination, CSV export, bulk delete, column picker

**What changes**:
- Remove role dropdown from create/edit form
- Remove role badge/column from list table
- Auto-assign a configurable default role on user creation (defaults to "viewer")
- User model `create()` and `update()` no longer touch `has_role` relations

**Routes** (unchanged except removal of role logic):
```
GET  /users              → list
GET  /users/new          → create form
POST /users              → create (auto-assigns default role)
GET  /users/{id}/edit    → edit form (no role field)
POST /users/{id}         → update (no role change)
POST /users/{id}/delete  → delete
POST /users/bulk-delete  → bulk delete
GET  /users/export.csv   → CSV export
POST /users/columns      → column preferences
```

---

### Page 2: Roles (`/roles`) — Role Assignment

**Operator**: Team lead / manager
**Permission**: `roles.assign` (new permission)

**What it does**:
- Assign and unassign roles to users
- Users can hold **multiple roles simultaneously**
- Effective permissions = union of all assigned role permissions
- Two views via tab toggle

**Tab 1: By Role** (default)
- Horizontal role selector (tabs/pills)
- Click a role → see member list
- Each member: display name, username, [Remove] button
- [+ Add User] → dropdown of users not in this role
- Footer: "N permissions · [Edit in Role Builder]"

**Tab 2: By User**
- Table: User | Assigned Roles (badges) | Actions
- Each role badge has an × to remove
- [+ Add Role] dropdown per user
- Search/filter by user name or role

**Menu Preview Panel**:
- Click a user → collapsible panel shows "Effective Menu for [User]"
- Shows combined nav items from all assigned roles
- Reuses `find_accessible_nav_items()` with union of permission IDs
- Updates when roles change

**Data model change**:
- `has_role` becomes many-to-many (remove "delete old before create new" logic)
- Session permission loading aggregates across all roles
- `get_permissions()` must union permissions from all user roles

**Routes**:
```
GET  /roles              → assignment page (both tabs)
POST /roles/assign       → assign role(s) to user(s)
POST /roles/unassign     → remove role from user
GET  /api/roles/preview  → menu preview JSON for a user (AJAX)
```

---

### Page 3: Role Builder (`/roles/builder`) — Role Definition

**Operator**: IT / security admin
**Permission**: `roles.manage` (existing)

**What it does**:
- Create, edit, delete role definitions
- Manage which permissions a role grants
- 2-step wizard: details → permissions with live menu preview

**What changes**:
- Remove legacy `/roles/new` form and route (delete `templates/roles/form.html`)
- Remove legacy role CRUD handlers (`new_form`, `create` in `role_handlers/crud.rs`)
- "New Role" button on roles list → `/roles/builder`
- Add delete button on builder edit form (if no users assigned)
- Role Builder becomes the sole path for role CRUD

**Future (not this phase)**: Role inheritance
- "Parent role" selector on Step 1
- Child inherits parent permissions + own additions
- Preview shows combined inherited + explicit

**Routes** (unchanged):
```
GET  /roles/builder              → create wizard
GET  /roles/builder/{id}/edit    → edit wizard
POST /roles/builder/create       → create role
POST /roles/builder/update       → update role
POST /roles/builder/preview      → permission preview JSON (AJAX)
POST /roles/builder/{id}/delete  → delete role (new)
```

---

## Navigation Structure

```
Admin
  ├─ Users          (users.manage)
  ├─ Roles          (roles.assign)     ← NEW permission
  ├─ Role Builder   (roles.manage)
  ├─ Audit Log
  └─ Settings
```

## Permission Changes

| Permission | Purpose | Operator |
|-----------|---------|----------|
| `users.manage` | Create/edit/delete users | HR |
| `roles.assign` | Assign/unassign roles to users | Team lead |
| `roles.manage` | Create/edit/delete role definitions | IT/Security |

`roles.assign` is a new permission entity to be added to the ontology seed.

## Multi-Role Data Model

**Current** (1:1): User → `has_role` → Role (old relation deleted before new one created)

**New** (many-to-many): User → `has_role` → Role (multiple relations allowed)

**Session permission loading** (`get_permissions()`):
```sql
SELECT DISTINCT p.name
FROM relations r_role
JOIN relations r_perm ON r_perm.source_id = r_role.target_id
JOIN entities p ON r_perm.target_id = p.id
WHERE r_role.source_id = ?user_id
  AND r_role.relation_type_id = (has_role type)
  AND r_perm.relation_type_id = (has_permission type)
  AND p.entity_type = 'permission'
```

**Default role on user creation**:
- Configurable via entity property on the system/app entity, or hardcoded to "viewer"
- `user::create()` inserts a `has_role` relation to the default role after creating the user entity

## Files Affected

### Delete
- `templates/roles/form.html` (legacy create form)
- Legacy role create handlers in `role_handlers/crud.rs` (`new_form`, `create`)

### Modify
- `src/handlers/user_handlers/crud.rs` — remove role_id from create/update
- `src/handlers/role_handlers/list.rs` — new assignment page with tabs
- `src/handlers/role_handlers/crud.rs` — new assign/unassign handlers
- `src/auth/session.rs` — aggregate permissions across multiple roles
- `templates/users/list.html` — remove role column
- `templates/users/form.html` — remove role dropdown
- `templates/roles/list.html` — complete rewrite as assignment page
- `src/models/role/queries.rs` — multi-role queries
- `src/models/user/queries.rs` — remove role handling from user CRUD
- `data/seed/ontology.json` — add `roles.assign` permission

### Create
- `templates/roles/assignment.html` — new assignment page template
- Assignment-specific handlers (could go in `role_handlers/` or new `role_assignment_handlers.rs`)
