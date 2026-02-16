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
| `nav_item` | Menu entry (module or page) | `url`, `parent` *(permission via relation)* |
| `setting` | Key-value config *(planned)* | `value`, `description` |

### Relations in Use

| Relation Type | Source → Target | Purpose |
|---|---|---|
| `has_role` | user → role | User's assigned role |
| `has_permission` | role → permission | Role's granted permissions |
| `requires_permission` | nav_item → permission | Nav item access requirement |

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
- 2.3 Nav permissions via relations: converted nav_item permission checks from `permission_code` text properties to `requires_permission` relations (nav_item→permission), making permissions visible in ontology graph and consistent with EAV model

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

### Audit Trail (7.3)
- Two-tier system: high-value events in database (EAV), all events in filesystem (JSON Lines)
- Database: audit_entry entities with properties (user_id, action, target_type, target_id, summary)
- Filesystem: Daily-rotated .jsonl files in data/audit/ with secure permissions (0600/0700)
- Settings: audit.enabled, audit.log_path, audit.retention_days
- Retention cleanup on startup (configurable, 0=forever)
- UI: /audit with search, action filter, target type filter, pagination
- Permission: audit.view for viewing logs
- Integration: user create/delete, role create/delete/permissions_changed logged
- Error handling: logging failures never block requests (logged to stderr)

### Custom Error Pages (6.2)
- HTML templates for 404 and 500 errors with branded design
- templates/errors/404.html: "Page Not Found" with Go to Dashboard / Go Back buttons
- templates/errors/500.html: "Server Error" with Go to Dashboard / Try Again buttons
- Centered layout with warm gradient background matching login page, large amber error code
- Updated AppError::error_response() to serve HTML via include_str!() instead of plain text
- Registered default_service() handler for 404 fallback on unmatched routes
- CSS: .error-page, .error-icon, .error-content, .error-actions

### Pagination on User List (6.3)
- Query parameters: `?page=1&per_page=25` with defaults and clamping (1-100 per page)
- `UserPage` struct: bundles users Vec + pagination metadata (page, per_page, total_count, total_pages)
- `user::find_paginated()`: SQL LIMIT/OFFSET pattern with total count calculation
- Handler: `web::Query<PaginationQuery>` with `Option<i64>` fields for flexible defaults
- Template: conditional pagination controls (`{% if total_pages > 1 %}`) with Previous/Next buttons
- Page info display: "Page X of Y (Z total)" format
- CSS: `.pagination`, `.pagination-info`, `.pagination-controls` with disabled button states
- Graceful degradation: single-page datasets show clean interface without pagination UI

### Search/Filter on User List (6.4)
- Query parameter: `?q=searchterm` for filtering users by username or display name
- `user::find_paginated()` extended with `search: Option<&str>` parameter
- SQL: `WHERE e.entity_type = 'user' AND (e.name LIKE ?1 OR e.label LIKE ?1)` pattern with wildcard wrapping
- Bug fix: count query was missing table alias `e`, causing search clause (`e.name LIKE ?`) to fail
- Search form UI: input + Search button + conditional Clear link when active
- Pagination links preserve search query parameter: `?page=N&per_page=M&q=term`
- Search input displays current query value on page load

### Code Cleanup & Refactoring (Tasks 1-28)

**Phase 1-2: Error Handling Foundation** (Tasks 1-9)
- Enhanced AppError enum with 8 variants (Db, Pool, Template, Hash, NotFound, PermissionDenied, Session, Csrf)
- Implemented ResponseError trait for HTTP error responses (403 for permissions/CSRF, 404 for NotFound, 500 for others)
- Created render() helper for template rendering with automatic error conversion
- Updated session helpers to return Result types (get_user_id, get_username, get_permissions, require_permission)
- Updated PageContext::build to return Result<Self, AppError>
- Updated csrf::validate_csrf to return Result<(), AppError>
- Fixed 10 clippy warnings (unused imports, redundant enum names, collapsible ifs)
- Removed dead code (find_all_display function)

**Phase 3: Handler Migration** (Tasks 10-21)
- Migrated all 27 handlers to AppError pattern across 8 files
- User handlers (6): list, new_form, create, edit_form, update, delete
- Role handlers (6): list, new_form, create, edit_form, update, delete
- Audit handlers (1): list
- Account handlers (2): form, submit
- Settings handlers (2): list, save
- Ontology handlers (6): concepts, graph, graph_data, schema_data, data, data_detail
- Auth handlers (3): login_page, login_submit, logout
- Dashboard handler (1): index
- Impact: ~280 lines of boilerplate eliminated, consistent ? operator usage
- Code reviews caught missing validation (role update) and audit logging (user update) early

**Phase 4: File Splitting** (Tasks 23-28)
- Split models/ontology.rs (471 lines) → 4 modules (schema.rs, instance.rs, entities.rs, mod.rs)
- Split models/user.rs (370 lines) → 3 modules (types.rs, queries.rs, mod.rs)
- Split models/role.rs (323 lines) → 3 modules (types.rs, queries.rs, mod.rs)
- Split handlers/user_handlers.rs (236 lines) → 3 modules (list.rs, crud.rs, mod.rs)
- Split handlers/role_handlers.rs (~280 lines) → 4 modules (helpers.rs, list.rs, crud.rs, mod.rs)
- Total: 1,680 lines reorganized into 17 focused modules
- Impact: Better code organization, clearer separation of concerns, easier navigation

**Documentation**
- Created CLAUDE.md with comprehensive project context:
  - Architecture overview (stack, directory structure, EAV pattern)
  - Key patterns (AppError, session helpers, template rendering, EAV)
  - Gotchas (Askama 0.14, Actix-web 4, SQLite + r2d2)
  - Development workflows (adding handlers, audit logging, migrations)
  - Refactoring workflow (phased approach with reviews)
  - Code review checklist (CRUD consistency, audit logging, validation)
  - Verification commands (cargo check patterns, build status)
  - Time-saving analysis (18-28 hours avoided with upfront context)
  - Recent refactoring summary (Phases 1-4)

**Commits:** 22 commits created with detailed messages and co-authorship

### Frontend Design Review (6.6)
- Audited all templates and CSS for UX consistency issues
- Color-coded role badges: Administrator (dark), Editor (green), Viewer (blue), Manager (brown), Analyst (purple)
- Unified search/filter bar component with styled custom select arrows
- Improved empty states with title + descriptive text
- Muted ID columns in tables (monospace, small, light gray)
- Redesigned dashboard: personalized greeting, stat cards, permission-gated quick action cards with amber accent borders
- Enhanced roles list: monospace name, bold label, badge-styled permission/user counts
- Enhanced audit log: unified search form, styled filter dropdowns, proper empty state
- All pages visually verified via Playwright screenshots

### Menu Builder / Permission Matrix (4.2)
- Visual permission matrix: roles as columns, permissions as rows grouped by section (Admin, Dashboard, Roles, Settings, Users)
- `src/models/permission.rs`: `find_all_with_groups()`, `find_all_role_grants()`, `grant_permission()`, `revoke_permission()`
- `src/templates_structs.rs`: `MatrixCell`, `PermissionRow`, `PageGroup`, `RoleColumn`, `MenuBuilderTemplate`
- `src/handlers/menu_builder_handlers.rs`: GET `index()` builds matrix, POST `save()` diffs and applies changes
- Pre-computed matrix cells (Askama can't call `.contains()` in templates)
- Diff-based save: compare submitted checkboxes vs DB state, only INSERT/DELETE changes
- Unique checkbox names `perm_{role_id}_{permission_id}` avoid serde_urlencoded duplicate key issue
- JavaScript: change tracking with asterisk indicator, unsaved-changes warning (beforeunload), column toggle
- Audit logging with descriptive summary ("N granted, M revoked via Menu Builder")
- Nav item: `admin.menu_builder` under Admin module, gated by `roles.manage`
- CSS: sticky left column, grouped section headers, hover states, accent-colored checkboxes

### Manual Testing (Complete)
- Created comprehensive test data seed script with 4 roles and 5 users
- Generated proper argon2 password hash for "password123"
- Verified login/logout functionality with multiple users
- Tested user CRUD operations:
  - List page with all users displayed correctly
  - Search functionality filtering by username/name
  - Edit form pre-populated with user data
  - Create form with role dropdown
- Validated permission-based access control:
  - alice (Editor): can create/edit users, see Ontology/Settings, no Roles access
  - bob (Viewer): read-only access, no edit buttons, minimal sidebar navigation
  - Confirmed Actions column empty for users without edit permissions
  - Verified sidebar navigation filtered by user permissions
- Screenshot evidence captured: bob-viewer-users-list.png
- All 27 refactored handlers working correctly with AppError pattern
- Zero compilation errors, clean build

### Phase 2a: Item Pipeline (Complete)
- Created complete suggestion→proposal workflow
- Suggestion creation with form validation
- Auto-proposal creation when suggestion accepted
- Proposal CRUD with status transitions (draft→submitted→under_review→approved→rejected)
- Pipeline view with tabs for suggestions/proposals
- Integration tests validating complete workflow
- E2E testing with Playwright

### Phase 2b: Agenda Points, COAs & Data-Driven Workflows (Complete)
- **Data-Driven Workflow Engine**: WorkflowStatus + WorkflowTransition entities replace all hardcoded transitions
- **Agenda Points**: Meeting items (informative or decision) with scheduling from proposal queue
- **Courses of Action (COAs)**: Decision options with nested section support (simple or complex)
- **Opinion Recording**: Advisory input from participants (separate from decisions)
- **Decision Making**: Authority makes final decisions with veto power
- **Proposal Queue**: Mark proposals ready, bulk-schedule into agenda points
- **Terminology Rename**: Systematic pipeline→workflow rename (40+ files)
- **19 Tasks Delivered**: Infrastructure, models, handlers, templates, routes, E2E tests
- **Test Coverage**: 12 new Phase 2b tests all passing, plus existing Phase 2a tests still passing
- **Code Quality**: 0 new errors, integrated subagent-driven development for quality gates
- **Production Ready**: All routes wired, permissions integrated, audit logging, CSRF protection

### Phase 2a Automated Testing (Complete)
- **Test Dependencies**: Added tempfile, rusqlite, regex, serde_json to dev-dependencies
- **Infrastructure**: Shared `setup_test_db()` with TempDir, `insert_entity/prop/relation` helpers, `get_permissions_for_user` query, CSRF extraction
- **17 Tests Covering**:
  - Infrastructure (3): schema compilation, CSRF extraction, missing token handling
  - Authentication (3): user lookup, nonexistent user, permission assignment through role chain
  - User CRUD (3): create+retrieve with properties, update via upsert, delete with CASCADE verification
  - User Search (1): LIKE search on name/label, LIMIT/OFFSET pagination, entity type filtering
  - Data Integrity (2): UNIQUE(entity_type, name) constraint, UNIQUE relation constraint preventing duplicates
  - Permission Enforcement (3): admin has all permissions, viewer has limited permissions, no-role user has zero permissions
  - Role Lifecycle (1): grant permissions, inherit through role, revoke permission, verify loss
  - Nav Gating (1): requires_permission relation filtering nav items by user permissions
- **Code Quality**: 0 errors, 0 warnings, all 32 tests passing (17 Phase 2a + 12 Phase 2b + 3 workflow)

---

## Remaining Backlog

### Testing & Deployment
- Production deployment preparation (env vars, session key, etc.)

### Future Features
- Warnings system (already has navbar badge placeholder)
- More entity types (projects, tasks, documents, etc.)
- Configurable workflows
- API access for external integrations

---

## Implementation Order

```
DONE                                NEXT                          LATER
════                                ════                          ═════
Epic 1: Ontology Foundation         Production deployment          Warnings system
Epic 2: Data-Driven Nav                                            More entity types
5.1 Self-deletion guard                                            API access
5.2 Last admin guard
5.3 Session key from env
5.4 CSRF protection
4.1 Role Management UI
Ontology Explorer
3.1 Settings entities
3.2 Settings page
3.3 Runtime settings
6.1 Change password
6.2 Custom error pages
6.3 Pagination
6.4 Search/filter users
6.5 Navbar avatar dropdown
7.1 Git + GitHub
7.2 Favicon
7.3 Audit trail
PageContext refactor
2.3 Nav perms via relations
Code cleanup (Tasks 1-28):
- Phase 1-2: Error foundation
- Phase 3: Handler migration
- Phase 4: File splitting
Manual testing (complete)
6.6 Frontend design review
4.2 Menu Builder
Phase 2b Workflows (complete)
Phase 2a Testing (complete):
- Infrastructure foundation
- Full test suite (17 tests)
```

## Architecture Decisions

### Handler Pattern
Each handler follows: permission check → get conn → build PageContext → page query → template render. The `PageContext::build()` helper consolidates the 5 common fields (username, permissions, flash, nav_modules, sidebar_items) into a single constructor call.

**Decision:** Keep explicit handler bodies (Approach A) at current scale. The ~15-20 lines per GET handler are clear, debuggable, and easy to customize. When the app grows to 10+ handler files (roles, settings, etc.), adopt a `render()` helper (Approach B) and a proper `AppError` type with `ResponseError` impl (Approach D) — together these reduce GET handlers to ~8 lines with idiomatic `?` error propagation. The `AppError` skeleton already exists in `src/errors.rs`. See `docs/handler-patterns.md` for the full analysis of all 6 approaches considered.

### Nav Item Hierarchy
Top-level items with no children are standalone (Dashboard). Items with children are modules (Admin) — visible if any child passes permission check. Children appear in sidebar when their parent module is active. Active module detection checks child URLs first for correct prefix matching.

### EAV Trade-offs
The generic schema means zero migrations when adding new entity types. The trade-off is more complex queries (LEFT JOINs on entity_properties). Typed domain structs (UserDisplay, RoleDisplay) provide a stable API layer over the generic storage.
