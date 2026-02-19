# Meeting Lifecycle — Quick Start Guide

## Overview

Meetings are first-class governance entities with a complete lifecycle: projected (calendar-based) → confirmed (user action) → in_progress (active) → completed (finished) / cancelled (aborted).

**Key fact:** Meetings are EAV entities with workflow-driven status, not virtual calendar events.

## Database Model

### Entity Type
- **entity_type:** `meeting`
- **name:** `{tor-name}-{YYYY-MM-DD}` (unique, kebab-case)
- **label:** `{ToR Label} — {YYYY-MM-DD}` (display)

### Properties
- `meeting_date` — ISO-8601 string (e.g., "2026-03-15")
- `status` — workflow-driven ("projected", "confirmed", "in_progress", "completed", "cancelled")
- `location` — optional string
- `notes` — optional string

### Relations
- `belongs_to_tor` — meeting → tor (scope)
- `scheduled_for_meeting` — agenda_point → meeting (agenda assignment)
- `minutes_of` — meeting → minutes (when completed)

## Routes & Permissions

| Route | Method | Permission | Purpose |
|-------|--------|-----------|---------|
| `/meetings` | GET | `meetings.view` | Cross-ToR list (upcoming + past) |
| `/tor/{id}/meetings/{mid}` | GET | `meetings.view` | Meeting detail page |
| `/tor/{id}/meetings/confirm` | POST | `tor.edit` | Confirm projected meeting |
| `/tor/{id}/meetings/{mid}/transition` | POST | `tor.edit` | Workflow transition |
| `/tor/{id}/meetings/{mid}/agenda/assign` | POST | `tor.edit` | Assign agenda point |
| `/tor/{id}/meetings/{mid}/agenda/remove` | POST | `tor.edit` | Unassign agenda point |
| `/tor/{id}/meetings/{mid}/minutes/generate` | POST | `minutes.generate` | Generate scaffold |

## Code Examples

### Create a Meeting (Confirm Form)
```html
<form method="post" action="/tor/{{ tor.id }}/meetings/confirm">
    <input type="hidden" name="csrf_token" value="{{ ctx.csrf_token }}">
    <input type="hidden" name="tor_name" value="{{ tor.label }}">
    <input type="date" name="meeting_date" required>
    <input type="text" name="location" placeholder="Location (optional)">
    <textarea name="notes" placeholder="Notes (optional)"></textarea>
    <button type="submit">Confirm Meeting</button>
</form>
```

The handler:
```rust
meeting::create(&conn, tor_id, &form.meeting_date, &form.tor_name, location, notes)?;
meeting::update_status(&conn, meeting_id, "confirmed")?; // transition immediately
```

### Transition a Meeting (Workflow)
```html
<form method="post" action="/tor/{{ tor.id }}/meetings/{{ meeting.id }}/transition">
    <input type="hidden" name="csrf_token" value="{{ ctx.csrf_token }}">
    <input type="hidden" name="new_status" value="in_progress">
    <button type="submit">Start Meeting</button>
</form>
```

The handler:
```rust
workflow::validate_transition(&conn, "meeting", &meeting.status, &form.new_status, &permissions, &HashMap::new())?;
meeting::update_status(&conn, mid, &form.new_status)?;
```

### Generate Minutes
```html
{% if meeting.status.as_str() == "completed" %}
  {% if minutes.is_none() %}
    <form method="post" action="/tor/{{ tor_id }}/meetings/{{ meeting.id }}/minutes/generate">
        <input type="hidden" name="csrf_token" value="{{ ctx.csrf_token }}">
        <button type="submit">Generate Minutes</button>
    </form>
  {% else %}
    <a href="/minutes/{{ minutes.unwrap().id }}">View Minutes</a>
  {% endif %}
{% endif %}
```

The handler:
```rust
if meeting_detail.status != "completed" { return Err(PermissionDenied); }
if minutes::find_by_meeting(&conn, mid)?.is_some() { return Err(PermissionDenied); }
let minutes_id = minutes::generate_scaffold(&conn, mid, tor_id, &meeting_detail.label)?;
```

## Testing

Run integration tests:
```bash
cargo test --test meeting_test
```

Test the full flow manually:
```bash
rm -f data/staging/app.db
APP_ENV=staging cargo run
# Then:
# 1. Login as admin / admin123
# 2. Navigate to a ToR
# 3. Scroll to "Meetings" section
# 4. Click "Confirm Meeting", pick a date
# 5. Click the meeting link to see detail page
# 6. Assign agenda points via dropdown
# 7. Transition to in_progress, then completed
# 8. Click "Generate Minutes"
# 9. View the generated minutes scaffold at /minutes/{id}
```

## Templates

### List Page
**File:** `templates/meetings/list.html`
**Data:** `MeetingsListTemplate { ctx, upcoming: Vec<MeetingListItem>, past: Vec<MeetingListItem> }`

Shows two sections:
- **Upcoming:** meetings where `status != "completed" && status != "cancelled"`
- **Past:** meetings where `status == "completed" || status == "cancelled"`

Each row: date, ToR name, status badge, agenda count, minutes status

### Detail Page
**File:** `templates/meetings/detail.html`
**Data:** `MeetingDetailTemplate { ctx, meeting, agenda_points, unassigned_points, protocol_steps, transitions, minutes, tor_id }`

Shows four sections:
1. **Header:** Date, ToR, status, transition buttons
2. **Agenda:** Assigned points with remove buttons, dropdown to assign unassigned points
3. **Protocol:** ToR protocol steps (read-only, referenced)
4. **Minutes:** Generate button (if completed + no minutes) or view link (if exists)

### ToR Detail Integration
**File:** `templates/tor/detail.html`
**Data:** Added `meetings: Vec<MeetingListItem>` to `TorDetailTemplate`

Section after Dependencies showing:
- **Upcoming:** confirmed/in_progress meetings with links
- **Past:** completed/cancelled meetings with minutes status
- **Confirm form:** Create new meeting by date

## Model Functions

**Query functions** in `src/models/meeting/queries.rs`:

```rust
pub fn create(conn, tor_id, meeting_date, tor_name, location, notes) -> rusqlite::Result<i64>
pub fn find_by_id(conn, id) -> rusqlite::Result<Option<MeetingDetail>>
pub fn find_by_tor(conn, tor_id) -> rusqlite::Result<Vec<MeetingListItem>>
pub fn find_upcoming_all(conn, from_date) -> rusqlite::Result<Vec<MeetingListItem>>
pub fn find_past_all(conn, before_date) -> rusqlite::Result<Vec<MeetingListItem>>
pub fn assign_agenda(conn, meeting_id, agenda_point_id) -> rusqlite::Result<()>
pub fn remove_agenda(conn, meeting_id, agenda_point_id) -> rusqlite::Result<()>
pub fn find_agenda_points(conn, meeting_id) -> rusqlite::Result<Vec<MeetingAgendaPoint>>
pub fn find_unassigned_agenda_points(conn, tor_id) -> rusqlite::Result<Vec<MeetingAgendaPoint>>
pub fn update_status(conn, meeting_id, status) -> rusqlite::Result<()>
```

## Troubleshooting

**Q: I see "minutes can only be generated for completed meetings"**
A: Transition the meeting to "completed" status first via the transition form.

**Q: Agenda points aren't showing up**
A: Make sure they belong to the same ToR and aren't already assigned to other meetings.

**Q: The meeting detail page 404s**
A: Check that you're using the correct URL pattern: `/tor/{tor_id}/meetings/{meeting_id}` (not just `/meetings/{id}`).

**Q: Stale template errors after code changes**
A: Run `cargo clean` then `cargo build` to clear Askama's template cache.

## Seed Data

All seed data is in `data/seed/ontology.json`:
- Relation types: `scheduled_for_meeting`
- Permissions: `meetings.view`
- Nav items: `governance.meetings`
- Workflow statuses: `meeting.projected`, `meeting.confirmed`, `meeting.in_progress`, `meeting.completed`, `meeting.cancelled`
- Transitions: All 5 status-to-status transitions with `tor.edit` permission requirement

To update seed data: Edit JSON, delete `data/{APP_ENV}/app.db`, restart.

## Commits

Implementation commits in order:
```
c35087e feat(seed): add meeting relation type, permission, and nav item
8275a5b feat(seed): add meeting workflow statuses and transitions
09fd9ae feat(model): add meeting types and module skeleton
630e660 feat(model): implement meeting create + find_by_id with tests (TDD)
08464ab feat(model): agenda assignment + update_status queries with 5 tests
55b3710 feat(handlers): meeting handler module skeleton with routes
af7072b feat(ui): meeting list page with upcoming/past sections
2e61414 feat(ui): meeting detail page with agenda, protocol, and minutes
ffab747 feat(handlers): meeting confirm, transition, agenda, and minutes handlers
7b8aa6e feat(ui): add meetings section to ToR detail page
```

## Design Document

See `docs/plans/2026-02-19-meeting-lifecycle-design.md` for:
- Full data model
- Rationale for design decisions
- Out-of-scope items
- Complete route list
