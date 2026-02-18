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
| `relation_type` | Named relationship kinds (26 defined) | — |
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
| `has_role` | user → role | User's assigned role |
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
| `opinion_by` / `opinion_on` / `prefers_coa` | opinion → user/agenda/coa | Opinion tracking |
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
| `admin.roles` | Roles | `admin` | `/roles` | `roles.manage` |
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
- Concepts tab: schema graph with entity type nodes (sized by count), relation pattern edges, toolbar, keyboard shortcuts
- Data tab: instance graph with type filtering, node hover highlighting, click detail panel
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
- New relations seeded: `fills_position`, `protocol_of`, `feeds_into`, `escalates_to`, `minutes_of`, `section_of`, `template_of`, `slide_of`, `requires_template`
- New permissions: `minutes.generate`, `minutes.edit`, `minutes.approve`
- New nav item: `governance.map` → `/governance/map` under Governance module

---

## Remaining Backlog

### Hardening & Quality

| ID | Item | Priority | Effort | Description |
|----|------|----------|--------|-------------|
| H.3 | **WebSocket error handling** | Medium | Small | Replace `conn_map.write().unwrap()` in ws.rs with proper error handling (RwLock poison recovery). |
| H.4 | **Test coverage expansion** | Medium | Large | ~10 handler modules lack isolated integration tests: account, auth, dashboard, audit, menu builder, ontology, ToR, agenda, COA, opinion. Currently 45 tests / 6 files. |
| H.5 | **Composite DB index** | Low | Small | Add `entity_properties(entity_id, key)` composite index for EAV lookup performance. |

### Features

| ID | Item | Priority | Effort | Description |
|----|------|----------|--------|-------------|
| F.1 | **Workflow builder UI** | High | Large | The workflow engine is fully built (statuses, transitions, permission-gated, condition support) but definitions can only be created via db.rs seeding. Add CRUD UI for creating/editing workflow definitions without code changes. |
| F.2 | **REST API layer** | Medium | Large | Only 2 JSON endpoints exist (`/ontology/api/{schema,graph}`). Add a `/api/v1/` prefix with JSON CRUD for entities, users, roles — enables external integrations and mobile clients. |
| F.3 | **More entity types** | Medium | Variable | Extend the platform with project, task, or document entity types. The EAV model requires zero schema migrations — just new model files, handlers, and templates per type. |
| F.4 | **Dark mode** | Low | Medium | All CSS uses light-theme only. Add CSS custom property system for theme switching with user preference persistence. |
| F.5 | **User profile enhancements** | Low | Small | Avatar upload, display name editing, notification preferences on the /account page. |
| F.6 | **Dashboard widgets** | Low | Medium | Make dashboard cards data-driven — recent activity feed, pending workflow items, system health indicators. |
| T.1 | **Governance map visual graph** | Medium | Medium | Replace the flat list on `/governance/map` with a Dagre+D3 DAG — nodes for ToRs with cadence badges, directed edges for feeds_into/escalates_to, click-through to ToR detail. **Plan ready**: `~/.claude/plans/piped-roaming-pearl.md` Task 1. |
| T.2 | **ToR vacancy warning generators** | Medium | Small | Background scheduler generators that fire warnings when mandatory positions in active ToRs are unfilled. Uses `fills_position` + `entity_properties membership_type=mandatory`. |
| T.3 | **Meeting outlook calendar** | Medium | Medium | `/tor/outlook` with day/week/month CSS grid views + `GET /api/tor/calendar` endpoint computing meeting instances from cadence rules. Hybrid: server renders initial week view, client fetch for tab/date switching. **Plan ready**: `~/.claude/plans/piped-roaming-pearl.md` Tasks 2-4. |
| T.4 | **Minutes export (PDF/Word)** | Low | Medium | Export approved minutes as a formatted PDF or docx using a template. The EAV structure means all sections are available as structured data. |

---

## Implementation Order

```
DONE                                    CANDIDATES (pick next)
════                                    ══════════════════════
Epic 1: Ontology Foundation             F.1  Workflow builder UI (high, large effort)
Epic 2: Data-Driven Nav                 H.4  Test coverage expansion (medium, large)
5.1–5.4 Security                        F.2  REST API layer (medium, large)
4.1 Role Management                     F.3  More entity types (medium, variable)
4.2 Menu Builder                        H.3  WebSocket error handling (medium, small)
4.3 Roles Builder                       H.5  Composite DB index (low, small)
3.1–3.3 App Settings                    F.4  Dark mode (low, medium)
6.1–6.6 UX features                     F.5  User profile enhancements (low, small)
7.1–7.3 Housekeeping                    F.6  Dashboard widgets (low, medium)
Ontology Explorer
Phase 2a: Item Pipeline
Phase 2b: Workflows + Governance
Warnings System
Production Deployment
Code Cleanup (Tasks 1-28)
Automated Testing (47 tests)
H.1 Rate Limiting
H.2 Input Validation
ToR Expansion (13 tasks)
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
