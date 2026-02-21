-- Initial PostgreSQL schema for im-ctrl (ahlt)
-- Converted from SQLite schema.sql

CREATE TABLE entities (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    entity_type TEXT NOT NULL,
    name        TEXT NOT NULL,
    label       TEXT NOT NULL,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    is_active   BOOLEAN NOT NULL DEFAULT TRUE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_type, name)
);

CREATE TABLE entity_properties (
    entity_id BIGINT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    key       TEXT NOT NULL,
    value     TEXT NOT NULL,
    PRIMARY KEY (entity_id, key)
);

CREATE TABLE relations (
    id               BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    relation_type_id BIGINT NOT NULL REFERENCES entities(id),
    source_id        BIGINT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    target_id        BIGINT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(relation_type_id, source_id, target_id)
);

CREATE TABLE relation_properties (
    relation_id BIGINT NOT NULL REFERENCES relations(id) ON DELETE CASCADE,
    key         TEXT NOT NULL,
    value       TEXT NOT NULL,
    PRIMARY KEY (relation_id, key)
);

CREATE INDEX idx_entities_type ON entities(entity_type);
CREATE INDEX idx_relations_source ON relations(source_id, relation_type_id);
CREATE INDEX idx_relations_target ON relations(target_id, relation_type_id);
CREATE INDEX idx_properties_entity ON entity_properties(entity_id);
CREATE INDEX idx_properties_entity_key ON entity_properties(entity_id, key);
