# Data Manager — Import/Export/JSON-LD Design

**Date**: 2026-02-18
**Status**: Approved

## Problem

Every seed data change requires editing Rust code in `src/db.rs`, recompiling, deleting the database, and restarting the server. This is a recurring friction point during development. Additionally, there is no way for admins to bulk-import or export data at runtime.

## Solution

A REST API (`/api/data/import`, `/api/data/export`, `/api/data/schema`) plus an Admin UI page (`/data-manager`) that supports three formats: native JSON, JSON-LD (RDF triples), and SQL export.

## Data Model Mapping (EAV <-> RDF)

| EAV Concept | RDF Triple | Example |
|---|---|---|
| Entity property | `<entity IRI> <prop predicate> "literal"` | `<ahlt:tor/budget_committee> ahlt:status "active"` |
| Relation | `<source IRI> <rel predicate> <target IRI>` | `<ahlt:user/charlie> ahlt:fills_position <ahlt:tor_function/bc_chair>` |
| Entity metadata | `<entity IRI> rdf:type <class>` | `<ahlt:tor/budget_committee> rdf:type ahlt:Tor` |

**IRI scheme**: `ahlt:{entity_type}/{name}` — deterministic from `entity_type` + `name`.

**JSON-LD `@context`** maps short keys to full IRIs:

```jsonld
{
  "@context": {
    "ahlt": "http://ahlt.local/ontology/",
    "status": "ahlt:status",
    "description": "ahlt:description",
    "fills_position": "ahlt:fills_position"
  },
  "@graph": [
    {
      "@id": "ahlt:tor/budget_committee",
      "@type": "ahlt:Tor",
      "ahlt:label": "Budget Committee",
      "ahlt:status": "active"
    }
  ]
}
```

## API Endpoints

### POST `/api/data/import`

Accepts native JSON or JSON-LD (detected by presence of `@context`/`@graph`).

**Native format:**
```json
{
  "conflict_mode": "skip" | "upsert" | "fail",
  "entities": [
    {
      "entity_type": "tor",
      "name": "budget_committee",
      "label": "Budget Committee",
      "sort_order": 1,
      "properties": { "status": "active", "meeting_cadence": "monthly" }
    }
  ],
  "relations": [
    {
      "relation_type": "belongs_to_tor",
      "source": "tor_function:bc_chair",
      "target": "tor:budget_committee"
    }
  ]
}
```

**JSON-LD format:**
```jsonld
{
  "@context": { ... },
  "@graph": [ ... ],
  "ahlt:conflict_mode": "upsert"
}
```

**Processing order**: Entities first (all inserted/resolved), then relations (which reference entities by `type:name`).

**Response:**
```json
{
  "created": 12,
  "updated": 3,
  "skipped": 2,
  "errors": [
    {
      "item": { "entity_type": "tor", "name": "xyz", ... },
      "reason": "missing relation target tor:nonexistent"
    }
  ]
}
```

### GET `/api/data/export`

Query params:
- `format=json` (default) | `format=jsonld` | `format=sql`
- `types=tor,tor_function,suggestion` (optional comma-separated filter)

### GET `/api/data/schema`

Returns the JSON-LD `@context` document — vocabulary of all property keys and relation types in the DB.

## Admin UI

Page at `/data-manager`, nav item `admin.data_manager` under Admin module, requires `settings.manage`.

### Import Panel
- File upload (drag-and-drop) for `.json` / `.jsonld`
- Conflict mode dropdown: Skip / Upsert / Fail
- Import button
- Result summary card: created/updated/skipped/error counts

### Export Panel
- Entity type filter: multi-select checkboxes
- Format radio: JSON / JSON-LD / SQL
- Export button triggers file download

### Error Mitigation
Failed items appear in an error table with per-item actions:
- **Skip** — remove from pending set
- **Edit** — inline JSON editor, pre-filled with failing entity. Fix and retry.
- **Force upsert** — override conflict mode for this item only

Batch controls:
- Skip all errors
- Retry all (re-submits failed subset)
- Change conflict mode and retry

Client holds failed items in JS state; retry sends only the failed subset back to the import API. No server-side session state.

## Architecture

### New Files
| File | Purpose |
|---|---|
| `src/handlers/data_handlers.rs` | Import/export/schema HTTP handlers |
| `src/models/data_manager/mod.rs` | Module root |
| `src/models/data_manager/import.rs` | Parse JSON/JSON-LD, conflict resolution, batch insert |
| `src/models/data_manager/export.rs` | Query full graph, serialize to JSON/JSON-LD/SQL |
| `src/models/data_manager/jsonld.rs` | JSON-LD <-> EAV: @context builder, IRI scheme, triple mapping |
| `templates/admin/data_manager.html` | Admin UI template |

### Modified Files
| File | Change |
|---|---|
| `src/main.rs` | Register routes |
| `src/models/mod.rs` | `pub mod data_manager` |
| `src/handlers/mod.rs` | `pub mod data_handlers` |
| `src/db.rs` | Nav item + permission relation in `seed_ontology()` |
| `static/css/style.css` | Error table / result card styles if needed |

### No External Dependencies
- JSON-LD handled with `serde_json` (we control both sides of the vocabulary)
- SQL export is string formatting of INSERT statements

### Security
- All endpoints gated by `settings.manage` permission
- CSRF on POST `/api/data/import`
- Entity type validation against known types in DB
- SQL export is read-only output, never executed by the server
