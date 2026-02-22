# Agenda Point Detail — Gap Fix Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make `/tor/{id}/workflow/agenda/{ap_id}` correctly render the agenda point detail page showing seeded COAs and opinions.

**Architecture:** Three independent gaps — two template URL bugs, and an opinion query that reads entity_properties but seeded opinions use relations instead. Fix the query to fall back to relation-based lookup when entity_properties are absent. No schema changes, no new files.

**Tech Stack:** Rust (rusqlite), Askama templates, SQLite EAV graph.

---

## Background

The handler, models, routes, and templates all exist and compile. The gaps are:

| # | File | Symptom |
|---|------|---------|
| 1 | `templates/meetings/detail.html:142` | Meeting agenda links 404 (wrong URL prefix) |
| 2 | `templates/agenda/detail.html` lines 113, 128, 142, 182, 200 | COA/opinion/transition links also use wrong prefix |
| 3 | `src/models/opinion/queries.rs` | Seeded opinions show empty user name and no COA (query reads entity_properties; seed stored data in relations) |

**COA property note:** Seeded COAs use property key `type` but the query reads `coa_type`, and they have no `title` property (query falls back to `''`). The `label` field on the entity IS the display title.

---

## Task 1: Fix meeting detail template link

**Files:**
- Modify: `templates/meetings/detail.html`

**Step 1: Locate the broken link**

Read `templates/meetings/detail.html`. Look for this line near the agenda points table:
```html
<td><a href="/tor/{{ tor_id }}/agenda-points/{{ point.id }}">{{ point.label }}</a></td>
```

**Step 2: Apply the fix**

Replace the `href` to use the real route:
```html
<td><a href="/tor/{{ tor_id }}/workflow/agenda/{{ point.id }}">{{ point.label }}</a></td>
```

**Step 3: Verify no other broken links in this file**

```bash
grep -n "agenda-points" templates/meetings/detail.html
```
Expected: no output (zero matches).

**Step 4: Commit**

```bash
git add templates/meetings/detail.html
git commit -m "fix(template): correct agenda point link in meeting detail"
```

---

## Task 2: Fix agenda detail template links

**Files:**
- Modify: `templates/agenda/detail.html`

The detail template has 5 broken links that use `/agenda-points/` instead of `/workflow/agenda/`. The real routes from `src/main.rs` are:

| Broken | Correct |
|--------|---------|
| `/tor/{{ tor_id }}/agenda-points/{{ agenda_point.id }}/coas` | `/tor/{{ tor_id }}/workflow/agenda/{{ agenda_point.id }}/coa/new` |
| `/tor/{{ tor_id }}/agenda-points/{{ agenda_point.id }}/coas/{{ coa.id }}` | `/tor/{{ tor_id }}/workflow/agenda/{{ agenda_point.id }}/coa/{{ coa.id }}/edit` |
| `/tor/{{ tor_id }}/agenda-points/{{ agenda_point.id }}/opinions/new` | `/tor/{{ tor_id }}/workflow/agenda/{{ agenda_point.id }}/input` |
| `/tor/{{ tor_id }}/agenda-points/{{ agenda_point.id }}/transition` | `/tor/{{ tor_id }}/workflow/agenda/{{ agenda_point.id }}/transition` |
| `/tor/{{ tor_id }}/agenda-points/{{ agenda_point.id }}/decision` | `/tor/{{ tor_id }}/workflow/agenda/{{ agenda_point.id }}/decide` |

**Step 1: Read the file**

Read `templates/agenda/detail.html` in full.

**Step 2: Fix each broken link**

Apply these five replacements (use the Edit tool for each):

1. Line ~113 — COA manage button:
   - Old: `href="/tor/{{ tor_id }}/agenda-points/{{ agenda_point.id }}/coas"`
   - New: `href="/tor/{{ tor_id }}/workflow/agenda/{{ agenda_point.id }}/coa/new"`

2. Line ~128 — COA view link:
   - Old: `href="/tor/{{ tor_id }}/agenda-points/{{ agenda_point.id }}/coas/{{ coa.id }}"`
   - New: `href="/tor/{{ tor_id }}/workflow/agenda/{{ agenda_point.id }}/coa/{{ coa.id }}/edit"`

3. Line ~142 — Record opinion button:
   - Old: `href="/tor/{{ tor_id }}/agenda-points/{{ agenda_point.id }}/opinions/new"`
   - New: `href="/tor/{{ tor_id }}/workflow/agenda/{{ agenda_point.id }}/input"`

4. Line ~182 — Transition form action:
   - Old: `action="/tor/{{ tor_id }}/agenda-points/{{ agenda_point.id }}/transition"`
   - New: `action="/tor/{{ tor_id }}/workflow/agenda/{{ agenda_point.id }}/transition"`

5. Line ~200 — Finalize decision link:
   - Old: `href="/tor/{{ tor_id }}/agenda-points/{{ agenda_point.id }}/decision"`
   - New: `href="/tor/{{ tor_id }}/workflow/agenda/{{ agenda_point.id }}/decide"`

**Step 3: Verify**

```bash
grep -n "agenda-points" templates/agenda/detail.html
```
Expected: no output.

**Step 4: Cargo check**

```bash
cargo check 2>&1 | tail -3
```
Expected: `Finished ...`

**Step 5: Commit**

```bash
git add templates/agenda/detail.html
git commit -m "fix(template): correct all agenda-point URLs in detail template"
```

---

## Task 3: Fix seeded COA property keys

**Files:**
- Modify: `data/seed/staging.json`

The two seeded COA entities use property key `type` but the query reads `coa_type`. They also have no `title` property; the query falls back to `''`. The entity `label` is the display title and should be mirrored into the `title` property.

**Step 1: Find the COA entities in staging.json**

Search for `"coa_igb_cloud_azure"` and `"coa_igb_cloud_aws"`. Each looks like:
```json
{
  "entity_type": "coa",
  "name": "coa_igb_cloud_azure",
  "label": "Adopt Azure Cloud Platform",
  "sort_order": 1,
  "properties": {
    "description": "...",
    "type": "simple"
  }
}
```

**Step 2: Update both COA property blocks**

For `coa_igb_cloud_azure`, replace the `properties` block:
```json
"properties": {
    "title": "Adopt Azure Cloud Platform",
    "description": "Migrate primary IT infrastructure to Microsoft Azure. Leverages existing M365 integration, enterprise agreement pricing, and Azure AD identity management.",
    "coa_type": "simple"
}
```

For `coa_igb_cloud_aws`, replace the `properties` block:
```json
"properties": {
    "title": "Adopt AWS Cloud Platform",
    "description": "Migrate primary IT infrastructure to Amazon Web Services. Offers broader service portfolio, stronger developer tooling, and competitive spot instance pricing.",
    "coa_type": "simple"
}
```

**Step 3: Validate JSON**

```bash
python3 -m json.tool data/seed/staging.json > /dev/null && echo OK
```
Expected: `OK`

**Step 4: Commit**

```bash
git add -f data/seed/staging.json
git commit -m "fix(seed): use coa_type and title property keys matching query expectations"
```

---

## Task 4: Extend opinion query for relation-based user and COA lookup

**Files:**
- Modify: `src/models/opinion/queries.rs` — `find_opinions_for_agenda_point` function

**Problem:** The current query reads `recorded_by_id` and `preferred_coa_id` from entity_properties. Seeded opinions never set these properties — they only have the `opinion_by` (opinion→user) and `prefers_coa` (opinion→coa) relations. The query needs to fall back to relations when properties are absent.

Note on `opinion_by` direction:
- **Seeded:** source=opinion, target=user
- **Programmatic** (`record_opinion()`): source=user, target=opinion (opposite)

The query must handle both directions.

**Step 1: Write a failing test**

Create `tests/opinion_relation_test.rs`:

```rust
use ahlt::models::{entity, relation, opinion};
mod common;

#[test]
fn test_find_opinions_via_relations_only() {
    // Simulate seeded opinions that have relations but no entity_properties
    let dir = tempfile::tempdir().unwrap();
    let conn = common::setup_test_db_at(dir.path());

    // Create ToR, agenda_point, two COAs, two users
    let tor_id = entity::create(&conn, "tor", "test_tor", "Test ToR").unwrap();
    let ap_id = entity::create(&conn, "agenda_point", "test_ap", "Test AP").unwrap();
    let coa_a_id = entity::create(&conn, "coa", "coa_a", "COA A").unwrap();
    let coa_b_id = entity::create(&conn, "coa", "coa_b", "COA B").unwrap();
    let user_alice = entity::create(&conn, "user", "alice_test", "Alice Test").unwrap();
    let user_bob = entity::create(&conn, "user", "bob_test", "Bob Test").unwrap();

    // Create opinion entities with only relations (no entity_properties for recorded_by_id etc)
    let op_alice = entity::create(&conn, "opinion", "opinion_alice_test", "Alice opinion").unwrap();
    let op_bob = entity::create(&conn, "opinion", "opinion_bob_test", "Bob opinion").unwrap();

    // opinion_on: opinion → agenda_point
    relation::create(&conn, "opinion_on", op_alice, ap_id).unwrap();
    relation::create(&conn, "opinion_on", op_bob, ap_id).unwrap();

    // opinion_by: opinion → user (seeded direction)
    relation::create(&conn, "opinion_by", op_alice, user_alice).unwrap();
    relation::create(&conn, "opinion_by", op_bob, user_bob).unwrap();

    // prefers_coa: opinion → coa
    relation::create(&conn, "prefers_coa", op_alice, coa_a_id).unwrap();
    relation::create(&conn, "prefers_coa", op_bob, coa_b_id).unwrap();

    // Query should return both opinions with correct user names and coa IDs
    let opinions = opinion::find_opinions_for_agenda_point(&conn, ap_id).unwrap();
    assert_eq!(opinions.len(), 2);

    let alice_op = opinions.iter().find(|o| o.recorded_by_name == "Alice Test");
    assert!(alice_op.is_some(), "Alice opinion not found by name");
    assert_eq!(alice_op.unwrap().preferred_coa_id, coa_a_id);

    let bob_op = opinions.iter().find(|o| o.recorded_by_name == "Bob Test");
    assert!(bob_op.is_some(), "Bob opinion not found by name");
    assert_eq!(bob_op.unwrap().preferred_coa_id, coa_b_id);
}

#[test]
fn test_find_opinions_via_entity_properties_still_works() {
    // Ensure programmatic opinions (with entity_properties) still work after query rewrite
    let dir = tempfile::tempdir().unwrap();
    let conn = common::setup_test_db_at(dir.path());

    let ap_id = entity::create(&conn, "agenda_point", "prog_ap", "Prog AP").unwrap();
    let coa_id = entity::create(&conn, "coa", "prog_coa", "Prog COA").unwrap();
    let user_id = entity::create(&conn, "user", "prog_user", "Prog User").unwrap();

    // Programmatic opinion creation
    opinion::record_opinion(&conn, ap_id, user_id, coa_id, "My commentary").unwrap();

    let opinions = opinion::find_opinions_for_agenda_point(&conn, ap_id).unwrap();
    assert_eq!(opinions.len(), 1);
    assert_eq!(opinions[0].recorded_by_name, "Prog User");
    assert_eq!(opinions[0].preferred_coa_id, coa_id);
    assert_eq!(opinions[0].commentary, "My commentary");
}
```

**Step 2: Run tests to verify they fail**

```bash
cargo test --test opinion_relation_test 2>&1 | tail -10
```
Expected: FAIL (`test_find_opinions_via_relations_only`) — Alice opinion not found.

**Step 3: Rewrite the query**

Replace the SQL in `find_opinions_for_agenda_point` (the string passed to `conn.prepare()`):

```rust
pub fn find_opinions_for_agenda_point(
    conn: &Connection,
    agenda_point_id: i64,
) -> Result<Vec<OpinionListItem>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT e.id, \
                -- User: try entity_property first, then seeded direction (op→user), then programmatic (user→op)
                COALESCE(p_by.value, \
                    CAST(r_by_seed.target_id AS TEXT), \
                    CAST(r_by_prog.source_id AS TEXT), \
                    '0') AS recorded_by_id, \
                COALESCE(u_prop.label, u_seed.label, u_prog.label, '') AS recorded_by_name, \
                -- COA: try entity_property first, then prefers_coa relation
                COALESCE(p_coa.value, \
                    CAST(r_pref.target_id AS TEXT), \
                    '0') AS preferred_coa_id, \
                -- Commentary: try commentary property, then rationale (seeded key)
                COALESCE(p_comment.value, p_rationale.value, '') AS commentary, \
                COALESCE(p_date.value, '') AS created_date \
         FROM entities e \
         JOIN relations r ON e.id = r.source_id \
         JOIN entities rt ON r.relation_type_id = rt.id AND rt.name = 'opinion_on' \
         -- entity property: recorded_by_id
         LEFT JOIN entity_properties p_by \
             ON e.id = p_by.entity_id AND p_by.key = 'recorded_by_id' \
         LEFT JOIN entities u_prop ON CAST(p_by.value AS INTEGER) = u_prop.id \
         -- relation: opinion_by seeded direction (opinion → user)
         LEFT JOIN relations r_by_seed ON r_by_seed.source_id = e.id \
             AND r_by_seed.relation_type_id = ( \
                 SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'opinion_by') \
         LEFT JOIN entities u_seed ON u_seed.id = r_by_seed.target_id AND u_seed.entity_type = 'user' \
         -- relation: opinion_by programmatic direction (user → opinion)
         LEFT JOIN relations r_by_prog ON r_by_prog.target_id = e.id \
             AND r_by_prog.relation_type_id = ( \
                 SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'opinion_by') \
         LEFT JOIN entities u_prog ON u_prog.id = r_by_prog.source_id AND u_prog.entity_type = 'user' \
         -- entity property: preferred_coa_id
         LEFT JOIN entity_properties p_coa \
             ON e.id = p_coa.entity_id AND p_coa.key = 'preferred_coa_id' \
         -- relation: prefers_coa (opinion → coa)
         LEFT JOIN relations r_pref ON r_pref.source_id = e.id \
             AND r_pref.relation_type_id = ( \
                 SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'prefers_coa') \
         -- commentary
         LEFT JOIN entity_properties p_comment \
             ON e.id = p_comment.entity_id AND p_comment.key = 'commentary' \
         LEFT JOIN entity_properties p_rationale \
             ON e.id = p_rationale.entity_id AND p_rationale.key = 'rationale' \
         LEFT JOIN entity_properties p_date \
             ON e.id = p_date.entity_id AND p_date.key = 'created_date' \
         WHERE e.entity_type = 'opinion' AND r.target_id = ?1 \
         ORDER BY COALESCE(p_date.value, '') ASC",
    ).map_err(|e| AppError::Db(e))?;

    let items = stmt
        .query_map(params![agenda_point_id], |row| {
            let recorded_by_id_str: String = row.get("recorded_by_id")?;
            let recorded_by_id: i64 = recorded_by_id_str.parse().unwrap_or(0);
            let preferred_coa_id_str: String = row.get("preferred_coa_id")?;
            let preferred_coa_id: i64 = preferred_coa_id_str.parse().unwrap_or(0);

            Ok(OpinionListItem {
                id: row.get("id")?,
                recorded_by: recorded_by_id,
                recorded_by_name: row.get("recorded_by_name")?,
                preferred_coa_id,
                commentary: row.get("commentary")?,
                created_date: row.get("created_date")?,
            })
        })
        .map_err(|e| AppError::Db(e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Db(e))?;

    Ok(items)
}
```

**Step 4: Run tests**

```bash
cargo test --test opinion_relation_test 2>&1 | tail -10
```
Expected: both tests PASS.

**Step 5: Run full test suite**

```bash
cargo test 2>&1 | tail -5
```
Expected: all tests pass (no regressions).

**Step 6: Commit**

```bash
git add src/models/opinion/queries.rs tests/opinion_relation_test.rs
git commit -m "fix(opinion): extend query to resolve user/coa from relations when entity_properties absent"
```

---

## Task 5: End-to-end verification

**Step 1: Re-seed**

```bash
rm -f data/staging/app.db
APP_ENV=staging cargo run
```

Wait for "Listening on http://127.0.0.1:8080".

**Step 2: Verify the agenda point detail page**

Log in as `admin` / `admin123`. Navigate to the IGB March meeting:
1. Go to `/tor` → IT Governance Board → March 18 meeting (confirmed)
2. Click "Select Cloud Platform for 2026" agenda point link
3. Confirm you land on `/tor/185/workflow/agenda/202` (or similar ID)

**Checklist:**
- [ ] Status: "Scheduled", Type: "Decision"
- [ ] Description: "Review and select primary cloud platform..."
- [ ] COAs section shows 2 entries: "Adopt Azure Cloud Platform", "Adopt AWS Cloud Platform"
- [ ] Opinions section shows Alice (prefers Azure) and Henry (prefers AWS)
- [ ] "Record Opinion" button visible (if logged in with agenda.participate permission)

**Step 3: Commit any tweaks**

If any issues found, fix and commit before marking done.

---

## Status Coverage

| Gap | Fix | Task |
|-----|-----|------|
| Meeting detail → wrong URL | Template fix | Task 1 |
| Agenda detail → 5 wrong URLs | Template fix | Task 2 |
| Seeded COA property keys | staging.json fix | Task 3 |
| Seeded opinions invisible | Query rewrite with relation fallback | Task 4 |
