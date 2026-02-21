# Ontology Graph Redesign — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Improve the ontology graph views with intuitive search+filter UI, right-click context menus for relation navigation, ego network focus, and schema→instance drill-down.

**Architecture:** Pure client-side enhancements to two existing Askama templates and one CSS file. No backend changes. The instance graph (`data.html`) gets the heaviest changes (search, filters, context menu, ego focus). The schema graph (`graph.html`) gets a simpler context menu for drill-down. All new DOM elements use `createElement`/`textContent` (no `innerHTML`).

**Tech Stack:** D3.js v7 (already loaded via CDN), vanilla JS, CSS with BEM naming, Askama templates.

---

## Task 1: CSS Foundation — New Graph Component Styles

**Files:**
- Modify: `static/css/style.css:1979` (append after existing graph section)

**Step 1: Add all new CSS classes after the existing `.detail-actions` block (line 1985)**

Append the following CSS after line 1985 (after the existing `/* === Data Browser === */` comment, insert before it):

```css
/* --- Graph Search --- */
.graph-search {
    margin-bottom: 0.625rem;
}

.graph-search__input {
    width: 100%;
    padding: 0.375rem 0.5rem 0.375rem 1.75rem;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font-family: var(--font-mono);
    font-size: 0.75rem;
    color: var(--text);
    background: var(--surface);
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='14' height='14' viewBox='0 0 24 24' fill='none' stroke='%239ca3af' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Ccircle cx='11' cy='11' r='8'/%3E%3Cline x1='21' y1='21' x2='16.65' y2='16.65'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: 0.5rem center;
    transition: border-color var(--duration) var(--ease);
}

.graph-search__input::placeholder {
    color: var(--text-muted);
}

.graph-search__input:focus {
    outline: none;
    border-color: var(--primary);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--primary) 20%, transparent);
}

/* --- Filter Sections --- */
.filter-section__title {
    font-size: 0.5625rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--text-muted);
    margin-top: 0.5rem;
    margin-bottom: 0.25rem;
}

.filter-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.625rem;
    padding-top: 0.5rem;
    border-top: 1px solid var(--border);
}

.filter-actions__btn {
    font-family: var(--font-mono);
    font-size: 0.625rem;
    color: var(--text-muted);
    background: none;
    border: none;
    cursor: pointer;
    padding: 0.125rem 0;
    transition: color var(--duration) var(--ease);
}

.filter-actions__btn:hover {
    color: var(--text);
}

.filter-actions__btn.active {
    color: var(--primary);
}

.chip-count {
    font-family: var(--font-mono);
    font-size: 0.625rem;
    color: var(--text-muted);
    margin-left: auto;
}

/* --- Search Highlight Pulse --- */
@keyframes search-pulse {
    0%, 100% { stroke-opacity: 0.6; r: 16; }
    50% { stroke-opacity: 1; r: 18; }
}

.node-highlight-ring {
    fill: none;
    stroke: var(--primary);
    stroke-width: 3;
    animation: search-pulse 1.2s ease-in-out infinite;
    pointer-events: none;
}

/* --- Context Menu --- */
.context-menu {
    position: absolute;
    z-index: 20;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg, var(--shadow-md));
    min-width: 220px;
    max-width: 300px;
    max-height: 400px;
    overflow-y: auto;
    animation: slideDown var(--duration-slow) var(--ease-out);
}

.context-menu__header {
    padding: 0.625rem 0.75rem 0.375rem;
    display: flex;
    align-items: center;
    gap: 0.375rem;
}

.context-menu__header-label {
    font-size: 0.875rem;
    font-weight: 600;
    color: var(--text);
}

.context-menu__header-badge {
    font-family: var(--font-mono);
    font-size: 0.625rem;
    font-weight: 600;
    padding: 0.0625rem 0.375rem;
    border-radius: var(--radius-sm);
    background: var(--bg-subtle);
}

.context-menu__divider {
    height: 1px;
    background: var(--border);
    margin: 0.25rem 0;
}

.context-menu__action {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    padding: 0.375rem 0.75rem;
    font-size: 0.8125rem;
    color: var(--text);
    cursor: pointer;
    transition: background var(--duration) var(--ease);
    border: none;
    background: none;
    width: 100%;
    text-align: left;
}

.context-menu__action:hover {
    background: var(--bg-subtle);
}

.context-menu__action-icon {
    font-size: 0.75rem;
    color: var(--text-muted);
    width: 1rem;
    text-align: center;
    flex-shrink: 0;
}

.context-menu__group {
    padding: 0.125rem 0;
}

.context-menu__group-header {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    padding: 0.3125rem 0.75rem;
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    font-weight: 600;
    color: var(--text-secondary);
    cursor: pointer;
    transition: background var(--duration) var(--ease);
}

.context-menu__group-header:hover {
    background: var(--bg-subtle);
}

.context-menu__group-toggle {
    font-size: 0.625rem;
    color: var(--text-muted);
    transition: transform var(--duration) var(--ease);
}

.context-menu__group.collapsed .context-menu__group-toggle {
    transform: rotate(-90deg);
}

.context-menu__group.collapsed .context-menu__group-items {
    display: none;
}

.context-menu__group-count {
    font-weight: 400;
    color: var(--text-muted);
    margin-left: 0.25rem;
}

.context-menu__item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.25rem 0.75rem 0.25rem 1.75rem;
    font-size: 0.8125rem;
    color: var(--text);
    cursor: pointer;
    transition: background var(--duration) var(--ease);
}

.context-menu__item:hover {
    background: var(--bg-subtle);
}

.context-menu__item-arrow {
    font-size: 0.6875rem;
    color: var(--text-muted);
    opacity: 0;
    transition: opacity var(--duration) var(--ease);
}

.context-menu__item:hover .context-menu__item-arrow {
    opacity: 1;
}

.context-menu__overflow {
    padding: 0.1875rem 0.75rem 0.1875rem 1.75rem;
    font-size: 0.6875rem;
    color: var(--text-muted);
    font-style: italic;
}

/* --- Focus Pill --- */
.focus-pill {
    position: absolute;
    top: 3rem;
    left: 50%;
    transform: translateX(-50%);
    z-index: 10;
    display: flex;
    align-items: center;
    gap: 0.375rem;
    padding: 0.3125rem 0.75rem;
    background: var(--primary);
    color: #fff;
    border: none;
    border-radius: var(--radius-full);
    font-family: var(--font-mono);
    font-size: 0.6875rem;
    font-weight: 600;
    cursor: pointer;
    box-shadow: var(--shadow-md);
    animation: slideDown var(--duration-slow) var(--ease-out);
    transition: background var(--duration) var(--ease);
}

.focus-pill:hover {
    background: var(--primary-hover, color-mix(in srgb, var(--primary) 85%, #000));
}

.focus-pill__close {
    font-size: 0.875rem;
    font-weight: 400;
    opacity: 0.7;
}
```

**Step 2: Verify the CSS compiles (no syntax errors)**

Run: open `static/css/style.css` in browser or visually inspect. CSS doesn't need compilation in this project.

**Step 3: Commit**

```bash
git add static/css/style.css
git commit -m "style: add CSS for graph search, context menu, focus pill, and filter sections"
```

---

## Task 2: Instance Graph — Rewrite Filter Panel with Search + Relation Types

**Files:**
- Modify: `templates/ontology/data.html:46-51` (filter panel HTML)
- Modify: `templates/ontology/data.html:134-236` (JS filter state + buildFilters + render)

### Overview

Replace the existing filter panel HTML and JS with the new search + entity type + relation type + actions layout. Convert the `render()` function from full-rebuild to visibility-toggle approach.

**Step 1: Replace the filter panel HTML (lines 46-51)**

Replace:
```html
    <div class="graph-controls" id="graph-controls">
        <div class="controls-header">
            <span class="controls-title">Filter by Type</span>
        </div>
        <div id="type-filters"></div>
    </div>
```

With:
```html
    <div class="graph-controls" id="graph-controls">
        <div class="graph-search">
            <input type="text" class="graph-search__input" id="graph-search" placeholder="Search entities..." autocomplete="off">
        </div>
        <span class="filter-section__title">Entity Types</span>
        <div id="type-filters"></div>
        <span class="filter-section__title">Relation Types</span>
        <div id="relation-filters"></div>
        <div class="filter-actions">
            <button class="filter-actions__btn" id="btn-arrows" title="Toggle arrows">→ Arrows</button>
            <button class="filter-actions__btn" id="btn-reset-filters" title="Reset all filters">⟳ Reset</button>
        </div>
    </div>
```

**Step 2: Rewrite the JS state management and filter building**

Replace the entire JS `<script>` block (lines 73-453) with an updated version. The key changes from the existing code are:

1. **New state variables**: `activeRelTypes` Set, `searchQuery` string, `focusNodeId` (null or node id), `arrowsVisible` boolean
2. **`buildFilters()`**: Now builds both entity type AND relation type filter chips, with counts on entity chips
3. **`render()`**: Builds SVG elements ONCE (initial render). No more `g.selectAll('*').remove()`. Simulation created once.
4. **`applyVisibility()`**: New function that shows/hides node groups and edges based on activeTypes + activeRelTypes + focusNodeId. Called by filter toggles instead of `render()`.
5. **`applySearch()`**: New function that adds/removes highlight rings based on search query
6. **URL param**: Read `?type=` on load to pre-select entity type

Replace the `<script>` content (lines 73-453) with:

```javascript
<script>
(function() {
    var TYPE_COLORS = {
        'nav_item':      '#0d9488',
        'permission':    '#059669',
        'relation_type': '#7c3aed',
        'role':          '#2563eb',
        'user':          '#d97706',
        'setting':       '#e11d48'
    };
    var FALLBACK_COLOR = '#78716c';
    function typeColor(t) { return TYPE_COLORS[t] || FALLBACK_COLOR; }

    function createEl(tag, attrs, text) {
        var el = document.createElement(tag);
        if (attrs) Object.keys(attrs).forEach(function(k) {
            if (k === 'style' && typeof attrs[k] === 'object') {
                Object.keys(attrs[k]).forEach(function(s) { el.style[s] = attrs[k][s]; });
            } else { el.setAttribute(k, attrs[k]); }
        });
        if (text) el.textContent = text;
        return el;
    }

    var canvas = document.getElementById('graph-canvas');
    var loading = document.getElementById('graph-loading');
    var detail = document.getElementById('graph-detail');
    var typeFiltersEl = document.getElementById('type-filters');
    var relFiltersEl = document.getElementById('relation-filters');
    var statEl = document.getElementById('toolbar-stat');
    var searchInput = document.getElementById('graph-search');
    var container = document.querySelector('.graph-container');

    var width = canvas.clientWidth;
    var height = canvas.clientHeight || 600;

    var svg = d3.select('#graph-canvas')
        .append('svg')
        .attr('width', '100%')
        .attr('height', '100%')
        .attr('viewBox', [0, 0, width, height]);

    svg.append('defs').selectAll('marker')
        .data(['arrow'])
        .join('marker')
        .attr('id', 'arrow')
        .attr('viewBox', '0 -5 10 10')
        .attr('refX', 22)
        .attr('refY', 0)
        .attr('markerWidth', 6)
        .attr('markerHeight', 6)
        .attr('orient', 'auto')
        .append('path')
        .attr('d', 'M0,-5L10,0L0,5')
        .attr('fill', 'var(--text-muted)');

    var g = svg.append('g');

    var zoom = d3.zoom()
        .scaleExtent([0.2, 5])
        .on('zoom', function(e) { g.attr('transform', e.transform); });
    svg.call(zoom);

    // State
    var allNodes = [], allEdges = [], allLinks = [];
    var activeTypes = new Set();
    var activeRelTypes = new Set();
    var focusNodeId = null;
    var arrowsVisible = true;
    var searchQuery = '';
    var simulation, linkGroup, nodeGroup, labelGroup, edgeLabelGroup, highlightGroup;
    var locked = false;

    // Read URL params for pre-filtering
    var urlParams = new URLSearchParams(window.location.search);
    var preFilterType = urlParams.get('type');

    // Fit all visible nodes into view
    function fitAll(animate) {
        var visible = allNodes.filter(function(n) { return isNodeVisible(n); });
        if (!visible.length) return;
        var padX = 80, padTop = 60, padBottom = 80;
        var xs = visible.map(function(n) { return n.x; });
        var ys = visible.map(function(n) { return n.y; });
        var x0 = Math.min.apply(null, xs) - padX;
        var y0 = Math.min.apply(null, ys) - padTop;
        var x1 = Math.max.apply(null, xs) + padX;
        var y1 = Math.max.apply(null, ys) + padBottom;
        var bw = x1 - x0, bh = y1 - y0;
        if (bw < 1 || bh < 1) return;
        var scale = Math.min(width / bw, height / bh, 1.8);
        var tx = (width - bw * scale) / 2 - x0 * scale;
        var ty = (height - bh * scale) / 2 - y0 * scale;
        var t = d3.zoomIdentity.translate(tx, ty).scale(scale);
        if (animate) {
            svg.transition().duration(500).call(zoom.transform, t);
        } else {
            svg.call(zoom.transform, t);
        }
    }

    function centerOnNode(node, animate) {
        var scale = 1.5;
        var tx = width / 2 - node.x * scale;
        var ty = height / 2 - node.y * scale;
        var t = d3.zoomIdentity.translate(tx, ty).scale(scale);
        if (animate) {
            svg.transition().duration(600).call(zoom.transform, t);
        } else {
            svg.call(zoom.transform, t);
        }
    }

    // Visibility logic
    function isNodeVisible(n) {
        if (!activeTypes.has(n.entity_type)) return false;
        if (focusNodeId !== null) {
            if (n.id !== focusNodeId && !focusNeighbors.has(n.id)) return false;
        }
        return true;
    }

    var focusNeighbors = new Set();

    function computeFocusNeighbors() {
        focusNeighbors.clear();
        if (focusNodeId === null) return;
        allEdges.forEach(function(e) {
            var sid = typeof e.source === 'object' ? e.source.id : e.source;
            var tid = typeof e.target === 'object' ? e.target.id : e.target;
            if (sid === focusNodeId) focusNeighbors.add(tid);
            if (tid === focusNodeId) focusNeighbors.add(sid);
        });
    }

    function isEdgeVisible(l) {
        var sid = l.source.id, tid = l.target.id;
        if (!activeTypes.has(l.source.entity_type) || !activeTypes.has(l.target.entity_type)) return false;
        if (!activeRelTypes.has(l.relation_type)) return false;
        if (focusNodeId !== null) {
            if (sid !== focusNodeId && tid !== focusNodeId) return false;
        }
        return true;
    }

    function applyVisibility() {
        computeFocusNeighbors();
        var visibleNodeIds = new Set();
        nodeGroup.each(function(d) {
            var vis = isNodeVisible(d);
            d3.select(this).style('display', vis ? null : 'none');
            if (vis) visibleNodeIds.add(d.id);
        });
        labelGroup.each(function(d) {
            d3.select(this).style('display', visibleNodeIds.has(d.id) ? null : 'none');
        });
        var visibleEdges = 0;
        linkGroup.each(function(d) {
            var vis = isEdgeVisible(d);
            d3.select(this).style('display', vis ? null : 'none');
            if (vis) visibleEdges++;
        });
        edgeLabelGroup.each(function(d) {
            d3.select(this).style('display', isEdgeVisible(d) ? null : 'none');
        });
        // Update arrow markers
        linkGroup.attr('marker-end', arrowsVisible ? 'url(#arrow)' : null);

        statEl.textContent = visibleNodeIds.size + ' nodes · ' + visibleEdges + ' edges';
        applySearch();
    }

    // Search highlighting
    function applySearch() {
        if (highlightGroup) highlightGroup.selectAll('*').remove();
        if (!searchQuery) {
            nodeGroup.attr('opacity', 1);
            labelGroup.attr('opacity', 1);
            return;
        }
        var q = searchQuery.toLowerCase();
        var matchIds = new Set();
        allNodes.forEach(function(n) {
            if (!isNodeVisible(n)) return;
            var name = (n.name || '').toLowerCase();
            var label = (n.label || '').toLowerCase();
            if (name.indexOf(q) !== -1 || label.indexOf(q) !== -1) {
                matchIds.add(n.id);
            }
        });
        nodeGroup.attr('opacity', function(d) {
            return !isNodeVisible(d) ? 0 : matchIds.has(d.id) ? 1 : 0.15;
        });
        labelGroup.attr('opacity', function(d) {
            return !isNodeVisible(d) ? 0 : matchIds.has(d.id) ? 1 : 0.1;
        });
        // Pulsing rings on matches
        var matchNodes = allNodes.filter(function(n) { return matchIds.has(n.id); });
        highlightGroup.selectAll('circle')
            .data(matchNodes, function(d) { return d.id; })
            .join('circle')
            .attr('class', 'node-highlight-ring')
            .attr('cx', function(d) { return d.x; })
            .attr('cy', function(d) { return d.y; })
            .attr('r', 16);
    }

    // Build filter chips
    function buildFilters(entityTypes, relationTypes) {
        // Entity type counts
        var typeCounts = {};
        allNodes.forEach(function(n) {
            typeCounts[n.entity_type] = (typeCounts[n.entity_type] || 0) + 1;
        });

        // Pre-filter: if ?type= param, only check that type
        if (preFilterType) {
            activeTypes.clear();
            entityTypes.forEach(function(t) {
                if (t === preFilterType) activeTypes.add(t);
            });
        }

        entityTypes.forEach(function(t) {
            var label = createEl('label', { 'class': 'filter-chip' });
            var input = createEl('input', { type: 'checkbox', 'data-type': t, 'data-filter': 'entity' });
            input.checked = activeTypes.has(t);
            var dot = createEl('span', { 'class': 'chip-dot' });
            dot.style.background = typeColor(t);
            var span = createEl('span', { 'class': 'chip-label' }, t);
            var count = createEl('span', { 'class': 'chip-count' }, '(' + (typeCounts[t] || 0) + ')');
            label.appendChild(input);
            label.appendChild(dot);
            label.appendChild(span);
            label.appendChild(count);
            input.addEventListener('change', function() {
                if (this.checked) activeTypes.add(t); else activeTypes.delete(t);
                applyVisibility();
            });
            typeFiltersEl.appendChild(label);
        });

        relationTypes.forEach(function(t) {
            var label = createEl('label', { 'class': 'filter-chip' });
            var input = createEl('input', { type: 'checkbox', 'data-type': t, 'data-filter': 'relation' });
            input.checked = true;
            var dot = createEl('span', { 'class': 'chip-dot' });
            dot.style.background = 'var(--text-muted)';
            var span = createEl('span', { 'class': 'chip-label' }, t);
            label.appendChild(input);
            label.appendChild(dot);
            label.appendChild(span);
            input.addEventListener('change', function() {
                if (this.checked) activeRelTypes.add(t); else activeRelTypes.delete(t);
                applyVisibility();
            });
            relFiltersEl.appendChild(label);
        });
    }

    // Arrow toggle
    var arrowBtn = document.getElementById('btn-arrows');
    arrowBtn.classList.add('active');
    arrowBtn.addEventListener('click', function() {
        arrowsVisible = !arrowsVisible;
        arrowBtn.classList.toggle('active', arrowsVisible);
        applyVisibility();
    });

    // Reset filters
    document.getElementById('btn-reset-filters').addEventListener('click', function() {
        // Re-check all entity + relation checkboxes
        document.querySelectorAll('#type-filters input, #relation-filters input').forEach(function(cb) {
            cb.checked = true;
            var t = cb.getAttribute('data-type');
            var filter = cb.getAttribute('data-filter');
            if (filter === 'entity') activeTypes.add(t);
            else activeRelTypes.add(t);
        });
        arrowsVisible = true;
        arrowBtn.classList.add('active');
        searchInput.value = '';
        searchQuery = '';
        clearFocus();
        applyVisibility();
        // Remove URL params
        if (window.history.replaceState) {
            window.history.replaceState({}, '', window.location.pathname);
        }
    });

    // Search input
    searchInput.addEventListener('input', function() {
        searchQuery = this.value.trim();
        applySearch();
    });

    // Focus / ego network
    function setFocus(nodeId) {
        focusNodeId = nodeId;
        computeFocusNeighbors();
        applyVisibility();
        // Show focus pill
        removeFocusPill();
        var node = allNodes.find(function(n) { return n.id === nodeId; });
        if (!node) return;
        var pill = createEl('button', { 'class': 'focus-pill', id: 'focus-pill' });
        pill.appendChild(document.createTextNode('Focus: ' + (node.label || node.name) + ' '));
        var close = createEl('span', { 'class': 'focus-pill__close' }, '✕');
        pill.appendChild(close);
        pill.addEventListener('click', function() { clearFocus(); });
        container.appendChild(pill);
        centerOnNode(node, true);
    }

    function clearFocus() {
        focusNodeId = null;
        focusNeighbors.clear();
        removeFocusPill();
        applyVisibility();
    }

    function removeFocusPill() {
        var existing = document.getElementById('focus-pill');
        if (existing) existing.remove();
    }

    // Toolbar buttons
    document.getElementById('btn-fit').addEventListener('click', function() { fitAll(true); });
    document.getElementById('btn-reset').addEventListener('click', function() {
        svg.transition().duration(500).call(zoom.transform, d3.zoomIdentity);
    });
    document.getElementById('btn-zoom-in').addEventListener('click', function() {
        svg.transition().duration(300).call(zoom.scaleBy, 1.5);
    });
    document.getElementById('btn-zoom-out').addEventListener('click', function() {
        svg.transition().duration(300).call(zoom.scaleBy, 1 / 1.5);
    });

    var lockBtn = document.getElementById('btn-lock');
    lockBtn.addEventListener('click', function() {
        locked = !locked;
        lockBtn.classList.toggle('active', locked);
        lockBtn.title = locked ? 'Unlock positions (L)' : 'Lock positions (L)';
        if (locked) {
            allNodes.forEach(function(n) { n.fx = n.x; n.fy = n.y; });
            if (simulation) simulation.alphaTarget(0).alpha(0);
        } else {
            allNodes.forEach(function(n) { n.fx = null; n.fy = null; });
            if (simulation) simulation.alphaTarget(0).alpha(0.3).restart();
        }
    });

    // Keyboard shortcuts
    document.addEventListener('keydown', function(e) {
        if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') return;
        switch (e.key.toLowerCase()) {
            case 'f': fitAll(true); break;
            case '0': svg.transition().duration(500).call(zoom.transform, d3.zoomIdentity); break;
            case '=': case '+': svg.transition().duration(300).call(zoom.scaleBy, 1.5); break;
            case '-': svg.transition().duration(300).call(zoom.scaleBy, 1 / 1.5); break;
            case 'l': lockBtn.click(); break;
            case 'escape':
                detail.style.display = 'none';
                dismissContextMenu();
                if (focusNodeId !== null) clearFocus();
                break;
        }
    });

    // Context menu
    var contextMenuEl = null;

    function dismissContextMenu() {
        if (contextMenuEl) {
            contextMenuEl.remove();
            contextMenuEl = null;
        }
    }

    document.addEventListener('click', function(e) {
        if (contextMenuEl && !contextMenuEl.contains(e.target)) {
            dismissContextMenu();
        }
    });

    function showContextMenu(event, d) {
        event.preventDefault();
        dismissContextMenu();

        var menu = createEl('div', { 'class': 'context-menu' });

        // Header
        var header = createEl('div', { 'class': 'context-menu__header' });
        var headerLabel = createEl('span', { 'class': 'context-menu__header-label' }, d.label || d.name);
        var headerBadge = createEl('span', { 'class': 'context-menu__header-badge' }, d.entity_type);
        headerBadge.style.color = typeColor(d.entity_type);
        header.appendChild(headerLabel);
        header.appendChild(headerBadge);
        menu.appendChild(header);
        menu.appendChild(createEl('div', { 'class': 'context-menu__divider' }));

        // Actions
        var focusAction = createEl('div', { 'class': 'context-menu__action' });
        focusAction.appendChild(createEl('span', { 'class': 'context-menu__action-icon' }, '◉'));
        focusAction.appendChild(document.createTextNode('Focus on this node'));
        focusAction.addEventListener('click', function() {
            dismissContextMenu();
            setFocus(d.id);
        });
        menu.appendChild(focusAction);

        var detailAction = createEl('div', { 'class': 'context-menu__action' });
        detailAction.appendChild(createEl('span', { 'class': 'context-menu__action-icon' }, '↗'));
        var detailLink = createEl('a', { href: '/ontology/data/' + d.id, style: { color: 'inherit', textDecoration: 'none' } }, 'Open full detail');
        detailAction.appendChild(detailLink);
        detailAction.addEventListener('click', function() {
            window.location.href = '/ontology/data/' + d.id;
        });
        menu.appendChild(detailAction);

        // Relations grouped by type
        var outgoing = allLinks.filter(function(l) { return l.source.id === d.id; });
        var incoming = allLinks.filter(function(l) { return l.target.id === d.id; });
        var allConns = [];
        outgoing.forEach(function(l) {
            allConns.push({ relType: l.relation_type, node: l.target, dir: 'out' });
        });
        incoming.forEach(function(l) {
            allConns.push({ relType: l.relation_type, node: l.source, dir: 'in' });
        });

        if (allConns.length) {
            menu.appendChild(createEl('div', { 'class': 'context-menu__divider' }));

            // Group by relation type
            var groups = {};
            allConns.forEach(function(c) {
                var key = c.relType;
                if (!groups[key]) groups[key] = [];
                groups[key].push(c);
            });

            Object.keys(groups).forEach(function(relType) {
                var items = groups[relType];
                var MAX_ITEMS = 5;
                var group = createEl('div', { 'class': 'context-menu__group' });
                if (items.length > MAX_ITEMS) group.classList.add('collapsed');

                var groupHeader = createEl('div', { 'class': 'context-menu__group-header' });
                var toggle = createEl('span', { 'class': 'context-menu__group-toggle' }, '▾');
                var relName = document.createTextNode(' ' + relType + ' ');
                var countSpan = createEl('span', { 'class': 'context-menu__group-count' }, '(' + items.length + ')');
                groupHeader.appendChild(toggle);
                groupHeader.appendChild(relName);
                groupHeader.appendChild(countSpan);
                groupHeader.addEventListener('click', function() {
                    group.classList.toggle('collapsed');
                });
                group.appendChild(groupHeader);

                var itemsContainer = createEl('div', { 'class': 'context-menu__group-items' });
                var shown = items.slice(0, MAX_ITEMS);
                shown.forEach(function(c) {
                    var item = createEl('div', { 'class': 'context-menu__item' });
                    var nameSpan = createEl('span', null, c.node.label || c.node.name);
                    nameSpan.style.color = typeColor(c.node.entity_type);
                    var arrow = createEl('span', { 'class': 'context-menu__item-arrow' }, '→');
                    item.appendChild(nameSpan);
                    item.appendChild(arrow);
                    item.addEventListener('click', function() {
                        dismissContextMenu();
                        centerOnNode(c.node, true);
                        showDetail(c.node, allLinks);
                    });
                    itemsContainer.appendChild(item);
                });
                if (items.length > MAX_ITEMS) {
                    var overflow = createEl('div', { 'class': 'context-menu__overflow' }, '...' + (items.length - MAX_ITEMS) + ' more');
                    overflow.style.cursor = 'pointer';
                    overflow.addEventListener('click', function() {
                        group.classList.remove('collapsed');
                        // Show remaining items
                        var remaining = items.slice(MAX_ITEMS);
                        remaining.forEach(function(c) {
                            var item = createEl('div', { 'class': 'context-menu__item' });
                            var nameSpan = createEl('span', null, c.node.label || c.node.name);
                            nameSpan.style.color = typeColor(c.node.entity_type);
                            var arrow = createEl('span', { 'class': 'context-menu__item-arrow' }, '→');
                            item.appendChild(nameSpan);
                            item.appendChild(arrow);
                            item.addEventListener('click', function() {
                                dismissContextMenu();
                                centerOnNode(c.node, true);
                                showDetail(c.node, allLinks);
                            });
                            itemsContainer.appendChild(item);
                        });
                        overflow.remove();
                    });
                    itemsContainer.appendChild(overflow);
                }
                group.appendChild(itemsContainer);
                menu.appendChild(group);
            });
        }

        // Position relative to graph-container
        var rect = container.getBoundingClientRect();
        var x = event.clientX - rect.left;
        var y = event.clientY - rect.top;
        menu.style.left = x + 'px';
        menu.style.top = y + 'px';
        container.appendChild(menu);
        contextMenuEl = menu;

        // Flip if near edges
        requestAnimationFrame(function() {
            var menuRect = menu.getBoundingClientRect();
            if (menuRect.right > rect.right - 10) {
                menu.style.left = (x - menuRect.width) + 'px';
            }
            if (menuRect.bottom > rect.bottom - 10) {
                menu.style.top = (y - menuRect.height) + 'px';
            }
        });
    }

    // Fetch and render
    fetch('/ontology/api/graph')
        .then(function(r) { return r.json(); })
        .then(function(data) {
            loading.style.display = 'none';
            allNodes = data.nodes;
            allEdges = data.edges;
            activeTypes = new Set(data.entity_types);

            // Compute relation types
            var relTypeSet = new Set();
            data.edges.forEach(function(e) { relTypeSet.add(e.relation_type); });
            var relationTypes = Array.from(relTypeSet).sort();
            activeRelTypes = new Set(relationTypes);

            buildFilters(data.entity_types, relationTypes);
            render();
        });

    function render() {
        var nodeMap = new Map(allNodes.map(function(n) { return [n.id, n]; }));
        allLinks = allEdges.map(function(e) {
            var sid = typeof e.source === 'object' ? e.source.id : e.source;
            var tid = typeof e.target === 'object' ? e.target.id : e.target;
            return {
                source: nodeMap.get(sid) || sid,
                target: nodeMap.get(tid) || tid,
                relation_type: e.relation_type,
                relation_label: e.relation_label
            };
        }).filter(function(l) { return l.source && l.target; });

        g.selectAll('*').remove();

        linkGroup = g.append('g').attr('class', 'links')
            .selectAll('line')
            .data(allLinks)
            .join('line')
            .attr('stroke', 'var(--border-strong)')
            .attr('stroke-width', 1.2)
            .attr('stroke-opacity', 0.6)
            .attr('marker-end', 'url(#arrow)');

        edgeLabelGroup = g.append('g').attr('class', 'edge-labels')
            .selectAll('text')
            .data(allLinks)
            .join('text')
            .text(function(d) { return d.relation_type; })
            .attr('font-size', 8)
            .attr('fill', 'var(--text-muted)')
            .attr('text-anchor', 'middle')
            .attr('dy', -4)
            .style('pointer-events', 'none')
            .style('font-family', 'var(--font-mono)');

        nodeGroup = g.append('g').attr('class', 'nodes')
            .selectAll('circle')
            .data(allNodes)
            .join('circle')
            .attr('r', function(d) { return d.entity_type === 'relation_type' ? 6 : 10; })
            .attr('fill', function(d) { return typeColor(d.entity_type); })
            .attr('stroke', '#fff')
            .attr('stroke-width', 2)
            .style('cursor', 'pointer')
            .call(d3.drag()
                .on('start', dragStart)
                .on('drag', dragging)
                .on('end', dragEnd))
            .on('mouseover', function(event, d) { highlightNode(d); })
            .on('mouseout', function() { unhighlight(); })
            .on('click', function(event, d) { showDetail(d, allLinks); })
            .on('contextmenu', function(event, d) { showContextMenu(event, d); });

        labelGroup = g.append('g').attr('class', 'labels')
            .selectAll('text')
            .data(allNodes)
            .join('text')
            .text(function(d) { return d.label || d.name; })
            .attr('font-size', 11)
            .attr('font-weight', 500)
            .attr('fill', 'var(--text)')
            .attr('dx', 14)
            .attr('dy', 4)
            .style('pointer-events', 'none')
            .style('font-family', 'var(--font-body)');

        highlightGroup = g.append('g').attr('class', 'highlights');

        var autoFitted = false;
        simulation = d3.forceSimulation(allNodes)
            .force('link', d3.forceLink(allLinks).id(function(d) { return d.id; }).distance(100))
            .force('charge', d3.forceManyBody().strength(-200))
            .force('x', d3.forceX(width / 2).strength(0.12))
            .force('y', d3.forceY(height / 2).strength(0.12))
            .force('collision', d3.forceCollide(25))
            .on('tick', function() {
                linkGroup
                    .attr('x1', function(d) { return d.source.x; })
                    .attr('y1', function(d) { return d.source.y; })
                    .attr('x2', function(d) { return d.target.x; })
                    .attr('y2', function(d) { return d.target.y; });
                edgeLabelGroup
                    .attr('x', function(d) { return (d.source.x + d.target.x) / 2; })
                    .attr('y', function(d) { return (d.source.y + d.target.y) / 2; });
                nodeGroup
                    .attr('cx', function(d) { return d.x; })
                    .attr('cy', function(d) { return d.y; });
                labelGroup
                    .attr('x', function(d) { return d.x; })
                    .attr('y', function(d) { return d.y; });
                if (highlightGroup) {
                    highlightGroup.selectAll('circle')
                        .attr('cx', function(d) { return d.x; })
                        .attr('cy', function(d) { return d.y; });
                }
                if (!autoFitted && simulation.alpha() < 0.05) {
                    autoFitted = true;
                    applyVisibility();
                    fitAll(true);
                }
            });

        // Apply initial visibility (handles ?type= prefilter)
        if (preFilterType) {
            applyVisibility();
        }
    }

    function highlightNode(d) {
        if (searchQuery) return; // Don't override search highlighting
        var connected = new Set();
        connected.add(d.id);
        allLinks.forEach(function(l) {
            if (l.source.id === d.id || l.target.id === d.id) {
                connected.add(l.source.id);
                connected.add(l.target.id);
            }
        });
        nodeGroup.attr('opacity', function(n) { return connected.has(n.id) ? 1 : 0.15; });
        labelGroup.attr('opacity', function(n) { return connected.has(n.id) ? 1 : 0.1; });
        linkGroup.attr('stroke-opacity', function(l) {
            return (l.source.id === d.id || l.target.id === d.id) ? 0.9 : 0.05;
        });
        edgeLabelGroup.attr('opacity', function(l) {
            return (l.source.id === d.id || l.target.id === d.id) ? 1 : 0.1;
        });
    }

    function unhighlight() {
        if (searchQuery) { applySearch(); return; }
        nodeGroup.attr('opacity', 1);
        labelGroup.attr('opacity', 1);
        linkGroup.attr('stroke-opacity', 0.6);
        edgeLabelGroup.attr('opacity', 1);
    }

    function showDetail(d, links) {
        detail.style.display = '';
        var detailType = document.getElementById('detail-type');
        detailType.textContent = d.entity_type;
        detailType.style.color = typeColor(d.entity_type);
        document.getElementById('detail-label').textContent = d.label || d.name;
        document.getElementById('detail-meta').textContent = '#' + d.id + ' · ' + d.name;

        var propsEl = document.getElementById('detail-props');
        while (propsEl.firstChild) propsEl.removeChild(propsEl.firstChild);
        if (d.properties && Object.keys(d.properties).length > 0) {
            propsEl.appendChild(createEl('span', { 'class': 'conn-heading' }, 'Properties'));
            Object.keys(d.properties).forEach(function(key) {
                var val = d.properties[key];
                if (key === 'password') val = '••••••••';
                var row = createEl('div', { 'class': 'conn-row' });
                row.appendChild(createEl('code', null, key));
                row.appendChild(document.createTextNode(' = '));
                row.appendChild(createEl('span', null, val));
                propsEl.appendChild(row);
            });
        }

        var connEl = document.getElementById('detail-connections');
        while (connEl.firstChild) connEl.removeChild(connEl.firstChild);
        var outgoing = links.filter(function(l) { return l.source.id === d.id; });
        var incoming = links.filter(function(l) { return l.target.id === d.id; });
        if (outgoing.length) {
            connEl.appendChild(createEl('span', { 'class': 'conn-heading' }, 'Outgoing'));
            outgoing.forEach(function(l) {
                var row = createEl('div', { 'class': 'conn-row' });
                row.appendChild(createEl('code', null, l.relation_type));
                row.appendChild(document.createTextNode(' → '));
                var name = createEl('span', null, l.target.label || l.target.name);
                name.style.color = typeColor(l.target.entity_type);
                name.style.cursor = 'pointer';
                name.addEventListener('click', function() {
                    centerOnNode(l.target, true);
                    showDetail(l.target, links);
                });
                row.appendChild(name);
                connEl.appendChild(row);
            });
        }
        if (incoming.length) {
            connEl.appendChild(createEl('span', { 'class': 'conn-heading' }, 'Incoming'));
            incoming.forEach(function(l) {
                var row = createEl('div', { 'class': 'conn-row' });
                var name = createEl('span', null, l.source.label || l.source.name);
                name.style.color = typeColor(l.source.entity_type);
                name.style.cursor = 'pointer';
                name.addEventListener('click', function() {
                    centerOnNode(l.source, true);
                    showDetail(l.source, links);
                });
                row.appendChild(name);
                row.appendChild(document.createTextNode(' → '));
                row.appendChild(createEl('code', null, l.relation_type));
                connEl.appendChild(row);
            });
        }
        if (!outgoing.length && !incoming.length) {
            connEl.appendChild(createEl('span', { 'class': 'conn-empty' }, 'No connections'));
        }

        var actionsEl = document.getElementById('detail-actions');
        while (actionsEl.firstChild) actionsEl.removeChild(actionsEl.firstChild);
        var link = createEl('a', { 'class': 'btn btn-sm', href: '/ontology/data/' + d.id }, 'Full detail');
        actionsEl.appendChild(link);
    }

    document.getElementById('btn-close-detail').addEventListener('click', function() {
        detail.style.display = 'none';
    });

    function dragStart(event, d) {
        if (!event.active) simulation.alphaTarget(0.3).restart();
        d.fx = d.x;
        d.fy = d.y;
    }
    function dragging(event, d) {
        d.fx = event.x;
        d.fy = event.y;
    }
    function dragEnd(event, d) {
        if (!event.active) simulation.alphaTarget(0);
        if (!locked) { d.fx = null; d.fy = null; }
    }
})();
</script>
```

**Step 3: Verify build**

Run: `cargo check` — template HTML changes require `cargo clean && cargo build` if Askama cache is stale, but HTML-only changes within existing `{% block content %}` usually work with just `cargo check`.

**Step 4: Commit**

```bash
git add templates/ontology/data.html
git commit -m "feat(graph): rewrite instance graph with search, relation filters, context menu, and ego focus"
```

---

## Task 3: Schema Graph — Add Context Menu for Drill-Down

**Files:**
- Modify: `templates/ontology/graph.html:260-417` (add context menu + right-click handler)

### Overview

Add a simpler context menu to the schema graph. Right-clicking a type node shows "View instances →" (links to `/ontology/data?type={type}`) and lists associated relation types.

**Step 1: Add context menu code to the schema graph JS**

Add the following code inside the `(function() { ... })()` block, after the `showDetail` function (after line 413 / before the `btn-close-detail` listener):

```javascript
    // Context menu
    var contextMenuEl = null;
    var containerEl = document.querySelector('.graph-container');

    function dismissContextMenu() {
        if (contextMenuEl) { contextMenuEl.remove(); contextMenuEl = null; }
    }

    document.addEventListener('click', function(e) {
        if (contextMenuEl && !contextMenuEl.contains(e.target)) dismissContextMenu();
    });

    function showContextMenu(event, d) {
        event.preventDefault();
        dismissContextMenu();
        var menu = createEl('div', { 'class': 'context-menu' });

        // Header
        var header = createEl('div', { 'class': 'context-menu__header' });
        var headerLabel = createEl('span', { 'class': 'context-menu__header-label' }, d.label);
        var headerBadge = createEl('span', { 'class': 'context-menu__header-badge' }, d.count + ' instance' + (d.count !== 1 ? 's' : ''));
        headerBadge.style.color = typeColor(d.id);
        header.appendChild(headerLabel);
        header.appendChild(headerBadge);
        menu.appendChild(header);
        menu.appendChild(createEl('div', { 'class': 'context-menu__divider' }));

        // View instances action
        var viewAction = createEl('div', { 'class': 'context-menu__action' });
        viewAction.appendChild(createEl('span', { 'class': 'context-menu__action-icon' }, '→'));
        viewAction.appendChild(document.createTextNode('View instances'));
        viewAction.addEventListener('click', function() {
            window.location.href = '/ontology/data?type=' + encodeURIComponent(d.id);
        });
        menu.appendChild(viewAction);

        // Relation types this entity type participates in
        var relatedLinks = allLinks.filter(function(l) {
            return l.source.id === d.id || l.target.id === d.id;
        });
        if (relatedLinks.length) {
            menu.appendChild(createEl('div', { 'class': 'context-menu__divider' }));
            var group = createEl('div', { 'class': 'context-menu__group' });
            var groupHeader = createEl('div', { 'class': 'context-menu__group-header' });
            groupHeader.appendChild(createEl('span', { 'class': 'context-menu__group-toggle' }, '▾'));
            groupHeader.appendChild(document.createTextNode(' Relation types'));
            groupHeader.addEventListener('click', function() {
                group.classList.toggle('collapsed');
            });
            group.appendChild(groupHeader);
            var itemsContainer = createEl('div', { 'class': 'context-menu__group-items' });
            relatedLinks.forEach(function(l) {
                var item = createEl('div', { 'class': 'context-menu__item' });
                var text = l.relation_type + ' (' + l.count + ')';
                item.appendChild(createEl('span', null, text));
                itemsContainer.appendChild(item);
            });
            group.appendChild(itemsContainer);
            menu.appendChild(group);
        }

        // Position
        var rect = containerEl.getBoundingClientRect();
        var x = event.clientX - rect.left;
        var y = event.clientY - rect.top;
        menu.style.left = x + 'px';
        menu.style.top = y + 'px';
        containerEl.appendChild(menu);
        contextMenuEl = menu;

        requestAnimationFrame(function() {
            var menuRect = menu.getBoundingClientRect();
            if (menuRect.right > rect.right - 10) menu.style.left = (x - menuRect.width) + 'px';
            if (menuRect.bottom > rect.bottom - 10) menu.style.top = (y - menuRect.height) + 'px';
        });
    }
```

**Step 2: Add the `contextmenu` event handler to existing node group**

Find the line where the schema graph `nodeGroup` has `.on('click', function(event, d) { showDetail(d); })` and add after it:

```javascript
            .on('contextmenu', function(event, d) { showContextMenu(event, d); });
```

**Step 3: Add Escape key dismissal to existing keyboard handler**

In the `case 'escape':` block, add `dismissContextMenu();` before the `detail.style.display = 'none'` line.

**Step 4: Commit**

```bash
git add templates/ontology/graph.html
git commit -m "feat(graph): add right-click context menu to schema graph with drill-down to instances"
```

---

## Task 4: Manual Testing + Polish

**Files:**
- Possibly touch: `static/css/style.css`, `templates/ontology/data.html`, `templates/ontology/graph.html`

**Step 1: Start the server with staging data**

Run: `APP_ENV=staging DATABASE_URL=postgresql://ahlt@localhost/ahlt_staging cargo run`

**Step 2: Test instance graph features**

Open `http://localhost:8080/ontology/data` and verify:
1. Search box appears above entity type filters
2. Entity type chips show counts (e.g., `user (12)`)
3. Relation type filter section appears below entity types
4. Toggling entity types hides/shows nodes WITHOUT repositioning
5. Toggling relation types hides/shows edges only
6. Arrow toggle works
7. Reset button clears all filters
8. Search highlights matching nodes with pulsing ring
9. Right-click shows context menu with grouped relations
10. Clicking a relation item in context menu centers + opens sidebar
11. "Focus on this node" shows only ego network + focus pill
12. Focus pill dismiss restores full graph

**Step 3: Test schema graph features**

Open `http://localhost:8080/ontology` and verify:
1. Right-click shows context menu with "View instances" and relation types
2. "View instances" navigates to `/ontology/data?type=...`
3. Instance graph loads pre-filtered to that type

**Step 4: Test URL param drill-down**

Navigate directly to `http://localhost:8080/ontology/data?type=role` and verify:
1. Only `role` type checkbox is checked
2. Only role nodes and their connections are visible
3. Other type checkboxes are unchecked but can be re-enabled

**Step 5: Fix any visual issues found during testing**

Adjust CSS spacing, colors, z-index, or flip logic as needed.

**Step 6: Final commit**

```bash
git add -A
git commit -m "polish: fine-tune graph redesign after manual testing"
```

---

## Summary

| Task | Files | Description |
|------|-------|-------------|
| 1 | `style.css` | CSS foundation: search, context menu, focus pill, filter sections |
| 2 | `data.html` | Instance graph: search + filters + context menu + ego focus |
| 3 | `graph.html` | Schema graph: context menu with drill-down |
| 4 | All three | Manual testing + polish |

**Total: 4 tasks, ~3 files modified, 0 new files, 0 backend changes.**
