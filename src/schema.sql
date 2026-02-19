
CREATE TABLE IF NOT EXISTS entities (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_type TEXT NOT NULL,
    name        TEXT NOT NULL,
    label       TEXT NOT NULL,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    is_active   INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S','now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S','now')),
    UNIQUE(entity_type, name)
);

CREATE TABLE IF NOT EXISTS entity_properties (
    entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    key       TEXT NOT NULL,
    value     TEXT NOT NULL,
    PRIMARY KEY (entity_id, key)
);

CREATE TABLE IF NOT EXISTS relations (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    relation_type_id INTEGER NOT NULL REFERENCES entities(id),
    source_id        INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    target_id        INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    created_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S','now')),
    UNIQUE(relation_type_id, source_id, target_id)
);

CREATE TABLE IF NOT EXISTS relation_properties (
    relation_id INTEGER NOT NULL,
    key         TEXT NOT NULL,
    value       TEXT NOT NULL,
    PRIMARY KEY (relation_id, key),
    FOREIGN KEY (relation_id) REFERENCES relations(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(entity_type);
CREATE INDEX IF NOT EXISTS idx_relations_source ON relations(source_id, relation_type_id);
CREATE INDEX IF NOT EXISTS idx_relations_target ON relations(target_id, relation_type_id);
CREATE INDEX IF NOT EXISTS idx_properties_entity ON entity_properties(entity_id);
CREATE INDEX IF NOT EXISTS idx_properties_entity_key ON entity_properties(entity_id, key);
