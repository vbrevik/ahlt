# Calendar Confirm Badge — Design

**Date:** 2026-02-19
**Status:** Approved

## Problem

Confirming a meeting requires navigating to the ToR detail page, finding the meeting in the list, and submitting a form. Users browsing the Meeting Outlook calendar have no way to confirm a meeting without leaving the calendar view.

## Solution

Add a `✓` checkmark badge to each future unconfirmed event pill in the calendar. Clicking the badge instantly confirms the meeting via AJAX — no page navigation, no form, no dialogs.

---

## Visual Design

- **Badge:** Small circular button (`~16px`), `position: absolute; top: 3px; right: 3px` on the pill (pill has `position: relative`)
- **Default state:** Near-invisible — white at ~30% opacity so it doesn't clutter dense views
- **Hover state:** Solid green circle, white `✓` checkmark, `cursor: pointer`
- **Loading state:** Spinner/disabled, `pointer-events: none` while the request is in flight
- **After success:** Badge disappears; pill swaps `.outlook-event--projected` for `.outlook-event--confirmed` (solid border, full opacity) in place — no reload

No text label. Icon only — small enough to not obscure the event name on narrow week-view cells.

---

## Which Pills Show the Badge

Badge shows when **both** conditions are true:

1. `evt.date > TODAY` — future events only; past meetings are irrelevant
2. `evt.meeting_status !== 'confirmed'` — covers:
   - Cadence-computed slots (no `meeting_id`, no `meeting_status`)
   - Persisted "projected" meetings (`meeting_id` present, `meeting_status === 'projected'`)

---

## Backend: New JSON Endpoint

**Route:** `POST /api/tor/{tor_id}/meetings/confirm-calendar`

**Request body** (form-encoded):

| Field | Required | Notes |
|---|---|---|
| `csrf_token` | Yes | Validated server-side |
| `meeting_date` | Yes | `YYYY-MM-DD` |
| `tor_name` | Yes | Used to name the created meeting entity |
| `meeting_id` | No | Present only if the meeting was already persisted as "projected" |

**Logic:**

- If `meeting_id` present → meeting entity already exists → call `meeting::update_status(conn, meeting_id, "confirmed")` only
- If no `meeting_id` → cadence slot with no entity yet → call existing create+confirm path (same logic as `POST /tor/{id}/meetings/confirm`)

**Response:**

```json
{ "ok": true, "meeting_id": 123 }
```

or

```json
{ "ok": false, "error": "Permission denied" }
```

The existing `POST /tor/{id}/meetings/confirm` handler is **unchanged** — still used by the ToR detail page form-submit flow.

---

## Frontend: JavaScript Changes in `outlook.html`

### Badge creation (in `makePill()`)

After building the pill element, check:

```js
var isFuture = evt.date > TODAY;
var isUnconfirmed = evt.meeting_status !== 'confirmed';

if (isFuture && isUnconfirmed) {
    // append confirm badge button to pill
}
```

### Confirm action (`confirmMeetingAjax(evt, pill, badge)`)

1. Disable badge (`pointer-events: none`, spinner class)
2. Fetch POST to `/api/tor/{tor_id}/meetings/confirm-calendar` with CSRF, date, tor_name, optional meeting_id
3. On success: remove badge from DOM, swap pill classes to "confirmed"
4. On error: restore badge so user can retry

### Unchanged

The existing `confirmMeeting()` form-submit function stays — it's used by the ToR detail page confirmation form and is not part of this change.

---

## CSS Additions (`static/css/style.css`)

```css
.outlook-event-confirm-badge {
    position: absolute;
    top: 3px;
    right: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.3);
    border: 1px solid rgba(0, 0, 0, 0.1);
    cursor: pointer;
    font-size: 0.625rem;
    line-height: 16px;
    text-align: center;
    transition: background var(--duration) var(--ease), border-color var(--duration) var(--ease);
    padding: 0;
}

.outlook-event-confirm-badge:hover {
    background: #16a34a;
    border-color: #15803d;
    color: white;
}

.outlook-event-confirm-badge--loading {
    pointer-events: none;
    opacity: 0.5;
}
```

The pill also needs `position: relative` (may already be set; verify).

---

## Files Changed

| File | Change |
|---|---|
| `src/handlers/meeting_handlers/crud.rs` | Add `confirm_calendar()` handler returning JSON |
| `src/handlers/meeting_handlers/mod.rs` | Export new handler |
| `src/main.rs` | Register new route |
| `templates/tor/outlook.html` | Add badge in `makePill()`, add `confirmMeetingAjax()` |
| `static/css/style.css` | Add badge CSS classes |

---

## Out of Scope

- Location editing from calendar (no form — use ToR detail page for that)
- Month view badge (month view uses `<a>` links, not pills — out of scope for now)
- Confirmation for past meetings
