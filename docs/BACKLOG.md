# Ahlt — Product Backlog

## Vision

Transform Ahlt from a hardcoded admin panel into a **data-driven platform** where behavior, access control, navigation, and configuration are all defined by an **ontology** (structured data in the database), not by code. The system should be extensible without recompilation.

---

## Ontology Model (Actual Implementation)

All domain objects share three generic tables — no dedicated tables per type:

```
┌──────────────────────────┐
│        entities           │
│ id, entity_type, name,    │
│ label, sort_order,        │
│ is_active, timestamps     │
│ UNIQUE(entity_type, name) │
└──────────┬───────────────┘
           │ 1:N
┌──────────┴───────────────┐     ┌──────────────────────────┐
│   entity_properties       │     │        relations          │
│ entity_id, key, value     │     │ id, relation_type_id,     │
│ PK(entity_id, key)        │     │ source_id, target_id      │
└──────────────────────────┘     │ UNIQUE(type,src,tgt)      │
                                  └──────────────────────────┘
```

### Entity Types in Use

| entity_type | Purpose | Key Properties |
|---|---|---|
| `relation_type` | Named relationship kinds | — |
| `role` | Named collection of permissions | `description`, `is_default` |
| `permission` | Atomic capability | `group_name` |
| `user` | Account with role relation | `password`, `email` |
| `nav_item` | Menu entry (module or page) | `url`, `permission_code`, `parent` |
| `setting` | Key-value config *(planned)* | `value`, `description` |

### Relations in Use

| Relation Type | Source → Target | Purpose |
|---|---|---|
| `has_role` | user → role | User's assigned role |
| `has_permission` | role → permission | Role's granted permissions |

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

### Navigation Hierarchy (seed data)

| Name | Label | Parent | URL | Permission |
|---|---|---|---|---|
| `dashboard` | Dashboard | — | `/dashboard` | *(all logged-in)* |
| `admin` | Admin | — | `/users` | *(visible if any child permitted)* |
| `admin.users` | Users | `admin` | `/users` | `users.list` |
| `admin.roles` | Roles | `admin` | `/roles` | `roles.manage` |
| `admin.ontology` | Ontology | `admin` | `/ontology` | `settings.manage` |
| `admin.settings` | Settings *(planned)* | `admin` | `/settings` | `settings.manage` |

---

## Completed Work

### Epic 1: Ontology Foundation
- 1.1 EAV schema (entities, entity_properties, relations) with role + permission entities
- 1.2 User→role via has_role relation (replaced text role field)
- 1.3 Data-driven role dropdown from DB query
- 1.4 Permission-based auth (session CSV storage, `require_permission()` helper)

### Epic 2: Data-Driven Navigation
- 2.1 Nav items as entities with parent-child hierarchy
- 2.2 Two-tier rendering: modules in header, pages in sidebar
- Active state detection (children first, then top-level fallback)
- Permission-gated visibility (module visible if any child permitted)

### Security (partial)
- 5.1 Self-deletion protection
- 5.2 Last admin protection

### Housekeeping (partial)
- 7.1 Git init + push to GitHub
- 7.2 Favicon (inline SVG data URI)

### Infrastructure
- PageContext struct (bundles username, permissions, flash, nav_modules, sidebar_items)
- `PageContext::build()` constructor reduces handler boilerplate

### Security (continued)
- 5.3 Persistent session key from `SESSION_KEY` env var (falls back to `Key::generate()` with warning)

### Role Management (4.1)
- Full CRUD: list (with permission/user counts), create, edit, delete (with user-assigned guard)
- Permission checkboxes with manual form body parsing (serde_urlencoded can't handle duplicate keys)
- `admin.roles` nav item under Admin module

### Ontology Explorer
- Three-tab explorer: Concepts (schema-level D3 graph) + Data (instance-level D3 graph) + Reference (entity type cards, relation patterns, schema docs)
- JSON APIs: `/ontology/api/schema` (entity types as nodes, relation patterns as edges) + `/ontology/api/graph` (all entity instances + properties)
- Concepts tab: schema graph with entity type nodes (sized by count), relation pattern edges, toolbar (fit-all, zoom in/out, reset, lock positions), keyboard shortcuts (F/+/-/0/L/Esc), auto-fit on simulation settle
- Data tab: instance graph with type filtering, node hover highlighting, click detail panel with properties + connections + "Full detail" link, drag/zoom/pan
- Reference tab: entity type summary cards, relation pattern breakdowns, schema reference tables
- `admin.ontology` nav item under Admin module, gated by `settings.manage`

### App Settings (3.1 + 3.2 + 3.3)
- Settings as entities with entity_type='setting', properties: `value`, `description`, `setting_type` (text/number/boolean)
- Seeded defaults: `app.name` = "Ahlt", `app.description` = "Administration Platform"
- `GET /settings` form + `POST /settings` save with upsert, protected by `settings.manage`
- `admin.settings` nav item under Admin module
- Supports text, number, and boolean (select dropdown) field types
- Runtime integration: `app.name` drives navbar brand, page titles (all templates), and login page brand
- `setting::get_value()` used in `PageContext::build()` (authenticated pages) and login handler
- No caching — simple DB lookup per request, sufficient at current scale

### CSRF Protection (5.4)
- Token generation: 32 random bytes hex-encoded, stored in session
- `src/auth/csrf.rs`: `get_or_create_token()` + `validate_csrf()` with constant-time comparison
- All 7 form templates updated with hidden `csrf_token` input field
- All 9 POST handlers validate CSRF before processing
- Form structs: added `csrf_token` field to `LoginForm`, `UserForm`; raw body handlers extract from parsed params; body-less handlers use shared `CsrfOnly` struct
- Dependencies: `rand = "0.9"`, `hex = "0.4"`

### Change Password (6.1)
- `GET /account`: form with current/new/confirm password fields + CSRF token
- `POST /account`: validates current password, checks new==confirm, updates via upsert on entity_properties
- New functions: `user::find_password_hash_by_id()`, `user::update_password()`
- Navbar username link changed to clickable link to `/account`
- Flash message on successful password change
- Form errors: wrong current password, mismatch confirmation

### Navbar Avatar Dropdown (6.5)
- Replaced username text + separate logout form with avatar dropdown
- Avatar: circular button showing user initial (first letter, uppercase) in accent color
- Dropdown panel: username header, Profile (→ /account), Warnings (→ /warnings with badge), divider, Logout (red, CSRF form)
- Badge support: `warning_count` field in PageContext (currently 0, ready for warnings feature), red notification badge on avatar when warnings > 0
- Three-section centered navbar layout: brand (left, flex:1), modules (center), user dropdown (right, flex:1)
- Click-outside-to-close with global document listener
- CSS: `.user-dropdown`, `.avatar`, `.dropdown-panel`, `.badge-count` classes with animation

---

## Remaining Backlog

### Phase 3: Polish

---

#### 6.2 — Custom error pages
**Priority:** Low | **Effort:** Small

Askama templates for 404 and 500 errors. Register via actix-web's custom error handlers.

**Files:** new `templates/errors/404.html`, `templates/errors/500.html`, `src/errors.rs`

---

#### 6.3 — Pagination on user list
**Priority:** Low | **Effort:** Small

Accept `?page=1&per_page=25` query params. SQL: `LIMIT ? OFFSET ?`. Render prev/next links.

**Files:** `src/handlers/user_handlers.rs`, `src/models/user.rs`, `templates/users/list.html`

---

#### 6.4 — Search/filter users
**Priority:** Low | **Effort:** Small

Accept `?q=searchterm` query param. SQL: `WHERE name LIKE ? OR label LIKE ?` on user entities.

**Files:** `src/handlers/user_handlers.rs`, `src/models/user.rs`, `templates/users/list.html`

---

#### 7.3 — Audit trail
**Priority:** Low | **Effort:** Medium

New entity_type `audit_entry` with properties: `user_id`, `action`, `target_type`, `target_id`, `details`. Write entries from handlers on create/update/delete.

---

## Implementation Order

```
DONE                          NEXT                        LATER
════                          ════                        ═════
Epic 1: Ontology Foundation   6.2 Error pages             6.3 Pagination
Epic 2: Data-Driven Nav       7.3 Audit trail             6.4 Search/filter
5.1 Self-deletion guard
5.2 Last admin guard
5.3 Session key from env
5.4 CSRF protection
4.1 Role Management UI
Ontology Explorer
3.1 Settings entities
3.2 Settings page
3.3 Runtime settings
6.1 Change password
6.5 Navbar avatar dropdown
7.1 Git + GitHub
7.2 Favicon
PageContext refactor
```

## Architecture Decisions

### Handler Pattern
Each handler follows: permission check → get conn → build PageContext → page query → template render. The `PageContext::build()` helper consolidates the 5 common fields (username, permissions, flash, nav_modules, sidebar_items) into a single constructor call.

**Decision:** Keep explicit handler bodies (Approach A) at current scale. The ~15-20 lines per GET handler are clear, debuggable, and easy to customize. When the app grows to 10+ handler files (roles, settings, etc.), adopt a `render()` helper (Approach B) and a proper `AppError` type with `ResponseError` impl (Approach D) — together these reduce GET handlers to ~8 lines with idiomatic `?` error propagation. The `AppError` skeleton already exists in `src/errors.rs`. See `docs/handler-patterns.md` for the full analysis of all 6 approaches considered.

### Nav Item Hierarchy
Top-level items with no children are standalone (Dashboard). Items with children are modules (Admin) — visible if any child passes permission check. Children appear in sidebar when their parent module is active. Active module detection checks child URLs first for correct prefix matching.

### EAV Trade-offs
The generic schema means zero migrations when adding new entity types. The trade-off is more complex queries (LEFT JOINs on entity_properties). Typed domain structs (UserDisplay, RoleDisplay) provide a stable API layer over the generic storage.
