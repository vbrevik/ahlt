# ToR Expansion — Enhanced Governance Data Model

**Date:** 2026-02-18
**Status:** Approved
**Approach:** Pure EAV — all new concepts modeled as entity types with properties and relations
**Extends:** Phase 1 (implemented), Phase 2a/2b/3 (designed, not yet built)

## Overview

Expands the Terms of Reference system with five new capabilities that make it enterprise-grade:

1. **Position-based membership** — authority flows from positions, not people
2. **Meeting protocol templates** — configurable meeting order with per-meeting overrides
3. **Meeting dependency map** — horizontal (feeds_into) and vertical (escalates_to) relationships between ToRs
4. **Structured minutes** — auto-scaffold from meeting outcomes + manual enrichment
5. **Presentation templates** — required slide deck structures for presenters

All concepts use the pure EAV pattern consistent with the existing architecture.

## Design Decisions

### Position-Based Membership (not Person-Based)

**Chosen:** Authority and membership attach to positions (tor_function), not people. People fill positions via `fills_position` relation. When a person leaves, the position retains all authority and membership — reassign to the new person.

**Rationale:** In enterprise governance, the CFO position is what grants Board membership, not the individual. Personnel changes should not require reconfiguring ToR membership or authority.

**Impact:** Replaces existing `member_of` (user → tor) and `has_tor_role` (user → tor_function) relations with a single `fills_position` (user → tor_function) relation. Membership is derived: user is a member of a ToR if they fill any position belonging to that ToR.

### Meeting Protocol Templates with Overrides

**Chosen:** ToR defines a default protocol template (ordered steps). Each meeting instance clones the template at creation time. Chair can add/remove/reorder non-required steps per meeting.

**Rationale:** Consistency across meetings of the same body, with flexibility for special sessions. Past meetings are unaffected by template changes.

### Meeting Dependencies at ToR Level

**Chosen:** Dependencies (feeds_into, escalates_to) are defined between ToRs, not between individual meetings. Meetings inherit their ToR's dependency relationships.

**Rationale:** Define the governance structure once. All future meetings automatically understand which other bodies they relate to.

### Hybrid Minutes Generation

**Chosen:** System auto-generates a structured scaffold (attendance, protocol steps, agenda items with outcomes, decisions). Secretary edits/enriches before the chair submits for approval.

**Rationale:** Eliminates blank-page problem. Ensures decisions and attendance are always captured. Manual enrichment adds narrative context the system can't generate.

### Slide Deck Templates as Structure Definitions

**Chosen:** Templates define required slide titles and guidance (what each slide should contain). The system does not generate or store actual PowerPoint files — it defines the structural requirement.

**Rationale:** Governance tool ensuring presentation consistency. Presenters see the required format and compose their own decks. Keeps the system simple.

### Mandatory/Optional Members — Soft Warning

**Chosen:** When a mandatory position is unfilled or the person is absent, the system shows a warning but does not block meeting proceedings.

**Rationale:** Governance flexibility — the chair decides whether to proceed. Hard blocking would create operational friction. Absence is recorded in minutes for the audit trail.

## New Entity Types

### `protocol_step` — Step in a ToR's Meeting Protocol Template

| Property | Values | Purpose |
|---|---|---|
| `step_type` | `procedural` / `agenda_slot` / `fixed` | procedural = Opening/Roll Call/Closing; agenda_slot = where agenda items get inserted; fixed = recurring item like "Previous Minutes Approval" |
| `sequence_order` | integer | Position in the protocol (1, 2, 3...) |
| `default_duration_minutes` | integer, optional | Expected time for this step |
| `description` | text, optional | Instructions or notes |
| `is_required` | true/false | Can this step be removed in per-meeting overrides? |

Entity metadata: `entity_type` = `protocol_step`, `name` = e.g. "opening", `label` = e.g. "Opening"

### `meeting_step` — A Step in a Specific Meeting Instance

Cloned from `protocol_step` at meeting creation time. Can be modified per-meeting.

| Property | Values | Purpose |
|---|---|---|
| `step_type` | `procedural` / `agenda_slot` / `fixed` / `custom` | Same as protocol_step, plus `custom` for per-meeting additions |
| `sequence_order` | integer | Position in this meeting's order |
| `duration_minutes` | integer, optional | Overridden duration |
| `description` | text, optional | Overridden instructions |
| `status` | `pending` / `in_progress` / `completed` / `skipped` | Step progress during meeting |
| `notes` | text, optional | Notes recorded during the meeting for this step |

Entity metadata: `entity_type` = `meeting_step`, `name` = auto-generated, `label` = inherited from protocol_step

### `minutes` — Minutes Document for a Meeting

| Property | Values | Purpose |
|---|---|---|
| `status` | `draft` / `pending_approval` / `approved` | Minutes lifecycle |
| `generated_date` | YYYY-MM-DD | When scaffold was created |
| `approved_date` | YYYY-MM-DD, optional | When formally approved |
| `approved_by_id` | i64, optional | Who approved |

Entity metadata: `entity_type` = `minutes`, `name` = auto-generated (e.g. "minutes_2026_02_18"), `label` = "Minutes — " + meeting label

### `minutes_section` — A Section Within Minutes

| Property | Values | Purpose |
|---|---|---|
| `section_type` | `attendance` / `protocol` / `agenda_item` / `decision` / `action_item` / `custom` | Content category |
| `sequence_order` | integer | Order in the document |
| `content` | text | The actual text — auto-generated or hand-written |
| `source_entity_id` | i64, optional | Which agenda point/meeting step this was generated from |
| `is_auto_generated` | true/false | Was this scaffolded by the system? |

Entity metadata: `entity_type` = `minutes_section`, `name` = auto-generated, `label` = section type + sequence

### `presentation_template` — Required Slide Deck Structure

| Property | Values | Purpose |
|---|---|---|
| `description` | text | When to use this template |
| `slide_count` | integer | Expected number of slides |
| `applies_to` | `all` / `decision` / `informative` | Which agenda item types require this template |

Entity metadata: `entity_type` = `presentation_template`, `name` = e.g. "budget_proposal_template", `label` = e.g. "Budget Proposal Template"

### `template_slide` — A Required Slide Within a Template

| Property | Values | Purpose |
|---|---|---|
| `slide_order` | integer | Position in the deck (1, 2, 3...) |
| `slide_title` | text | Required title, e.g. "Executive Summary" |
| `guidance` | text | Instructions for the presenter |
| `is_required` | true/false | Must this slide be present? |

Entity metadata: `entity_type` = `template_slide`, `name` = auto-generated, `label` = slide_title

## New Relation Types

### Membership (replacing existing)

| Relation Type | Source → Target | Purpose |
|---|---|---|
| `fills_position` | user → tor_function | Person currently holding this position |

**Replaces:** `member_of` (user → tor) and `has_tor_role` (user → tor_function)

**Relation property** (via `relation_properties` table):
- `membership_type`: `mandatory` / `optional`

**Keeps:** `belongs_to_tor` (tor_function → tor) — unchanged

### Protocol

| Relation Type | Source → Target | Purpose |
|---|---|---|
| `protocol_of` | protocol_step → tor | This step belongs to this ToR's protocol template |
| `step_of_meeting` | meeting_step → meeting | This step is part of this meeting's order |
| `cloned_from` | meeting_step → protocol_step | This meeting step was derived from this template step |

### Meeting Dependencies

| Relation Type | Source → Target | Purpose |
|---|---|---|
| `feeds_into` | tor → tor | Horizontal: outputs flow from source to target ToR |
| `escalates_to` | tor → tor | Vertical: items can be escalated to target ToR |

**Relation properties** (via `relation_properties` table):
- `output_types`: comma-separated list of `decisions`, `minutes`, `proposals`, `action_items`
- `description`: text explanation of the relationship
- `is_blocking`: true/false — must source complete before target proceeds (advisory)

### Minutes

| Relation Type | Source → Target | Purpose |
|---|---|---|
| `minutes_of` | minutes → meeting | These minutes belong to this meeting |
| `section_of` | minutes_section → minutes | This section is part of these minutes |

### Presentation Templates

| Relation Type | Source → Target | Purpose |
|---|---|---|
| `template_of` | presentation_template → tor | This template is defined for this ToR |
| `slide_of` | template_slide → presentation_template | This slide belongs to this template |
| `requires_template` | agenda_point → presentation_template | This agenda point requires this presentation format |

## Derived Membership Query

Membership is no longer a direct relation. It's derived through positions:

```sql
-- "Who are the members of ToR X?"
SELECT u.id, u.name, u.label,
       f.id as position_id, f.label as position_label,
       COALESCE(rp.value, 'optional') as membership_type
FROM entities f
JOIN relations r_tor ON f.id = r_tor.source_id
  AND r_tor.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor')
LEFT JOIN relations r_fills ON f.id = r_fills.target_id
  AND r_fills.relation_type_id = (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'fills_position')
LEFT JOIN entities u ON r_fills.source_id = u.id
LEFT JOIN relation_properties rp ON r_fills.id = rp.relation_id AND rp.key = 'membership_type'
WHERE r_tor.target_id = ?1  -- tor_id
  AND f.entity_type = 'tor_function'
ORDER BY COALESCE(rp.value, 'optional') DESC, f.label  -- mandatory first
```

This returns positions even when unfilled (user columns will be NULL), which is useful for showing vacancies.

## Minutes Auto-Scaffold Algorithm

When "Generate Minutes" is triggered for a completed meeting:

1. **Create `minutes` entity** with `status=draft`, `generated_date=today`
2. **Attendance section** — query all positions for the ToR, check which users were marked present (from meeting attendance, if tracked) or infer from meeting_step completion. Flag missing mandatory positions.
3. **Protocol sections** — one section per `meeting_step` (ordered by `sequence_order`), pre-filled with step label, status (completed/skipped), and any notes.
4. **Agenda item sections** — one section per agenda point (ordered by `scheduled_order`), pre-filled with: title, presenter, item type badge, outcome summary, decision text (if decision type), opinion summary (if opinions were recorded).
5. **Decisions summary section** — consolidated list of all decision-type agenda points that reached `completed` status, with decision text.
6. **Action items section** — empty placeholder for manual entry.

## Meeting Protocol Clone Algorithm

When a meeting is created (recurring or ad-hoc):

1. Query all `protocol_step` entities where `protocol_of → tor` matches the meeting's ToR
2. For each protocol_step (ordered by `sequence_order`):
   - Create a `meeting_step` entity
   - Copy properties: `step_type`, `sequence_order`, `default_duration_minutes` → `duration_minutes`, `description`
   - Create `step_of_meeting` relation (meeting_step → meeting)
   - Create `cloned_from` relation (meeting_step → protocol_step)
3. Chair can then modify, add, or remove non-required meeting steps

## Dependency Map Visualization

The ToR detail page shows:

**Upstream (feeds this ToR):**
- List of ToRs with `feeds_into` relation targeting this ToR
- Shows output types and descriptions

**Downstream (this ToR feeds):**
- List of ToRs this ToR has `feeds_into` relation to

**Escalation path:**
- ToRs this body can escalate to (`escalates_to`)
- ToRs that can escalate to this body (reverse `escalates_to`)

A separate "Governance Map" page shows all ToRs and their dependency/escalation relationships in a table or simple diagram format.

## Phased Delivery

These extensions integrate into the existing phase structure:

### Phase 1b — Position-Based Membership Migration
- Add `fills_position` relation type to seed data
- Migrate existing `member_of` / `has_tor_role` to `fills_position`
- Add `relation_properties` table (from Phase 2b design, pulled forward)
- Update membership queries in `src/models/tor/queries.rs`
- Update ToR detail template to show positions with current holders
- Update member management UI to assign people to positions

### Phase 3a — Meeting Protocol Templates
- Add `protocol_step` entity type
- CRUD for protocol steps on ToR detail page
- Protocol template management UI (reorder, add, remove steps)

### Phase 3b — Meeting Instance Protocol
- Add `meeting_step` entity type
- Clone algorithm when creating meetings
- Per-meeting step management (add/remove/reorder non-required)
- Step progress tracking during meetings

### Phase 3c — Meeting Dependencies
- Add `feeds_into` and `escalates_to` relation types
- Dependency management UI on ToR detail page
- Governance map page
- "Pending upstream outputs" prompt when building agenda

### Phase 3d — Minutes Generation
- Add `minutes` and `minutes_section` entity types
- Auto-scaffold algorithm
- Minutes editing UI
- Approval workflow (draft → pending_approval → approved)
- Link to "Approval of Previous Minutes" protocol step

### Phase 3e — Presentation Templates
- Add `presentation_template` and `template_slide` entity types
- Template CRUD on ToR admin page
- Link templates to agenda points
- Presenter sees required slide format on agenda point detail

## Implementation Task Contracts

Each implementation task follows the prompt-contract format with GOAL, CONSTRAINTS, FORMAT, and FAILURE CONDITIONS.

---

### Task 1: Add `relation_properties` Table and `fills_position` Relation Type

**GOAL:** Extend the database schema with the `relation_properties` table and seed the `fills_position` relation type. After running, `relation_properties` table exists and `fills_position` appears in `entities` as a relation_type. Verify by querying: `SELECT * FROM entities WHERE entity_type='relation_type' AND name='fills_position'` returns one row.

**CONSTRAINTS:**
- Modify `src/schema.sql` for the new table
- Modify `src/db.rs` seed_ontology for the new relation type
- SQLite `ON DELETE CASCADE` on `relation_id` foreign key
- PRIMARY KEY on `(relation_id, key)` matching EAV pattern
- No new Rust dependencies

**FORMAT:**
1. Add `CREATE TABLE relation_properties` to `src/schema.sql`
2. Add `fills_position` relation type insert to `src/db.rs` seed_ontology()
3. Remove `member_of` and `has_tor_role` relation type inserts from seed (if not referenced elsewhere)

**FAILURE CONDITIONS:**
- `relation_properties` table missing CASCADE on foreign key
- Missing PRIMARY KEY constraint
- `fills_position` not seeded
- Old `member_of` / `has_tor_role` still seeded without migration path
- Schema doesn't match EAV convention

---

### Task 2: Update ToR Model Types for Position-Based Membership

**GOAL:** Update `src/models/tor/types.rs` so `TorMember` reflects position-based membership: each entry is a position (tor_function) with its current holder (optional — position may be vacant) and membership type (mandatory/optional). Verify: `cargo check` passes.

**CONSTRAINTS:**
- Keep existing `TorFunctionDetail` and `TorFunctionListItem` types
- Position-first data model: `TorMember` wraps a position, not a user
- Handle vacant positions (user fields are `Option`)
- Follow existing type patterns (derive Debug, Clone)

**FORMAT:**
- Modify: `src/models/tor/types.rs`
- `TorMember` struct: `position_id: i64`, `position_name: String`, `position_label: String`, `membership_type: String`, `holder_id: Option<i64>`, `holder_name: Option<String>`, `holder_label: Option<String>`

**FAILURE CONDITIONS:**
- `TorMember` still has `user_id` as non-optional primary field
- No representation of vacant positions
- Missing `membership_type` field
- `cargo check` fails

---

### Task 3: Update ToR Membership Queries for Position-Based Model

**GOAL:** Rewrite `find_members()`, `add_member()`, `remove_member()` and related queries in `src/models/tor/queries.rs` to use `fills_position` instead of `member_of` + `has_tor_role`. Add `assign_to_position()` and `vacate_position()` query functions. Verify: `cargo check` passes.

**CONSTRAINTS:**
- Derived membership: user is member if they fill any position belonging to the ToR
- `find_members()` returns positions (including vacant ones) with optional holder
- `assign_to_position(conn, user_id, function_id, membership_type)` creates `fills_position` relation + `relation_properties` entry
- `vacate_position(conn, function_id)` removes the `fills_position` relation for a position
- `find_non_members()` returns users not currently filling any position in the ToR
- Use `relation_properties` table for `membership_type`

**FORMAT:**
- Modify: `src/models/tor/queries.rs`
- Functions: `find_members()`, `assign_to_position()`, `vacate_position()`, `find_non_members()`
- Remove or deprecate: `add_member()`, `remove_member()`

**FAILURE CONDITIONS:**
- Still queries `member_of` relation type
- Still queries `has_tor_role` relation type
- Vacant positions not returned by `find_members()`
- `membership_type` not read from `relation_properties`
- `cargo check` fails

---

### Task 4: Update ToR Handlers and Templates for Position-Based Membership

**GOAL:** Update the ToR detail page and member management handlers to show positions with holders instead of flat member lists. The "Add Member" flow becomes "Assign Person to Position". Verify: `cargo build` passes. Manual test: ToR detail page shows positions with holder names and mandatory/optional badges.

**CONSTRAINTS:**
- ToR detail shows positions table: Position | Membership Type | Current Holder | Actions
- "Assign" button opens a dropdown of users not already in this ToR
- "Vacate" button removes the person from the position (position stays)
- Mandatory positions without a holder show a warning badge
- Follow existing template patterns (Askama, `ctx.permissions.has()`)
- CSRF on all mutations

**FORMAT:**
- Modify: `src/handlers/tor_handlers/members.rs` (assign_to_position, vacate_position actions)
- Modify: `src/handlers/tor_handlers/crud.rs` (detail handler builds new TorMember data)
- Modify: `templates/tor/detail.html` (positions table replacing members table)
- Modify: `src/templates_structs.rs` (update TorDetailTemplate if needed)

**FAILURE CONDITIONS:**
- Still shows flat "Members" table without position context
- No mandatory/optional distinction visible
- Vacant positions not shown
- Missing CSRF on assign/vacate
- `cargo build` fails

---

### Task 5: Seed Protocol Step Entity Type and Create Model

**GOAL:** Add `protocol_step` entity type support. Seed data includes the entity type recognition. Model types and queries exist for CRUD operations on protocol steps scoped to a ToR. Verify: `cargo check` passes.

**CONSTRAINTS:**
- `protocol_of` relation type seeded in `src/db.rs`
- Properties: `step_type`, `sequence_order`, `default_duration_minutes`, `description`, `is_required`
- Queries scoped to a ToR via `protocol_of` relation
- Ordered by `sequence_order`
- New module: `src/models/protocol/` (types.rs, queries.rs, mod.rs)
- Register in `src/models/mod.rs`

**FORMAT:**
- Modify: `src/db.rs` (seed `protocol_of` relation type)
- Create: `src/models/protocol/types.rs` — `ProtocolStep` struct
- Create: `src/models/protocol/queries.rs` — `find_steps_for_tor()`, `create_step()`, `update_step()`, `delete_step()`, `reorder_steps()`
- Create: `src/models/protocol/mod.rs`
- Modify: `src/models/mod.rs` — add `pub mod protocol`

**FAILURE CONDITIONS:**
- `protocol_of` relation type not seeded
- Steps not ordered by `sequence_order`
- Missing `reorder_steps()` function
- Module not registered in `src/models/mod.rs`
- `cargo check` fails

---

### Task 6: Protocol Template Management UI

**GOAL:** ToR detail page gets a "Meeting Protocol" section where users with `tor.edit` permission can view, add, reorder, and remove protocol steps. Verify: manual test — add 5 protocol steps to a ToR, reorder them, delete one, see correct order preserved.

**CONSTRAINTS:**
- Section on existing ToR detail page (not a separate page)
- Table: # | Label | Type | Duration | Required | Actions
- "Add Step" form: name, label, step_type (dropdown), duration, description, is_required (checkbox)
- Reorder via up/down buttons (POST actions that swap sequence_order)
- Delete with confirmation (only non-required steps)
- Permission gated: `tor.edit`
- CSRF on all mutations

**FORMAT:**
- Modify: `templates/tor/detail.html` — add Protocol section
- Modify: `src/handlers/tor_handlers/crud.rs` — detail handler loads protocol steps
- Create: `src/handlers/tor_handlers/protocol.rs` — add_step, delete_step, reorder_step handlers
- Modify: `src/handlers/tor_handlers/mod.rs` — add `pub mod protocol`
- Modify: `src/main.rs` — wire protocol routes under `/tor/{id}/protocol/...`
- Modify: `src/templates_structs.rs` — add protocol steps to TorDetailTemplate

**FAILURE CONDITIONS:**
- Protocol section missing from ToR detail page
- Can delete required steps
- Reorder doesn't persist correctly
- Missing CSRF on add/delete/reorder
- Missing permission check
- `cargo build` fails

---

### Task 7: Meeting Step Clone and Per-Meeting Override

**GOAL:** When a meeting entity is created, the system clones the ToR's protocol steps into `meeting_step` entities linked to that meeting. Chair can add custom steps and remove non-required steps. Verify: create a meeting for a ToR with 5 protocol steps — meeting detail shows 5 meeting steps. Add a custom step, remove an optional step — meeting shows correct modified order.

**CONSTRAINTS:**
- `meeting_step` entity type, `step_of_meeting` and `cloned_from` relation types seeded
- Clone happens in a `clone_protocol_to_meeting(conn, tor_id, meeting_id)` query function
- Custom steps have `step_type = "custom"` and no `cloned_from` relation
- Cannot remove steps where source protocol_step has `is_required = true`
- New module: `src/models/meeting_step/` (types.rs, queries.rs, mod.rs)

**FORMAT:**
- Modify: `src/db.rs` — seed `meeting_step` entity type recognition, `step_of_meeting` and `cloned_from` relation types
- Create: `src/models/meeting_step/types.rs` — `MeetingStep` struct
- Create: `src/models/meeting_step/queries.rs` — `clone_protocol_to_meeting()`, `find_steps_for_meeting()`, `add_custom_step()`, `remove_step()`, `reorder_steps()`
- Create: `src/models/meeting_step/mod.rs`
- Modify: `src/models/mod.rs`

**FAILURE CONDITIONS:**
- Clone doesn't copy all properties from protocol_step
- `cloned_from` relation not created
- Required steps can be removed from meeting
- Custom steps don't get `step_type = "custom"`
- `cargo check` fails

---

### Task 8: Meeting Dependency Relations and ToR UI

**GOAL:** Add `feeds_into` and `escalates_to` relation types between ToRs. ToR detail page shows a "Dependencies" section listing upstream and downstream ToRs. Admin can add/remove dependencies. Verify: create 3 ToRs, add feeds_into and escalates_to relations between them, see correct display on each ToR's detail page.

**CONSTRAINTS:**
- `feeds_into` and `escalates_to` relation types seeded
- Relation properties: `output_types`, `description`, `is_blocking` (via `relation_properties` table)
- Self-referencing prevented (a ToR cannot feed into or escalate to itself)
- Circular dependency warning (advisory, not blocking)
- Permission gated: `tor.edit`
- CSRF on all mutations

**FORMAT:**
- Modify: `src/db.rs` — seed relation types
- Create: `src/models/tor/dependencies.rs` — `find_upstream()`, `find_downstream()`, `find_escalation_targets()`, `find_escalation_sources()`, `add_dependency()`, `remove_dependency()`
- Modify: `src/models/tor/mod.rs` — add `pub mod dependencies`
- Modify: `templates/tor/detail.html` — add Dependencies section
- Create: `src/handlers/tor_handlers/dependencies.rs` — add/remove dependency handlers
- Modify: `src/handlers/tor_handlers/mod.rs`
- Modify: `src/main.rs` — wire dependency routes
- Modify: `src/templates_structs.rs` — add dependency data to TorDetailTemplate

**FAILURE CONDITIONS:**
- Self-referencing allowed
- Relation properties not stored/displayed
- Missing upstream or downstream direction
- Missing CSRF or permission check
- `cargo build` fails

---

### Task 9: Minutes Entity Model and Auto-Scaffold

**GOAL:** Add `minutes` and `minutes_section` entity types with a `generate_minutes_scaffold(conn, meeting_id)` function that auto-creates a structured minutes document from meeting data. Verify: for a meeting with 3 completed agenda points (1 decision, 2 informative), scaffold generates minutes with attendance, protocol, agenda item, decisions summary, and action items sections.

**CONSTRAINTS:**
- `minutes_of` and `section_of` relation types seeded
- Scaffold pulls data from: meeting steps (attendance/protocol), agenda points (items/decisions), opinions (for decision items)
- Each section has `is_auto_generated = true`
- Attendance section flags vacant mandatory positions
- Decisions summary only includes decision-type agenda points with `completed` status
- New module: `src/models/minutes/` (types.rs, queries.rs, mod.rs)

**FORMAT:**
- Modify: `src/db.rs` — seed relation types
- Create: `src/models/minutes/types.rs` — `Minutes`, `MinutesSection` structs
- Create: `src/models/minutes/queries.rs` — `generate_minutes_scaffold()`, `find_minutes_for_meeting()`, `find_sections()`, `update_section()`, `update_minutes_status()`, `add_custom_section()`
- Create: `src/models/minutes/mod.rs`
- Modify: `src/models/mod.rs`

**FAILURE CONDITIONS:**
- Scaffold missing any of the 6 section types (attendance, protocol, agenda_item, decision, action_item)
- Decision summary includes non-completed or informative items
- Vacant mandatory positions not flagged in attendance
- `is_auto_generated` not set
- `cargo check` fails

---

### Task 10: Minutes UI — Editing and Approval

**GOAL:** Meeting detail page gets a "Minutes" section. Chair can trigger scaffold generation, secretary can edit sections, chair can submit for approval. Verify: generate minutes for a completed meeting, edit a section, change status to pending_approval, then to approved.

**CONSTRAINTS:**
- "Generate Minutes" button only on completed meetings without existing minutes
- Each section rendered as editable textarea (if user has `minutes.edit` permission)
- Auto-generated badge on scaffolded sections
- Status transitions: draft → pending_approval → approved
- `approved_by_id` and `approved_date` set on approval
- New permissions: `minutes.generate`, `minutes.edit`, `minutes.approve`
- CSRF on all mutations
- Audit logging: `minutes.generated`, `minutes.edited`, `minutes.approved`

**FORMAT:**
- Create: `templates/minutes/view.html` — minutes document with editable sections
- Create: `src/handlers/minutes_handlers/` (mod.rs, crud.rs)
- Modify: `src/main.rs` — wire minutes routes under `/meetings/{id}/minutes/...`
- Modify: `src/db.rs` — seed new permissions
- Modify: `src/templates_structs.rs` — `MinutesViewTemplate`

**FAILURE CONDITIONS:**
- Can generate minutes for non-completed meeting
- Can generate duplicate minutes for same meeting
- Approved minutes still editable
- Missing audit logging on status changes
- Missing CSRF or permission checks
- `cargo build` fails

---

### Task 11: Presentation Template Entity Model

**GOAL:** Add `presentation_template` and `template_slide` entity types with CRUD queries. Verify: `cargo check` passes. Create a template with 4 slides, retrieve them in order.

**CONSTRAINTS:**
- `template_of` and `slide_of` relation types seeded
- Slides ordered by `slide_order`
- Templates scoped to a ToR via `template_of` relation
- New module: `src/models/presentation_template/` (types.rs, queries.rs, mod.rs)

**FORMAT:**
- Modify: `src/db.rs` — seed relation types
- Create: `src/models/presentation_template/types.rs` — `PresentationTemplate`, `TemplateSlide` structs
- Create: `src/models/presentation_template/queries.rs` — `find_templates_for_tor()`, `find_template_by_id()`, `create_template()`, `update_template()`, `delete_template()`, `find_slides()`, `create_slide()`, `update_slide()`, `delete_slide()`, `reorder_slides()`
- Create: `src/models/presentation_template/mod.rs`
- Modify: `src/models/mod.rs`

**FAILURE CONDITIONS:**
- Slides not ordered by `slide_order`
- Templates not scoped to ToR
- Missing reorder function
- Module not registered
- `cargo check` fails

---

### Task 12: Presentation Template Management UI

**GOAL:** ToR admin page gets a "Presentation Templates" section for managing templates and their required slides. When creating/scheduling an agenda point, optionally link a template. Presenter sees required format on agenda point detail. Verify: create a template with 3 slides on a ToR, link it to an agenda point, view the agenda point and see slide requirements.

**CONSTRAINTS:**
- Template CRUD on ToR detail page (or dedicated sub-page)
- Slide management: add, edit, reorder, delete slides within a template
- `requires_template` relation type seeded (agenda_point → presentation_template)
- Agenda point form gets optional "Presentation Template" dropdown
- Agenda point detail shows required slides if template linked
- Permission gated: `tor.edit` for template management
- CSRF on all mutations

**FORMAT:**
- Modify: `src/db.rs` — seed `requires_template` relation type
- Create: `templates/tor/presentation_templates.html` — template + slide management
- Create: `src/handlers/tor_handlers/presentation.rs` — template and slide CRUD handlers
- Modify: `src/handlers/tor_handlers/mod.rs`
- Modify: `src/main.rs` — wire routes under `/tor/{id}/templates/...`
- Modify: `src/templates_structs.rs`

**FAILURE CONDITIONS:**
- Slides not manageable (add/edit/reorder/delete)
- Template not linkable to agenda points
- Presenter doesn't see required format
- Missing CSRF or permission checks
- `cargo build` fails

---

### Task 13: Governance Map Page

**GOAL:** A dedicated page showing all ToRs and their dependency/escalation relationships. Accessible from the Governance nav module. Verify: with 4 ToRs having various feeds_into and escalates_to relations, the page renders a table/matrix showing all relationships.

**CONSTRAINTS:**
- Route: `GET /governance/map`
- Table/matrix format showing ToR-to-ToR relationships
- Color-coded: blue for feeds_into, orange for escalates_to
- Each cell shows output types if relationship exists
- Click ToR name to navigate to its detail page
- Permission gated: `tor.list`
- Seed nav item for "Governance Map"

**FORMAT:**
- Create: `templates/governance/map.html`
- Create: `src/handlers/governance_handlers/map.rs`
- Create: `src/handlers/governance_handlers/mod.rs`
- Modify: `src/handlers/mod.rs` — add `pub mod governance_handlers`
- Modify: `src/main.rs` — wire route
- Modify: `src/db.rs` — seed nav item
- Modify: `src/templates_structs.rs` — `GovernanceMapTemplate`

**FAILURE CONDITIONS:**
- Missing either feeds_into or escalates_to visualization
- Not clickable to ToR detail
- Missing permission check
- `cargo build` fails

---

## Relationship Diagram

```
                    ┌─────────────┐
                    │     tor     │
                    └──────┬──────┘
       ┌──────────────┬────┼────┬──────────────┬──────────────┐
       │              │    │    │              │              │
 belongs_to_tor  protocol_of  template_of  feeds_into   escalates_to
       │              │    │    │              │              │
┌──────┴──────┐  ┌────┴────┐  ┌┴───────────┐  │            other
│ tor_function │  │protocol │  │presentation│  other         tors
└──────┬──────┘  │  step   │  │ template   │  tors
       │         └─────────┘  └──────┬─────┘
 fills_position          │      slide_of
       │           cloned_from       │
┌──────┴──────┐          │    ┌──────┴──────┐
│    user     │    ┌─────┴─────┐  │template_slide│
└─────────────┘    │meeting_step│  └─────────────┘
                   └─────┬─────┘
                  step_of_meeting
                         │
                   ┌─────┴─────┐     minutes_of     ┌─────────┐
                   │  meeting  │◀────────────────────│ minutes │
                   └───────────┘                     └────┬────┘
                                                    section_of
                                                         │
                                                  ┌──────┴───────┐
                                                  │minutes_section│
                                                  └──────────────┘
```

## Summary

- **6 new entity types:** protocol_step, meeting_step, minutes, minutes_section, presentation_template, template_slide
- **11 new relation types:** fills_position, protocol_of, step_of_meeting, cloned_from, feeds_into, escalates_to, minutes_of, section_of, template_of, slide_of, requires_template
- **2 removed relation types:** member_of, has_tor_role (replaced by fills_position)
- **1 new table:** relation_properties (EAV for relation metadata)
- **13 implementation tasks** with prompt-contract format
