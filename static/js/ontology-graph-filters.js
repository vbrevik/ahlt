/**
 * Ontology graph — filter, visibility, search & focus/ego network.
 *
 * Factory: ontologyGraphFilters(deps)
 *   deps.state — shared state object with: allNodes, allEdges, activeTypes, activeRelTypes,
 *                focusNodeId, focusNeighbors, arrowsVisible, searchQuery,
 *                nodeGroup, labelGroup, linkGroup, edgeLabelGroup, highlightGroup
 *   deps.typeColor  — function(entityType) → color
 *   deps.createEl   — function(tag, attrs, text) → element
 *   deps.typeFiltersEl, deps.relFiltersEl — filter container elements
 *   deps.container   — .graph-container element
 *   deps.centerOnNode — function(node, animate)
 *   deps.preFilterType — URL ?type= param or null
 *
 * Returns { buildFilters, applyVisibility, applySearch, setFocus, clearFocus,
 *           isNodeVisible, isEdgeVisible }
 */
function ontologyGraphFilters(deps) {
    var s = deps.state;
    var typeColor = deps.typeColor;
    var createEl = deps.createEl;

    function isNodeVisible(n) {
        if (!s.activeTypes.has(n.entity_type)) return false;
        if (s.focusNodeId !== null) {
            if (n.id !== s.focusNodeId && !s.focusNeighbors.has(n.id)) return false;
        }
        return true;
    }

    function computeFocusNeighbors() {
        s.focusNeighbors.clear();
        if (s.focusNodeId === null) return;
        s.allEdges.forEach(function(e) {
            var sid = typeof e.source === 'object' ? e.source.id : e.source;
            var tid = typeof e.target === 'object' ? e.target.id : e.target;
            if (sid === s.focusNodeId) s.focusNeighbors.add(tid);
            if (tid === s.focusNodeId) s.focusNeighbors.add(sid);
        });
    }

    function isEdgeVisible(l) {
        if (!s.activeTypes.has(l.source.entity_type) || !s.activeTypes.has(l.target.entity_type)) return false;
        if (!s.activeRelTypes.has(l.relation_type)) return false;
        if (s.focusNodeId !== null) {
            if (l.source.id !== s.focusNodeId && l.target.id !== s.focusNodeId) return false;
        }
        return true;
    }

    function applyVisibility() {
        computeFocusNeighbors();
        var visibleNodeIds = new Set();
        s.nodeGroup.each(function(d) {
            var vis = isNodeVisible(d);
            d3.select(this).style('display', vis ? null : 'none');
            if (vis) visibleNodeIds.add(d.id);
        });
        s.labelGroup.each(function(d) {
            d3.select(this).style('display', visibleNodeIds.has(d.id) ? null : 'none');
        });
        var visibleEdges = 0;
        s.linkGroup.each(function(d) {
            var vis = isEdgeVisible(d);
            d3.select(this).style('display', vis ? null : 'none');
            if (vis) visibleEdges++;
        });
        s.edgeLabelGroup.each(function(d) {
            d3.select(this).style('display', isEdgeVisible(d) ? null : 'none');
        });
        s.linkGroup.attr('marker-end', s.arrowsVisible ? 'url(#arrow)' : null);
        deps.statEl.textContent = visibleNodeIds.size + ' nodes \u00b7 ' + visibleEdges + ' edges';
        applySearch();
    }

    function applySearch() {
        if (s.highlightGroup) s.highlightGroup.selectAll('*').remove();
        if (!s.searchQuery) {
            s.nodeGroup.attr('opacity', function(d) { return isNodeVisible(d) ? 1 : 0; });
            s.labelGroup.attr('opacity', function(d) { return isNodeVisible(d) ? 1 : 0; });
            return;
        }
        var q = s.searchQuery.toLowerCase();
        var matchIds = new Set();
        s.allNodes.forEach(function(n) {
            if (!isNodeVisible(n)) return;
            var name = (n.name || '').toLowerCase();
            var label = (n.label || '').toLowerCase();
            if (name.indexOf(q) !== -1 || label.indexOf(q) !== -1) matchIds.add(n.id);
        });
        s.nodeGroup.attr('opacity', function(d) {
            if (!isNodeVisible(d)) return 0;
            return matchIds.has(d.id) ? 1 : 0.15;
        });
        s.labelGroup.attr('opacity', function(d) {
            if (!isNodeVisible(d)) return 0;
            return matchIds.has(d.id) ? 1 : 0.1;
        });
        var matchNodes = s.allNodes.filter(function(n) { return matchIds.has(n.id); });
        s.highlightGroup.selectAll('circle')
            .data(matchNodes, function(d) { return d.id; })
            .join('circle')
            .attr('class', 'node-highlight-ring')
            .attr('cx', function(d) { return d.x; })
            .attr('cy', function(d) { return d.y; })
            .attr('r', 16);
    }

    function buildFilters(entityTypes, relationTypes) {
        var typeCounts = {};
        s.allNodes.forEach(function(n) {
            typeCounts[n.entity_type] = (typeCounts[n.entity_type] || 0) + 1;
        });
        if (deps.preFilterType) {
            s.activeTypes.clear();
            entityTypes.forEach(function(t) {
                if (t === deps.preFilterType) s.activeTypes.add(t);
            });
        }
        entityTypes.forEach(function(t) {
            var label = createEl('label', { 'class': 'filter-chip' });
            var input = createEl('input', { type: 'checkbox', 'data-type': t, 'data-filter': 'entity' });
            input.checked = s.activeTypes.has(t);
            var dot = createEl('span', { 'class': 'chip-dot' });
            dot.style.background = typeColor(t);
            var span = createEl('span', { 'class': 'chip-label' }, t);
            var count = createEl('span', { 'class': 'chip-count' }, '(' + (typeCounts[t] || 0) + ')');
            label.appendChild(input);
            label.appendChild(dot);
            label.appendChild(span);
            label.appendChild(count);
            input.addEventListener('change', function() {
                if (this.checked) s.activeTypes.add(t); else s.activeTypes.delete(t);
                applyVisibility();
            });
            deps.typeFiltersEl.appendChild(label);
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
                if (this.checked) s.activeRelTypes.add(t); else s.activeRelTypes.delete(t);
                applyVisibility();
            });
            deps.relFiltersEl.appendChild(label);
        });
    }

    // Focus / ego network
    function setFocus(nodeId) {
        s.focusNodeId = nodeId;
        computeFocusNeighbors();
        applyVisibility();
        removeFocusPill();
        var node = s.allNodes.find(function(n) { return n.id === nodeId; });
        if (!node) return;
        var pill = createEl('button', { 'class': 'focus-pill', id: 'focus-pill' });
        pill.appendChild(document.createTextNode('Focus: ' + (node.label || node.name) + ' '));
        var close = createEl('span', { 'class': 'focus-pill__close' }, '\u2715');
        pill.appendChild(close);
        pill.addEventListener('click', function() { clearFocus(); });
        deps.container.appendChild(pill);
        deps.centerOnNode(node, true);
    }

    function clearFocus() {
        s.focusNodeId = null;
        s.focusNeighbors.clear();
        removeFocusPill();
        applyVisibility();
    }

    function removeFocusPill() {
        var existing = document.getElementById('focus-pill');
        if (existing) existing.remove();
    }

    return {
        buildFilters: buildFilters,
        applyVisibility: applyVisibility,
        applySearch: applySearch,
        setFocus: setFocus,
        clearFocus: clearFocus,
        isNodeVisible: isNodeVisible,
        isEdgeVisible: isEdgeVisible
    };
}
