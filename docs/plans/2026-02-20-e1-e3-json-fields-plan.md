# E.1/E.3 JSON Fields — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add four structured JSON fields to meeting and minutes entities: `roll_call_data` (meeting), `distribution_list`, `structured_attendance`, and `structured_action_items` (minutes). Exposed via dynamic-row UI with hidden JSON submit pattern.

**Architecture:** EAV storage (entity_properties) for all four fields. Rust model structs gain typed helper structs and list methods. Three new POST handlers save each field. Two templates gain interactive sections with no-innerHTML JS.

**Tech Stack:** Rust/Actix-web 4, Askama 0.14, SQLite/rusqlite, vanilla JS (createElement/textContent pattern)

**Design doc:** `docs/plans/2026-02-20-e1-e3-json-fields-design.md`

---

### Task 1: Rust Types — Extend MeetingDetail and Minutes Structs

**Files:**
- Modify: `src/models/meeting/types.rs`
- Modify: `src/models/minutes/types.rs`

**GOAL:** `MeetingDetail` gains `roll_call_data: String` field + `roll_call_list()` method. `Minutes` gains three JSON fields + list helpers. Success = `cargo check` passes with no errors.

**CONSTRAINTS:**
- Helper structs must be simple (pub fields, no methods)
- `parse_json_list` pattern from `TorDetail` — `serde_json::from_str(...).unwrap_or_default()`
- No schema changes, no new dependencies
- Keep existing fields unchanged

**FORMAT:**

Add to `src/models/meeting/types.rs` after the last field in `MeetingDetail`:

```rust
pub roll_call_data: String,    // JSON: [{username, status}]
```

Add impl block on `MeetingDetail`:

```rust
/// A single roll call entry parsed from roll_call_data JSON.
#[derive(Debug, Clone)]
pub struct RollCallEntry {
    pub username: String,
    pub status: String,      // "present" | "absent" | "excused"
}

impl MeetingDetail {
    fn parse_roll_call(json: &str) -> Vec<RollCallEntry> {
        let raw: Vec<serde_json::Value> = serde_json::from_str(json).unwrap_or_default();
        raw.into_iter().filter_map(|v| {
            Some(RollCallEntry {
                username: v.get("username")?.as_str()?.to_string(),
                status: v.get("status")?.as_str()?.to_string(),
            })
        }).collect()
    }

    pub fn roll_call_list(&self) -> Vec<RollCallEntry> {
        Self::parse_roll_call(&self.roll_call_data)
    }
}
```

Add to `src/models/minutes/types.rs` after `approved_date`:

```rust
pub distribution_list: String,       // JSON: ["name/email"]
pub structured_attendance: String,   // JSON: [{user_id, name, status, delegation_to}]
pub structured_action_items: String, // JSON: [{description, responsible, due_date, status}]
```

Add helper structs and impl block to `src/models/minutes/types.rs`:

```rust
#[derive(Debug, Clone)]
pub struct AttendanceEntry {
    pub name: String,
    pub status: String,        // "present" | "absent" | "excused"
    pub delegation_to: String,
}

#[derive(Debug, Clone)]
pub struct ActionItem {
    pub description: String,
    pub responsible: String,
    pub due_date: String,
    pub status: String,        // "open" | "in_progress" | "done"
}

impl Minutes {
    pub fn distribution_items(&self) -> Vec<String> {
        serde_json::from_str(&self.distribution_list).unwrap_or_default()
    }

    pub fn attendance_list(&self) -> Vec<AttendanceEntry> {
        let raw: Vec<serde_json::Value> = serde_json::from_str(&self.structured_attendance)
            .unwrap_or_default();
        raw.into_iter().filter_map(|v| {
            Some(AttendanceEntry {
                name: v.get("name")?.as_str()?.to_string(),
                status: v.get("status")?.as_str()?.to_string(),
                delegation_to: v.get("delegation_to")
                    .and_then(|s| s.as_str()).unwrap_or("").to_string(),
            })
        }).collect()
    }

    pub fn action_items_list(&self) -> Vec<ActionItem> {
        let raw: Vec<serde_json::Value> = serde_json::from_str(&self.structured_action_items)
            .unwrap_or_default();
        raw.into_iter().filter_map(|v| {
            Some(ActionItem {
                description: v.get("description")?.as_str()?.to_string(),
                responsible: v.get("responsible")?.as_str()?.to_string(),
                due_date: v.get("due_date").and_then(|s| s.as_str()).unwrap_or("").to_string(),
                status: v.get("status")?.as_str()?.to_string(),
            })
        }).collect()
    }
}
```

**Step 1: Apply the type changes above**

**Step 2: Run `cargo check`**

```bash
cargo check 2>&1 | grep -E "error|warning: unused"
```

Expected: Compiler errors about `roll_call_data` field not set in `find_by_id` row mapper and the `Minutes` struct. Note which files have errors — you'll fix them in Tasks 2 and 3.

**Step 3: Commit**

```bash
git add src/models/meeting/types.rs src/models/minutes/types.rs
git commit -m "feat(e1-e3): add JSON field types and helper structs to Meeting/Minutes models"
```

**FAILURE CONDITIONS:**
- Removes or renames existing fields
- Uses `serde_json::Value` directly in template-facing structs
- Panics instead of `unwrap_or_default()` on bad JSON

---

### Task 2: Meeting Queries — Add `roll_call_data` Field + Upsert Helper

**Files:**
- Modify: `src/models/meeting/queries.rs`

**GOAL:** `find_by_id` returns a `MeetingDetail` with `roll_call_data` populated. New `update_roll_call()` function upserts the JSON string. Success = `cargo check` shows no meeting-related errors.

**CONSTRAINTS:**
- Follow the exact LEFT JOIN + COALESCE pattern from the existing `p_meetnum`, `p_chair` joins
- `update_roll_call` uses the same upsert pattern as `update_status`
- No changes to `create()` — roll call is set after meeting creation

**FORMAT:**

In `find_by_id`, add one more LEFT JOIN to the SQL string:

```sql
LEFT JOIN entity_properties p_roll ON e.id = p_roll.entity_id AND p_roll.key = 'roll_call_data'
```

Add to SELECT columns:

```sql
COALESCE(p_roll.value, '[]') AS roll_call_data
```

Add to row mapper:

```rust
roll_call_data: row.get("roll_call_data")?,
```

Add after `update_status`:

```rust
/// Upsert roll_call_data JSON string for a meeting.
pub fn update_roll_call(conn: &Connection, meeting_id: i64, json: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'roll_call_data', ?2) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value",
        params![meeting_id, json],
    )?;
    Ok(())
}
```

**Step 1: Apply the changes to `src/models/meeting/queries.rs`**

**Step 2: Run `cargo check`**

```bash
cargo check 2>&1 | grep "error"
```

Expected: No meeting model errors. May still see minutes errors (fixed in Task 3).

**Step 3: Commit**

```bash
git add src/models/meeting/queries.rs
git commit -m "feat(e1-e3): extend meeting find_by_id with roll_call_data, add update_roll_call"
```

**FAILURE CONDITIONS:**
- Breaks existing tests (COALESCE default must be `'[]'` not `''`)
- Forgets to update the row mapper
- `update_roll_call` uses UPDATE instead of upsert (fails on first save)

---

### Task 3: Minutes Queries — Add 3 New Fields + Upsert Helpers

**Files:**
- Modify: `src/models/minutes/queries.rs`

**GOAL:** Both `find_by_meeting` and `find_by_id` return `Minutes` with the three new fields populated. Three new upsert helpers added. Success = `cargo check` passes clean.

**CONSTRAINTS:**
- Must update **both** `find_by_meeting` AND `find_by_id` — they have separate SQL strings
- COALESCE defaults: `'[]'` for all three JSON fields
- Follow the exact pattern of `p_appr_by` / `p_appr_date` joins added in E.3

**FORMAT:**

For **each** of the two query functions, add three LEFT JOINs:

```sql
LEFT JOIN entity_properties p_dist ON m.id = p_dist.entity_id AND p_dist.key = 'distribution_list'
LEFT JOIN entity_properties p_att ON m.id = p_att.entity_id AND p_att.key = 'structured_attendance'
LEFT JOIN entity_properties p_ai ON m.id = p_ai.entity_id AND p_ai.key = 'structured_action_items'
```

Add to SELECT in both queries:

```sql
COALESCE(p_dist.value, '[]') AS distribution_list,
COALESCE(p_att.value, '[]') AS structured_attendance,
COALESCE(p_ai.value, '[]') AS structured_action_items
```

Add to both row mappers:

```rust
distribution_list: row.get("distribution_list")?,
structured_attendance: row.get("structured_attendance")?,
structured_action_items: row.get("structured_action_items")?,
```

Add three upsert helpers after `update_status`:

```rust
pub fn update_distribution_list(conn: &Connection, minutes_id: i64, json: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'distribution_list', ?2) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value",
        params![minutes_id, json],
    )?;
    Ok(())
}

pub fn update_structured_attendance(conn: &Connection, minutes_id: i64, json: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'structured_attendance', ?2) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value",
        params![minutes_id, json],
    )?;
    Ok(())
}

pub fn update_structured_action_items(conn: &Connection, minutes_id: i64, json: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO entity_properties (entity_id, key, value) VALUES (?1, 'structured_action_items', ?2) \
         ON CONFLICT(entity_id, key) DO UPDATE SET value = excluded.value",
        params![minutes_id, json],
    )?;
    Ok(())
}
```

**Step 1: Apply the changes to `src/models/minutes/queries.rs`**

**Step 2: Run `cargo check && cargo test`**

```bash
cargo check 2>&1 | grep "error"
cargo test 2>&1 | tail -5
```

Expected: Clean check. All tests pass.

**Step 3: Commit**

```bash
git add src/models/minutes/queries.rs
git commit -m "feat(e1-e3): extend minutes queries with 3 JSON fields, add upsert helpers"
```

**FAILURE CONDITIONS:**
- Only updates one of the two find functions
- COALESCE default is `''` instead of `'[]'` (breaks `distribution_items()` list helper)
- Missing row mapper updates in one of the functions

---

### Task 4: Roll Call Handler + Route

**Files:**
- Modify: `src/handlers/meeting_handlers/crud.rs`
- Modify: `src/handlers/meeting_handlers/mod.rs`
- Modify: `src/main.rs`

**GOAL:** `POST /tor/{id}/meetings/{mid}/roll-call` accepts a form field `roll_call_data` (JSON string), validates CSRF, checks permission, upserts via `meeting::update_roll_call()`, redirects back with flash. Success = submitting the form saves to DB and shows "Roll call saved".

**CONSTRAINTS:**
- Permission: `tor.edit`
- CSRF validation required
- Accept raw JSON string in form — no server-side parsing, store as-is
- Redirect to `/tor/{tor_id}/meetings/{meeting_id}` (same page) on success and error
- Audit log the save action

**FORMAT:**

Add form struct and handler to `src/handlers/meeting_handlers/crud.rs`:

```rust
#[derive(serde::Deserialize)]
pub struct RollCallForm {
    pub csrf_token: String,
    pub roll_call_data: String,  // raw JSON string from hidden input
}

pub async fn save_roll_call(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<(i64, i64)>,
    form: web::Form<RollCallForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "tor.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;
    let (tor_id, meeting_id) = path.into_inner();
    let conn = pool.get()?;

    meeting::update_roll_call(&conn, meeting_id, &form.roll_call_data)?;

    let user_id = get_user_id(&session).unwrap_or(0);
    let _ = audit::log(&conn, user_id, "meeting.roll_call_saved", "meeting", meeting_id,
        serde_json::json!({"summary": "Roll call updated"}));

    let _ = session.insert("flash", "Roll call saved");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/tor/{}/meetings/{}", tor_id, meeting_id)))
        .finish())
}
```

In `src/handlers/meeting_handlers/mod.rs`, add `pub use crud::save_roll_call;` (or re-export via the existing pattern — check how other handlers are exported in this mod.rs).

In `src/main.rs`, add route **before** the parameterized `{mid}` route (check existing route order):

```rust
.route("/tor/{id}/meetings/{mid}/roll-call", web::post().to(handlers::meeting_handlers::save_roll_call))
```

**Step 1: Add form struct + handler to crud.rs**

**Step 2: Export in mod.rs and register route in main.rs**

**Step 3: Run `cargo check`**

```bash
cargo check 2>&1 | grep "error"
```

Expected: Clean.

**Step 4: Commit**

```bash
git add src/handlers/meeting_handlers/crud.rs src/handlers/meeting_handlers/mod.rs src/main.rs
git commit -m "feat(e1-e3): add roll call POST handler and route"
```

**FAILURE CONDITIONS:**
- Missing CSRF validation
- Missing permission check
- Route registered after `{mid}` path parameter (breaks routing)
- Tries to parse the JSON server-side instead of passing string through

---

### Task 5: Minutes Structured Field Handlers + Routes

**Files:**
- Modify: `src/handlers/minutes_handlers.rs`
- Modify: `src/main.rs`

**GOAL:** Three new POST handlers save distribution list, structured attendance, and action items for a minutes document. Each requires `minutes.edit` permission + CSRF. Success = submitting any of the three forms saves to DB and redirects back.

**CONSTRAINTS:**
- Permission: `minutes.edit`
- Block saving if minutes status is `"approved"` — return flash error and redirect
- Accept raw JSON strings — no server-side JSON parsing
- Redirect to `/minutes/{id}` on success and error
- Audit log each save

**FORMAT:**

Add to `src/handlers/minutes_handlers.rs`:

```rust
#[derive(serde::Deserialize)]
pub struct DistributionForm {
    pub csrf_token: String,
    pub distribution_list: String,   // raw JSON array string
}

pub async fn save_distribution(
    pool: web::Data<DbPool>,
    session: Session,
    path: web::Path<i64>,
    form: web::Form<DistributionForm>,
) -> Result<HttpResponse, AppError> {
    require_permission(&session, "minutes.edit")?;
    csrf::validate_csrf(&session, &form.csrf_token)?;
    let minutes_id = path.into_inner();
    let conn = pool.get()?;

    // Guard: don't edit approved minutes
    if let Some(m) = minutes::find_by_id(&conn, minutes_id)? {
        if m.status == "approved" {
            let _ = session.insert("flash", "Cannot edit approved minutes");
            return Ok(HttpResponse::SeeOther()
                .insert_header(("Location", format!("/minutes/{}", minutes_id)))
                .finish());
        }
    }

    minutes::update_distribution_list(&conn, minutes_id, &form.distribution_list)?;
    let user_id = get_user_id(&session).unwrap_or(0);
    let _ = audit::log(&conn, user_id, "minutes.distribution_saved", "minutes", minutes_id,
        serde_json::json!({"summary": "Distribution list updated"}));
    let _ = session.insert("flash", "Distribution list saved");
    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/minutes/{}", minutes_id)))
        .finish())
}

// Repeat the same pattern for save_attendance (field: structured_attendance, action: minutes.attendance_saved)
// and save_action_items (field: structured_action_items, action: minutes.action_items_saved)
```

Add routes in `src/main.rs` (after existing `/minutes/{id}/status` route):

```rust
.route("/minutes/{id}/distribution", web::post().to(handlers::minutes_handlers::save_distribution))
.route("/minutes/{id}/attendance", web::post().to(handlers::minutes_handlers::save_attendance))
.route("/minutes/{id}/action-items", web::post().to(handlers::minutes_handlers::save_action_items))
```

**Step 1: Add all three form structs and handlers to minutes_handlers.rs**

**Step 2: Register all three routes in main.rs**

**Step 3: Run `cargo check`**

```bash
cargo check 2>&1 | grep "error"
```

**Step 4: Run all tests**

```bash
cargo test 2>&1 | tail -5
```

Expected: All tests pass.

**Step 5: Commit**

```bash
git add src/handlers/minutes_handlers.rs src/main.rs
git commit -m "feat(e1-e3): add minutes structured field handlers (distribution, attendance, action items)"
```

**FAILURE CONDITIONS:**
- Missing approved-status guard
- Routes registered without checking order relative to `/minutes/{id}/sections/{section_id}`
- Missing audit logging on any handler

---

### Task 6: Template — Meeting Detail Roll Call Section

**Files:**
- Modify: `templates/meetings/detail.html`

**GOAL:** A "Roll Call" section appears at the bottom of the meeting detail page with a dynamic-row table (Name, Status columns). Users can add/remove rows and save via form submission. Existing roll call rows are pre-populated from `meeting.roll_call_data`. Success = adding a row, saving, reloading shows the saved row.

**CONSTRAINTS:**
- **No `innerHTML`** — use `createElement`/`textContent`/`appendChild` only
- JSON data injected via `<script type="application/json">{{ meeting.roll_call_data|safe }}</script>` — note the `|safe` filter (Askama escapes `"` to `&#34;` by default, breaking JSON.parse)
- On submit: JS writes all row values as JSON to `<input type="hidden" name="roll_call_data">`
- Status options: present / absent / excused
- Do NOT show save button if user lacks `tor.edit` permission

**FORMAT:**

Add after the `<!-- Minutes -->` section at the bottom of `templates/meetings/detail.html`:

```html
<!-- Roll Call -->
<section class="section">
    <div class="section-header">
        <h2>Roll Call</h2>
    </div>

    <script type="application/json" id="roll-call-data">{{ meeting.roll_call_data|safe }}</script>

    <table class="table" id="roll-call-table">
        <thead>
            <tr>
                <th>Name</th>
                <th>Status</th>
                {% if ctx.permissions.has("tor.edit") %}
                <th></th>
                {% endif %}
            </tr>
        </thead>
        <tbody id="roll-call-body">
            <!-- Populated by JS -->
        </tbody>
    </table>

    {% if ctx.permissions.has("tor.edit") %}
    <div style="margin-top: 0.75rem; display: flex; gap: 0.5rem;">
        <button type="button" class="btn btn-sm btn-secondary" id="add-roll-call-row">+ Add Person</button>
    </div>

    <form method="post" action="/tor/{{ tor_id }}/meetings/{{ meeting.id }}/roll-call" style="margin-top: 1rem;">
        <input type="hidden" name="csrf_token" value="{{ ctx.csrf_token }}">
        <input type="hidden" name="roll_call_data" id="roll-call-json">
        <button type="submit" class="btn btn-primary btn-sm" id="save-roll-call">Save Roll Call</button>
    </form>
    {% endif %}
</section>

<script>
(function() {
    const STATUS_OPTIONS = ['present', 'absent', 'excused'];
    const tbody = document.getElementById('roll-call-body');
    const jsonInput = document.getElementById('roll-call-json');
    const addBtn = document.getElementById('add-roll-call-row');
    const canEdit = !!addBtn;  // presence of add button = has permission

    function makeRow(username, status) {
        const tr = document.createElement('tr');

        const nameTd = document.createElement('td');
        const nameInput = document.createElement('input');
        nameInput.type = 'text';
        nameInput.className = 'input input--sm';
        nameInput.placeholder = 'Name';
        nameInput.value = username || '';
        nameInput.readOnly = !canEdit;
        nameTd.appendChild(nameInput);
        tr.appendChild(nameTd);

        const statusTd = document.createElement('td');
        const statusSel = document.createElement('select');
        statusSel.className = 'input input--sm';
        statusSel.disabled = !canEdit;
        STATUS_OPTIONS.forEach(opt => {
            const o = document.createElement('option');
            o.value = opt;
            o.textContent = opt;
            if (opt === status) o.selected = true;
            statusSel.appendChild(o);
        });
        statusTd.appendChild(statusSel);
        tr.appendChild(statusTd);

        if (canEdit) {
            const actionTd = document.createElement('td');
            const removeBtn = document.createElement('button');
            removeBtn.type = 'button';
            removeBtn.className = 'btn btn-sm btn-danger';
            removeBtn.textContent = '×';
            removeBtn.addEventListener('click', () => tr.remove());
            actionTd.appendChild(removeBtn);
            tr.appendChild(actionTd);
        }

        return tr;
    }

    // Load existing data
    const existing = JSON.parse(document.getElementById('roll-call-data').textContent || '[]');
    existing.forEach(entry => tbody.appendChild(makeRow(entry.username, entry.status)));

    if (canEdit) {
        addBtn.addEventListener('click', () => tbody.appendChild(makeRow('', 'present')));

        // Serialize on form submit
        document.getElementById('save-roll-call').closest('form').addEventListener('submit', () => {
            const rows = Array.from(tbody.querySelectorAll('tr'));
            const data = rows.map(tr => {
                const inputs = tr.querySelectorAll('input, select');
                return { username: inputs[0].value.trim(), status: inputs[1].value };
            }).filter(e => e.username);
            jsonInput.value = JSON.stringify(data);
        });
    }
})();
</script>
```

**Note:** The `MeetingDetailTemplate` struct in `src/templates_structs.rs` has a `meeting: MeetingDetail` field. Since `MeetingDetail` now includes `roll_call_data`, the template has access to `{{ meeting.roll_call_data|safe }}` automatically. No struct changes needed.

**Step 1: Apply the Roll Call section to `templates/meetings/detail.html`**

**Step 2: Run `cargo build` to verify Askama compiles the template**

```bash
cargo build 2>&1 | grep "error"
```

Expected: Clean build.

**Step 3: Manual verification**

```bash
APP_ENV=staging cargo run
```

Open a meeting detail page. Verify:
- Roll Call section appears at bottom
- "Add Person" button adds a new row
- Removing a row works
- Saving persists on reload (check DB: `sqlite3 data/staging/app.db "SELECT * FROM entity_properties WHERE key='roll_call_data';"`)

**Step 4: Commit**

```bash
git add templates/meetings/detail.html
git commit -m "feat(e1-e3): add dynamic roll call section to meeting detail page"
```

**FAILURE CONDITIONS:**
- Uses `innerHTML` anywhere in the script
- Missing `|safe` on `roll_call_data` JSON (causes JSON.parse to fail on `&#34;`)
- Save button visible to users without `tor.edit` permission
- Shows the form for read-only display (users without permission should still see the table, read-only)

---

### Task 7: Template — Minutes View Structured Sections

**Files:**
- Modify: `templates/minutes/view.html`

**GOAL:** The minutes view page gains three new editable sections at the bottom: Distribution List (textarea), Attendance (dynamic rows), and Action Items (dynamic rows). All sections are read-only when minutes status is `approved`. Success = adding an action item, saving, reloading shows it.

**CONSTRAINTS:**
- **No `innerHTML`**
- `|safe` filter on all JSON embedded in `<script type="application/json">` elements
- All three sections are in separate `<form>` elements (each posts to its own endpoint)
- Hide form controls (but show data) when `minutes.status == "approved"`
- Action Items status options: open / in_progress / done
- Attendance status options: present / absent / excused

**FORMAT:**

First, check the minutes template to find where to add content. Look for the end of the page (after the last section or the approval controls). Add three new sections **after** the existing approve/reject form block.

**Distribution List section:**
```html
<section class="section">
    <div class="section-header"><h2>Distribution List</h2></div>
    {% if minutes.status.as_str() != "approved" %}
    <form method="post" action="/minutes/{{ minutes.id }}/distribution">
        <input type="hidden" name="csrf_token" value="{{ ctx.csrf_token }}">
        <textarea name="distribution_list" class="input" rows="4"
                  placeholder="One name or email per line">{{ minutes.distribution_items().join("\n") }}</textarea>
        <div style="margin-top:0.5rem;">
            <button type="submit" class="btn btn-sm btn-primary">Save Distribution List</button>
        </div>
    </form>
    {% else %}
    <ul>
        {% for item in minutes.distribution_items() %}
        <li>{{ item }}</li>
        {% endfor %}
    </ul>
    {% endif %}
</section>
```

**Note on distribution_list:** The handler receives a textarea value (newlines), but we need it as JSON. Two options:
1. Convert in handler: `lines_to_json()` pattern from `tor_handlers/crud.rs`
2. Use the hidden-JSON pattern (overkill for a simple list)

**Use option 1** — simpler and consistent with ToR. Update the distribution form handler (`save_distribution`) to call `lines_to_json()` on the textarea value before storing. Change the `DistributionForm.distribution_list` field to accept plain text and add a `lines_to_json` call in the handler:

```rust
// In save_distribution handler, before calling update_distribution_list:
let json = lines_to_json(&form.distribution_list);
minutes::update_distribution_list(&conn, minutes_id, &json)?;
```

Where `lines_to_json` is the same helper as in `tor_handlers/crud.rs` — copy it into `minutes_handlers.rs` or extract to a shared utility.

**Attendance section:** Use the hidden-JSON pattern (same as roll call). Dynamic rows with Name, Status, Delegation To columns.

**Action Items section:** Dynamic rows with Description, Responsible, Due Date, Status columns.

**Full JS for attendance rows** (follows exact same pattern as Task 6 roll call, adapted for 3 columns):

```javascript
// attendance: {name, status, delegation_to}
// action items: {description, responsible, due_date, status}
```

(Implement following the same makeRow + serialize-on-submit pattern from Task 6.)

**Step 1: Add all three sections to `templates/minutes/view.html`**

Also update `save_distribution` handler in `src/handlers/minutes_handlers.rs` to use `lines_to_json()` (copy from tor_handlers/crud.rs).

**Step 2: Run `cargo build`**

```bash
cargo build 2>&1 | grep "error"
```

**Step 3: Manual verification**

```bash
APP_ENV=staging cargo run
```

Open a meeting → generate minutes → view minutes. Verify:
- All three sections appear
- Distribution: save a list, reload, see items
- Attendance: add rows, save, reload
- Action Items: add rows with statuses, save, reload
- Approved minutes show data read-only (manually set status: `sqlite3 data/staging/app.db "UPDATE entity_properties SET value='approved' WHERE key='status' AND entity_id=<minutes_id>;"`)

**Step 4: Commit**

```bash
git add templates/minutes/view.html src/handlers/minutes_handlers.rs
git commit -m "feat(e1-e3): add distribution, attendance, and action items sections to minutes view"
```

**FAILURE CONDITIONS:**
- `innerHTML` used anywhere
- Missing `|safe` on any JSON injected into `<script>` elements
- Edit forms visible on approved minutes
- `distribution_list` stored as raw textarea text (newlines) instead of JSON array
- Askama `&&` in conditions (use nested `{% if %}` instead)

---

### Task 8: Verification and Backlog Update

**GOAL:** All new fields round-trip correctly. Build and tests pass. Backlog updated to mark E.1 and E.3 complete.

**Step 1: Run full test suite**

```bash
cargo test 2>&1 | tail -5
```

Expected: All tests pass (count should match or exceed previous run).

**Step 2: Run clippy**

```bash
cargo clippy 2>&1 | grep "warning\|error" | grep -v "dead_code"
```

Fix any warnings in files you modified.

**Step 3: Update backlog**

In `docs/BACKLOG.md`, find the E.1/E.3 remaining fields section and mark complete:

```markdown
| E.1 | **Meeting** | ~~`roll_call_data`~~ | ~~Low~~ | ~~Small~~ | **DONE** |
| E.3 | **Minutes** | ~~`distribution_list`, `structured_action_items`, `structured_attendance`~~ | ~~Low~~ | ~~Small~~ | **DONE** |
```

**Step 4: Final commit**

```bash
git add docs/BACKLOG.md
git commit -m "docs: mark E.1/E.3 JSON fields complete in backlog"
```
