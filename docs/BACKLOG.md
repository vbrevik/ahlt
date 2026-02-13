# Alt — Product Backlog

## Vision

Transform Alt from a hardcoded admin panel into a **data-driven platform** where behavior, access control, navigation, and configuration are all defined by an **ontology** (structured data in the database), not by code. The system should be extensible without recompilation.

---

## Ontology Model

The core ontology introduces these entities and relationships:

```
┌──────────┐     ┌──────────────┐     ┌──────────────┐
│   Role   │────<│ RolePermission│>────│  Permission  │
└──────────┘     └──────────────┘     └──────────────┘
     │                                       │
     │ (user.role_id FK)                     │ (nav_item.permission)
     │                                       │
┌──────────┐                          ┌──────────────┐
│   User   │                          │   NavItem    │
└──────────┘                          └──────────────┘
                                             │
                                      ┌──────────────┐
                                      │  AppSetting  │
                                      └──────────────┘
```

### Entity Definitions

| Entity | Purpose | Key Fields |
|--------|---------|------------|
| **Role** | Named collection of permissions | `id`, `name`, `label`, `description`, `is_default`, `sort_order` |
| **Permission** | Atomic capability | `id`, `code` (e.g. `users.create`), `label`, `group` |
| **RolePermission** | Many-to-many link | `role_id`, `permission_id` |
| **User** | Account with role FK | existing fields + `role_id` (FK to Role, replaces `role` text) |
| **NavItem** | Menu entry | `id`, `label`, `url`, `icon`, `permission_code`, `sort_order`, `parent_id` |
| **AppSetting** | Key-value config | `key`, `value`, `description` |

### Permission Codes (seed data)

| Code | Description |
|------|-------------|
| `dashboard.view` | View the dashboard |
| `users.list` | View user list |
| `users.create` | Create new users |
| `users.edit` | Edit existing users |
| `users.delete` | Delete users |
| `roles.manage` | Create/edit/delete roles and assign permissions |
| `settings.manage` | Modify app settings |

---

## Backlog Items

### Epic 1: Ontology Foundation

> Introduce the core ontology tables and migrate from hardcoded role strings to data-driven roles and permissions.

#### 1.1 — Create Role, Permission, RolePermission tables
**Priority:** Critical
**Effort:** Medium

Add new tables to the migration in `src/db.rs`:

```sql
CREATE TABLE IF NOT EXISTS roles (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL UNIQUE,   -- 'admin', 'editor', 'viewer'
    label       TEXT NOT NULL,          -- 'Administrator', 'Editor', 'Viewer'
    description TEXT NOT NULL DEFAULT '',
    is_default  INTEGER NOT NULL DEFAULT 0,
    sort_order  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS permissions (
    id    INTEGER PRIMARY KEY AUTOINCREMENT,
    code  TEXT NOT NULL UNIQUE,   -- 'users.create', 'users.delete'
    label TEXT NOT NULL,          -- 'Create Users'
    group_name TEXT NOT NULL DEFAULT ''  -- 'Users', 'Dashboard', 'Settings'
);

CREATE TABLE IF NOT EXISTS role_permissions (
    role_id       INTEGER NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission_id INTEGER NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    PRIMARY KEY (role_id, permission_id)
);
```

Seed default roles and permissions on startup (same pattern as admin user seed).

**Files:** `src/db.rs`, new `src/models/role.rs`, new `src/models/permission.rs`

---

#### 1.2 — Migrate User.role from text to role_id FK
**Priority:** Critical
**Effort:** Medium

- Add `role_id INTEGER REFERENCES roles(id)` to users table
- Remove the `role TEXT` column
- Update `User` struct, `NewUser`, `UserForm`, all CRUD queries
- Update the seed logic: create "admin" role first, then create admin user with `role_id`
- Update session: store `role_id` instead of role string

**Files:** `src/db.rs`, `src/models/user.rs`, `src/handlers/user_handlers.rs`, `src/handlers/auth_handlers.rs`, `src/main.rs`

---

#### 1.3 — Data-driven role dropdown in user form
**Priority:** High
**Effort:** Small

Replace the hardcoded `<option value="admin">` / `<option value="user">` in `templates/users/form.html` with a loop over roles fetched from the database.

- Add `roles: Vec<RoleDisplay>` to `UserFormTemplate`
- Query all roles in `new_form` and `edit_form` handlers
- Template: `{% for role in roles %}<option value="{{ role.id }}">{{ role.label }}</option>{% endfor %}`

**Files:** `src/templates_structs.rs`, `templates/users/form.html`, `src/handlers/user_handlers.rs`, `src/models/role.rs`

---

#### 1.4 — Permission-based auth middleware
**Priority:** Critical
**Effort:** Large

Replace the binary "logged in or not" check with permission-aware authorization.

- On login, load the user's permissions from DB (via role → role_permissions → permissions)
- Store permission codes in session (or cache them per-request)
- Create a `require_permission(code: &str)` helper that handlers can call
- Alternatively: store permissions in a middleware-injected `AuthContext` that handlers extract

**Design decision:** Per-route permission vs per-handler check:
- **Option A — Route-level:** Wrap groups of routes with permission-specific middleware. Clean but inflexible.
- **Option B — Handler-level:** Each handler checks `auth.has_permission("users.delete")`. More flexible, recommended.

**Files:** `src/auth/middleware.rs`, new `src/auth/context.rs`, `src/handlers/*.rs`

---

### Epic 2: Data-Driven Navigation

> Navigation should be driven by the ontology, not hardcoded HTML.

#### 2.1 — Create NavItem table and seed menu entries
**Priority:** High
**Effort:** Small

```sql
CREATE TABLE IF NOT EXISTS nav_items (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    label           TEXT NOT NULL,
    url             TEXT NOT NULL,
    icon            TEXT NOT NULL DEFAULT '',
    permission_code TEXT NOT NULL DEFAULT '',  -- empty = visible to all logged-in
    sort_order      INTEGER NOT NULL DEFAULT 0,
    parent_id       INTEGER REFERENCES nav_items(id) ON DELETE SET NULL
);
```

Seed: Dashboard (sort 1), Users (sort 2, permission `users.list`).

**Files:** `src/db.rs`, new `src/models/nav_item.rs`

---

#### 2.2 — Render navigation from database
**Priority:** High
**Effort:** Medium

- Query visible nav items for the current user's permissions
- Pass `nav_items: Vec<NavItemDisplay>` to every template that includes the nav partial
- Replace hardcoded links in `templates/partials/nav.html` with a loop:
  ```
  {% for item in nav_items %}
  <a href="{{ item.url }}">{{ item.label }}</a>
  {% endfor %}
  ```
- Active state: compare `item.url` with current path

**Files:** `src/models/nav_item.rs`, `src/templates_structs.rs` (all structs), `templates/partials/nav.html`

---

### Epic 3: App Settings (Key-Value Config)

> Move configuration out of code and into the database.

#### 3.1 — Create AppSetting table
**Priority:** Medium
**Effort:** Small

```sql
CREATE TABLE IF NOT EXISTS app_settings (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT ''
);
```

Seed defaults: `app.name` = "Alt Admin", `app.bind_address` = "127.0.0.1:8080", `auth.session_timeout_minutes` = "1440", `seed.default_password` = "admin123".

**Files:** `src/db.rs`, new `src/models/setting.rs`

---

#### 3.2 — Settings management page
**Priority:** Medium
**Effort:** Medium

- `GET /settings` — list all settings in a form
- `POST /settings` — save changes
- Protected by `settings.manage` permission
- New template: `templates/settings.html`
- Add "Settings" nav item (permission-gated)

**Files:** new `src/handlers/settings_handlers.rs`, `src/templates_structs.rs`, `templates/settings.html`, `src/main.rs`

---

#### 3.3 — Use settings in runtime
**Priority:** Medium
**Effort:** Small

Replace hardcoded values with DB lookups:
- Page title from `app.name` (passed to base template)
- Navbar brand text from `app.name`
- Default role for new users from `auth.default_role`

Cache settings at startup and refresh on change.

**Files:** `src/main.rs`, `templates/base.html`, `templates/partials/nav.html`

---

### Epic 4: Role Management UI

> Allow admins to create, edit, and delete roles and assign permissions through the UI.

#### 4.1 — Role CRUD pages
**Priority:** Medium
**Effort:** Medium

| Route | Purpose |
|-------|---------|
| `GET /roles` | List all roles with permission counts |
| `GET /roles/new` | Create role form with permission checkboxes |
| `POST /roles` | Submit new role |
| `GET /roles/{id}/edit` | Edit role form |
| `POST /roles/{id}` | Update role |
| `POST /roles/{id}/delete` | Delete role (prevent if users assigned) |

Templates: `templates/roles/list.html`, `templates/roles/form.html`

**Files:** new `src/handlers/role_handlers.rs`, `src/templates_structs.rs`, `templates/roles/`, `src/main.rs`

---

### Epic 5: Security Hardening

#### 5.1 — Self-deletion protection
**Priority:** High
**Effort:** Tiny

In `user_handlers::delete`, check if `auth.user_id == id` and reject with flash error "You cannot delete your own account".

**Files:** `src/handlers/user_handlers.rs`

---

#### 5.2 — Last admin protection
**Priority:** High
**Effort:** Small

Prevent deleting or demoting the last user with admin role. Query count of admin-role users before allowing the operation.

**Files:** `src/handlers/user_handlers.rs`, `src/models/user.rs`

---

#### 5.3 — Persistent session key from environment
**Priority:** High
**Effort:** Tiny

Load `SESSION_SECRET` from env var (base64-encoded 64-byte key). Fall back to `Key::generate()` with a warning log.

**Files:** `src/main.rs`

---

#### 5.4 — CSRF protection
**Priority:** Medium
**Effort:** Medium

Generate a random token per session, embed as a hidden form field, validate on POST handlers. Reject requests with missing/invalid tokens.

**Files:** `src/auth/csrf.rs`, all templates with forms, all POST handlers

---

### Epic 6: UX Polish

#### 6.1 — Change own password page
**Priority:** Medium
**Effort:** Small

`GET /account` — form with current password, new password, confirm. `POST /account` — validate and update.

**Files:** new `src/handlers/account_handlers.rs`, `templates/account.html`

---

#### 6.2 — Custom error pages
**Priority:** Low
**Effort:** Small

Askama templates for 404 and 500 errors. Register via actix-web's custom error handlers.

**Files:** `templates/errors/404.html`, `templates/errors/500.html`, `src/errors.rs`

---

#### 6.3 — Pagination on user list
**Priority:** Low
**Effort:** Small

Accept `?page=1&per_page=25` query params. SQL: `LIMIT ? OFFSET ?`. Render prev/next links.

**Files:** `src/handlers/user_handlers.rs`, `src/models/user.rs`, `templates/users/list.html`

---

#### 6.4 — Search/filter users
**Priority:** Low
**Effort:** Small

Accept `?q=searchterm` query param. SQL: `WHERE username LIKE ? OR display_name LIKE ? OR email LIKE ?`.

**Files:** `src/handlers/user_handlers.rs`, `src/models/user.rs`, `templates/users/list.html`

---

### Epic 7: Housekeeping

#### 7.1 — Git init + .gitignore
**Priority:** High
**Effort:** Tiny

Initialize repo. Ignore: `/target`, `/data`, `*.db`, `.env`.

---

#### 7.2 — Favicon
**Priority:** Low
**Effort:** Tiny

Add a simple favicon.ico to `static/` to stop the 404 on every page load.

---

#### 7.3 — Audit trail table
**Priority:** Low
**Effort:** Medium

Log who did what and when: `audit_log(id, user_id, action, entity_type, entity_id, details, created_at)`. Write entries from handlers on create/update/delete.

---

## Implementation Order

The dependency chain dictates this sequence:

```
Phase 1: Ontology Core          Phase 2: Data-Driven UI       Phase 3: Polish
─────────────────────           ──────────────────────        ──────────────
1.1 Role/Permission tables      2.1 NavItem table             5.4 CSRF
        │                       2.2 Dynamic nav               6.1 Change password
        ▼                               │                     6.2 Error pages
1.2 Migrate user.role → role_id         ▼                     6.3 Pagination
        │                       3.1 AppSetting table          6.4 Search
        ▼                       3.2 Settings page             7.2 Favicon
1.3 Data-driven role dropdown   3.3 Runtime settings          7.3 Audit trail
        │
        ▼
1.4 Permission-based auth
        │
        ▼
4.1 Role management UI
        │
        ▼
5.1 Self-deletion guard
5.2 Last admin guard
5.3 Persistent session key

Standalone (do anytime):
7.1 Git init
```

## Current Hardcoded Items → Ontology Migration Map

| What's Hardcoded | Where | Becomes |
|-------------------|-------|---------|
| Role strings `"admin"`, `"user"` | `main.rs`, `db.rs`, `form.html` | `roles` table rows |
| Role dropdown options | `templates/users/form.html:40-41` | Loop over `roles` query |
| Permission check (none — binary auth) | `auth/middleware.rs` | `role_permissions` lookup |
| Nav links (Dashboard, Users) | `templates/partials/nav.html` | `nav_items` table rows |
| App name "Alt Admin" | `templates/partials/nav.html` | `app_settings.app.name` |
| Default role `'user'` | `db.rs:13` | `roles.is_default = 1` |
| Seed password `"admin123"` | `main.rs:18` | `app_settings.seed.default_password` |
| Bind address `127.0.0.1:8080` | `main.rs:89` | `app_settings.app.bind_address` or env var |
