# Terms of Reference — Governance System Design

**Date:** 2026-02-14
**Status:** Approved
**Approach:** Pure EAV — all concepts modeled as entity types with properties and relations

## Overview

A meeting governance system built on the existing EAV ontology. A Terms of Reference (ToR) defines how a meeting body operates: who sits in it, what authority they have, and what kinds of items flow through their meetings.

Key capabilities:
- ToR management with membership and role-based authority
- Item pipeline: Suggestion → Proposal → Agenda Point (separate linked entities)
- Meeting scheduling (recurring + ad-hoc) with agenda assembly
- Unified calendar across all ToRs with day/week/month views
- Delegable authority via EAV properties on function entities
- ABAC-ready data model (ToR-scoped RBAC first, full ABAC later)

## Entity Types

### New Entity Types

| Entity Type | Purpose | Key Properties |
|---|---|---|
| `tor` | Terms of Reference — a governance body | `description`, `status` (active/archived), `meeting_cadence` (weekly/biweekly/monthly/ad-hoc), `cadence_day` (monday..friday), `cadence_time` (HH:MM), `cadence_duration_minutes`, `default_location` (physical room/place), `remote_url` (Teams/Skype/Zoom link), `background_repo_url` (link to background document repository) |
| `tor_function` | An authority function within a ToR | `description`, `category` (governance/procedural/administrative), plus authority properties (see Authority Model) |
| `suggestion` | Initial idea submitted for consideration | `description`, `submitted_date`, `status` (open/accepted/rejected), `submitted_by_id` |
| `proposal` | Formalized suggestion with detail | `description`, `rationale`, `submitted_date`, `status` (draft/submitted/under_review/approved/rejected), `rejection_reason` |
| `agenda_point` | Item scheduled for a specific meeting | `description`, `point_type` (informative/decision), `sequence_order`, `status` (pending/discussed/decided/deferred), `decision_text`, `vote_result` |
| `meeting` | A scheduled meeting instance | `scheduled_date` (YYYY-MM-DD), `scheduled_time` (HH:MM), `status` (planned/in_progress/completed/cancelled), `location` (overrides ToR default), `remote_url` (overrides ToR default), `minutes_text` |
| `delegation` | A temporary authority transfer | `start_date`, `end_date`, `reason`, `status` (active/expired/revoked) |

### New Relation Types

| Relation Type | Source → Target | Meaning |
|---|---|---|
| `member_of` | user → tor | User is a member of this ToR body |
| `has_tor_role` | user → tor_function | User holds this function |
| `belongs_to_tor` | tor_function → tor | This function is defined within this ToR |
| `delegates_to` | delegation → user | This delegation grants authority to this user |
| `delegates_function` | delegation → tor_function | This delegation covers this specific function |
| `delegated_by` | delegation → user | This delegation was created by this user |
| `spawns_proposal` | suggestion → proposal | This suggestion was formalized into this proposal |
| `spawns_agenda_point` | proposal → agenda_point | This proposal became this agenda point |
| `scheduled_in` | agenda_point → meeting | This agenda point is on this meeting's agenda |
| `meeting_of` | meeting → tor | This meeting belongs to this ToR |
| `suggested_to` | suggestion → tor | This suggestion was submitted to this ToR |

## Authority Model

### ToR-Scoped Roles

A user's authority is determined by membership + function ownership within the same ToR:

```
user --member_of--> tor
user --has_tor_role--> tor_function --belongs_to_tor--> tor
```

Example: Alice is Chair of the Budget Committee:
- `alice --member_of--> budget_committee`
- `alice --has_tor_role--> chair_function --belongs_to_tor--> budget_committee`

### Authority Properties (EAV on tor_function)

| Property | Values | Governs |
|---|---|---|
| `can_review_suggestions` | true/false | Accept/reject suggestions |
| `can_create_proposals` | true/false | Create and submit proposals |
| `can_approve_proposals` | true/false | Approve/reject proposals, schedule decision agenda points |
| `can_manage_agenda` | true/false | Add informative agenda points, reorder agenda |
| `can_record_decisions` | true/false | Record decisions and vote results |
| `can_call_meetings` | true/false | Schedule ad-hoc meetings |

These properties are data-driven — new authority capabilities can be added by inserting new properties without code changes. When ABAC is implemented, these become policy attributes.

### Delegation

A delegation temporarily grants specific function authority to another user:

```
delegation --delegated_by--> alice (original holder)
delegation --delegates_to--> bob (temporary holder)
delegation --delegates_function--> chair_function (what's being delegated)
```

Properties `start_date` and `end_date` bound the delegation. Authority checks:
1. Check user's own `has_tor_role` relations
2. If not found, check active delegations (`status = active`, current date within range)

## Item Pipeline

### Lifecycle

Separate entities linked by relations, enforcing temporal ordering:

```
Suggestion ──spawns_proposal──▶ Proposal ──spawns_agenda_point──▶ Agenda Point ──scheduled_in──▶ Meeting
```

### Status Transitions

**Suggestion:** `open` → `accepted` (spawns proposal) | `rejected`

**Proposal:** `draft` → `submitted` → `under_review` → `approved` (spawns decision agenda point) | `rejected`

**Agenda Point:** `pending` → `discussed` → `decided` (records decision_text, vote_result) | `deferred` (re-linked to future meeting)

**Meeting:** `planned` → `in_progress` → `completed` | `cancelled`

### Ordering Rules

1. A Suggestion must exist before a Proposal can be created from it
2. A Proposal must be `submitted` or `under_review` before spawning an Agenda Point
3. Decision-type Agenda Points require a source proposal chain (traceability)
4. Informative Agenda Points can be created directly (no suggestion/proposal required)
5. Within a meeting, `sequence_order` property controls presentation order

## Meeting Scheduling

### Recurring Meetings

ToR cadence properties (`meeting_cadence`, `cadence_day`, `cadence_time`) drive auto-generation of meeting entities for upcoming periods. Each generated meeting gets:
- Properties from the cadence
- `status: planned`
- `meeting_of` relation to the ToR
- `name` derived from ToR name + date (e.g., "Budget Committee — 2026-02-21")

### Ad-hoc Meetings

Created manually by users with `can_call_meetings` authority on the ToR.

### Date Querying in EAV

Dates stored as ISO-8601 text properties (`YYYY-MM-DD`) sort correctly with string comparison in SQLite:

```sql
SELECT e.id, e.name, ep_date.value as scheduled_date
FROM entities e
JOIN entity_properties ep_date ON e.id = ep_date.entity_id AND ep_date.key = 'scheduled_date'
WHERE e.entity_type = 'meeting'
  AND ep_date.value BETWEEN '2026-02-17' AND '2026-02-23'
ORDER BY ep_date.value
```

Performance optimization: add index on `entity_properties(key, value)` if calendar queries become slow.

## Calendar Views

### Unified Calendar

Shows meetings from ALL ToRs the user is a member of.

**Day view:** All meetings for a date, with agenda points listed inline, grouped by ToR.

**Week view:** 7-column grid with meeting blocks showing title, time, agenda point count. Color-coded by ToR.

**Month view:** Traditional grid with meeting dots/chips per day. Click to expand to day view.

### Filtering

| Filter | Mechanism |
|---|---|
| By ToR | Filter `meeting_of` relation |
| By role/function | Show meetings where user holds specific `tor_function` |
| By item pipeline | Highlight meetings with pending decisions or deferred items |
| Access control | Only show meetings for ToRs where user has `member_of` relation |

### Pipeline Overlay

- Meetings with pending decisions show badge count
- Deferred items show target meeting
- In-progress suggestions/proposals appear as "upcoming" on next eligible meeting

## Phased Delivery

### Phase 1 — ToR Foundation
Create and manage Terms of Reference with membership and functions.

- Entity types: `tor`, `tor_function`
- Relation types: `member_of`, `has_tor_role`, `belongs_to_tor`
- CRUD handlers for ToR
- ToR detail page: members, functions, authority properties
- Assign/remove members and functions
- Permissions: `tor.list`, `tor.create`, `tor.edit`, `tor.manage_members`
- Nav items under new "Governance" module

### Phase 2 — Item Pipeline
Suggestion → Proposal → Agenda Point lifecycle.

- Entity types: `suggestion`, `proposal`, `agenda_point`
- Relation types: `suggested_to`, `spawns_proposal`, `spawns_agenda_point`
- CRUD per item type, scoped to a ToR
- Status transitions with authority checks
- Pipeline view per ToR (kanban-style or list)
- Ordering enforcement
- Audit logging for state transitions

### Phase 3 — Meetings & Scheduling
Meeting management with agenda assembly.

- Entity type: `meeting`
- Relation types: `meeting_of`, `scheduled_in`
- Recurring meeting generation from cadence properties
- Ad-hoc meeting creation (authority-gated)
- Meeting detail page with ordered agenda points
- Agenda point reordering
- Meeting status transitions
- Decision recording

### Phase 4 — Calendar Views
Unified calendar across all ToRs.

- Day/week/month views (server-rendered HTML)
- Meeting blocks color-coded by ToR
- Pipeline indicators
- Filtering by ToR, function, item status
- Access control (member_of gating)
- Click-through: calendar → meeting → agenda points

### Phase 5 — Delegation
Temporary authority transfer between members.

- Entity type: `delegation`
- Relation types: `delegates_to`, `delegates_function`, `delegated_by`
- Delegation create/revoke UI
- Date-bounded with auto-expire
- Authority check fallthrough (own functions → active delegations)
- Audit trail

### Phase 6 — ABAC Preparation
Internal refactoring for attribute-based access control.

- Refactor authority checks into policy evaluation layer
- Authority properties become policy attributes
- Design policy rule entity type (EAV-modeled)
- No UI changes — internal only
- Document ABAC policy model

Each phase is independently deployable and builds on the previous. Phases 1-3 form the core system. Phases 4-6 add polish and future-proofing.
