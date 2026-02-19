# Meeting Lifecycle — Design Document

**Date:** 2026-02-19
**Status:** Approved
**Scope:** Full meeting lifecycle with persisted entities, workflow engine integration, agenda assignment, and minutes generation

---

## Problem

Meetings exist only as virtual computations from ToR cadence rules (`calendar.rs`). The minutes system expects a `meeting_id` entity that never gets created. The `POST /minutes/generate` handler is wired but unreachable from any UI. Agenda points have `scheduled_date` but no association to a specific meeting instance.

This means:
- No meeting history or audit trail
- No way to generate minutes (the pipeline is broken)
- No per-meeting agenda management
- No cross-ToR view of upcoming meetings

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Meeting persistence | EAV entities | Consistent with all other domain objects, zero schema migration |
| Meeting creation | Hybrid: calendar projects, user confirms | Orgs sometimes skip meetings; explicit confirmation prevents phantom records |
| Lifecycle management | Workflow engine | Reuses existing `WorkflowStatus`/`WorkflowTransition` seeded for `entity_type_scope = 'meeting'`; permission-gated, auditable |
| Navigation | Cross-ToR list (`/meetings`) + scoped detail (`/tor/{id}/meetings/{mid}`) | Users on multiple ToRs need a single view; detail stays scoped for context |
| Agenda linking | Relation-based (`scheduled_for_meeting`) | Agenda points relate to meetings, not copied; one point can be rescheduled |
| Scope | Meeting entity + confirm + agenda assignment + minutes generation + editing/approval | Full meeting preparation and documentation flow |

## Prompt Contract

### GOAL

Implement a full meeting lifecycle where meetings are persisted EAV entities with a workflow: `projected → confirmed → in_progress → completed` (+ `cancelled`). The calendar projects future meetings from ToR cadence rules. Users confirm projected meetings to activate them. Confirmed meetings get agenda points assigned, and completing a meeting triggers minutes scaffold generation. A cross-ToR meeting list at `/meetings` shows all upcoming meetings; scoped detail pages live at `/tor/{id}/meetings/{mid}`.

**Success =** A user browsing a ToR sees projected meetings from the cadence, confirms one, assigns agenda points to it, views the meeting detail page with agenda + protocol, transitions it to completed, and gets a minutes document with 5 auto-generated sections ready for editing.

### CONSTRAINTS

- Meetings are EAV entities (`entity_type = 'meeting'`) — no new database tables
- Meeting lifecycle uses the existing workflow engine (seed `WorkflowStatus` + `WorkflowTransition` entities with `entity_type_scope = 'meeting'`)
- All mutations: `require_permission()` + CSRF validation + audit logging
- ToR membership required for meeting actions (reuse `tor::require_tor_membership`)
- Reuse existing infrastructure: `calendar.rs` computation, `minutes::generate_scaffold`, protocol model
- The existing `POST /minutes/generate` handler gets wired to the meeting entity (fix the `meeting_id` gap)
- No new Rust crate dependencies
- Templates follow existing patterns: Askama structs, `PageContext::build()`, BEM CSS
- Cross-ToR list requires `tor.list` permission; per-meeting actions require ToR membership

### FORMAT

**Model layer:**
1. `src/models/meeting/types.rs` — `Meeting`, `MeetingListItem`, `MeetingDetail` structs
2. `src/models/meeting/queries.rs` — CRUD, find_by_tor, find_upcoming_all, confirm, transition helpers
3. `src/models/meeting/mod.rs` — module re-exports

**Handler layer:**
4. `src/handlers/meeting_handlers/list.rs` — cross-ToR meeting list (`GET /meetings`)
5. `src/handlers/meeting_handlers/crud.rs` — ToR-scoped handlers (confirm, detail, transition, assign agenda)
6. `src/handlers/meeting_handlers/mod.rs` — module re-exports

**Templates:**
7. `templates/meetings/list.html` — cross-ToR upcoming + past meetings
8. `templates/meetings/detail.html` — single meeting: info, agenda, protocol, minutes link
9. Update `templates/tor/detail.html` — add Meetings section showing projected + confirmed

**Seed data:**
10. Update `data/seed/ontology.json` — meeting workflow statuses, transitions, relation types, nav item, permission

**Wiring:**
11. Update `src/handlers/mod.rs` — declare `meeting_handlers`
12. Update `src/lib.rs` — if needed for model module
13. Update `src/main.rs` — register routes + nav item seed

### FAILURE CONDITIONS

- Meeting entities use dedicated database tables instead of EAV
- Meeting lifecycle is hardcoded status strings instead of using the workflow engine
- `generate_scaffold` is called without a real `meeting_id` (the whole point is fixing this gap)
- Any handler missing permission check, CSRF, or audit log on mutations
- Minutes generation wired to ToR+date instead of a meeting entity
- The cross-ToR list fetches agenda/minutes data N+1 style (must be efficient queries)
- Agenda points get duplicated when assigned to meetings (should be a relation, not a copy)
- Protocol steps are copied into the meeting instead of referenced from the ToR's template
- Templates use `innerHTML` (security hook will reject)
- Any file exceeds 200 lines without good reason

## Data Model

### Meeting Entity

```
entity_type: 'meeting'
name: '{tor_name}-{YYYY-MM-DD}'   (unique, machine-readable)
label: '{ToR Label} — {formatted date}'

Properties:
  meeting_date: ISO-8601 date string
  status: workflow-driven (projected/confirmed/in_progress/completed/cancelled)
  location: inherited from ToR or overridden
  notes: optional free-text

Relations:
  belongs_to_tor: meeting → tor
  scheduled_for_meeting: agenda_point → meeting (agenda assigned to this meeting)
  minutes_of: meeting → minutes (existing relation type, now usable)
```

### Meeting Workflow (seeded)

```
Statuses (entity_type_scope = 'meeting'):
  projected    [is_initial]  — computed from calendar, not yet confirmed
  confirmed                  — user confirmed this meeting will happen
  in_progress                — meeting is underway
  completed    [is_terminal] — meeting finished, minutes can be generated
  cancelled    [is_terminal] — meeting was cancelled

Transitions:
  projected   → confirmed     (requires: tor.edit)
  projected   → cancelled     (requires: tor.edit)
  confirmed   → in_progress   (requires: tor.edit)
  confirmed   → cancelled     (requires: tor.edit)
  in_progress → completed     (requires: tor.edit)
```

### New Relation Types (to seed)

| Name | Purpose |
|------|---------|
| `belongs_to_tor` | meeting → tor (scoping) |
| `scheduled_for_meeting` | agenda_point → meeting (agenda assignment) |

Note: `minutes_of` already exists in seed data.

### New Permission

| Code | Group | Description |
|------|-------|-------------|
| `meetings.view` | Governance | View meetings list |

Meeting mutations reuse `tor.edit` (gated via workflow transitions). View permission is separate so read-only users can see the cross-ToR list.

### New Nav Item

| Name | Label | Parent | URL | Permission |
|------|-------|--------|-----|------------|
| `governance.meetings` | Meetings | `governance` | `/meetings` | `meetings.view` |

## Routes

| Method | Path | Handler | Permission | Description |
|--------|------|---------|------------|-------------|
| GET | `/meetings` | `list` | `meetings.view` | Cross-ToR meeting list |
| GET | `/tor/{id}/meetings/{mid}` | `detail` | ToR membership | Meeting detail page |
| POST | `/tor/{id}/meetings/confirm` | `confirm` | ToR membership | Confirm a projected meeting |
| POST | `/tor/{id}/meetings/{mid}/transition` | `transition` | ToR membership | Workflow transition |
| POST | `/tor/{id}/meetings/{mid}/agenda/assign` | `assign_agenda` | ToR membership | Assign agenda point to meeting |
| POST | `/tor/{id}/meetings/{mid}/agenda/remove` | `remove_agenda` | ToR membership | Remove agenda point from meeting |
| POST | `/tor/{id}/meetings/{mid}/minutes/generate` | `generate_minutes` | `minutes.generate` | Generate minutes scaffold |

## UI Sections

### ToR Detail — Meetings Section (new)

Shows two groups:
- **Upcoming:** projected meetings (from calendar computation) with "Confirm" button; confirmed meetings with link to detail
- **Past:** completed meetings with minutes status badge (none / draft / approved)

### Cross-ToR Meeting List (`/meetings`)

Table with columns: Date, ToR, Status, Agenda Items count, Minutes status. Filterable by status. Links to scoped detail page.

### Meeting Detail (`/tor/{id}/meetings/{mid}`)

- **Header:** meeting date, ToR name, status badge, transition buttons
- **Agenda section:** assigned agenda points with remove button; dropdown to assign unassigned points from this ToR
- **Protocol section:** read-only view of ToR's protocol steps (referenced, not copied)
- **Minutes section:** "Generate Minutes" button (if completed + no minutes yet) or link to existing minutes

## Existing Code Changes

- `src/handlers/minutes_handlers/crud.rs` — `generate_minutes` handler receives `meeting_id` from a real entity now; remove the broken form-data path
- `templates/tor/detail.html` — add Meetings section after Dependencies
- `data/seed/ontology.json` — add meeting workflow statuses/transitions, relation types, nav item, permission
- `src/models/mod.rs` — declare `meeting` module
- `src/handlers/mod.rs` — declare `meeting_handlers` module
- `src/main.rs` — register new routes

## Out of Scope

- Live meeting mode (real-time agenda progression, vote capture)
- Minutes export (PDF/Word) — tracked as T.4
- Meeting notifications / reminders
- Recurring meeting series entity (cadence handles this)
- Meeting attendance tracking beyond what minutes scaffold generates
