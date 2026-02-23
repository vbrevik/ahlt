/**
 * Ontology graph — main orchestrator.
 * Depends on: graph-helpers.js, ontology-graph-filters.js,
 *             ontology-graph-context-menu.js, ontology-graph-detail.js
 */
(function() {
    var typeColor = graphHelpers.typeColor;
    var createEl = graphHelpers.createEl;

    var canvas = document.getElementById('graph-canvas');
    var loading = document.getElementById('graph-loading');
    var container = document.querySelector('.graph-container');
    var searchInput = document.getElementById('graph-search');

    var width = canvas.clientWidth;
    var height = canvas.clientHeight || 600;

    var svg = d3.select('#graph-canvas')
        .append('svg')
        .attr('width', '100%').attr('height', '100%')
        .attr('viewBox', [0, 0, width, height]);

    svg.append('defs').selectAll('marker')
        .data(['arrow']).join('marker')
        .attr('id', 'arrow').attr('viewBox', '0 -5 10 10')
        .attr('refX', 22).attr('refY', 0)
        .attr('markerWidth', 6).attr('markerHeight', 6).attr('orient', 'auto')
        .append('path').attr('d', 'M0,-5L10,0L0,5').attr('fill', 'var(--text-muted)');

    var g = svg.append('g');
    var zoom = d3.zoom().scaleExtent([0.2, 5])
        .on('zoom', function(e) { g.attr('transform', e.transform); });
    svg.call(zoom);

    // Shared mutable state — passed to sub-modules
    var state = {
        allNodes: [], allEdges: [], allLinks: [],
        activeTypes: new Set(), activeRelTypes: new Set(),
        focusNodeId: null, arrowsVisible: true, searchQuery: '',
        simulation: null, locked: false, focusNeighbors: new Set(),
        nodeGroup: null, labelGroup: null, linkGroup: null,
        edgeLabelGroup: null, highlightGroup: null
    };

    var urlParams = new URLSearchParams(window.location.search);
    var preFilterType = urlParams.get('type');

    // Initialize sub-modules
    var filters = ontologyGraphFilters({
        state: state, typeColor: typeColor, createEl: createEl,
        typeFiltersEl: document.getElementById('type-filters'),
        relFiltersEl: document.getElementById('relation-filters'),
        statEl: document.getElementById('toolbar-stat'),
        container: container, preFilterType: preFilterType,
        centerOnNode: centerOnNode
    });

    var detailPanel = ontologyGraphDetailPanel({
        detailEl: document.getElementById('graph-detail'),
        typeColor: typeColor, createEl: createEl, centerOnNode: centerOnNode
    });

    var contextMenu = ontologyGraphContextMenu({
        container: container, typeColor: typeColor, createEl: createEl,
        centerOnNode: centerOnNode, showDetail: function(d, links) { detailPanel.show(d, links); },
        setFocus: filters.setFocus, allLinks: function() { return state.allLinks; }
    });

    function fitAll(animate) {
        var visible = state.allNodes.filter(function(n) { return filters.isNodeVisible(n); });
        if (!visible.length) return;
        var xs = visible.map(function(n) { return n.x; });
        var ys = visible.map(function(n) { return n.y; });
        var x0 = Math.min.apply(null, xs) - 80, y0 = Math.min.apply(null, ys) - 60;
        var x1 = Math.max.apply(null, xs) + 80, y1 = Math.max.apply(null, ys) + 80;
        var bw = x1 - x0, bh = y1 - y0;
        if (bw < 1 || bh < 1) return;
        var scale = Math.min(width / bw, height / bh, 1.8);
        var tx = (width - bw * scale) / 2 - x0 * scale;
        var ty = (height - bh * scale) / 2 - y0 * scale;
        var t = d3.zoomIdentity.translate(tx, ty).scale(scale);
        if (animate) svg.transition().duration(500).call(zoom.transform, t);
        else svg.call(zoom.transform, t);
    }

    function centerOnNode(node, animate) {
        var scale = 1.5;
        var t = d3.zoomIdentity.translate(width / 2 - node.x * scale, height / 2 - node.y * scale).scale(scale);
        if (animate) svg.transition().duration(600).call(zoom.transform, t);
        else svg.call(zoom.transform, t);
    }

    // Toolbar buttons
    var arrowBtn = document.getElementById('btn-arrows');
    arrowBtn.classList.add('active');
    arrowBtn.addEventListener('click', function() {
        state.arrowsVisible = !state.arrowsVisible;
        arrowBtn.classList.toggle('active', state.arrowsVisible);
        filters.applyVisibility();
    });
    document.getElementById('btn-reset-filters').addEventListener('click', function() {
        document.querySelectorAll('#type-filters input, #relation-filters input').forEach(function(cb) {
            cb.checked = true;
            var t = cb.getAttribute('data-type');
            if (cb.getAttribute('data-filter') === 'entity') state.activeTypes.add(t);
            else state.activeRelTypes.add(t);
        });
        state.arrowsVisible = true; arrowBtn.classList.add('active');
        searchInput.value = ''; state.searchQuery = '';
        filters.clearFocus(); filters.applyVisibility();
        if (window.history.replaceState) window.history.replaceState({}, '', window.location.pathname);
    });
    searchInput.addEventListener('input', function() {
        state.searchQuery = this.value.trim(); filters.applySearch();
    });
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
        state.locked = !state.locked;
        lockBtn.classList.toggle('active', state.locked);
        lockBtn.title = state.locked ? 'Unlock positions (L)' : 'Lock positions (L)';
        if (state.locked) {
            state.allNodes.forEach(function(n) { n.fx = n.x; n.fy = n.y; });
            if (state.simulation) state.simulation.alphaTarget(0).alpha(0);
        } else {
            state.allNodes.forEach(function(n) { n.fx = null; n.fy = null; });
            if (state.simulation) state.simulation.alphaTarget(0).alpha(0.3).restart();
        }
    });

    document.addEventListener('keydown', function(e) {
        if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') return;
        switch (e.key.toLowerCase()) {
            case 'f': fitAll(true); break;
            case '0': svg.transition().duration(500).call(zoom.transform, d3.zoomIdentity); break;
            case '=': case '+': svg.transition().duration(300).call(zoom.scaleBy, 1.5); break;
            case '-': svg.transition().duration(300).call(zoom.scaleBy, 1 / 1.5); break;
            case 'l': lockBtn.click(); break;
            case 'escape':
                detailPanel.hide(); contextMenu.dismiss();
                if (state.focusNodeId !== null) filters.clearFocus();
                break;
        }
    });

    // Fetch and render
    fetch('/ontology/api/graph').then(function(r) { return r.json(); }).then(function(data) {
        loading.style.display = 'none';
        state.allNodes = data.nodes;
        state.allEdges = data.edges;
        state.activeTypes = new Set(data.entity_types);
        var relTypeSet = new Set();
        data.edges.forEach(function(e) { relTypeSet.add(e.relation_type); });
        var relationTypes = Array.from(relTypeSet).sort();
        state.activeRelTypes = new Set(relationTypes);
        filters.buildFilters(data.entity_types, relationTypes);
        render();
    });

    function render() {
        var nodeMap = new Map(state.allNodes.map(function(n) { return [n.id, n]; }));
        state.allLinks = state.allEdges.map(function(e) {
            var sid = typeof e.source === 'object' ? e.source.id : e.source;
            var tid = typeof e.target === 'object' ? e.target.id : e.target;
            return { source: nodeMap.get(sid) || sid, target: nodeMap.get(tid) || tid,
                     relation_type: e.relation_type, relation_label: e.relation_label };
        }).filter(function(l) { return l.source && l.target; });

        g.selectAll('*').remove();
        state.linkGroup = g.append('g').attr('class', 'links')
            .selectAll('line').data(state.allLinks).join('line')
            .attr('stroke', 'var(--border-strong)').attr('stroke-width', 1.2)
            .attr('stroke-opacity', 0.6).attr('marker-end', 'url(#arrow)');
        state.edgeLabelGroup = g.append('g').attr('class', 'edge-labels')
            .selectAll('text').data(state.allLinks).join('text')
            .text(function(d) { return d.relation_type; })
            .attr('font-size', 8).attr('fill', 'var(--text-muted)')
            .attr('text-anchor', 'middle').attr('dy', -4)
            .style('pointer-events', 'none').style('font-family', 'var(--font-mono)');
        state.nodeGroup = g.append('g').attr('class', 'nodes')
            .selectAll('circle').data(state.allNodes).join('circle')
            .attr('r', function(d) { return d.entity_type === 'relation_type' ? 6 : 10; })
            .attr('fill', function(d) { return typeColor(d.entity_type); })
            .attr('stroke', '#fff').attr('stroke-width', 2).style('cursor', 'pointer')
            .call(d3.drag().on('start', dragStart).on('drag', dragging).on('end', dragEnd))
            .on('mouseover', function(ev, d) { highlightNode(d); })
            .on('mouseout', unhighlight)
            .on('click', function(ev, d) { detailPanel.show(d, state.allLinks); })
            .on('contextmenu', function(ev, d) { contextMenu.show(ev, d); });
        state.labelGroup = g.append('g').attr('class', 'labels')
            .selectAll('text').data(state.allNodes).join('text')
            .text(function(d) { return d.label || d.name; })
            .attr('font-size', 11).attr('font-weight', 500).attr('fill', 'var(--text)')
            .attr('dx', 14).attr('dy', 4)
            .style('pointer-events', 'none').style('font-family', 'var(--font-body)');
        state.highlightGroup = g.append('g').attr('class', 'highlights');

        var autoFitted = false;
        state.simulation = d3.forceSimulation(state.allNodes)
            .force('link', d3.forceLink(state.allLinks).id(function(d) { return d.id; }).distance(100))
            .force('charge', d3.forceManyBody().strength(-200))
            .force('x', d3.forceX(width / 2).strength(0.12))
            .force('y', d3.forceY(height / 2).strength(0.12))
            .force('collision', d3.forceCollide(25))
            .on('tick', function() {
                state.linkGroup.attr('x1', function(d) { return d.source.x; })
                    .attr('y1', function(d) { return d.source.y; })
                    .attr('x2', function(d) { return d.target.x; })
                    .attr('y2', function(d) { return d.target.y; });
                state.edgeLabelGroup.attr('x', function(d) { return (d.source.x + d.target.x) / 2; })
                    .attr('y', function(d) { return (d.source.y + d.target.y) / 2; });
                state.nodeGroup.attr('cx', function(d) { return d.x; }).attr('cy', function(d) { return d.y; });
                state.labelGroup.attr('x', function(d) { return d.x; }).attr('y', function(d) { return d.y; });
                if (state.highlightGroup) {
                    state.highlightGroup.selectAll('circle')
                        .attr('cx', function(d) { return d.x; }).attr('cy', function(d) { return d.y; });
                }
                if (!autoFitted && state.simulation.alpha() < 0.05) {
                    autoFitted = true; filters.applyVisibility(); fitAll(true);
                }
            });
        if (preFilterType) filters.applyVisibility();
    }

    function highlightNode(d) {
        if (state.searchQuery) return;
        var connected = new Set([d.id]);
        state.allLinks.forEach(function(l) {
            if (l.source.id === d.id || l.target.id === d.id) {
                connected.add(l.source.id); connected.add(l.target.id);
            }
        });
        state.nodeGroup.attr('opacity', function(n) { return connected.has(n.id) ? 1 : 0.15; });
        state.labelGroup.attr('opacity', function(n) { return connected.has(n.id) ? 1 : 0.1; });
        state.linkGroup.attr('stroke-opacity', function(l) {
            return (l.source.id === d.id || l.target.id === d.id) ? 0.9 : 0.05;
        });
        state.edgeLabelGroup.attr('opacity', function(l) {
            return (l.source.id === d.id || l.target.id === d.id) ? 1 : 0.1;
        });
    }
    function unhighlight() {
        if (state.searchQuery) { filters.applySearch(); return; }
        state.nodeGroup.attr('opacity', 1); state.labelGroup.attr('opacity', 1);
        state.linkGroup.attr('stroke-opacity', 0.6); state.edgeLabelGroup.attr('opacity', 1);
    }
    function dragStart(event, d) {
        if (!event.active) state.simulation.alphaTarget(0.3).restart();
        d.fx = d.x; d.fy = d.y;
    }
    function dragging(event, d) { d.fx = event.x; d.fy = event.y; }
    function dragEnd(event, d) {
        if (!event.active) state.simulation.alphaTarget(0);
        if (!state.locked) { d.fx = null; d.fy = null; }
    }
})();
