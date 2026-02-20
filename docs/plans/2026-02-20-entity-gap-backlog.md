# Entity Gap Backlog

Based on analysis of real TOR 602 CISWG document structure vs current data model.

## Meeting Entity

Currently stores: `meeting_date`, `label`, `location`, `notes`, `status`, `agenda_count`, `has_minutes`.

### Missing Properties

| Property | Type | Purpose |
|----------|------|---------|
| `meeting_number` | string | Sequential meeting number within ToR (e.g. "Meeting #14") |
| `classification` | string | Security classification of the meeting |
| `vtc_details` | string | Video teleconference connection details |
| `chair_user_id` | string | User ID of the meeting chair (may differ from ToR chair) |
| `secretary_user_id` | string | User ID of the meeting secretary/note-taker |
| `roll_call_data` | JSON | Structured attendance: `[{user_id, status: "present"|"absent"|"delegated"}]` |

## Agenda Point Entity

Currently stores: `title`, `description`, `point_type`, `due_date`, `duration_minutes`, `status`, `created_by`.

### Missing Properties

| Property | Type | Purpose |
|----------|------|---------|
| `presenter` | string | Person responsible for presenting this agenda item |
| `priority` | string | Priority level: "normal", "high", "urgent" |
| `pre_read_url` | string | Link to pre-read materials for this item |

## Minutes Entity

Currently stores: `label`, `status`, `meeting_id`, sections (attendance, protocol, agenda_items, decisions, action_items).

### Missing Properties

| Property | Type | Purpose |
|----------|------|---------|
| `approved_by` | string | User ID who approved the final minutes |
| `approved_date` | string | Date minutes were approved |
| `distribution_list` | JSON | `["group1", "group2"]` — who receives copies |
| `structured_action_items` | JSON | `[{description, responsible, due_date, status}]` |
| `structured_attendance` | JSON | `[{user_id, name, status, delegation_to}]` |

## Protocol Step Entity (DONE)

- `responsible` property added in this session.

## Implementation Notes

- All properties use EAV pattern (`entity_properties` table), no schema changes needed.
- JSON properties use the same `parse_json_list` / `lines_to_json` pattern established for TOR objectives.
- Roll call and structured attendance overlap — consider whether to store at meeting level, minutes level, or both.
- `chair_user_id` and `secretary_user_id` could alternatively be modeled as relations (`chairs_meeting`, `records_meeting`) for referential integrity, but simple EAV string properties are sufficient for display purposes.
