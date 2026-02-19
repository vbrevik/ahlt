# Calendar Confirm Badge — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a `✓` checkmark badge to future unconfirmed event pills in the Meeting Outlook calendar so users can confirm meetings in one click without leaving the calendar view.

**Architecture:** New JSON endpoint `POST /api/tor/{id}/meetings/confirm-calendar` handles both cadence-only slots (creates + confirms) and already-projected meetings (status update only). Client-side `fetch()` hits the endpoint, then mutates the pill DOM in place — no page reload.

**Tech Stack:** Rust/Actix-web (handler), Askama HTML template (calendar JS), CSS custom properties (badge styling), `serde_json` for JSON response.

**Design doc:** `docs/plans/2026-02-19-calendar-confirm-badge-design.md`

---

## Task 1: Backend — `confirm_calendar` JSON handler

### Context

```
src/handlers/meeting_handlers/crud.rs   ← modify this file
src/handlers/meeting_handlers/mod.rs    ← re-exports (pub use crud::*; — no change needed)
```

The existing `ConfirmForm` struct (line 20) and `confirm()` handler (line 103) handle the ToR detail page form-submit flow. They stay unchanged. We add a new form struct and handler alongside them.

`meeting::create()` signature:
```rust
pub fn create(conn: &Connection, tor_id: i64, date: &str, tor_name: &str, location: &str, notes: &str) -> rusqlite::Result<i64>
```

`meeting::update_status()` signature:
```rust
pub fn update_status(conn: &Connection, meeting_id: i64, status: &str) -> rusqlite::Result<()>
```

`meeting::find_by_id()` returns `rusqlite::Result<Option<MeetingDetail>>` where `MeetingDetail` has `.tor_id: i64` and `.status: String`.

### Task

Add to `src/handlers/meeting_handlers/crud.rs` after the existing `ConfirmForm` struct definition (after line 26):

```rust
#[derive(serde::Deserialize)]
pub struct CalendarConfirmForm {
    pub csrf_token: String,
    pub meeting_date: String,
    pub tor_name: String,
    pub meeting_id: Option<i64>,
}
```

Then add the handler after the existing `confirm()` function (after line 154):

```rust
// ---------------------------------------------------------------------------
// POST — confirm from calendar (returns JSON)
// ---------------------------------------------------------------------------

/// POST /api/tor/{id}/meetings/confirm-calendar — confirm a meeting from the calendar view.
///
/// Returns JSON `{"ok":true,"meeting_id":N}` on success.
/// Handles two cases:
///   - meeting_id present  → meeting already exists as "projected", just update status
///   - meeting_id absent   → cadence slot, create the meeting entity then confirm it
pub async fn confirm_calendar(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<CalendarConfirmForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;

    let tor_id = path.into_inner();
    let conn = pool.get()?;
    let current_user_id = get_user_id(&session).unwrap_or(0);

    let meeting_id = if let Some(mid) = form.meeting_id {
        // Meeting already exists — verify ownership then update status
        let existing = meeting::find_by_id(&conn, mid)?.ok_or(AppError::NotFound)?;
        if existing.tor_id != tor_id {
            return Err(AppError::NotFound);
        }
        meeting::update_status(&conn, mid, "confirmed")?;
        mid
    } else {
        // No persisted meeting yet — create it and confirm in one step
        let mid = meeting::create(&conn, tor_id, &form.meeting_date, &form.tor_name, "", "")?;
        meeting::update_status(&conn, mid, "confirmed")?;
        mid
    };

    let details = serde_json::json!({
        "meeting_id": meeting_id,
        "tor_id": tor_id,
        "meeting_date": &form.meeting_date,
        "summary": format!("Meeting confirmed for {} on {}", &form.tor_name, &form.meeting_date),
    });
    let _ = crate::audit::log(
        &conn,
        current_user_id,
        "meeting.confirmed",
        "meeting",
        meeting_id,
        details,
    );

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .body(serde_json::json!({"ok": true, "meeting_id": meeting_id}).to_string()))
}
```

### Constraints

- Do NOT modify `ConfirmForm`, `confirm()`, or any existing handler
- `CalendarConfirmForm` must go immediately after the existing `ConfirmForm` block
- `confirm_calendar()` must go immediately after the existing `confirm()` function
- Use `AppError::NotFound` for unknown meeting_id — do NOT panic or unwrap
- Return `HttpResponse::Ok()` with `content_type("application/json")` — not a redirect

### Verification

```bash
cargo check 2>&1 | tail -5
```

Expected: `Finished` with no errors. If there are type errors, check that `serde_json` is imported (it's already in `Cargo.toml`) and that `meeting::find_by_id`, `meeting::create`, `meeting::update_status` signatures match.

**Commit:**
```bash
git add src/handlers/meeting_handlers/crud.rs
git commit -m "feat(api): add confirm_calendar JSON handler for calendar view"
```

---

## Task 2: Register the route in `main.rs`

### Context

```
src/main.rs   ← modify this file
```

The API route `/api/tor/calendar` is at line 154. Meeting routes start around line 225–232:

```rust
.route("/tor/{id}/meetings/confirm", web::post().to(handlers::meeting_handlers::confirm))
.route("/tor/{id}/meetings/{mid}", web::get().to(handlers::meeting_handlers::detail))
```

The new route must be registered BEFORE the `{mid}` path-param catch-all patterns (Actix-web matches routes in registration order).

### Task

Add the new route immediately after the existing `/api/tor/calendar` GET route (line 154):

```rust
.route("/api/tor/{id}/meetings/confirm-calendar", web::post().to(handlers::meeting_handlers::confirm_calendar))
```

The target region to insert after:
```rust
.route("/api/tor/calendar", web::get().to(handlers::tor_handlers::calendar_api))
```

### Constraints

- Place the route in the API section (near line 154), not buried in the meeting routes — it's a JSON API route
- Route method must be `web::post()` — not get
- Handler name is `handlers::meeting_handlers::confirm_calendar` (already exported via `pub use crud::*`)
- Do NOT add `{id}` inside a nested `.service()` scope — add it flat like the existing API routes

### Verification

```bash
cargo check 2>&1 | tail -5
```

Expected: `Finished` with no errors. If you see "function not found", check that `confirm_calendar` is exported from `meeting_handlers::mod.rs` (it is via `pub use crud::*`).

**Commit:**
```bash
git add src/main.rs
git commit -m "feat(routes): register POST /api/tor/{id}/meetings/confirm-calendar"
```

---

## Task 3: Badge CSS in `static/css/style.css`

### Context

```
static/css/style.css   ← modify this file
```

Existing classes to be aware of (around line 3840–3896):

```css
.outlook-event--projected { opacity: 0.7; border-left-style: dashed; }
.outlook-event--confirmed  { opacity: 1; font-weight: 500; }
.outlook-confirm-btn { ... }   /* OLD button — will be replaced */
```

The pill (`.outlook-event`) currently has `position: absolute` set (it's placed inside grid cells that are `position: relative`). We need to make the pill itself `position: relative` so we can absolutely position the badge inside it.

Grep for the existing `.outlook-event {` rule to find its location:
```bash
grep -n "^\.outlook-event {" static/css/style.css
```

### Task

**Step 1:** Find the `.outlook-event {` rule and add `position: relative;` to it if it isn't already there.

**Step 2:** Replace the entire `.outlook-confirm-btn` and `.outlook-confirm-btn:hover` blocks with:

```css
/* Calendar confirm badge — ✓ button on future unconfirmed pills */
.outlook-event-confirm-badge {
    position: absolute;
    top: 3px;
    right: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.25);
    border: 1px solid rgba(0, 0, 0, 0.08);
    color: inherit;
    cursor: pointer;
    font-size: 0.625rem;
    line-height: 14px;
    text-align: center;
    padding: 0;
    transition: background var(--duration) var(--ease), border-color var(--duration) var(--ease), color var(--duration) var(--ease);
    z-index: 1;
}

.outlook-event-confirm-badge:hover {
    background: #16a34a;
    border-color: #15803d;
    color: #fff;
}

.outlook-event-confirm-badge--loading {
    pointer-events: none;
    opacity: 0.4;
}
```

Also remove the now-unused `.outlook-event-actions` rule if it exists (search for it and delete).

### Constraints

- `.outlook-event-confirm-badge` must use `position: absolute` — the badge overlays the pill, it does not shift content
- Do NOT remove `.outlook-event--projected` or `.outlook-event--confirmed` — they're still used
- Keep `--duration` and `--ease` CSS variables in transitions — they're defined globally in the stylesheet
- The `z-index: 1` ensures the badge renders above any pill background overlaps

### Verification

```bash
cargo check 2>&1 | tail -3
```

CSS is not compiled by Rust — just verify no syntax errors by visual inspection. Check that:
- `.outlook-confirm-btn` is gone (search the file)
- `.outlook-event-confirm-badge` exists with `position: absolute`

**Commit:**
```bash
git add static/css/style.css
git commit -m "feat(css): add calendar confirm badge styles"
```

---

## Task 4: Frontend — badge in `makePill()` + `confirmMeetingAjax()`

### Context

```
templates/tor/outlook.html   ← modify this file
```

Key existing variables and functions (all inside the IIFE):

- `TODAY` (string `'YYYY-MM-DD'`) — injected from Rust template at line 78
- `makePill(evt, showLoc)` — builds the pill DOM element, line 202–260
- `confirmMeeting(evt)` — current form-submit implementation, line 262–289. **This function stays unchanged** — it's used by `templates/tor/detail.html` indirectly? No — it's only used in this file. But keep it for now since it's referenced in the existing badge button code.
- `document.getElementById('csrf_token')` — the hidden input with CSRF token at line 37

`evt` shape (from API / cached JSON):
```js
{
  tor_id: Number,
  tor_label: String,
  date: String,         // "YYYY-MM-DD"
  start_time: String,
  duration_minutes: Number,
  location: String,
  meeting_id: Number | null,
  meeting_status: String | null  // "projected" | "confirmed" | null
}
```

### Task

**Step 1:** Replace the entire existing confirm button block inside `makePill()` (lines 239–257, the `if (evt.meeting_status === 'projected' && evt.meeting_id)` block) with:

```js
// Add confirm badge for future unconfirmed meetings
if (evt.date > TODAY && evt.meeting_status !== 'confirmed') {
    var badge = document.createElement('button');
    badge.type = 'button';
    badge.className = 'outlook-event-confirm-badge';
    badge.textContent = '\u2713';
    badge.title = 'Confirm this meeting';
    badge.addEventListener('click', function(e) {
        e.preventDefault();
        e.stopPropagation();
        confirmMeetingAjax(evt, pill, badge);
    });
    pill.appendChild(badge);
}
```

**Step 2:** Replace the entire `confirmMeeting(evt)` function (lines 262–289) with `confirmMeetingAjax()`:

```js
function confirmMeetingAjax(evt, pill, badge) {
    var csrfToken = document.getElementById('csrf_token').value;
    badge.classList.add('outlook-event-confirm-badge--loading');
    badge.textContent = '\u22ef';  // ellipsis while loading

    var body = 'csrf_token=' + encodeURIComponent(csrfToken) +
               '&meeting_date=' + encodeURIComponent(evt.date) +
               '&tor_name=' + encodeURIComponent(evt.tor_label);
    if (evt.meeting_id) {
        body += '&meeting_id=' + encodeURIComponent(evt.meeting_id);
    }

    fetch('/api/tor/' + evt.tor_id + '/meetings/confirm-calendar', {
        method: 'POST',
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        body: body,
        credentials: 'same-origin'
    })
    .then(function(r) {
        if (!r.ok) { throw new Error('Server error ' + r.status); }
        return r.json();
    })
    .then(function(data) {
        if (!data.ok) { throw new Error(data.error || 'Unknown error'); }
        // Success: update pill in place
        pill.classList.remove('outlook-event--projected');
        pill.classList.add('outlook-event--confirmed');
        if (badge.parentNode) { badge.parentNode.removeChild(badge); }
    })
    .catch(function() {
        // Restore badge on error so user can retry
        badge.classList.remove('outlook-event-confirm-badge--loading');
        badge.textContent = '\u2713';
    });
}
```

### Constraints

- The DOM construction must NOT use `innerHTML` — the security hook will block the build. Use `createElement`, `textContent`, `appendChild` only.
- `evt.date > TODAY` is a lexicographic string comparison — valid for ISO date strings (`YYYY-MM-DD`)
- `credentials: 'same-origin'` is required so the session cookie is sent with the fetch
- `Content-Type: application/x-www-form-urlencoded` must match — the Rust handler uses `web::Form<>` which expects this content type
- Do NOT remove the `outlook-event-content` wrapper div or the existing link structure inside `makePill()` — only replace the confirm button block at the bottom of the function

### Verification

**Step 1 — Compile check:**
```bash
cargo build 2>&1 | tail -3
```
Expected: `Finished dev`. Templates are compiled by Askama at build time — any template syntax errors appear here.

**Step 2 — Manual browser test with staging data:**
```bash
APP_ENV=staging cargo run
```
Navigate to `http://localhost:8080/tor/outlook`. Log in as `admin` / `admin123`.

Checklist:
- [ ] Future event pills show a small `✓` badge in the top-right corner
- [ ] Past event pills show NO badge
- [ ] Already-confirmed meetings show NO badge
- [ ] Hovering the badge highlights it green
- [ ] Clicking the badge: badge shows loading state (ellipsis), then disappears; pill becomes solid/full-opacity
- [ ] On the confirmed pill, the border changes from dashed to solid (`.outlook-event--confirmed` effect)
- [ ] Clicking again on the same pill is not possible (badge is gone)
- [ ] Network tab (DevTools) shows a `POST /api/tor/{id}/meetings/confirm-calendar` request returning `{"ok":true,...}`

**Commit:**
```bash
git add templates/tor/outlook.html
git commit -m "feat(ui): calendar confirm badge with AJAX in-place update"
```

---

## Task 5: Build verification + cleanup

### Task

**Step 1:** Run the full build and test suite:

```bash
cargo build 2>&1 | tail -3
cargo test 2>&1 | tail -10
```

Expected:
- Build: `Finished dev`
- Tests: `test result: ok. N passed; 0 failed`

**Step 2:** Verify the CSRF hidden input is present in `outlook.html`:

```bash
grep -n "csrf_token" templates/tor/outlook.html
```

Expected: line 37 has `<input type="hidden" name="csrf_token" id="csrf_token" value="{{ ctx.csrf_token }}">`. If it's missing, add it inside `{% block content %}` before the `<script>` tag.

**Step 3:** Check that the old `confirmMeeting` form-submit function is gone:

```bash
grep -n "confirmMeeting\b" templates/tor/outlook.html
```

Expected: only `confirmMeetingAjax` references remain. If `confirmMeeting` (without Ajax) still appears, delete it.

**Step 4:** Verify no `innerHTML` usage crept in:

```bash
grep -n "innerHTML" templates/tor/outlook.html
```

Expected: no output.

**Final commit:**
```bash
git add -A
git commit -m "chore: verify calendar confirm badge implementation complete"
```

---

## Summary of Changes

| File | Change |
|---|---|
| `src/handlers/meeting_handlers/crud.rs` | Add `CalendarConfirmForm` struct + `confirm_calendar()` JSON handler |
| `src/main.rs` | Register `POST /api/tor/{id}/meetings/confirm-calendar` in API section |
| `static/css/style.css` | Replace `.outlook-confirm-btn` with `.outlook-event-confirm-badge` styles |
| `templates/tor/outlook.html` | Replace confirm button block in `makePill()` with badge; replace `confirmMeeting()` with `confirmMeetingAjax()` |
