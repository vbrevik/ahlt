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

---

## Remaining Backlog

### Phase 2: Settings + CSRF

#### 3.1 — App Settings (entity_type = 'setting')
**Priority:** Medium | **Effort:** Small | **Status:** Ready

No new tables — settings are just entities with entity_type='setting'. Properties: `value`, `description`, `setting_type` (text/number/boolean).

Seed defaults: `app.name` = "Ahlt", `app.description` = "Administration Platform".

**Files:** `src/db.rs` (seed), new `src/models/setting.rs`

---

#### 3.2 — Settings management page
**Priority:** Medium | **Effort:** Medium

- `GET /settings` — list all settings in a form
- `POST /settings` — save changes
- Protected by `settings.manage` permission
- New seed: `admin.settings` nav_item with parent=admin, permission=settings.manage

**Files:** new `src/handlers/settings_handlers.rs`, `src/templates_structs.rs`, new `templates/settings.html`, `src/main.rs`, `src/db.rs` (seed nav item)

---

#### 3.3 — Use settings in runtime
**Priority:** Medium | **Effort:** Small

Replace hardcoded values with DB lookups:
- Page title / navbar brand from `app.name`
- Cache settings at startup, refresh on save

**Files:** `src/main.rs`, `templates/base.html`, `templates/partials/nav.html`

---

#### 5.4 — CSRF protection
**Priority:** Medium | **Effort:** Medium

Generate a random token per session, embed as hidden form field, validate on POST handlers.

**Files:** new `src/auth/csrf.rs`, all templates with forms, all POST handlers

---

### Phase 3: Polish

#### 6.1 — Change own password page
**Priority:** Medium | **Effort:** Small

`GET /account` — form with current password, new password, confirm.
`POST /account` — validate and update.

**Files:** new `src/handlers/account_handlers.rs`, new `templates/account.html`

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
Epic 1: Ontology Foundation   3.1 Settings entities       5.4 CSRF
Epic 2: Data-Driven Nav       3.2 Settings page           6.1 Change password
5.1 Self-deletion guard       3.3 Runtime settings        6.2 Error pages
5.2 Last admin guard                                      6.3 Pagination
5.3 Session key from env                                  6.4 Search/filter
4.1 Role Management UI                                    7.3 Audit trail
Ontology Explorer
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
