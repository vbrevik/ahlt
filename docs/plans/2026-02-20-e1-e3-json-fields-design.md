# E.1/E.3 JSON Fields — Design

**Date:** 2026-02-20
**Status:** Approved

## Overview

Complete the partially-implemented E.1 (Meeting) and E.3 (Minutes) metadata by adding four structured JSON fields. These are low-priority additions that extend the existing EAV properties on meetings and minutes entities.

## Fields

| Field | Entity | JSON Shape | Status Enum |
|---|---|---|---|
| `roll_call_data` | `meeting` | `[{user_id, username, status}]` | present / absent / excused |
| `distribution_list` | `minutes` | `["email@org"]` | — (simple string list) |
| `structured_attendance` | `minutes` | `[{user_id, name, status, delegation_to}]` | present / absent / excused |
| `structured_action_items` | `minutes` | `[{description, responsible, due_date, status}]` | open / in_progress / done |

**Why `username` in `roll_call_data`:** Human-readable without a join, consistent with `structured_attendance` which also stores both `user_id` and `name`.

**Why both `roll_call_data` and `structured_attendance`:** They serve different purposes at different stages. Roll call is captured on the meeting detail page as a live record of who attended. Structured attendance is part of the minutes formal record, may include delegation notes, and is authoritative for the approved minutes document.

## Data Storage

All four fields stored as EAV `entity_properties` — same as `approved_by`, `objectives`, etc. No schema changes. Empty/absent = `""` or `"[]"`, both treated as empty by helpers.

## Submit Pattern: Hidden JSON

JS manages a live table of rows. On form submit, JS serializes all row values into a JSON string and writes it into `<input type="hidden" name="field_name">`. Server receives the JSON string, stores it directly as an EAV property. No multi-value form parsing needed.

```
User edits rows → JS serializes → hidden input → form POST → handler stores JSON string
```

On page load, the existing JSON is parsed and rows are rendered from it (same hidden-JSON pattern in reverse).

## Edit Locations

### Meeting Detail Page (`/meetings/{id}`)

New "Roll Call" section at the bottom, with its own small form:
- Table with columns: Name (text input), Status (select: present/absent/excused)
- "Add Person" button appends a new row
- "×" button removes a row
- POST handler: `POST /meetings/{id}/roll-call`

### Minutes View Page (`/meetings/{id}/minutes/{minutes_id}`)

Three additions to the existing page (only editable when status is `draft` or `pending_approval`):

1. **Distribution List** — textarea, one entry per line → converted to JSON array
2. **Attendance Table** — dynamic rows: Name, Status (present/absent/excused), Delegation To
3. **Action Items Table** — dynamic rows: Description, Responsible, Due Date, Status (open/in_progress/done)

Each has its own form + handler to avoid one large form. POST handlers:
- `POST /meetings/{id}/minutes/{minutes_id}/distribution`
- `POST /meetings/{id}/minutes/{minutes_id}/attendance`
- `POST /meetings/{id}/minutes/{minutes_id}/action-items`

## Rust Model Changes

### `MeetingDetail` (src/models/meeting/types.rs)

Add field: `pub roll_call_data: String`

New impl methods:
```rust
pub fn roll_call_list(&self) -> Vec<serde_json::Value> { ... }
```

### `Minutes` (src/models/minutes/types.rs)

Add fields:
```rust
pub distribution_list: String,       // JSON array of strings
pub structured_attendance: String,   // JSON array of objects
pub structured_action_items: String, // JSON array of objects
```

New impl methods on `Minutes`:
```rust
pub fn distribution_list_items(&self) -> Vec<String> { ... }
pub fn attendance_list(&self) -> Vec<serde_json::Value> { ... }
pub fn action_items_list(&self) -> Vec<serde_json::Value> { ... }
```

## Security

- No `innerHTML` anywhere — all row cells built via `createElement`/`textContent`/`appendChild`
- JSON parsed server-side with `serde_json::from_str(...).unwrap_or_default()` — invalid JSON silently produces empty list
- All POST handlers: CSRF validation + `tor.edit` / `minutes.edit` permission check

## Out of Scope

- Auto-populating roll call from ToR membership (could be a future feature)
- Linking action items to workflow proposals
- Locking fields once minutes are approved (minutes are already read-only once approved)
