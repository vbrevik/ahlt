# Navigation Permissions via Relations Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Convert nav_item permission checks from text property (`permission_code`) to EAV relations (`requires_permission` nav_item→permission), making the ontology consistent and graph-visible.

**Architecture:** Currently nav items store permission requirements as a `permission_code` text property, and runtime code does string matching. This refactoring creates a `requires_permission` relation type and links nav_item entities to permission entities via the relations table, making permissions part of the living ontology graph. Nav query logic will join on relations instead of property lookup.

**Tech Stack:** Rust, rusqlite, SQLite EAV schema (entities + entity_properties + relations)

---

## Task 1: Create `requires_permission` relation type

**Files:**
- Modify: `src/db/seed.rs` (relation type creation section)
- Test: Manual DB inspection after re-seed

**Step 1: Add relation type to seed data**

In `src/db/seed.rs`, find the section that creates relation types (look for `INSERT INTO entities` with `entity_type='relation_type'`). Add the new relation type after `has_permission`:

```rust
// After the has_permission relation type insert
conn.execute(
    "INSERT INTO entities (entity_type, name, label, sort_order, is_active)
     VALUES ('relation_type', 'requires_permission', 'Requires Permission', 3, 1)",
    [],
)?;
```

**Step 2: Delete and re-seed database**

```bash
rm -f data/app.db
cargo run
```

Expected: Server starts, seeds database with new relation type

**Step 3: Verify relation type exists**

```bash
sqlite3 data/app.db "SELECT id, name, label FROM entities WHERE entity_type='relation_type' ORDER BY id"
```

Expected output should include:
```
1|has_role|Has Role
2|has_permission|Has Permission
3|requires_permission|Requires Permission
```

**Step 4: Commit**

```bash
git add src/db/seed.rs
git commit -m "feat(nav): add requires_permission relation type

Create new relation type entity for nav_item→permission relationships.
Part of task 2.3 - converting nav permissions from properties to relations."
```

---

## Task 2: Create nav→permission relations in seed data

**Files:**
- Modify: `src/db/seed.rs` (nav item creation section)

**Step 1: Find permission IDs for seed relations**

In `src/db/seed.rs`, after creating permission entities, add queries to get permission IDs for later use:

```rust
// After creating all permission entities
let dashboard_view_perm_id: i64 = conn.query_row(
    "SELECT id FROM entities WHERE entity_type='permission' AND name='dashboard.view'",
    [],
    |row| row.get(0),
)?;

let users_list_perm_id: i64 = conn.query_row(
    "SELECT id FROM entities WHERE entity_type='permission' AND name='users.list'",
    [],
    |row| row.get(0),
)?;

let roles_manage_perm_id: i64 = conn.query_row(
    "SELECT id FROM entities WHERE entity_type='permission' AND name='roles.manage'",
    [],
    |row| row.get(0),
)?;

let settings_manage_perm_id: i64 = conn.query_row(
    "SELECT id FROM entities WHERE entity_type='permission' AND name='settings.manage'",
    [],
    |row| row.get(0),
)?;
```

**Step 2: Get relation type ID**

```rust
let requires_permission_rel_type_id: i64 = conn.query_row(
    "SELECT id FROM entities WHERE entity_type='relation_type' AND name='requires_permission'",
    [],
    |row| row.get(0),
)?;
```

**Step 3: Create relations for nav items**

After creating all nav_item entities and before the final Ok(()), add:

```rust
// Get nav item IDs
let dashboard_nav_id: i64 = conn.query_row(
    "SELECT id FROM entities WHERE entity_type='nav_item' AND name='dashboard'",
    [],
    |row| row.get(0),
)?;

let admin_users_nav_id: i64 = conn.query_row(
    "SELECT id FROM entities WHERE entity_type='nav_item' AND name='admin.users'",
    [],
    |row| row.get(0),
)?;

let admin_roles_nav_id: i64 = conn.query_row(
    "SELECT id FROM entities WHERE entity_type='nav_item' AND name='admin.roles'",
    [],
    |row| row.get(0),
)?;

let admin_ontology_nav_id: i64 = conn.query_row(
    "SELECT id FROM entities WHERE entity_type='nav_item' AND name='admin.ontology'",
    [],
    |row| row.get(0),
)?;

let admin_settings_nav_id: i64 = conn.query_row(
    "SELECT id FROM entities WHERE entity_type='nav_item' AND name='admin.settings'",
    [],
    |row| row.get(0),
)?;

// Create nav→permission relations
// Dashboard requires dashboard.view
conn.execute(
    "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?, ?, ?)",
    [requires_permission_rel_type_id, dashboard_nav_id, dashboard_view_perm_id],
)?;

// Admin > Users requires users.list
conn.execute(
    "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?, ?, ?)",
    [requires_permission_rel_type_id, admin_users_nav_id, users_list_perm_id],
)?;

// Admin > Roles requires roles.manage
conn.execute(
    "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?, ?, ?)",
    [requires_permission_rel_type_id, admin_roles_nav_id, roles_manage_perm_id],
)?;

// Admin > Ontology requires settings.manage
conn.execute(
    "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?, ?, ?)",
    [requires_permission_rel_type_id, admin_ontology_nav_id, settings_manage_perm_id],
)?;

// Admin > Settings requires settings.manage
conn.execute(
    "INSERT INTO relations (relation_type_id, source_id, target_id) VALUES (?, ?, ?)",
    [requires_permission_rel_type_id, admin_settings_nav_id, settings_manage_perm_id],
)?;
```

**Step 4: Re-seed and verify relations**

```bash
rm -f data/app.db
cargo run
```

Expected: Server starts successfully

**Step 5: Verify relations in database**

```bash
sqlite3 data/app.db "SELECT
  e1.name as nav_item,
  e2.name as permission
FROM relations r
JOIN entities e1 ON r.source_id = e1.id
JOIN entities e2 ON r.target_id = e2.id
JOIN entities rt ON r.relation_type_id = rt.id
WHERE rt.name = 'requires_permission'
ORDER BY e1.name"
```

Expected output:
```
admin.ontology|settings.manage
admin.roles|roles.manage
admin.settings|settings.manage
admin.users|users.list
dashboard|dashboard.view
```

**Step 6: Commit**

```bash
git add src/db/seed.rs
git commit -m "feat(nav): create nav→permission relations in seed data

Add requires_permission relations linking nav_item entities to permission
entities. This replaces the permission_code text properties with proper
ontology relations.

Part of task 2.3 - converting nav permissions from properties to relations."
```

---

## Task 3: Update nav model to query relations

**Files:**
- Modify: `src/models/nav.rs` (query logic)

**Step 1: Read current nav model implementation**

```bash
cat src/models/nav.rs
```

Understand the current query that uses `permission_code` property.

**Step 2: Update query to join on relations**

Find the query in `nav.rs` that fetches nav items and their permission codes. Replace the LEFT JOIN on entity_properties for permission_code with a relation-based join:

Old pattern (property-based):
```sql
LEFT JOIN entity_properties ep_perm
  ON e.id = ep_perm.entity_id AND ep_perm.key = 'permission_code'
```

New pattern (relation-based):
```sql
-- Join to get required permission via relation
LEFT JOIN relations r_perm
  ON e.id = r_perm.source_id
  AND r_perm.relation_type_id = (
    SELECT id FROM entities
    WHERE entity_type = 'relation_type' AND name = 'requires_permission'
  )
LEFT JOIN entities perm
  ON r_perm.target_id = perm.id AND perm.entity_type = 'permission'
```

Update the SELECT to use `perm.name` instead of `ep_perm.value`.

**Step 3: Test the query manually**

```bash
sqlite3 data/app.db
```

```sql
SELECT
  e.id,
  e.name,
  e.label,
  ep_url.value as url,
  ep_parent.value as parent,
  perm.name as permission_code
FROM entities e
LEFT JOIN entity_properties ep_url
  ON e.id = ep_url.entity_id AND ep_url.key = 'url'
LEFT JOIN entity_properties ep_parent
  ON e.id = ep_parent.entity_id AND ep_parent.key = 'parent'
LEFT JOIN relations r_perm
  ON e.id = r_perm.source_id
  AND r_perm.relation_type_id = (
    SELECT id FROM entities
    WHERE entity_type = 'relation_type' AND name = 'requires_permission'
  )
LEFT JOIN entities perm
  ON r_perm.target_id = perm.id AND perm.entity_type = 'permission'
WHERE e.entity_type = 'nav_item' AND e.is_active = 1
ORDER BY e.sort_order, e.id;
```

Expected: Should return all nav items with their permission codes from relations.

**Step 4: Update Rust code**

Replace the query in `src/models/nav.rs`. The exact location will depend on the function (likely `find_all()` or similar). The result row parsing should remain the same since we're still selecting `permission_code` (just from a different source).

**Step 5: Build and test**

```bash
cargo build
```

Expected: Compiles successfully

**Step 6: Manual test in browser**

```bash
cargo run
```

Visit `http://localhost:8080` (after login), verify navigation still appears correctly with proper permission filtering.

**Step 7: Commit**

```bash
git add src/models/nav.rs
git commit -m "refactor(nav): query permissions via relations instead of properties

Update nav model to join on relations table instead of entity_properties
for permission checks. Nav items now link to permissions through
requires_permission relations.

Part of task 2.3 - converting nav permissions from properties to relations."
```

---

## Task 4: Remove permission_code properties from seed data

**Files:**
- Modify: `src/db/seed.rs` (nav item property creation)

**Step 1: Remove permission_code property inserts**

In `src/db/seed.rs`, find all the `INSERT INTO entity_properties` statements that create `permission_code` properties for nav items. Comment them out or delete them.

Example of what to remove:
```rust
// DELETE these lines (or similar):
conn.execute(
    "INSERT INTO entity_properties (entity_id, key, value) VALUES (?, 'permission_code', 'dashboard.view')",
    [dashboard_nav_id],
)?;
```

**Step 2: Re-seed database**

```bash
rm -f data/app.db
cargo run
```

Expected: Server starts successfully

**Step 3: Verify no permission_code properties exist**

```bash
sqlite3 data/app.db "SELECT COUNT(*) FROM entity_properties ep
JOIN entities e ON ep.entity_id = e.id
WHERE e.entity_type = 'nav_item' AND ep.key = 'permission_code'"
```

Expected output: `0`

**Step 4: Verify nav still works**

Visit `http://localhost:8080`, login, verify navigation appears and permission filtering works.

**Step 5: Commit**

```bash
git add src/db/seed.rs
git commit -m "refactor(nav): remove permission_code properties from seed data

Nav items no longer use permission_code properties. Permissions are now
linked via requires_permission relations only.

Part of task 2.3 - converting nav permissions from properties to relations."
```

---

## Task 5: Verify ontology graph shows relations

**Files:**
- Test: Manual browser inspection of ontology explorer

**Step 1: Start server**

```bash
cargo run
```

**Step 2: Check Concepts tab**

1. Visit `http://localhost:8080/ontology`
2. Verify the schema graph shows a `requires_permission` edge between `nav_item` and `permission` nodes

**Step 3: Check Data tab**

1. Click "Data" tab (`http://localhost:8080/ontology/data`)
2. Enable "nav_item" and "permission" filters
3. Verify you can see individual nav_item nodes connected to permission nodes with `requires_permission` edges
4. Click a nav_item node and verify the detail panel shows outgoing `requires_permission` relations

**Step 4: Test a specific nav item**

Click on "admin.users" nav_item node. Verify it shows:
- Outgoing relation: `requires_permission → users.list`

**Step 5: Document success**

If all checks pass, the ontology refactoring is complete. Nav permissions are now visible in the graph and consistent with the EAV model.

---

## Task 6: Update documentation

**Files:**
- Modify: `docs/BACKLOG.md`

**Step 1: Move task 2.3 to completed**

In BACKLOG.md, move "2.3 — Navigation permissions via relations" from "Remaining Backlog" to "Completed Work" section under "Epic 2: Data-Driven Navigation".

Format:
```markdown
### Epic 2: Data-Driven Navigation (continued)
- 2.3 Nav permissions via relations: converted nav_item permission checks from `permission_code` text properties to `requires_permission` relations (nav_item→permission), making permissions visible in ontology graph and consistent with EAV model
```

**Step 2: Update "Relations in Use" table**

Remove the "*(planned for 2.3)*" note from the `requires_permission` entry.

**Step 3: Update "Implementation Order"**

Remove "2.3 Nav perms via relations" from NEXT column, move "6.4 Search/filter" to NEXT.

**Step 4: Commit**

```bash
git add docs/BACKLOG.md
git commit -m "docs: mark task 2.3 (nav permissions via relations) as complete

Updated backlog to reflect completion of ontology refactoring."
```

---

## Final Verification

**Complete checklist:**
- [ ] `requires_permission` relation type exists in database
- [ ] Nav items linked to permissions via relations
- [ ] Nav model queries relations instead of properties
- [ ] No `permission_code` properties in entity_properties for nav items
- [ ] Navigation works correctly in browser (permission filtering)
- [ ] Ontology Concepts tab shows `requires_permission` edge in schema
- [ ] Ontology Data tab shows nav→permission connections
- [ ] BACKLOG.md updated with completed work
- [ ] All commits pushed

**Testing:**
1. Delete database and re-seed: `rm data/app.db && cargo run`
2. Login as admin user
3. Verify all nav items appear
4. Visit ontology explorer and verify graph shows relations
5. Verify navigation permission filtering (admin sees all, non-admin sees limited)

---

## Rollback Plan

If issues arise, revert commits in reverse order:
```bash
git log --oneline  # find commit hashes
git revert <hash>  # for each commit, newest first
```

Or hard reset:
```bash
git reset --hard <commit-before-task-2.3>
rm data/app.db
cargo run
```
