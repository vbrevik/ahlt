# Ontology Graph View Redesign

**Date**: 2026-02-21
**Status**: Approved
**Approach**: Incremental enhancement of existing templates (no new files)

## Goals

1. Make filters more intuitive with search + entity type + relation type filtering
2. Add right-click context menu for navigating relations between entities
3. Add ego network "Focus on this node" view
4. Connect schema graph to instance graph via click-through navigation
5. Preserve node positions when toggling filters (no full re-render)

## Scope

**In scope**: `templates/ontology/graph.html`, `templates/ontology/data.html`, `static/css/style.css`
**Out of scope**: Backend API changes (existing endpoints provide all needed data), handler changes (except reading `?type=` query param in JS)

---

## Feature 1: Search + Filter Combo (Instance Graph)

### Current State
- Checkbox chips in `.graph-controls` panel (top-left)
- Only entity type filters, no relation type filters
- Toggling a filter destroys and rebuilds the entire SVG + simulation
- No search capability

### New Design

```
┌─────────────────────────────────┐
│ [🔍 Search entities...        ] │
│                                 │
│ ENTITY TYPES                    │
│ [x] user (12)   [x] role (3)   │
│ [x] permission (45)            │
│ [ ] nav_item (28)              │
│                                 │
│ RELATION TYPES                  │
│ [x] has_role     [x] has_perm   │
│ [ ] requires     [x] parent_of  │
│                                 │
│ [→] Show arrows  [⟳] Reset all  │
└─────────────────────────────────┘
```

### Behavior

**Search box**:
- Text input above filter sections
- Typing highlights matching nodes (by `name` or `label`, case-insensitive substring) with a pulsing CSS ring animation
- Non-matching visible nodes dim to `opacity: 0.2`
- Clearing restores all nodes to full opacity
- Search composes with type filters — only searches among currently visible nodes

**Entity type filters**:
- Checkbox chips with colored dot + type name + count (computed client-side from nodes array)
- Toggle = `display: none` on node `<g>` groups and connected edges. **No simulation restart.**
- Positions fully preserved across filter toggles
- URL param support: `?type=role` pre-selects only that type on page load

**Relation type filters**:
- New section below entity types
- Checkbox chips per unique relation type (computed from edges array)
- Unchecking hides those edges (and their labels/arrows). Source/target nodes stay visible.

**Arrow toggle**: Show/hide directional arrow markers on all edges

**Reset all**: Restores all checkboxes to checked, clears search input, removes URL params

---

## Feature 2: Right-Click Context Menu

### Trigger
- Right-click on any entity node (both schema and instance graphs)
- Suppresses browser default context menu on graph nodes only

### Instance Graph Menu Layout

```
┌──────────────────────────────┐
│  John (user)                 │  <- Header: label + colored type badge
│  ─────────────────────────── │
│  ◉ Focus on this node        │  <- Ego network filter
│  ↗ Open full detail          │  <- Navigate to /ontology/data/{id}
│  ─────────────────────────── │
│  ▸ has_role (2)              │  <- Collapsible relation group
│      Admin            →      │  <- Click: center + open sidebar
│      Editor           →      │
│  ▸ has_permission (5)        │
│      users.view       →      │
│      users.edit       →      │
│      roles.view       →      │
│      ...2 more               │  <- Truncate at 5, show overflow count
└──────────────────────────────┘
```

### Schema Graph Menu Layout

```
┌──────────────────────────────┐
│  role (3 instances)          │
│  ─────────────────────────── │
│  View instances →            │  <- Navigate to /ontology/data?type=role
│  ─────────────────────────── │
│  ▸ Relation types:           │
│      has_permission (12)     │
│      has_role (3)            │
└──────────────────────────────┘
```

### Interaction Behavior

- **Clicking a connected entity**: Pan/zoom to center that node in the viewport + open its detail in the sidebar panel
- **"Open full detail"**: Navigate to `/ontology/data/{id}` (full page)
- **"View instances"** (schema): Navigate to `/ontology/data?type={type}`
- **"Focus on this node"**: Activate ego network view (Feature 3)
- **Dismissal**: Click outside menu, press Escape, or right-click elsewhere
- **Positioning**: Appears at cursor. Flips horizontally/vertically if near viewport edges.
- **Relation groups**: Open by default if ≤5 items total in group; collapsed if >5

### Implementation

- Pure DOM element (`<div>`) positioned absolutely within `.graph-container`
- Built with `createElement`/`textContent`/`appendChild` (no innerHTML per security rules)
- Z-index above SVG layer
- Connected entities data sourced from the existing edges array (filter by source/target matching node ID)

---

## Feature 3: Ego Network Focus

### Trigger
- "Focus on this node" action in context menu

### Behavior
1. Hide all nodes except the selected node and its direct neighbors (1-hop via edges)
2. Hide all edges except those connecting to/from the selected node
3. Show a floating **"Clear focus: {node label} ✕"** pill button at top-center of graph
4. Composable with type/relation filters — focus respects current filter state
5. Clicking "Clear focus" restores pre-focus visibility state
6. Centering animation on the focused node after applying the filter

### Visual Treatment
- Focused node gets a subtle highlight ring (thicker stroke, glow)
- Neighbors retain normal styling
- The "Clear focus" pill uses existing `.graph-toolbar` styling patterns

---

## Feature 4: Schema → Instance Click-Through

### Current State
- Schema type nodes open detail sidebar on click
- No way to drill down to instance-level view

### New Behavior
- **Left-click** on schema type node: Opens detail sidebar (existing, preserved)
- **Right-click** on schema type node: Context menu with "View instances →"
- "View instances" navigates to `/ontology/data?type={entity_type}`

### Instance Graph URL Param Support
- On page load, JS reads `URLSearchParams` for `type` parameter
- If present, only that entity type checkbox is checked; all others unchecked
- This creates a pre-filtered view showing only the requested type and its direct connections
- The filter panel reflects this state (user can then manually check other types)

---

## CSS Additions

All new styles go in `static/css/style.css` in the existing graph section (~lines 1728-1979).

### New Classes

| Class | Purpose |
|-------|---------|
| `.graph-search` | Search input in filter panel |
| `.graph-search__input` | The text input element |
| `.node--highlighted` | Pulsing ring on search match |
| `.node--dimmed` | Reduced opacity for non-matches |
| `.context-menu` | Positioned absolute container |
| `.context-menu__header` | Node label + type badge |
| `.context-menu__divider` | Horizontal separator line |
| `.context-menu__action` | Clickable action row (focus, detail link) |
| `.context-menu__group` | Collapsible relation type section |
| `.context-menu__group-header` | Relation type name + count |
| `.context-menu__item` | Connected entity row |
| `.context-menu__overflow` | "...N more" truncation text |
| `.focus-pill` | "Clear focus" floating button |
| `.filter-section__title` | "ENTITY TYPES" / "RELATION TYPES" headings |
| `.filter-actions` | Container for arrow toggle + reset button |

### BEM Naming
Following project convention for new CSS classes. Existing `.filter-chip` / `.chip-dot` / `.chip-label` classes are reused for both entity and relation type checkboxes.

---

## Data Dependencies

All data needed is already provided by existing API endpoints:

| Feature | Data Source | Notes |
|---------|-------------|-------|
| Entity type counts | `nodes` array | Count by `entity_type` client-side |
| Relation types list | `edges` array | Unique `relation_type` values |
| Context menu relations | `edges` array | Filter by `source`/`target` matching node ID |
| Ego network neighbors | `edges` array | 1-hop neighbor set from edges |

**No backend API changes required.**

---

## Non-Goals

- No mobile/touch-specific context menu (long-press) — desktop-first for graph views
- No drag-and-drop rearrangement of filter chips
- No saved filter presets
- No changes to the Reference tab (`concepts.html`)
- No changes to entity detail page (`detail.html`)
- No extraction of JS into separate files (can be done later)
