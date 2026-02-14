# Phase 2b: Agenda Points, COAs & Data-Driven Workflows

**Date:** 2026-02-14
**Status:** Approved
**Phase:** Phase 2b (Item Workflow - Part 2 of 2)
**Dependencies:** Phase 2a (Suggestions + Proposals)
**Related:** Phase 3 will add Meeting entities + calendar views

## Overview

Extends the governance item workflow with **Agenda Points**, **Courses of Action (COAs)**, and a **data-driven workflow engine**. Renames "Pipeline" to "Workflow" throughout the application. Introduces opinion recording, decision authority, and structured document support for COAs.

**Key capabilities:**
- Data-driven workflow engine replacing all hardcoded status transitions
- Agenda points with scheduling, opinion recording, and decision making
- COAs as structured documents (simple yes/no or nested sections)
- Proposal queue with bulk scheduling to agenda
- Opinion recording from meeting attendees (advisory, not formal voting)
- Decision recording by authority holder (`agenda.decide` permission)
- Rename "Pipeline" → "Workflow" throughout

## Design Decisions

### Terminology
**Chosen:** Rename "Pipeline" to "Workflow"
**Rationale:** Better reflects the governance nature of the system. Affects URLs, nav items, templates, handlers.

### Workflow Engine
**Chosen:** Data-driven transitions via `workflow_status` and `workflow_transition` entities
**Rationale:** Eliminates hardcoded status transitions. New entity types can define workflows via seed data, not code changes. Retroactively replaces hardcoded suggestion/proposal transitions.

### COA Structure
**Chosen:** Nested sections (tree structure) for complex COAs, simple title+description for yes/no COAs
**Rationale:** Supports legal-document-style structures. Decision-makers can reference specific sections when synthesizing hybrid decisions.

### Opinion Recording (Not Voting)
**Chosen:** Advisory opinions — members record preferences and comments, decision-maker has final authority
**Rationale:** Governance model has a designated decision-maker with veto/final say. Member input is recorded for transparency but is advisory, not binding.

### Agenda Point Scheduling
**Chosen:** Standalone scheduling with date/order properties (no meeting entity yet)
**Rationale:** Phase 2b focuses on ToR workflow management. Phase 3 adds meeting entities and calendar views.

### Proposal Queue
**Chosen:** Hybrid queue — approved proposals flagged as "ready for agenda", then bulk-scheduled by chair
**Rationale:** Efficient workflow for chairs managing multiple proposals.

## Entity Types

### `workflow_status` — A State in a Workflow

Properties (EAV):
- `entity_type_scope` (text) - Which entity type this status applies to (e.g., "agenda_point", "proposal", "suggestion")
- `status_code` (text) - Machine-readable code (e.g., "scheduled", "in_progress")
- `is_initial` (boolean) - Is this the starting state?
- `is_terminal` (boolean) - Is this a final state?

Entity metadata:
- `entity_type` = `workflow_status`
- `name` = e.g., "agenda_point.scheduled"
- `label` = e.g., "Scheduled"

### `workflow_transition` — A Valid Move Between Statuses

Properties (EAV):
- `required_permission` (text) - Permission code needed (e.g., "agenda.manage")
- `condition` (text, optional) - Additional condition (e.g., "item_type=decision")
- `requires_outcome` (boolean) - Must record outcome_summary to complete?
- `transition_label` (text) - Button label in UI (e.g., "Start Discussion")

Entity metadata:
- `entity_type` = `workflow_transition`
- `name` = e.g., "agenda_point.scheduled_to_in_progress"
- `label` = e.g., "Start Discussion"

### `agenda_point` — Scheduled Item for Discussion

Properties (EAV):
- `title` (text) - Carried over from proposal or entered manually
- `description` (text) - Full text of what will be discussed
- `item_type` (enum: `informative` | `decision`) - Determines if opinions are recorded
- `scheduled_date` (YYYY-MM-DD, ISO-8601) - When this will be discussed
- `scheduled_order` (integer) - Display order within the date (1, 2, 3...)
- `status` (text) - Current workflow status code
- `presenter_id` (i64, optional) - User who will present this item
- `time_allocation_minutes` (integer, optional) - Expected discussion duration
- `created_from_proposal_id` (i64, optional) - Source proposal if promoted
- `created_by_id` (i64) - User who created this agenda point
- `outcome_summary` (text, optional) - Notes on what was decided/discussed
- `decided_by_id` (i64, optional) - User who recorded the decision
- `decision_date` (YYYY-MM-DD, optional) - When decision was recorded
- `selected_coa_id` (i64, optional) - If one COA was chosen outright

Entity metadata:
- `entity_type` = `agenda_point`
- `name` = auto-generated (e.g., "agenda_2026_02_14_001")
- `label` = title

### `coa` — Course of Action

Properties (EAV):
- `title` (text) - Short name, e.g., "5% Budget Increase"
- `description` (text) - Summary overview
- `coa_type` (enum: `simple` | `complex`)
- `created_by_id` (i64) - Author
- `created_date` (YYYY-MM-DD)
- `coa_order` (integer) - Display order among sibling COAs (COA1, COA2, COA3)

Entity metadata:
- `entity_type` = `coa`
- `name` = auto-generated (e.g., "coa_2026_02_14_001")
- `label` = title

### `coa_section` — Section/Subsection of a Complex COA

Properties (EAV):
- `section_number` (text) - Hierarchical numbering: "1", "1.1", "1.2", "2", "2.1"
- `section_title` (text) - E.g., "Financial Impact"
- `content` (text) - The actual paragraph/body text
- `section_order` (integer) - Sort order within parent

Entity metadata:
- `entity_type` = `coa_section`
- `name` = auto-generated (e.g., "coa_section_1_1")
- `label` = section_number + ": " + section_title

### `opinion` — A Member's Input on an Agenda Point

Properties (EAV):
- `comment` (text, optional) - Synthesis suggestions, concerns
- `recorded_date` (YYYY-MM-DD)

Entity metadata:
- `entity_type` = `opinion`
- `name` = auto-generated (e.g., "opinion_2026_02_14_001")
- `label` = username + " on " + agenda_point label

### `proposal` — Enhanced with Queue Status

New property added to existing proposal entity:
- `ready_for_agenda` (boolean, default false) - Marks approved proposals awaiting scheduling

## Relation Types

### New Relation Types

| Relation Type | Source → Target | Purpose |
|---|---|---|
| `transition_from` | workflow_transition → workflow_status | Source state of a transition |
| `transition_to` | workflow_transition → workflow_status | Target state of a transition |
| `considers_coa` | agenda_point → coa | This agenda point evaluates this COA |
| `originates_from` | coa → proposal | COA was derived from this proposal |
| `has_section` | coa → coa_section | Top-level sections of a COA |
| `has_subsection` | coa_section → coa_section | Nested sub-sections (tree) |
| `agenda_submitted_to` | agenda_point → tor | This agenda point belongs to this ToR |
| `spawns_agenda_point` | proposal → agenda_point | Proposal was promoted to this agenda point |
| `opinion_by` | opinion → user | Who expressed this opinion |
| `opinion_on` | opinion → agenda_point | Which agenda point |
| `prefers_coa` | opinion → coa | Which COA they prefer (if any) |
| `presents` | user → agenda_point | User assigned to present this item |

### New Database Table: `relation_properties`

Follows EAV pattern for relation metadata:

```sql
CREATE TABLE relation_properties (
    relation_id INTEGER NOT NULL REFERENCES relations(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (relation_id, key)
);
```

## Data-Driven Workflow Engine

### Concept

All status transitions for all entity types are defined as `workflow_status` and `workflow_transition` entities. No hardcoded `matches!()` blocks in Rust. The transition validation function queries the ontology to determine valid transitions.

### Transition Validation

```rust
fn validate_and_get_transition(
    conn: &Connection,
    entity_type: &str,
    current_status: &str,
    new_status: &str,
    user_permissions: &Permissions,
    entity_properties: &HashMap<String, String>,
) -> Result<TransitionInfo, AppError> {
    // 1. Find workflow_transition where:
    //    transition_from → workflow_status with status_code = current_status
    //    transition_to   → workflow_status with status_code = new_status
    //    both scoped to entity_type
    // 2. Check required_permission against user_permissions
    // 3. Evaluate condition against entity_properties
    // 4. Return transition info (label, requires_outcome, etc.)
    // 5. If no matching transition found → AppError::PermissionDenied
}
```

### Seed Data: Suggestion Workflow (Migration from Hardcoded)

```
workflow_status "Open"       (scope=suggestion, status_code=open, is_initial=true)
workflow_status "Accepted"   (scope=suggestion, status_code=accepted, is_terminal=true)
workflow_status "Rejected"   (scope=suggestion, status_code=rejected, is_terminal=true)

workflow_transition "Accept"
  transition_from → "Open"
  transition_to   → "Accepted"
  required_permission = "suggestion.review"
  transition_label = "Accept"

workflow_transition "Reject"
  transition_from → "Open"
  transition_to   → "Rejected"
  required_permission = "suggestion.review"
  transition_label = "Reject"
```

### Seed Data: Proposal Workflow (Migration from Hardcoded)

```
workflow_status "Draft"         (scope=proposal, status_code=draft, is_initial=true)
workflow_status "Submitted"     (scope=proposal, status_code=submitted)
workflow_status "Under Review"  (scope=proposal, status_code=under_review)
workflow_status "Approved"      (scope=proposal, status_code=approved, is_terminal=true)
workflow_status "Rejected"      (scope=proposal, status_code=rejected)

workflow_transition "Submit"
  transition_from → "Draft"
  transition_to   → "Submitted"
  required_permission = "proposal.submit"
  transition_label = "Submit"

workflow_transition "Start Review"
  transition_from → "Submitted"
  transition_to   → "Under Review"
  required_permission = "proposal.review"
  transition_label = "Start Review"

workflow_transition "Approve"
  transition_from → "Under Review"
  transition_to   → "Approved"
  required_permission = "proposal.approve"
  transition_label = "Approve"

workflow_transition "Reject"
  transition_from → "Under Review"
  transition_to   → "Rejected"
  required_permission = "proposal.approve"
  transition_label = "Reject"

workflow_transition "Resubmit"
  transition_from → "Rejected"
  transition_to   → "Submitted"
  required_permission = "proposal.submit"
  transition_label = "Resubmit"
```

### Seed Data: Agenda Point Workflow (New)

```
workflow_status "Scheduled"    (scope=agenda_point, status_code=scheduled, is_initial=true)
workflow_status "In Progress"  (scope=agenda_point, status_code=in_progress)
workflow_status "Voted"        (scope=agenda_point, status_code=voted)
workflow_status "Completed"    (scope=agenda_point, status_code=completed, is_terminal=true)

workflow_transition "Start Discussion"
  transition_from → "Scheduled"
  transition_to   → "In Progress"
  required_permission = "agenda.manage"
  transition_label = "Start Discussion"

workflow_transition "Record Opinions"
  transition_from → "In Progress"
  transition_to   → "Voted"
  required_permission = "agenda.manage"
  condition = "item_type=decision"
  transition_label = "Record Opinions"

workflow_transition "Complete (Informative)"
  transition_from → "In Progress"
  transition_to   → "Completed"
  required_permission = "agenda.manage"
  condition = "item_type=informative"
  requires_outcome = true
  transition_label = "Complete"

workflow_transition "Record Decision"
  transition_from → "Voted"
  transition_to   → "Completed"
  required_permission = "agenda.decide"
  requires_outcome = true
  transition_label = "Record Decision"
```

## Proposal Queue Workflow

### "Ready for Agenda" State

After a proposal is approved, it can be flagged for scheduling:
- User clicks "Add to Queue" on approved proposal
- Sets `ready_for_agenda = true` property
- Requires `agenda.queue` permission + ToR membership
- Proposal appears in queue view

### Queue View — `/tor/{id}/workflow/queue`

Shows proposals where `status='approved' AND ready_for_agenda=true` for this ToR.

Table: checkbox | Title | Approved Date | Author | Actions

**Bulk action:** "Create Agenda Points for Selected"
- Opens modal: date picker + starting order + default item_type
- Creates agenda_point entities for all selected proposals
- Creates `spawns_agenda_point` and `agenda_submitted_to` relations
- Sets `ready_for_agenda = false` on source proposals
- Flash: "Created N agenda points for YYYY-MM-DD"

**Individual action:** "Create Agenda Point" on single proposal (same modal flow).

## Agenda Point Creation

### Path 1: From Queued Proposals (Primary Workflow)

Auto-populated: title, description from proposal; `item_type` defaults to `decision`; `created_from_proposal_id` set.

User-specified: `scheduled_date` (required), `scheduled_order` (auto-increment or manual).

Optional: `presenter_id`, `time_allocation_minutes`.

### Path 2: From Scratch (Routine Items)

Route: `GET /tor/{id}/workflow/agenda/new`

Form: Title, Description, Item Type (informative/decision), Scheduled Date, Scheduled Order, Presenter (optional), Time Allocation (optional).

Use cases: Administrative announcements, regular reports, items not needing suggestion→proposal workflow.

## Opinion Recording & Decision Making

### Recording Member Opinions

Who: Users with `agenda.participate` permission + ToR membership.

Process:
1. Member selects preferred COA (radio buttons, optional)
2. Adds comment (synthesis suggestions, concerns)
3. Creates `opinion` entity + relations (`opinion_by`, `opinion_on`, `prefers_coa`)
4. Can change opinion before agenda point reaches `completed`

Display:
- Per-COA preference count: "COA1: 3 prefer | COA2: 5 prefer"
- Expandable: member name + comment
- Labeled as "Member Input" (not "Votes")

### Recording Final Decision

Authority: User with `agenda.decide` permission. ToR can configure `substitute_decider_id` property for absent decision-maker.

Process:
1. Reviews member input (preferences + comments)
2. Records `outcome_summary` on agenda_point
3. Sets `decided_by_id`, `decision_date`
4. Optionally sets `selected_coa_id` if one COA chosen outright
5. Triggers workflow transition to `completed`

Decision types:
- "Approved COA2 as presented"
- "Approved hybrid: COA2 base + paragraph 3 from COA1"
- "Deferred pending revised proposal"
- "Rejected all options"

## Permission Model

### New Permissions (Seed Data)

| Permission Code | Description | Typical Roles |
|---|---|---|
| `agenda.view` | View agenda points in ToR workflow | All members |
| `agenda.create` | Create agenda points (from scratch) | Chairs |
| `agenda.queue` | Mark approved proposals as ready for agenda | Chairs, reviewers |
| `agenda.manage` | Progress agenda point status, switch item_type | Chairs |
| `agenda.participate` | Record opinions on decision items | All members |
| `agenda.decide` | Record final decision (veto/final authority) | Decision-maker, chair |
| `coa.create` | Create courses of action | Chairs, proposal authors |
| `coa.edit` | Edit COA content and sections | Chairs, COA authors |
| `workflow.manage` | Manage workflow definitions (admin-level) | Administrator |

### Authorization

Two-layer: Global permission + ToR membership (unchanged from Phase 2a).

Decision-maker substitute: ToR property `substitute_decider_id` for absent authority.

## UI Components

### Rename: Workflow View — `/tor/{id}/workflow`

Three-tab layout (all functional):
- **Suggestions** tab (unchanged)
- **Proposals** tab (enhanced with "Add to Queue" button on approved items)
- **Agenda Points** tab (new)

### Proposals Tab Enhancement

Approved proposals get "Add to Queue" button (with `agenda.queue` permission). Queued proposals show "Queued" indicator.

### Agenda Points Tab

Table columns: Order (#) | Title | Item Type (badge) | Scheduled Date | Status (badge from workflow_status label) | Actions (data-driven from workflow_transition labels)

Action buttons generated from available `workflow_transition` entities — no hardcoded button labels.

### Agenda Point Detail — `/tor/{id}/workflow/agenda/{id}`

Sections:
1. Header — Title, status badge, item type badge
2. Metadata — Scheduled date, order, presenter, time allocation
3. Linked COAs (decision items) — Expandable list with section trees
4. Member Input (decision items, status=voted) — Opinion summary per COA + comments
5. Decision (if completed) — Outcome summary, decided by, date
6. Actions — Data-driven buttons from available transitions

### COA Management — `/tor/{id}/workflow/agenda/{id}/coa`

- Create COA form: title, description, type (simple/complex)
- Complex COA editor: add/edit/reorder/nest sections
- Link existing COA from proposals via `originates_from`

### Opinion Recording — `/tor/{id}/workflow/agenda/{id}/input`

- Shows linked COAs as radio options
- Comment textarea
- Submit creates opinion entity + relations
- "Change Input" if already recorded
- Summary table: COA preferences + expandable comments

### Queue View — `/tor/{id}/workflow/queue`

- Checkbox + table of approved/queued proposals
- Bulk "Create Agenda Points" with scheduling modal

## Audit Logging

### New Auditable Events

| Event | Action Code | Target Type | Important? |
|---|---|---|---|
| Queue proposal | `proposal.queued` | proposal | No |
| Remove from queue | `proposal.unqueued` | proposal | No |
| Create agenda point (from queue) | `agenda.created_from_proposal` | agenda_point | No |
| Create agenda point (from scratch) | `agenda.created` | agenda_point | No |
| Status transition | `agenda.status_changed` | agenda_point | No |
| Item type switched | `agenda.type_switched` | agenda_point | No |
| COA created | `coa.created` | coa | No |
| COA section added | `coa.section_added` | coa_section | No |
| Opinion recorded | `agenda.opinion_recorded` | opinion | No |
| Opinion changed | `agenda.opinion_changed` | opinion | No |
| **Decision recorded** | `agenda.decision_recorded` | agenda_point | **Yes** |
| **Workflow definition changed** | `workflow.transition_modified` | workflow_transition | **Yes** |

Important events stored in DB. All others filesystem only (existing pattern).

## Implementation Scope Summary

1. **Workflow engine** — `workflow_status` + `workflow_transition` entities, generic validation function
2. **Migrate existing workflows** — Suggestion + Proposal transitions to data-driven
3. **Rename pipeline → workflow** — URLs, nav items, templates, handlers
4. **`relation_properties` table** — EAV for relation metadata
5. **Agenda point entity** — Full CRUD + lifecycle
6. **COA entity** — Simple + complex with nested sections
7. **Opinion entity** — Record/change member input
8. **Proposal queue** — Ready-for-agenda flag, queue view, bulk scheduling
9. **Decision recording** — Authority-based final decision
10. **9 new permissions** — Seeded and assigned to Administrator
11. **Audit logging** — All new events captured
12. **UI** — Agenda tab, queue view, COA editor, opinion recording, data-driven action buttons

## Phase 3 Preview

Phase 3 will add:
- `meeting` entity type with date, time, location, attendees
- `scheduled_in` relation (agenda_point → meeting)
- Calendar views (day, week, month)
- Meeting minutes generation from completed agenda points
- Attendance tracking

Phase 2b establishes the agenda/workflow foundation. Phase 3 connects agenda points to formal meetings.
