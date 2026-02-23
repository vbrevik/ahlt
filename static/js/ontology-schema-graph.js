/**
 * Ontology schema graph — core (setup, render, simulation, highlight).
 * Depends on: graph-helpers.js, graph-toolkit.js, schema-graph-panels.js
 */
(function() {
    var typeColor = graphHelpers.typeColor;
    var createEl = graphHelpers.createEl;

    var canvas = document.getElementById('graph-canvas');
    var loading = document.getElementById('graph-loading');
    var statEl = document.getElementById('toolbar-stat');

    var width = canvas.clientWidth;
    var height = canvas.clientHeight || 600;

    var svg = d3.select('#graph-canvas')
        .append('svg')
        .attr('width', '100%').attr('height', '100%')
        .attr('viewBox', [0, 0, width, height]);

    svg.append('defs').selectAll('marker')
        .data(['schema-arrow']).join('marker')
        .attr('id', 'schema-arrow').attr('viewBox', '0 -5 10 10')
        .attr('refX', 32).attr('refY', 0)
        .attr('markerWidth', 6).attr('markerHeight', 6).attr('orient', 'auto')
        .append('path').attr('d', 'M0,-5L10,0L0,5').attr('fill', 'var(--text-muted)');

    var g = svg.append('g');
    var zoom = d3.zoom().scaleExtent([0.2, 5])
        .on('zoom', function(e) { g.attr('transform', e.transform); });
    svg.call(zoom);

    var simulation, linkGroup, nodeGroup, edgeLabelGroup;
    var allNodes = [], allLinks = [];
    var locked = false;

    var toolkit = graphToolkit({
        svg: svg, zoomBehavior: zoom, width: width, height: height,
        getNodePositions: function() {
            return { xs: allNodes.map(function(n) { return n.x; }), ys: allNodes.map(function(n) { return n.y; }) };
        },
        fitPadding: { top: 60, right: 80, bottom: 80, left: 80 }, maxScale: 1.8
    });
    toolkit.setupToolbar({ fit: 'btn-fit', reset: 'btn-reset', zoomIn: 'btn-zoom-in', zoomOut: 'btn-zoom-out' });

    var panels = schemaGraphPanels({
        typeColor: typeColor, createEl: createEl,
        allLinks: function() { return allLinks; },
        containerEl: document.querySelector('.graph-container'),
        detailEl: document.getElementById('graph-detail')
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

    toolkit.setupKeyboardShortcuts(function(e) {
        switch (e.key.toLowerCase()) {
            case 'l': lockBtn.click(); break;
            case 'escape':
                panels.dismissContextMenu();
                document.getElementById('graph-detail').style.display = 'none';
                break;
        }
    });

    var FETCH_TIMEOUT_MS = 30000;
    fetch('/ontology/api/schema', { signal: AbortSignal.timeout(FETCH_TIMEOUT_MS) })
        .then(function(r) { return r.json(); })
        .then(function(data) { loading.style.display = 'none'; render(data); })
        .catch(function(e) {
            loading.style.display = 'none';
            canvas.textContent = (e.name === 'TimeoutError' || e.name === 'AbortError')
                ? 'Request timed out. Please refresh to retry.' : 'Failed to load graph data.';
        });

    function render(data) {
        allNodes = data.nodes;
        statEl.textContent = data.nodes.length + ' types \u00b7 ' + data.edges.length + ' relations';

        var nodeMap = new Map(data.nodes.map(function(n) { return [n.id, n]; }));
        allLinks = data.edges.map(function(e) {
            return { source: nodeMap.get(e.source), target: nodeMap.get(e.target),
                     relation_type: e.relation_type, relation_label: e.relation_label, count: e.count };
        }).filter(function(l) { return l.source && l.target; });

        linkGroup = g.append('g').attr('class', 'links')
            .selectAll('line').data(allLinks).join('line')
            .attr('stroke', 'var(--border-strong)')
            .attr('stroke-width', function(d) { return Math.max(1.5, Math.min(4, d.count)); })
            .attr('stroke-opacity', 0.5).attr('marker-end', 'url(#schema-arrow)');

        edgeLabelGroup = g.append('g').attr('class', 'edge-labels')
            .selectAll('text').data(allLinks).join('text')
            .text(function(d) { return d.relation_type + ' (' + d.count + ')'; })
            .attr('font-size', 10).attr('fill', 'var(--text-muted)')
            .attr('text-anchor', 'middle').attr('dy', -6)
            .style('pointer-events', 'none').style('font-family', 'var(--font-mono)');

        nodeGroup = g.append('g').attr('class', 'nodes')
            .selectAll('g').data(data.nodes).join('g')
            .style('cursor', 'pointer')
            .call(d3.drag().on('start', dragStart).on('drag', dragging).on('end', dragEnd))
            .on('mouseover', function(ev, d) { highlightNode(d); })
            .on('mouseout', unhighlight)
            .on('click', function(ev, d) { panels.showDetail(d); })
            .on('contextmenu', function(ev, d) { panels.showContextMenu(ev, d); });

        nodeGroup.append('circle')
            .attr('r', function(d) { return Math.max(18, Math.min(40, 12 + d.count * 2)); })
            .attr('fill', function(d) { return typeColor(d.id); })
            .attr('stroke', '#fff').attr('stroke-width', 3).attr('opacity', 0.9);
        nodeGroup.append('text')
            .text(function(d) { return d.count; })
            .attr('text-anchor', 'middle').attr('dy', '0.35em')
            .attr('font-size', 13).attr('font-weight', 700).attr('fill', '#fff')
            .style('pointer-events', 'none').style('font-family', 'var(--font-mono)');
        nodeGroup.append('text')
            .text(function(d) { return d.label; })
            .attr('text-anchor', 'middle')
            .attr('dy', function(d) { return Math.max(18, Math.min(40, 12 + d.count * 2)) + 16; })
            .attr('font-size', 13).attr('font-weight', 600).attr('fill', 'var(--text)')
            .style('pointer-events', 'none').style('font-family', 'var(--font-body)');

        var autoFitted = false;
        simulation = d3.forceSimulation(data.nodes)
            .force('link', d3.forceLink(allLinks).id(function(d) { return d.id; }).distance(160))
            .force('charge', d3.forceManyBody().strength(-350))
            .force('x', d3.forceX(width / 2).strength(0.12))
            .force('y', d3.forceY(height / 2).strength(0.12))
            .force('collision', d3.forceCollide(55))
            .on('tick', function() {
                linkGroup.attr('x1', function(d) { return d.source.x; }).attr('y1', function(d) { return d.source.y; })
                    .attr('x2', function(d) { return d.target.x; }).attr('y2', function(d) { return d.target.y; });
                edgeLabelGroup.attr('x', function(d) { return (d.source.x + d.target.x) / 2; })
                    .attr('y', function(d) { return (d.source.y + d.target.y) / 2; });
                nodeGroup.attr('transform', function(d) { return 'translate(' + d.x + ',' + d.y + ')'; });
                if (!autoFitted && simulation.alpha() < 0.05) { autoFitted = true; toolkit.fitToView(true); }
            });

        function dragStart(event, d) { if (!event.active) simulation.alphaTarget(0.3).restart(); d.fx = d.x; d.fy = d.y; }
        function dragging(event, d) { d.fx = event.x; d.fy = event.y; }
        function dragEnd(event, d) { if (!event.active) simulation.alphaTarget(0); if (!locked) { d.fx = null; d.fy = null; } }
    }

    function highlightNode(d) {
        var connected = new Set([d.id]);
        linkGroup.each(function(l) {
            if (l.source.id === d.id || l.target.id === d.id) { connected.add(l.source.id); connected.add(l.target.id); }
        });
        nodeGroup.attr('opacity', function(n) { return connected.has(n.id) ? 1 : 0.2; });
        linkGroup.attr('stroke-opacity', function(l) { return (l.source.id === d.id || l.target.id === d.id) ? 0.8 : 0.05; });
        edgeLabelGroup.attr('opacity', function(l) { return (l.source.id === d.id || l.target.id === d.id) ? 1 : 0.1; });
    }
    function unhighlight() {
        nodeGroup.attr('opacity', 1); linkGroup.attr('stroke-opacity', 0.5); edgeLabelGroup.attr('opacity', 1);
    }
})();
