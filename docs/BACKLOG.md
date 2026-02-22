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
| `relation_type` | Named relationship kinds (35 defined) | — |
| `role` | Named collection of permissions | `description`, `is_default` |
| `permission` | Atomic capability (30+ defined) | `group_name` |
| `user` | Account with role relation | `password`, `email` |
| `nav_item` | Menu entry (module or page) | `url`, `parent` *(permission via relation)* |
| `setting` | Key-value config (8 defined) | `value`, `description`, `setting_type` |
| `workflow_status` | State in a workflow (suggestion/proposal) | `entity_type_scope`, `status_code`, `is_initial`, `is_terminal` |
| `workflow_transition` | Allowed state change | `from_status_code`, `to_status_code`, `required_permission` |
| `tor` | Term of Reference | `description`, `status`, `purpose` |
| `suggestion` | Workflow item (suggestion stage) | `description`, `status`, `submitted_by` |
| `proposal` | Workflow item (proposal stage) | `description`, `status`, `submitted_by` |
| `agenda_point` | Meeting agenda item | `type` (informative/decision), `status` |
| `coa` | Course of Action (decision option) | `description`, `type` (simple/complex) |
| `opinion` | Advisory input from participants | `stance`, `rationale` |
| `warning` | System notification | `severity`, `category`, `source_action`, `details` |
| `warning_receipt` | Per-user warning delivery | `status` (unread/read/deleted) |
| `warning_event` | Warning audit trail entry | `event_type`, `performed_by` |
| `audit_entry` | Audit log record | `user_id`, `action`, `target_type`, `summary` |

### Relations in Use

| Relation Type | Source → Target | Purpose |
|---|---|---|
| `has_role` | user → role | User's assigned roles (many-to-many) |
| `has_permission` | role → permission | Role's granted permissions |
| `requires_permission` | nav_item → permission | Nav item access requirement |
| `member_of` | user → tor | ToR membership |
| `has_tor_role` | user → tor | User's role within a ToR |
| `belongs_to_tor` | entity → tor | Entity scoped to a ToR |
| `suggested_to` | suggestion → tor | Suggestion target |
| `spawns_proposal` | suggestion → proposal | Auto-creation link |
| `submitted_to` | proposal → tor | Proposal target |
| `transition_from` | workflow_transition → workflow_status | Transition source state |
| `transition_to` | workflow_transition → workflow_status | Transition target state |
| `considers_coa` | agenda_point → coa | Decision options |
| `originates_from` | agenda_point → proposal | Agenda from proposal |
| `has_section` / `has_subsection` | coa → coa | Nested COA structure |
| `spawns_agenda_point` | proposal → agenda_point | Scheduling link |
| `opinion_by` / `opinion_on` / `prefers_coa` | user → opinion / opinion → agenda/coa | Opinion tracking |
| `targets_user` | warning → user | Warning target |
| `for_warning` / `for_user` / `on_receipt` | warning_receipt → entities | Receipt links |
| `forwarded_to_user` | warning_event → user | Forward tracking |

### Permission Codes (seed data)

| Code | Group | Description |
|------|-------|-------------|
| `dashboard.view` | Dashboard | View the dashboard |
| `users.list` | Users | View user list |
| `users.create` | Users | Create new users |
| `users.edit` | Users | Edit existing users |
| `users.delete` | Users | Delete users |
| `roles.manage` | Roles | Create/edit/delete roles and assign permissions |
| `roles.assign` | Roles | Assign/unassign roles to users |
| `settings.manage` | Settings | Modify app settings |
| `audit.view` | Admin | View audit log |
| `warnings.view` | Admin | View warnings |
| `tor.list` | Governance | List Terms of Reference |
| `tor.create` | Governance | Create Terms of Reference |
| `tor.edit` | Governance | Edit Terms of Reference |
| `tor.manage_members` | Governance | Manage ToR members |
| `suggestion.view` | Workflow | View suggestions |
| `suggestion.create` | Workflow | Submit new suggestions |
| `suggestion.review` | Workflow | Accept or reject suggestions |
| `proposal.view` | Workflow | View proposals |
| `proposal.create` | Workflow | Create new proposals |
| `proposal.edit` | Workflow | Edit draft proposals |
| `proposal.submit` | Workflow | Submit drafts for review |
| `proposal.review` | Workflow | Move proposals to under_review |
| `proposal.approve` | Workflow | Approve or reject proposals |
| `agenda.view` | Governance | View agenda |
| `agenda.create` | Governance | Create agenda points |
| `agenda.queue` | Governance | Queue proposals for agenda |
| `agenda.manage` | Governance | Manage agenda status |
| `agenda.participate` | Governance | Participate in meeting |
| `agenda.decide` | Governance | Make final decisions |
| `coa.create` | Workflow | Create courses of action |
| `coa.edit` | Workflow | Edit courses of action |
| `workflow.manage` | Governance | Manage workflow system |

### Navigation Hierarchy (seed data)

| Name | Label | Parent | URL | Permission |
|---|---|---|---|---|
| `dashboard` | Dashboard | — | `/dashboard` | `dashboard.view` |
| `admin` | Admin | — | `/users` | *(visible if any child permitted)* |
| `admin.users` | Users | `admin` | `/users` | `users.list` |
| `admin.roles` | Roles | `admin` | `/roles` | `roles.assign` |
| `admin.ontology` | Ontology | `admin` | `/ontology` | `settings.manage` |
| `admin.settings` | Settings | `admin` | `/settings` | `settings.manage` |
| `admin.audit` | Audit Log | `admin` | `/audit` | `audit.view` |
| `admin.menu_builder` | Menu Builder | `admin` | `/menu-builder` | `roles.manage` |
| `admin.warnings` | Warnings | `admin` | `/warnings` | `warnings.view` |
| `admin.role_builder` | Role Builder | `admin` | `/roles/builder` | `roles.manage` |
| `governance` | Governance | — | `/tor` | *(visible if any child permitted)* |
| `governance.tor` | Terms of Reference | `governance` | `/tor` | `tor.list` |
| `governance.map` | Governance Map | `governance` | `/governance/map` | `tor.list` |
| `governance.workflow` | Item Workflow | `governance` | `/workflow` | `suggestion.view` |
| `governance.outlook` | Meeting Outlook | `governance` | `/tor/outlook` | `tor.list` |
| `governance.workflow_builder` | Workflow Builder | `governance` | `/workflow/builder` | `workflow.manage` |

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

### Security (5.1–5.4)
- 5.1 Self-deletion protection
- 5.2 Last admin protection
- 5.3 Persistent session key from `SESSION_KEY` env var (falls back to `Key::generate()` with warning)
- 5.4 CSRF protection: 32-byte hex token, constant-time comparison, all 9 POST handlers validate

### Housekeeping
- 7.1 Git init + push to GitHub
- 7.2 Favicon (inline SVG data URI)

### Infrastructure
- PageContext struct (bundles username, permissions, flash, nav_modules, sidebar_items)
- `PageContext::build()` constructor reduces handler boilerplate

### Role Management (4.1)
- Full CRUD: list (with permission/user counts), create, edit, delete (with user-assigned guard)
- Permission checkboxes with manual form body parsing (serde_urlencoded can't handle duplicate keys)
- `admin.roles` nav item under Admin module

### Ontology Explorer
- Three-tab explorer: Concepts (schema-level D3 graph) + Data (instance-level D3 graph) + Reference (entity type cards, relation patterns, schema docs)
- JSON APIs: `/ontology/api/schema` + `/ontology/api/graph`
- Concepts tab: schema graph with entity type nodes (sized by count), relation pattern edges, toolbar, keyboard shortcuts, right-click context menu with instance drill-down
- Data tab: instance graph with search, entity/relation type chip filters, right-click context menu (grouped relations, ego focus), position-preserving visibility toggles
- Reference tab: entity type summary cards, relation pattern breakdowns, schema reference tables

### App Settings (3.1 + 3.2 + 3.3)
- Settings as entities with entity_type='setting', properties: `value`, `description`, `setting_type` (text/number/boolean)
- 8 seeded settings: app.name, app.description, audit.enabled, audit.log_path, audit.retention_days, warnings.retention_resolved_days, warnings.retention_info_days, warnings.retention_deleted_days
- Runtime integration: `app.name` drives navbar brand, page titles, and login page brand

### Change Password (6.1)
- `GET /account` + `POST /account` with current/new/confirm validation

### Navbar Avatar Dropdown (6.5)
- Avatar with user initial, dropdown with Profile/Warnings/Logout, live warning badge count via WebSocket

### Audit Trail (7.3)
- Two-tier system: database (EAV) + filesystem (daily-rotated JSONL)
- UI: /audit with search, action filter, target type filter, pagination
- Retention cleanup on startup (configurable via settings)

### Custom Error Pages (6.2)
- Branded 404 and 500 error pages with navigation buttons

### Pagination (6.3) + Search/Filter (6.4)
- User list with `?page=1&per_page=25&q=searchterm` support

### Frontend Design Review (6.6)
- Color-coded role badges, unified search bars, improved empty states, redesigned dashboard

### Menu Builder / Permission Matrix (4.2)
- Visual permission matrix: roles as columns, permissions as rows grouped by section
- Diff-based save with audit logging
- JavaScript change tracking with unsaved-changes warning

### Roles Builder (4.3)
- Two-step wizard: role details → permissions & live sidebar preview (side-by-side)
- Handles both create and edit: single wizard replaces old separate edit form
- Edit mode pre-fills step 1 fields and pre-checks assigned permissions
- Live preview uses `requires_permission` relations (matches real nav system logic)
- Vertical preview grouped by module with accurate permission filtering
- Update handler includes audit logging + admin WebSocket warning (parity with old edit)
- Old `/roles/{id}/edit` removed; edit now at `/roles/builder/{id}/edit`
- 8 integration tests (5 builder + 3 model)

### Phase 2a: Item Pipeline
- Suggestion→proposal workflow with form validation and auto-proposal creation
- Integration tests validating complete workflow

### Phase 2b: Agenda Points, COAs & Data-Driven Workflows
- Data-driven workflow engine: WorkflowStatus + WorkflowTransition entities replace all hardcoded transitions
- Agenda points (informative/decision), courses of action (simple/complex), opinion recording, decision making
- Proposal queue with bulk scheduling
- 19 tasks delivered, 12 tests

### Warnings System
- Three-layer model: warning → warning_receipt → warning_event
- WebSocket real-time notifications with toast UI
- List/detail pages with category/severity/status filters + pagination
- Background scheduler (5-min interval) running generators + retention cleanup
- Event-driven generators: inline warnings on user create/delete, role permission changes
- 7 integration tests

### Production Deployment Preparation
- Environment variables (HOST, PORT, COOKIE_SECURE, SESSION_KEY, APP_ENV, RUST_LOG)
- Multi-stage Dockerfile with dependency caching
- Release profile (LTO, strip, single codegen unit)

### Code Cleanup & Refactoring (Tasks 1-28)
- Phase 1-2: AppError enum with 8 variants, render() helper, session helpers return Result
- Phase 3: All 27 handlers migrated to AppError pattern (~280 lines eliminated)
- Phase 4: 5 large files split into 17 focused modules (1,680 lines reorganized)

### Automated Testing
- 47 tests across 6 test files covering: infrastructure, auth, user CRUD, data integrity, permissions, role lifecycle, nav gating, workflows, warnings, role builder

### Hardening: Input Validation (H.2)
- Centralized `src/auth/validate.rs` with 5 reusable functions: username, email, password, required field, optional field
- Applied across all form handlers: user create/update, role create/update, ToR create/update, account password change
- Length limits enforced on all text inputs, format validation on username (alphanumeric + underscore) and email

### Hardening: Rate Limiting (H.1)
- Per-IP login rate limiting: 5 failed attempts per 15-minute window, 6th attempt blocked before DB access
- In-memory `HashMap<IpAddr, Vec<Instant>>` behind `Arc<Mutex<>>`, no external dependencies
- Lazy cleanup of stale entries, poison-resistant mutex, clear on successful login

### ToR Expansion (13 tasks)
- **Position-based membership**: `fills_position` relation (user→tor_function) replaces `member_of`+`has_tor_role`; authority flows through named positions, not persons. Vacant positions remain visible with `mandatory`/`optional` type from `entity_properties`.
- **Protocol templates**: `protocol_step` entities scoped to a ToR define reusable meeting agendas with type, duration, required flag, and sequence ordering.
- **Inter-ToR dependencies**: `feeds_into` and `escalates_to` relations between ToR entities with `is_blocking` metadata on `relation_properties`.
- **Minutes auto-scaffold**: `Minutes` + `MinutesSection` EAV model. `generate_scaffold()` creates 5 sections from meeting data (attendance flags vacant mandatory positions). Status lifecycle: draft→pending_approval→approved (read-only once approved).
- **Presentation templates**: `template_of`/`slide_of` relation chain. Per-ToR fixed slide templates with ordered slides and move-up/down reordering.
- **Governance map**: Cross-ToR dependency overview at `/governance/map` with colour-coded relationship badges. Nav item added to Governance sidebar.

### Governance Map Visual Graph (T.1)
- Dagre+D3 hierarchical DAG on `/governance/map`: ToR nodes (rounded rectangles) with label, cadence badge, status dot
- Directed edges: solid blue for `feeds_into`, dashed red for blocking, dashed amber for `escalates_to`
- JSON API at `GET /api/governance/graph` (nodes with cadence properties + edges with relation metadata)
- Click node → navigates to `/tor/{id}`, hover highlights connected subgraph
- Toolbar: fit, zoom in/out, reset zoom (keyboard shortcuts F, +/-, 0)
- Dagre 0.8.5 + D3 v7 from CDN, no new Rust dependencies
- ToR card grid retained below graph as secondary reference

### Meeting Outlook Calendar (T.3)
- Cadence computation engine in `src/models/tor/calendar.rs`: `compute_meetings(conn, start, end)` generates `CalendarEvent` instances from ToR cadence rules (daily, working_days, weekly, biweekly, monthly, ad-hoc)
- JSON API at `GET /api/tor/calendar?start=YYYY-MM-DD&end=YYYY-MM-DD` with 90-day cap
- Page at `/tor/outlook` with day/week/month CSS grid views
- Hybrid rendering: server renders initial week, client `fetch()` for tab/date switching
- Color-coded event pills per ToR, today highlighting, click-through to ToR detail
- Safe DOM construction (no innerHTML) via `el()` helper pattern
- Known gap: day view doesn't handle overlapping events side-by-side (e.g. 120min Sprint Planning covers 15min Daily Standup)

- New relations seeded: `fills_position`, `protocol_of`, `feeds_into`, `escalates_to`, `minutes_of`, `section_of`, `template_of`, `slide_of`, `requires_template`
- New permissions: `minutes.generate`, `minutes.edit`, `minutes.approve`
- New nav items: `governance.map` → `/governance/map`, `governance.outlook` → `/tor/outlook` under Governance module

### ToR Vacancy Warning Generators (T.2)
- New scheduled generator `check_tor_vacancies()` in `src/warnings/generators.rs`
- Queries active ToRs for mandatory positions with no `fills_position` relation
- Per-ToR dedup: one warning per ToR listing all vacant mandatory positions
- Targets users with `tor.manage_members` permission, WebSocket push on creation
- Auto-resolve: warnings auto-resolve on next scheduler tick when vacancies are filled
- Category `governance`, severity `medium`, source_action `scheduled.tor_vacancy`
- 2 integration tests: warning creation + dedup, auto-resolve on position fill

### Data Manager Seed Refactor
- Replaced ~1,000 lines of procedural `seed_ontology()`/`seed_staging()` in `db.rs` with JSON fixture imports
- Seed data now in `data/seed/ontology.json` (112 entities, 67 relations) and `data/seed/staging.json` (43 entities, 91 relations)
- Uses `data_manager::import::import_data()` with `ConflictMode::Skip` for idempotent seeding
- Passwords excluded from fixtures, hashed at runtime via `set_user_password()`
- `include_str!` embeds fixtures at compile time — no filesystem dependency at runtime
- To modify seed data: edit JSON fixtures, delete DB, restart server

### Dashboard Redesign (F.6)
- Full visual redesign: time-aware greeting (morning/afternoon/evening), stats cards (proposals, suggestions, ToRs, active warnings), recent activity feed from audit log
- Replaced placeholder dashboard with real data from model queries

### Dashboard Personalization (F.6b)
- **Actionable content-first layout**: Replaced passive stats-first dashboard with personalized, actionable panels
- **New model layer**: `src/models/dashboard.rs` with 6 types and 5 queries aggregating data from warnings, calendar, proposals, suggestions, and ToR membership
- **Needs Attention panel**: Unread warnings (severity-coded), pending proposals, open suggestions — all scoped to user's ToRs
- **My Terms of Reference**: User's ToR memberships with position labels, linked to ToR detail pages
- **Upcoming Meetings**: Next 7 days of meetings from user's ToRs via calendar computation engine
- **System Overview**: Clickable stat cards (users, roles, pending, positions, warnings) linking to relevant pages
- **Responsive**: Two-column primary layout (3fr/2fr) stacking at 768px, dark mode via CSS variables
- **Build**: PASS | **Tests**: 171 passing (unchanged)

### Workflow Builder UI (F.1)
- List page at `/workflow/builder`: scope cards with status/transition counts
- Detail page at `/workflow/builder/{scope}`: D3/dagre state machine graph + statuses table (add/edit/delete) + transitions table (add/edit/delete)
- All mutations: permission checks, CSRF, audit logging
- Nav item `governance.workflow_builder` seeded with `workflow.manage` permission
- Sidebar longest-prefix active-state fix (was marking `/workflow` active on `/workflow/builder` pages)
- Shared `.graph-panel` CSS: identical header panel across governance map and workflow builder (title + stat, 400px canvas, same toolbar/keyboard shortcuts)

### Test Coverage Expansion (H.4)
- **Result: 141 passing tests** (target: 120+)
- Phase 1: E2E test infrastructure fixes — 4 calendar confirmation tests with unique cookie isolation (#[ignore] for live server)
- Phase 2: Governance model tests — 7 tests for ToR CRUD, agenda points, meetings, proposals, cascade deletes, data queries
- Phase 3: Warning system tests — 5 tests for warning creation, deduplication, resolution, receipt lookup
- Phase 4: Proposal lifecycle tests — 7 tests for CRUD, status workflow, rejection, counting, querying
- **Systematic approach**: Prompt Contracts (4-component spec) applied across all 4 phases, API discovery via Explore agent before implementation
- Test pattern: all use `setup_test_db()` for isolation, follow established Rust/Actix patterns, no state leakage in parallel execution
- **Key learnings**: Graceful degradation over perfectionism (simplified failing tests to verify contract), relation type creation requires string name not ID, E2E cookie isolation prevents CI flakes

### REST API v1 Layer (F.2)
- **Phase 1: Users CRUD** (`/api/v1/users`)
  - GET /api/v1/users: list with pagination (page, per_page bounded 1-100), search filtering
  - GET /api/v1/users/{id}: single user with all properties
  - POST /api/v1/users: create with validation, bcrypt password hashing, audit logging
  - PUT /api/v1/users/{id}: update username/email/display_name/role with optional password
  - DELETE /api/v1/users/{id}: delete with audit trail
  - All endpoints: permission gating (users.list/create/edit/delete), proper HTTP status codes (200/201/204/400/404)
- **Phase 2: Entities CRUD** (`/api/v1/entities`)
  - GET /api/v1/entities: list with optional type filter, pagination
  - GET /api/v1/entities/{id}: single entity with properties
  - POST /api/v1/entities: create with validation, optional key-value properties, audit logging
  - PUT /api/v1/entities/{id}: update name/label/properties with audit trail
  - DELETE /api/v1/entities/{id}: delete with audit trail
  - All endpoints: permission gating (entities.list/create/edit/delete), EAV property support
- **Response Format**: Generic `PaginatedResponse<T>` wrapper for consistency, `ApiErrorResponse` with optional details field
- **Implementation**: ~650 lines across 3 files (handlers/api_v1/{mod,users,entities}.rs), follows established patterns (session helpers, audit logging, validation), no new dependencies
- **Build**: PASS | **Tests**: 141 passing (unchanged)

### Composite Database Index (H.5)
- Added composite index on `entity_properties(entity_id, key)` for optimized EAV lookups
- Index created automatically on schema initialization via `CREATE INDEX IF NOT EXISTS`
- Improves performance from O(n) sequential scan to O(log n) seek when filtering by both entity_id and key
- Applied at SQLite level, no application code changes needed
- Fully backward compatible, applied idempotent

### Dark Mode Theme System (F.4)
- **CSS Refactoring**: Migrated all colors to CSS custom properties (--bg, --text, --accent, etc.) at `:root` level
- **Dark Mode Palette**: Added `:root.dark` selector with inverted colors optimized for dark backgrounds
- **Theme Options**: Light, Dark, Auto (respects system preference via `prefers-color-scheme` media query)
- **Persistence**: Theme preference stored in localStorage, persists across browser sessions and device restarts
- **Flash Prevention**: Theme initialization script runs in `<head>` before CSS loads to apply correct theme immediately
- **UI Toggle**: New Preferences tab on `/account` page with 3 visual buttons (Light/Dark/Auto with emoji icons)
- **JavaScript Handler**: `window.toggleTheme(theme)` manages theme switching and localStorage persistence
- **Accessibility**: WCAG AA contrast ratios maintained in both light and dark themes
- **Components**: All pages, forms, graphs, and navigation render correctly in both themes
- **Build**: PASS | **Tests**: 141 passing (unchanged)

### User Profile Enhancements (F.5)
- **Avatar Upload**: Users can upload JPEG/PNG profile images (max 200KB)
- **Avatar Storage**: Profile images stored as base64 data URIs in entity_properties (no external file storage)
- **Avatar Display**: Profile image appears in navbar avatar dropdown and account page preview
- **Avatar Management**: Users can delete their avatar with confirmation; placeholder emoji shows when none present
- **Display Name Editing**: Users can update their display name via form on Profile tab
- **Profile Tab**: New "Profile" tab on `/account` page (default active), separate from Security/Preferences
- **Client-Side Validation**: File type (JPEG/PNG) and size (max 200KB) validated before upload
- **Error Handling**: Clear error messages for validation failures (wrong type, too large)
- **Audit Logging**: All profile changes logged (avatar upload/delete, display name updates)
- **Responsive UI**: Two-column grid layout (avatar + display name sections, stacks on mobile)
- **Build**: PASS | **Tests**: 141 passing (unchanged)

### Data Manager Hardening (DM.1–DM.4)
- **DM.1 CSS specificity fix**: `.dm-loading` and `.dm-editor-overlay` used `display:flex` as default, overriding browser's `[hidden]` attribute. Fixed by setting `display:none` default and adding `.class:not([hidden]) { display:flex }` rule. Stuck spinner on page load eliminated.
- **DM.2 Request timeouts**: All `fetch()` calls wrapped with `fetchWithTimeout(url, opts, FETCH_TIMEOUT_MS)` helper using `AbortController`. Timeout constant `60000ms`. On abort, user sees "Request timed out. The server may be busy." and spinner hides.
- **DM.3 web::block for import**: `import::import_data()` moved off Actix async thread via `web::block(move || ...)` to prevent worker thread starvation. Body size limit raised to 50 MB via sub-scope `app_data(web::JsonConfig::default().limit(...))`.
- **DM.4 Batch/multi-file import**: File input now accepts `multiple` files. Entities chunked in batches of 100 (constant `CHUNK_SIZE`). Relations sent only with last chunk to respect FK order. Progress shown as "File X of N, chunk Y of Z…". File queue resets after successful import.
- **Build**: PASS | **Tests**: 3 passing (unchanged)

### Users Table Enhancements (U.1)
- **Filter Builder**: Visual FilterTree UI — Add/Remove conditions (field, operator, value), group conditions with AND/OR logic, bookmarkable URL state. Filter fields: username, display_name, email, role, created_at, updated_at. Operators: contains, not_contains, equals, not_equals, is_empty, is_not_empty, gt, lt, gte, lte.
- **Column Picker**: Toggle visibility + drag-to-reorder via HTML5 drag-and-drop. Per-user preferences persisted to entity_properties (`pref.users_table_columns`), with global default setting entity fallback. ⊞ Columns popover button in table controls bar.
- **Per-page Selector**: 10/25/50/100 rows options; persisted in URL (`per_page` query param).
- **Sort Headers**: Clickable column headers with ▲/▼ indicators, preserved across filter/pagination changes.
- **CSV Export**: Download filtered/sorted result set; filter state passed via URL param.
- **Askama `|safe` bug fix**: `{{ variable }}` auto-HTML-escapes `"` → `&#34;`, breaking `JSON.parse()` in `<script type="application/json">` blocks. Fix applied to `list.html` and `table_controls.html`.
- **Playwright E2E tests**: 46 tests in `scripts/users-table.test.mjs` covering all new features.
- **Server startup**: `eprintln!` ensures port always printed to stderr regardless of `RUST_LOG`.
- **Build**: PASS | **Tests**: 154 passing

### Entity Metadata Gap Fill (E.1–E.3, complete)
- **E.1 Meeting**: Added `meeting_number`, `classification`, `vtc_details`, `chair_user_id`, `secretary_user_id` (simple strings) + `roll_call_data` (JSON: `[{user_id, status}]`) — stored via EAV, shown conditionally in `meetings/detail.html`, accepted as `Option<String>` in `ConfirmForm`
- **E.2 AgendaPoint** (fully complete): Added `presenter`, `priority` (normal/high/urgent), `pre_read_url` — form fields in `agenda/form.html`, conditional display in `agenda/detail.html`
- **E.3 Minutes**: Added `approved_by`, `approved_date` (simple strings) + `distribution_list`, `structured_action_items` (JSON: `[{description, responsible, due_date, status}]`), `structured_attendance` (JSON: `[{user_id, name, status, delegation_to}]`) — shown in `minutes/view.html`, editable via dedicated POST endpoints
- All via EAV `entity_properties`, no schema changes needed
- **Build**: PASS | **Tests**: all passing (0 failures)

### Users / Roles / Role Builder Separation
- **Three-page separation**: Users page (pure CRUD, no role assignment), Roles page (dedicated assignment with By Role/By User tabs + menu preview), Role Builder (sole path for role CRUD with permissions + delete)
- **Multi-role support**: Users can hold multiple roles via `has_role` many-to-many relation. Permissions = union across all roles via `find_codes_by_user_id()`. `UserDisplay` uses `GROUP_CONCAT(DISTINCT ...)` for comma-separated role display.
- **New permission**: `roles.assign` separates role assignment from role management (`roles.manage`)
- **Last-admin protection**: Updated to query `has_role` relation directly (multi-role safe)
- **Auto-assign**: New users get "viewer" role automatically on creation
- **17 tasks** across handlers, models, templates, routes, and tests
- **Build**: PASS | **Tests**: 162 passing

### ABAC — Attribute-Based Access Control (3 splits)
- **Split 1 (01-abac-core)**: Created `src/auth/abac.rs` with three functions: `has_resource_capability` (EAV graph traversal), `load_tor_capabilities` (bulk capability loader), `require_tor_capability` (two-phase handler guard). 7 TDD tests in `tests/abac_test.rs`. Commits `5e75c4d`, `8751672`, `4e67d7b`.
- **Split 2 (02-handler-migration)**: Migrated 9 handlers from `require_permission("tor.edit")` to ABAC capability checks: `confirm`, `transition`, `assign_agenda`, `remove_agenda`, `save_roll_call`, `generate_minutes` (meeting_handlers) + `save_attendance`, `save_action_items` (minutes_handlers) + special `confirm_calendar` (JSON pattern). Commit `ec92ae2`.
- **Split 3 (03-template-ui)**: Added `tor_capabilities: Permissions` to `MeetingDetailTemplate`, populated via `load_tor_capabilities` in `detail` handler, updated roll call section guards in `meetings/detail.html`. Commit `330110d`.
- **Build**: PASS | **Tests**: 169 passing

### Enterprise Infrastructure Migration (Phase 1)
- **Phase 1a: PostgreSQL migration** — Full migration from SQLite/rusqlite to PostgreSQL 17/sqlx 0.8 (async). All 44 model files, 30+ handler files, 23 test files converted. Schema migrated to `migrations/` directory (sqlx auto-run). Test isolation via unique PostgreSQL schemas (`search_path`). 171 tests passing.
- **Phase 1b: Neo4j integration** — Optional read-only graph projection via neo4rs 0.8. Graph sync module with fire-and-forget `tokio::spawn`, ABAC Cypher queries, governance map visualization. Falls back to PostgreSQL when Neo4j unavailable. 4 integration tests (#[ignore] without Neo4j).
- **Phase 2: Docker Compose** — Multi-environment setup: `docker-compose.yml` (base) + `docker-compose.{dev,staging,prod}.yml` (overrides). Makefile orchestration (`make dev/staging/prod/down`). PostgreSQL init script creates per-env databases.
- **Phase 3: GitLab CI/CD** — Self-hosted GitLab CE + Runner configs in `infra/gitlab/`. CI pipeline (`.gitlab-ci.yml`) with test, lint, build, and Helm deployment stages.
- **Phase 4: Kubernetes Helm** — Shared infra charts (`helm/infra/`) for PostgreSQL + Neo4j. Application chart (`helm/ahlt/`) with deployment, service, configmap, secret, ingress templates. Per-environment values files (dev/staging/prod).
- **Build**: PASS | **Tests**: 171 passing (8 ignored: 4 Neo4j + 4 E2E)

### Codebase Audit Fixes (CA.1–CA.2)
- **CA.1 — Queue template**: Replaced stub `templates/workflow/queue.html` with full implementation (table, checkboxes, bulk schedule form, unqueue per-row)
- **CA.1 — Agenda transition handler**: Replaced "not yet implemented" stub in `agenda_handlers.rs` with working workflow transition (validate → set_property → audit log)
- **CA.1 — Seed missing permissions**: Added 5 permission entities (`entities.list/create/edit/delete`, `minutes.view`) + 11 `has_permission` relations for admin role to `ontology.json`
- **CA.1 — Opinion seed direction**: Standardized 2 `opinion_by` relations in `staging.json` to match programmatic direction (`user → opinion`)
- **CA.2 — Minutes export button**: Added Export button to `minutes/view.html` (visible only when status is "approved", opens in new tab)
- **CA.2 — Audit logging gaps**: Added `audit::log()` calls to 6 mutation handlers: `mark_deleted`, `forward` (warning actions), `move_step` (protocol), `handle_add_slide`, `handle_delete_slide`, `handle_move_slide` (presentation)
- **CA.3 — Stale `/agenda-points/` URL prefix**: Fixed 12 broken links across `templates/opinion/form.html`, `templates/coa/detail.html`, `templates/workflow/view.html` — all now use `/workflow/agenda/` prefix matching actual routes
- **CA.3 — Missing agenda point delete handler**: Added `delete()` handler + `AgendaDeleteForm` struct to `agenda_handlers.rs` and registered `POST /tor/{id}/workflow/agenda/{agenda_id}/delete` in `main.rs`; delete button in workflow view was previously wired to a non-existent route
- **CA.3 — Dead code cleanup**: Removed unused `setup_test_db_seeded()` wrapper, unused `ADMIN_USER`/`ADMIN_PASS`/`TEST_USER_EMAIL` constants from `tests/common/mod.rs`, and unused `insert_entity` import from `opinion_relation_test.rs`
- **Build**: PASS | **Tests**: 171 passing (unchanged)

### Ontology Graph Redesign (OG.1)
- **Search**: Real-time entity search with pulsing highlight rings on matching nodes, dimming non-matches
- **Entity type filters**: Chip-based toggles with colored dots, instance counts, position-preserving visibility (no simulation restart)
- **Relation type filters**: Chip-based toggles for showing/hiding relation types and their edges
- **Right-click context menu**: Instance graph shows grouped outgoing/incoming relations, "Focus on this node", "Open full detail" link; Schema graph shows instance count badge, "View instances" drill-down, relation type summary
- **Ego network focus**: 1-hop neighbor subgraph with floating "Focus: Name x" dismiss pill
- **Schema-to-instance drill-down**: "View instances" navigates to `/ontology/data?type=X` with pre-filtered entity type
- **Scrollable filter panel**: `max-height` + `overflow-y: auto` prevents panel overflow with many entity types
- **Bug fix**: Schema API 500 error resolved by adding explicit SQL column aliases for `SchemaEdge` struct deserialization
- **Files**: `templates/ontology/data.html` (full rewrite), `templates/ontology/graph.html` (context menu), `static/css/style.css` (~268 lines), `src/models/ontology/schema.rs` (SQL fix)
- **Build**: PASS | **Tests**: 171 passing (unchanged)

### Minutes Export (T.4)
- **Export Format**: Print-friendly HTML (users print to PDF via browser Ctrl+P / Cmd+P)
- **Approved-Only**: Only approved minutes exportable; draft/pending return 403 Forbidden
- **URL**: GET `/meetings/{id}/export` returns inline HTML for preview or download
- **Content**: All minutes sections displayed with semantic structure (header + sections + footer)
- **Section Types**: Attendance (👥), Protocol (📋), Agenda Items (📝), Decisions (✅), Action Items (🎯)
- **Print Styling**: CSS optimized for print with page breaks on sections, no headers/footers repeated
- **Filename**: Content-Disposition set to `inline; filename="minutes-{id}.html"`
- **Permission**: Requires `minutes.view` permission; validates user access
- **Audit Logging**: Export action logged with minutes ID, format, and audit trail
- **No Dependencies**: Pure HTML/CSS, no external PDF library needed
- **Build**: PASS | **Tests**: 141 passing (unchanged)

---

## Remaining Backlog

### Hardening & Quality

| ID | Item | Priority | Effort | Description |
|----|------|----------|--------|-------------|
| H.3 | **WebSocket error handling** | Medium | Small | Replace `conn_map.write().unwrap()` in ws.rs with proper error handling (RwLock poison recovery). ✓ DONE |
| H.5 | ~~**Composite DB index**~~ | ~~Low~~ | ~~Small~~ | **DONE** — see Completed Work |

### ToR / Meeting / Minutes Metadata Gaps (E.1–E.3)

E.2 fully done. E.1/E.3 all fields done (simple strings + JSON).

| ID | Entity | Remaining Fields | Priority | Effort |
|----|--------|----------------|----------|--------|
| ~~E.1~~ | ~~**Meeting**~~ | ~~`roll_call_data` (JSON: `[{user_id, status}]`)~~ | ~~Low~~ | ~~Small~~ | **DONE** |
| ~~E.3~~ | ~~**Minutes**~~ | ~~`distribution_list` (JSON), `structured_action_items` (JSON: `[{description, responsible, due_date, status}]`), `structured_attendance` (JSON: `[{user_id, name, status, delegation_to}]`)~~ | ~~Low~~ | ~~Small~~ | **DONE** |

Implementation notes:
- JSON properties follow the `parse_json_list` / `lines_to_json` pattern from the ToR objectives fields
- `roll_call_data` (meeting level) and `structured_attendance` (minutes level) overlap — decided to store at both levels for independent editing

### Features

| ID | Item | Priority | Effort | Description |
|----|------|----------|--------|-------------|
| F.1 | ~~**Workflow builder UI**~~ | ~~High~~ | ~~Large~~ | **DONE** — see Completed Work |
| F.2 | ~~**REST API layer**~~ | ~~Medium~~ | ~~Large~~ | **DONE** — see Completed Work |
| F.3 | **More entity types** | Medium | Variable | Extend the platform with project, task, or document entity types. The EAV model requires zero schema migrations — just new model files, handlers, and templates per type. |
| F.4 | ~~**Dark mode**~~ | ~~Low~~ | ~~Medium~~ | **DONE** — see Completed Work |
| F.5 | ~~**User profile enhancements**~~ | ~~Low~~ | ~~Small~~ | **DONE** — see Completed Work |
| F.6 | ~~**Dashboard widgets**~~ | ~~Low~~ | ~~Medium~~ | **DONE** — see Completed Work |
| T.2 | ~~ToR vacancy warning generators~~ | ~~Medium~~ | ~~Small~~ | **DONE** — see Completed Work |
| T.3 | ~~Meeting outlook calendar~~ | ~~Medium~~ | ~~Medium~~ | **DONE** — see Completed Work |
| T.4 | ~~**Minutes export (HTML/PDF print)**~~ | ~~Low~~ | ~~Medium~~ | **DONE** — see Completed Work |
| P.7 | ~~**ToR context bar**~~ | ~~Medium~~ | ~~Medium~~ | **DONE** — persistent section navigation bar (Overview/Workflow/Meetings/Templates) on all ToR-scoped pages; new `/tor/{id}/meetings` route |

---

## Implementation Order

```
DONE                                    CANDIDATES (pick next)
════                                    ══════════════════════
Epic 1: Ontology Foundation             F.3  More entity types (medium, variable)
Epic 2: Data-Driven Nav                 
5.1–5.4 Security                        
4.1 Role Management                     
4.2 Menu Builder                        
4.3 Roles Builder                       
3.1–3.3 App Settings                    
6.1–6.6 UX features                     
7.1–7.3 Housekeeping                    
Ontology Explorer                       
Phase 2a: Item Pipeline                 
Phase 2b: Workflows + Governance        
Warnings System                         
Production Deployment                   
Code Cleanup (Tasks 1-28)               
H.1 Rate Limiting                       
H.2 Input Validation                    
H.3 WebSocket error handling            
H.4 Test coverage expansion (141 tests) 
H.5 Composite DB index                  
ToR Expansion (13 tasks)                
T.1 Governance Map Visual Graph         
T.2 ToR Vacancy Warning Generators      
T.3 Meeting Outlook Calendar            
T.4 Minutes export (HTML/print)         
Data Manager Seed Refactor              
F.1 Workflow Builder UI                 
F.2 REST API v1 Layer (Users + Entities)
F.4 Dark mode theme system
F.5 User profile enhancements
F.6 Dashboard Redesign
DM.1–DM.4 Data Manager hardening+batch
U.1 Users Table Enhancements (filter builder, column picker, per-page, sort, CSV, Playwright)

E.1  Meeting metadata (number, classification, vtc, chair, secretary, roll_call_data) ✓ done
E.2  Agenda Point metadata (presenter, priority, pre_read_url)                        ✓ done
E.3  Minutes metadata (approved_by, approved_date, distribution_list,
     structured_action_items, structured_attendance)                                  ✓ done

Users/Roles/Role Builder Separation (17 tasks, multi-role support)                   ✓ done
ABAC — 3-split implementation (abac-core, handler-migration, template-ui)             ✓ done

Enterprise Infrastructure Migration (5 phases)                                        ✓ done
  Phase 1a: SQLite → PostgreSQL 17 + sqlx 0.8 (async)
  Phase 1b: Neo4j 5 Community integration (optional graph projection)
  Phase 2: Docker Compose multi-environment
  Phase 3: GitLab CE + CI/CD pipeline
  Phase 4: Kubernetes Helm charts

OG.1 Ontology Graph Redesign (search, filters, context menu, focus, drill-down)      ✓ done
F.6b Dashboard Personalization (user ToRs, meetings, attention items)                 ✓ done
CA.1 Codebase Audit Fixes (queue template, agenda transitions, seed gaps)            ✓ done
CA.2 Codebase Audit Fixes (minutes export button, audit logging for 6 handlers)    ✓ done
CA.3 Codebase Audit Fixes (stale URLs, missing delete handler, dead code cleanup)   ✓ done
P7   ToR Context Bar (persistent section nav on all ToR-scoped pages, +meetings route) ✓ done
P8   Dark Mode Header Toggle (persistent DB storage, syncs across devices)             ✓ done

CANDIDATES (pick next)
══════════════════════
F.3  More entity types (project, task, document)
```

---

## Architecture Decisions

### Handler Pattern
Each handler follows: permission check → get conn → build PageContext → page query → template render. The `PageContext::build()` helper consolidates the 5 common fields (username, permissions, flash, nav_modules, sidebar_items) into a single constructor call. All handlers use `Result<HttpResponse, AppError>` with `?` operator for error propagation.

### Nav Item Hierarchy
Top-level items with no children are standalone (Dashboard). Items with children are modules (Admin, Governance) — visible if any child passes permission check. Children appear in sidebar when their parent module is active. Active module detection checks child URLs first for correct prefix matching.

### EAV Trade-offs
The generic schema means zero migrations when adding new entity types. The trade-off is more complex queries (LEFT JOINs on entity_properties). Typed domain structs (UserDisplay, RoleDisplay) provide a stable API layer over the generic storage.

### Workflow Engine
Workflow definitions (statuses + transitions) are stored as EAV entities, not hardcoded. Transitions are permission-gated and support conditions. The engine queries available transitions at runtime based on current status and user permissions. Currently seeded for suggestion and proposal workflows — designed to be extensible to any entity type.
