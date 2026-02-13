-- Seed script for testing refactored CRUD handlers
-- Password for all users: "password123"
-- Hash generated with: argon2 (bcrypt-compatible)

-- Admin user already exists from init, let's add more roles and users

-- Insert new roles
INSERT OR IGNORE INTO entities (entity_type, name, label, sort_order, is_active) VALUES
('role', 'editor', 'Editor', 2, 1),
('role', 'viewer', 'Viewer', 3, 1),
('role', 'manager', 'Manager', 4, 1),
('role', 'analyst', 'Analyst', 5, 1);

-- Add role descriptions
INSERT OR REPLACE INTO entity_properties (entity_id, key, value)
SELECT e.id, 'description', 'Can edit content and manage users'
FROM entities e WHERE e.entity_type = 'role' AND e.name = 'editor';

INSERT OR REPLACE INTO entity_properties (entity_id, key, value)
SELECT e.id, 'description', 'Read-only access to view content'
FROM entities e WHERE e.entity_type = 'role' AND e.name = 'viewer';

INSERT OR REPLACE INTO entity_properties (entity_id, key, value)
SELECT e.id, 'description', 'Can manage projects and teams'
FROM entities e WHERE e.entity_type = 'role' AND e.name = 'manager';

INSERT OR REPLACE INTO entity_properties (entity_id, key, value)
SELECT e.id, 'description', 'Can view reports and analytics'
FROM entities e WHERE e.entity_type = 'role' AND e.name = 'analyst';

-- Assign permissions to new roles

-- Editor: users.list, users.create, users.edit, settings.manage
INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id)
SELECT
    (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission'),
    e.id,
    p.id
FROM entities e
CROSS JOIN entities p
WHERE e.entity_type = 'role' AND e.name = 'editor'
  AND p.entity_type = 'permission'
  AND p.name IN ('users.list', 'users.create', 'users.edit', 'settings.manage');

-- Viewer: dashboard.view, users.list
INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id)
SELECT
    (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission'),
    e.id,
    p.id
FROM entities e
CROSS JOIN entities p
WHERE e.entity_type = 'role' AND e.name = 'viewer'
  AND p.entity_type = 'permission'
  AND p.name IN ('dashboard.view', 'users.list');

-- Manager: all except roles.manage
INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id)
SELECT
    (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission'),
    e.id,
    p.id
FROM entities e
CROSS JOIN entities p
WHERE e.entity_type = 'role' AND e.name = 'manager'
  AND p.entity_type = 'permission'
  AND p.name IN ('dashboard.view', 'users.list', 'users.create', 'users.edit', 'users.delete', 'settings.manage', 'audit.view');

-- Analyst: dashboard.view, audit.view, settings.manage
INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id)
SELECT
    (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_permission'),
    e.id,
    p.id
FROM entities e
CROSS JOIN entities p
WHERE e.entity_type = 'role' AND e.name = 'analyst'
  AND p.entity_type = 'permission'
  AND p.name IN ('dashboard.view', 'audit.view', 'settings.manage');

-- Insert new users
INSERT OR IGNORE INTO entities (entity_type, name, label, sort_order, is_active) VALUES
('user', 'alice', 'Alice Johnson', 1, 1),
('user', 'bob', 'Bob Smith', 2, 1),
('user', 'carol', 'Carol Davis', 3, 1),
('user', 'david', 'David Wilson', 4, 1),
('user', 'emma', 'Emma Brown', 5, 1);

-- Add user properties (email and password)
-- Password hash for "password123": $argon2id$v=19$m=19456,t=2,p=1$...
-- Note: Using a fixed hash for testing. In production, hash should be unique per user.

INSERT OR REPLACE INTO entity_properties (entity_id, key, value)
SELECT e.id, 'email', e.name || '@example.com'
FROM entities e WHERE e.entity_type = 'user' AND e.name IN ('alice', 'bob', 'carol', 'david', 'emma');

-- Password hash for "password123" generated with argon2
INSERT OR REPLACE INTO entity_properties (entity_id, key, value)
SELECT e.id, 'password', '$argon2id$v=19$m=19456,t=2,p=1$krDsMYt9ECIPhVLyEonHZQ$5lIg+9EYqRK6m4HNi92ouxBjAEoLp44I59zbSoYJ2to'
FROM entities e WHERE e.entity_type = 'user' AND e.name IN ('alice', 'bob', 'carol', 'david', 'emma');

-- Assign roles to users
-- Alice -> Editor
INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id)
SELECT
    (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role'),
    u.id,
    r.id
FROM entities u, entities r
WHERE u.entity_type = 'user' AND u.name = 'alice'
  AND r.entity_type = 'role' AND r.name = 'editor';

-- Bob -> Viewer
INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id)
SELECT
    (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role'),
    u.id,
    r.id
FROM entities u, entities r
WHERE u.entity_type = 'user' AND u.name = 'bob'
  AND r.entity_type = 'role' AND r.name = 'viewer';

-- Carol -> Manager
INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id)
SELECT
    (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role'),
    u.id,
    r.id
FROM entities u, entities r
WHERE u.entity_type = 'user' AND u.name = 'carol'
  AND r.entity_type = 'role' AND r.name = 'manager';

-- David -> Analyst
INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id)
SELECT
    (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role'),
    u.id,
    r.id
FROM entities u, entities r
WHERE u.entity_type = 'user' AND u.name = 'david'
  AND r.entity_type = 'role' AND r.name = 'analyst';

-- Emma -> Editor
INSERT OR IGNORE INTO relations (relation_type_id, source_id, target_id)
SELECT
    (SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'has_role'),
    u.id,
    r.id
FROM entities u, entities r
WHERE u.entity_type = 'user' AND u.name = 'emma'
  AND r.entity_type = 'role' AND r.name = 'editor';
