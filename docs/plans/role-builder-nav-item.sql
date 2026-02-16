-- Add Role Builder nav item
-- Run this SQL to add "Role Builder" to the navigation menu

-- Step 1: Create the nav item entity
INSERT INTO entities (entity_type, name, label)
VALUES ('nav_item', 'role_builder', 'Role Builder');

-- Step 2: Get the nav item ID (it will be the last inserted ID)
-- You'll need to replace ? with the actual ID from the previous insert

-- Step 3: Add properties for the nav item
-- Replace ? with the actual nav item ID
INSERT INTO entity_properties (entity_id, key, value)
VALUES
  (?, 'path', '/roles/builder'),
  (?, 'permission_required', 'admin.roles'),
  (?, 'position', '2');

-- Step 4: Link to admin module
-- First, find the admin module ID:
SELECT id FROM entities WHERE entity_type='nav_module' AND name='admin';

-- Then, get the in_module relation type ID:
SELECT id FROM entities WHERE entity_type='relation_type' AND name='in_module';

-- Finally, create the relation (replace ?1 with relation_type id, ?2 with nav_item id, ?3 with module id):
INSERT INTO relations (relation_type_id, source_id, target_id)
VALUES (?, ?, ?);

-- Complete SQL (all in one, using a transaction):
-- Replace the SELECT queries with actual IDs from your database

BEGIN TRANSACTION;

INSERT INTO entities (entity_type, name, label)
VALUES ('nav_item', 'role_builder', 'Role Builder');

-- Get the last inserted ID
-- In SQLite, use last_insert_rowid()

INSERT INTO entity_properties (entity_id, key, value)
SELECT last_insert_rowid(), 'path', '/roles/builder'
UNION ALL
SELECT last_insert_rowid(), 'permission_required', 'admin.roles'
UNION ALL
SELECT last_insert_rowid(), 'position', '2';

INSERT INTO relations (relation_type_id, source_id, target_id)
SELECT
  (SELECT id FROM entities WHERE entity_type='relation_type' AND name='in_module'),
  last_insert_rowid(),
  (SELECT id FROM entities WHERE entity_type='nav_module' AND name='admin');

COMMIT;
