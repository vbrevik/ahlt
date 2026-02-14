# Phase 2a: Item Pipeline — Suggestions + Proposals

**Date:** 2026-02-14
**Status:** Approved
**Phase:** Phase 2a (Item Pipeline - Part 1 of 2)
**Dependencies:** Phase 1 (ToR Foundation)
**Related:** Phase 2b will add Agenda Points + full pipeline integration

## Overview

Implements the first two stages of the governance item pipeline: **Suggestions** and **Proposals**. Users submit suggestions to a ToR, which are reviewed and either accepted (auto-converting to draft proposals) or rejected. Proposals progress through a multi-stage review workflow (draft → submitted → under review → approved/rejected).

**Key capabilities:**
- Submit suggestions to any ToR where user is a member
- Review and accept/reject suggestions (creates draft proposals automatically)
- Create, edit, and submit proposals (manual or auto-created)
- Multi-stage proposal review workflow with approval/rejection
- Configurable cross-ToR transparency (read-only proposal visibility)
- Pipeline view with tabbed interface (Suggestions | Proposals | Agenda Points*)
- Granular permissions for each action type
- Comprehensive audit logging

*Agenda Points tab present but disabled until Phase 2b/3

## Design Decisions

### UI Approach
**Chosen:** Tabbed lists (Suggestions tab, Proposals tab, Agenda Points placeholder)
**Rationale:** Aligns with server-rendered Askama template architecture, simpler than kanban, better for displaying detailed metadata in table format.

### Permission Model
**Chosen:** Granular global permissions (`suggestion.create`, `proposal.approve`, etc.)
**Rationale:** Follows existing permission pattern in the app, easier to manage in Menu Builder, clear separation of capabilities.

### Cross-ToR Visibility
**Chosen:** Configurable transparency via `allow_public_proposals` ToR property
**Rationale:** Supports the test scenario requirement ("Departments can view each other's proposals during working phase"), provides flexibility per ToR.

### Suggestion → Proposal Flow
**Chosen:** Auto-create draft proposal when suggestion accepted
**Rationale:** Maintains pipeline traceability automatically, preserves attribution, gives users a chance to refine before submitting.

### Phasing Strategy
**Chosen:** Incremental — Suggestions + Proposals first (Phase 2a), Agenda Points later (Phase 2b)
**Rationale:** Faster to first value, easier to test, natural dependency (agenda points need meetings from Phase 3).

## Entity Types

### `suggestion` - Initial Ideas Submitted for Consideration

**Properties (stored as EAV in `entity_properties`):**
- `description` (text) - What is being suggested
- `submitted_date` (YYYY-MM-DD, ISO-8601) - When submitted
- `status` (enum: `open` | `accepted` | `rejected`)
- `submitted_by_id` (i64) - User who submitted (for audit trail)
- `rejection_reason` (text, optional) - Why rejected (required if status = rejected)

**Entity metadata:**
- `entity_type` = `suggestion`
- `name` = auto-generated (e.g., "suggestion_2026_02_14_001")
- `label` = first 50 chars of description + "..." if longer

### `proposal` - Formalized Suggestion with Detail

**Properties (stored as EAV in `entity_properties`):**
- `title` (text) - Short summary/subject line
- `description` (text) - Detailed proposal text
- `rationale` (text) - Justification/reasoning
- `submitted_date` (YYYY-MM-DD, ISO-8601) - When first submitted
- `status` (enum: `draft` | `submitted` | `under_review` | `approved` | `rejected`)
- `submitted_by_id` (i64) - User who submitted
- `rejection_reason` (text, optional) - Why rejected (required if status = rejected)
- `related_suggestion_id` (i64, optional) - For manual linking if not auto-created

**Entity metadata:**
- `entity_type` = `proposal`
- `name` = sanitized version of title (lowercase, underscores)
- `label` = title

## Relation Types

| Relation Type | Source → Target | Purpose |
|---|---|---|
| `suggested_to` | suggestion → tor | This suggestion was submitted to this ToR |
| `spawns_proposal` | suggestion → proposal | This suggestion was converted into this proposal (auto-created on accept) |
| `submitted_to` | proposal → tor | This proposal belongs to this ToR |

**Usage Patterns:**

**Suggestion creation:**
```
user creates suggestion
→ suggestion entity created with status='open'
→ suggestion --suggested_to--> tor
→ submitted_by_id property set to current user
```

**Suggestion acceptance (auto-creates proposal):**
```
user accepts suggestion (has suggestion.review permission + ToR membership)
→ suggestion.status = 'accepted'
→ system creates new proposal entity (status='draft')
→ proposal inherits description from suggestion
→ proposal.submitted_by_id = suggestion.submitted_by_id (preserve attribution)
→ suggestion --spawns_proposal--> proposal
→ proposal --submitted_to--> same tor as suggestion
→ audit log: suggestion.accepted + proposal.auto_created
```

**Cross-ToR transparency query:**
```
Find proposals visible to user:
1. Find ToRs where user has member_of relation (direct access)
2. Find proposals with submitted_to relation to those ToRs
3. UNION: Find proposals in OTHER ToRs where tor.allow_public_proposals='true' (read-only)
```

## Status Transitions & Lifecycle

### Suggestion Lifecycle

```
[Created] → open
           ↓
    ┌──────┴──────┐
    ↓             ↓
accepted      rejected
    ↓
spawns_proposal
(auto-creates draft proposal)
```

**Valid transitions:**
- `open` → `accepted` (requires `suggestion.review` permission + ToR membership)
- `open` → `rejected` (requires `suggestion.review` permission + ToR membership + rejection_reason)
- No transitions from `accepted` or `rejected` (terminal states)

**Business rules:**
1. Only users with `suggestion.review` permission can accept/reject
2. User must be a member of the target ToR (has `member_of` relation)
3. Rejection requires non-empty `rejection_reason`
4. Accepting triggers auto-creation of draft proposal
5. Once accepted/rejected, suggestion is read-only

### Proposal Lifecycle

```
[Created] → draft
           ↓
       submitted
           ↓
      under_review
           ↓
    ┌──────┴──────┐
    ↓             ↓
approved      rejected
    ↓             ↓
(terminal)    (can resubmit)
              ↓
          submitted (again)
```

**Valid transitions:**
- `draft` → `submitted` (by creator OR anyone with `proposal.submit` permission in the ToR)
- `submitted` → `under_review` (requires `proposal.review` permission + ToR membership)
- `under_review` → `approved` (requires `proposal.approve` permission + ToR membership)
- `under_review` → `rejected` (requires `proposal.approve` permission + ToR membership + rejection_reason)
- `rejected` → `submitted` (resubmission after rework, requires `proposal.submit` permission)

**Business rules:**
1. Draft proposals can be edited by creator OR anyone with `proposal.edit` permission
2. Once submitted, proposals are read-only except for status transitions
3. Rejection requires non-empty `rejection_reason`
4. Resubmission clears `rejection_reason` and moves back to `submitted` status
5. Auto-created proposals start in `draft` status
6. Manual proposals also start in `draft` status

## Permission Model

### New Permissions (added to seed data)

**Permission Group:** `Pipeline`

| Permission Code | Description | Typical Roles |
|---|---|---|
| `suggestion.view` | View suggestions in ToRs where user is a member | All members |
| `suggestion.create` | Submit new suggestions to a ToR | All members |
| `suggestion.review` | Accept or reject suggestions | Chairs, reviewers |
| `proposal.view` | View proposals in ToRs where user is a member | All members |
| `proposal.create` | Create new proposals (manual or edit auto-created drafts) | All members |
| `proposal.submit` | Submit draft proposals for review | All members |
| `proposal.edit` | Edit draft proposals (own or others') | Editors, chairs |
| `proposal.review` | Move proposals to under_review status | Reviewers |
| `proposal.approve` | Approve or reject proposals under review | Chairs, approvers |

### Authorization Logic

**Two-layer checks:**

1. **Global permission check** - User's role must grant the permission
2. **ToR membership check** - User must have `member_of` relation to the target ToR

```rust
// Example: Accept a suggestion
pub async fn accept_suggestion(
    pool: web::Data<DbPool>,
    session: Session,
    tor_id: web::Path<i64>,
    suggestion_id: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    // Layer 1: Global permission
    require_permission(&session, "suggestion.review")?;

    // Layer 2: ToR membership
    let user_id = get_user_id(&session)?;
    let conn = pool.get()?;
    require_tor_membership(&conn, user_id, *tor_id)?;

    // ... proceed with acceptance logic
}
```

### Cross-ToR Transparency Exception

**Special case for viewing proposals:**
- Users can VIEW proposals from ToRs where `allow_public_proposals=true` even without membership
- But cannot create/edit/submit in those ToRs (membership still required for all actions)
- Read-only access only (no status transitions, no edits)

```rust
fn can_view_proposal(conn: &Connection, user_id: i64, proposal_id: i64) -> Result<bool, AppError> {
    // 1. Get proposal's ToR
    let tor_id = get_proposal_tor(conn, proposal_id)?;

    // 2. Check membership (always allows full access)
    if has_tor_membership(conn, user_id, tor_id)? {
        return Ok(true);
    }

    // 3. Check transparency flag (read-only access)
    let allow_public = get_tor_property(conn, tor_id, "allow_public_proposals")?
        .unwrap_or("false".to_string());

    Ok(allow_public == "true")
}
```

## UI Components

### Pipeline View - `/tor/{id}/pipeline`

**Three-tab layout:**
- **Suggestions** tab (active)
- **Proposals** tab
- **Agenda Points** tab (placeholder, shows "Coming in Phase 3" message)

**Suggestions Tab:**

Table with columns:
- Description (truncated to 100 chars with "..." if longer)
- Submitted By (user label)
- Date (YYYY-MM-DD)
- Status (badge: green for accepted, red for rejected, blue for open)
- Actions (based on status + permissions)

**Actions:**
- Open suggestion:
  - **Accept** button (if has `suggestion.review` permission)
  - **Reject** button (if has `suggestion.review` permission)
  - **View** link (always)
- Accepted suggestion:
  - **View Proposal** link (to auto-created proposal)
  - **View** link
- Rejected suggestion:
  - **View** link (shows rejection reason)

**Proposals Tab:**

Table with columns:
- Title
- Submitted By (user label)
- Date (YYYY-MM-DD)
- Status (badge with color-coding)
- Actions (based on status + permissions)

**Actions:**
- Draft proposal:
  - **Edit** button (if creator OR has `proposal.edit`)
  - **Submit** button (if has `proposal.submit`)
  - **Delete** button (if creator)
- Submitted proposal:
  - **Move to Review** button (if has `proposal.review`)
  - **View** link
- Under Review proposal:
  - **Approve** button (if has `proposal.approve`)
  - **Reject** button (if has `proposal.approve`)
  - **View** link
- Approved proposal:
  - **View** link
  - Badge: "Awaiting agenda scheduling" (Phase 3)
- Rejected proposal:
  - **View Reason** link
  - **Edit & Resubmit** button (if has `proposal.submit`)

**Filtering Controls (per tab):**
- Status dropdown (All | status options)
- Submitted by filter (All | My Items | specific user)
- Sort by: Date (newest/oldest)

### CRUD Forms

**Create Suggestion - `/tor/{id}/suggestions/new`**

Fields:
- **Description** (textarea, required, max 2000 chars)
- Hidden: `csrf_token`

Auto-set on creation:
- `submitted_by_id` = current user
- `submitted_date` = today (ISO-8601)
- `status` = 'open'
- Creates `suggested_to` relation to ToR

Submit button: "Submit Suggestion"

**Create Proposal (manual) - `/tor/{id}/proposals/new`**

Fields:
- **Title** (text input, required, max 200 chars)
- **Description** (textarea, required, max 5000 chars)
- **Rationale** (textarea, required, max 2000 chars)
- **Link to Suggestion** (dropdown, optional, shows accepted suggestions)
- Hidden: `csrf_token`

Auto-set on creation:
- `submitted_by_id` = current user
- `submitted_date` = today (ISO-8601)
- `status` = 'draft'
- Creates `submitted_to` relation to ToR
- If linked to suggestion, creates `spawns_proposal` relation

Submit button: "Create Draft Proposal"

**Edit Proposal - `/tor/{id}/proposals/{proposal_id}/edit`**

Same fields as Create Proposal form.

Constraints:
- Only editable if `status = 'draft'`
- Permission check: must be creator OR have `proposal.edit` permission
- If user is not creator, show warning: "You are editing another user's proposal"

Submit button: "Save Changes"

**Accept/Reject Modals:**

**Reject Suggestion Modal:**
- **Rejection Reason** (textarea, required, max 1000 chars)
- Cancel / Reject buttons

**Reject Proposal Modal:**
- **Rejection Reason** (textarea, required, max 1000 chars)
- Cancel / Reject buttons

**Accept Confirmation:**
- Simple dialog: "Accept this suggestion? A draft proposal will be created automatically."
- Cancel / Accept buttons

## Validation Rules

### ToR Membership Validation

Every pipeline action (except cross-ToR read-only viewing) requires ToR membership:

```rust
fn require_tor_membership(conn: &Connection, user_id: i64, tor_id: i64) -> Result<(), AppError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM relations r
         JOIN entities rt ON r.relation_type_id = rt.id
         WHERE rt.name = 'member_of'
           AND r.source_id = ?1
           AND r.target_id = ?2",
        params![user_id, tor_id],
        |row| row.get(0)
    )?;

    if count == 0 {
        return Err(AppError::PermissionDenied("Not a member of this ToR".into()));
    }
    Ok(())
}
```

### Status Transition Validation

All status changes must follow valid transition paths:

```rust
fn validate_suggestion_transition(current: &str, new: &str) -> Result<(), AppError> {
    let valid = matches!(
        (current, new),
        ("open", "accepted") | ("open", "rejected")
    );

    if !valid {
        return Err(AppError::ValidationError(
            format!("Invalid suggestion status transition: {} -> {}", current, new)
        ));
    }
    Ok(())
}

fn validate_proposal_transition(current: &str, new: &str) -> Result<(), AppError> {
    let valid = matches!(
        (current, new),
        ("draft", "submitted") |
        ("submitted", "under_review") |
        ("under_review", "approved") |
        ("under_review", "rejected") |
        ("rejected", "submitted")  // resubmission
    );

    if !valid {
        return Err(AppError::ValidationError(
            format!("Invalid proposal status transition: {} -> {}", current, new)
        ));
    }
    Ok(())
}
```

### Rejection Reason Requirement

When rejecting, `rejection_reason` must be non-empty:

```rust
if new_status == "rejected" && rejection_reason.trim().is_empty() {
    return Err(AppError::ValidationError(
        "Rejection reason is required when rejecting".into()
    ));
}
```

### Auto-Create Proposal Validation

When accepting a suggestion, the system must:

1. Verify user has `suggestion.review` permission
2. Verify user is member of target ToR
3. Verify suggestion status is currently 'open'
4. Create proposal entity with `status='draft'`
5. Copy `description` from suggestion to proposal
6. Set `submitted_by_id` to suggestion's submitter (preserve attribution)
7. Create `spawns_proposal` relation (suggestion → proposal)
8. Create `submitted_to` relation (proposal → tor)
9. Set suggestion status to 'accepted'
10. Log audit events (suggestion.accepted + proposal.auto_created)

All steps must succeed or transaction is rolled back.

## Audit Logging

### Auditable Events

| Event | Action Code | Target Type | Details Captured | Important? |
|---|---|---|---|---|
| Create suggestion | `suggestion.created` | suggestion | `{tor_id, tor_name, description_preview, submitted_by}` | No |
| Accept suggestion | `suggestion.accepted` | suggestion | `{tor_id, spawned_proposal_id, spawned_proposal_title}` | Yes |
| Reject suggestion | `suggestion.rejected` | suggestion | `{tor_id, rejection_reason}` | No |
| Create proposal (manual) | `proposal.created` | proposal | `{tor_id, tor_name, title, submitted_by}` | No |
| Create proposal (auto) | `proposal.auto_created` | proposal | `{tor_id, from_suggestion_id, title}` | No |
| Submit proposal | `proposal.submitted` | proposal | `{tor_id, title}` | No |
| Move to review | `proposal.review_started` | proposal | `{tor_id, title, reviewer}` | No |
| Approve proposal | `proposal.approved` | proposal | `{tor_id, title, approved_by}` | Yes |
| Reject proposal | `proposal.rejected` | proposal | `{tor_id, title, rejection_reason, rejected_by}` | Yes |
| Resubmit proposal | `proposal.resubmitted` | proposal | `{tor_id, title, previous_rejection_reason}` | No |
| Edit proposal | `proposal.edited` | proposal | `{tor_id, title, editor, is_own: bool}` | No |
| Delete proposal | `proposal.deleted` | proposal | `{tor_id, title, status_at_deletion}` | No |

### Logging Pattern

Using existing `audit::log()` helper from Phase 1:

```rust
let details = serde_json::json!({
    "tor_id": tor_id,
    "tor_name": tor_name,
    "title": proposal.title,
    "status_change": format!("{} → {}", old_status, new_status)
});

audit::log(
    &conn,
    current_user_id,
    "proposal.submitted",
    "proposal",
    proposal_id,
    details
)?;
```

### Important Events (Database + Filesystem)

Following the existing `audit::is_important()` pattern, these events are stored in the database:
- `suggestion.accepted` - Spawns proposals, important workflow milestone
- `proposal.approved` - Major decision point
- `proposal.rejected` - Major decision point

All other events: Filesystem only (following existing retention policy from Phase 1).

## Database Queries

### Key Queries

**Find all suggestions for a ToR:**
```sql
SELECT e.id, e.name, e.label,
       ep_desc.value as description,
       ep_date.value as submitted_date,
       ep_status.value as status,
       ep_by.value as submitted_by_id,
       u.label as submitted_by_name
FROM entities e
JOIN relations r ON e.id = r.source_id
JOIN entities rt ON r.relation_type_id = rt.id AND rt.name = 'suggested_to'
LEFT JOIN entity_properties ep_desc ON e.id = ep_desc.entity_id AND ep_desc.key = 'description'
LEFT JOIN entity_properties ep_date ON e.id = ep_date.entity_id AND ep_date.key = 'submitted_date'
LEFT JOIN entity_properties ep_status ON e.id = ep_status.entity_id AND ep_status.key = 'status'
LEFT JOIN entity_properties ep_by ON e.id = ep_by.entity_id AND ep_by.key = 'submitted_by_id'
LEFT JOIN entities u ON CAST(ep_by.value AS INTEGER) = u.id
WHERE e.entity_type = 'suggestion'
  AND r.target_id = ?1  -- tor_id
ORDER BY ep_date.value DESC
```

**Find proposals visible to user (with transparency):**
```sql
-- Member ToRs
SELECT e.id, e.name, e.label,
       ep_title.value as title,
       -- ... other properties
       1 as is_member
FROM entities e
JOIN relations r ON e.id = r.source_id
JOIN entities rt ON r.relation_type_id = rt.id AND rt.name = 'submitted_to'
WHERE e.entity_type = 'proposal'
  AND r.target_id IN (
    SELECT target_id FROM relations r2
    JOIN entities rt2 ON r2.relation_type_id = rt2.id AND rt2.name = 'member_of'
    WHERE r2.source_id = ?1  -- user_id
  )

UNION

-- Public ToRs (transparency)
SELECT e.id, e.name, e.label,
       ep_title.value as title,
       -- ... other properties
       0 as is_member
FROM entities e
JOIN relations r ON e.id = r.source_id
JOIN entities rt ON r.relation_type_id = rt.id AND rt.name = 'submitted_to'
JOIN entity_properties ep_pub ON r.target_id = ep_pub.entity_id
  AND ep_pub.key = 'allow_public_proposals'
  AND ep_pub.value = 'true'
WHERE e.entity_type = 'proposal'
  AND r.target_id NOT IN (
    -- Exclude ToRs where already a member (avoid duplicates)
    SELECT target_id FROM relations r2
    JOIN entities rt2 ON r2.relation_type_id = rt2.id AND rt2.name = 'member_of'
    WHERE r2.source_id = ?1
  )
```

## Implementation Checklist

**Database & Seed:**
- [ ] Add 3 new relation types to seed data (`suggested_to`, `spawns_proposal`, `submitted_to`)
- [ ] Add 9 new permissions to seed data (Pipeline group)
- [ ] Add `allow_public_proposals` property to existing ToRs (default: false)
- [ ] Create `require_tor_membership()` helper in `src/models/tor/queries.rs`
- [ ] Add nav item: `governance.pipeline` → "Item Pipeline" → `/tor/{id}/pipeline` (permission: `suggestion.view`)

**Models:**
- [ ] Create `src/models/suggestion/` module (types.rs, queries.rs, mod.rs)
  - [ ] SuggestionListItem, SuggestionDetail types
  - [ ] find_all_for_tor(), find_by_id(), create(), update_status(), delete()
- [ ] Create `src/models/proposal/` module (types.rs, queries.rs, mod.rs)
  - [ ] ProposalListItem, ProposalDetail types
  - [ ] find_all_for_tor(), find_visible_to_user(), find_by_id(), create(), update(), update_status(), delete()
  - [ ] auto_create_from_suggestion() - special helper

**Templates:**
- [ ] Create `templates/pipeline/view.html` - three-tab layout
- [ ] Create `templates/pipeline/suggestions_tab.html` - suggestions table
- [ ] Create `templates/pipeline/proposals_tab.html` - proposals table
- [ ] Create `templates/pipeline/agenda_placeholder.html` - "Coming in Phase 3"
- [ ] Create `templates/suggestions/form.html` - create suggestion form
- [ ] Create `templates/proposals/form.html` - create/edit proposal form
- [ ] Create `templates/proposals/detail.html` - proposal detail view
- [ ] Update `templates/tor/detail.html` - add "View Pipeline" button

**Handlers:**
- [ ] Create `src/handlers/pipeline_handlers/` module
  - [ ] view.rs - pipeline view handler (GET /tor/{id}/pipeline)
- [ ] Create `src/handlers/suggestion_handlers/` module
  - [ ] list.rs, crud.rs - CRUD handlers
  - [ ] accept.rs, reject.rs - status transition handlers
- [ ] Create `src/handlers/proposal_handlers/` module
  - [ ] list.rs, crud.rs - CRUD handlers
  - [ ] submit.rs, review.rs, approve.rs, reject.rs - status transition handlers

**Routes (src/main.rs):**
- [ ] GET `/tor/{id}/pipeline` → pipeline view
- [ ] GET `/tor/{id}/suggestions/new` → create suggestion form
- [ ] POST `/tor/{id}/suggestions` → create suggestion
- [ ] POST `/tor/{id}/suggestions/{suggestion_id}/accept` → accept suggestion
- [ ] POST `/tor/{id}/suggestions/{suggestion_id}/reject` → reject suggestion
- [ ] GET `/tor/{id}/proposals/new` → create proposal form
- [ ] POST `/tor/{id}/proposals` → create proposal
- [ ] GET `/tor/{id}/proposals/{proposal_id}/edit` → edit proposal form
- [ ] POST `/tor/{id}/proposals/{proposal_id}` → update proposal
- [ ] POST `/tor/{id}/proposals/{proposal_id}/submit` → submit proposal
- [ ] POST `/tor/{id}/proposals/{proposal_id}/review` → move to review
- [ ] POST `/tor/{id}/proposals/{proposal_id}/approve` → approve proposal
- [ ] POST `/tor/{id}/proposals/{proposal_id}/reject` → reject proposal
- [ ] POST `/tor/{id}/proposals/{proposal_id}/delete` → delete proposal

**Testing:**
- [ ] Manual test: Create suggestion
- [ ] Manual test: Accept suggestion (verify auto-created proposal)
- [ ] Manual test: Reject suggestion
- [ ] Manual test: Create manual proposal
- [ ] Manual test: Submit proposal
- [ ] Manual test: Approve proposal
- [ ] Manual test: Reject proposal + resubmit
- [ ] Manual test: Cross-ToR transparency (enable `allow_public_proposals`)
- [ ] Manual test: Permission gating (test each permission)
- [ ] Verify audit logs captured correctly

## Phase 2b Preview

**What comes next:**
- Entity type: `agenda_point`
- Relation types: `spawns_agenda_point`, `scheduled_in`
- Proposal → Agenda Point conversion (approved proposals)
- Third tab in pipeline view becomes functional
- Integration with Phase 3 (Meetings)

Phase 2a establishes the foundation. Phase 2b/3 will connect proposals to meetings.

## Notes

- All dates use ISO-8601 format (YYYY-MM-DD) for correct string sorting in SQLite
- Follow Phase 1 patterns: AppError, render() helper, PageContext, session helpers
- ToR membership is foundational - every action checks it (except cross-ToR read-only viewing)
- Auto-creation workflow preserves attribution (original submitter's ID carries through)
- Rejection requires reasoning (improves governance transparency)
- Audit logging follows existing two-tier pattern (important events in DB, all events in filesystem)
