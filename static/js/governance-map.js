(function() {
    var canvas = document.getElementById('gov-canvas');
    if (!canvas) return;
    var loading = document.getElementById('gov-loading');
    var statEl = document.getElementById('gov-stat');

    var EDGE_COLORS = {
        feeds_into:   { stroke: '#2563eb', label: '#1d4ed8' },
        escalates_to: { stroke: '#d97706', label: '#92400e' }
    };
    var BLOCKING_COLOR = '#b91c1c';
    var NODE_FILL = '#ffffff';
    var NODE_STROKE = '#d6d3d1';
    var NODE_HOVER_STROKE = '#b45309';

    var CADENCE_LABELS = {
        daily: 'Daily', working_days: 'Work days', weekly: 'Weekly',
        biweekly: 'Biweekly', monthly: 'Monthly', 'ad-hoc': 'Ad-hoc'
    };
    var STATUS_COLORS = {
        active: '#059669', draft: '#d97706', archived: '#78716c'
    };

    var width = canvas.clientWidth;
    var height = canvas.clientHeight || 463;

    var svg = d3.select('#gov-canvas')
        .append('svg')
        .attr('width', '100%')
        .attr('height', '100%')
        .attr('viewBox', [0, 0, width, height]);

    // Arrow markers for each edge type
    var defs = svg.append('defs');
    ['feeds_into', 'escalates_to', 'blocking'].forEach(function(type) {
        var color = type === 'blocking' ? BLOCKING_COLOR
            : (EDGE_COLORS[type] ? EDGE_COLORS[type].stroke : '#78716c');
        defs.append('marker')
            .attr('id', 'gov-arrow-' + type)
            .attr('viewBox', '0 -5 10 10')
            .attr('refX', 10)
            .attr('refY', 0)
            .attr('markerWidth', 7)
            .attr('markerHeight', 7)
            .attr('orient', 'auto')
            .append('path')
            .attr('d', 'M0,-4L10,0L0,4')
            .attr('fill', color);
    });

    var g = svg.append('g');

    var zoom = d3.zoom()
        .scaleExtent([0.2, 5])
        .on('zoom', function(e) { g.attr('transform', e.transform); });
    svg.call(zoom);

    // Initialize shared graph toolkit
    // Governance nodes are rectangles (170x56), so right/bottom padding accounts for node size
    var toolkit = graphToolkit({
        svg: svg,
        zoomBehavior: zoom,
        width: width,
        height: height,
        getNodePositions: function() {
            var xs = [], ys = [];
            g.selectAll('.gov-graph-node').each(function(d) { xs.push(d.x); ys.push(d.y); });
            return { xs: xs, ys: ys };
        },
        fitPadding: { top: 80, right: 260, bottom: 140, left: 80 },
        maxScale: 1.5
    });

    // Toolbar (shared)
    toolkit.setupToolbar({
        fit: 'gov-btn-fit',
        reset: 'gov-btn-reset',
        zoomIn: 'gov-btn-zoom-in',
        zoomOut: 'gov-btn-zoom-out'
    });

    // Keyboard shortcuts (shared, no extras for governance)
    toolkit.setupKeyboardShortcuts();

    var FETCH_TIMEOUT_MS = 30000;
    fetch('/api/governance/graph', { signal: AbortSignal.timeout(FETCH_TIMEOUT_MS) })
        .then(function(r) { return r.json(); })
        .then(function(data) {
            loading.style.display = 'none';
            renderGraph(data);
        })
        .catch(function(e) {
            loading.style.display = 'none';
            var msg = (e.name === 'TimeoutError' || e.name === 'AbortError')
                ? 'Request timed out. Please refresh to retry.'
                : 'Failed to load graph data.';
            canvas.textContent = msg;
        });

    function renderGraph(data) {
        var nodes = data.nodes;
        var edges = data.edges;

        statEl.textContent = nodes.length + ' ToRs \u00b7 ' + edges.length + ' dependencies';

        if (!nodes.length) {
            g.append('text')
                .attr('x', width / 2).attr('y', height / 2)
                .attr('text-anchor', 'middle')
                .attr('fill', 'var(--text-muted)')
                .attr('font-size', 14)
                .text('No ToRs to display');
            return;
        }

        // Build dagre graph
        var dagreGraph = new dagre.graphlib.Graph();
        dagreGraph.setGraph({
            rankdir: 'LR',
            nodesep: 40,
            ranksep: 100,
            marginx: 40,
            marginy: 40
        });
        dagreGraph.setDefaultEdgeLabel(function() { return {}; });

        var nodeW = 170, nodeH = 56;
        nodes.forEach(function(n) {
            dagreGraph.setNode(String(n.id), { width: nodeW, height: nodeH });
        });
        edges.forEach(function(e) {
            dagreGraph.setEdge(String(e.source), String(e.target));
        });

        dagre.layout(dagreGraph);

        // Read computed positions back onto our data
        var nodeMap = {};
        nodes.forEach(function(n) {
            var pos = dagreGraph.node(String(n.id));
            n.x = pos.x;
            n.y = pos.y;
            nodeMap[n.id] = n;
        });

        // Draw edges (paths with curves via dagre points)
        var edgeGroup = g.append('g').attr('class', 'gov-graph-edges');
        var line = d3.line().curve(d3.curveBasis);

        edges.forEach(function(e) {
            var dagreEdge = dagreGraph.edge(String(e.source), String(e.target));
            var points = dagreEdge.points.map(function(p) { return [p.x, p.y]; });

            var isBlocking = e.is_blocking;
            var type = e.relation_type;
            var edgeColor = isBlocking ? BLOCKING_COLOR
                : (EDGE_COLORS[type] ? EDGE_COLORS[type].stroke : '#78716c');
            var markerType = isBlocking ? 'blocking' : type;

            edgeGroup.append('path')
                .attr('d', line(points))
                .attr('fill', 'none')
                .attr('stroke', edgeColor)
                .attr('stroke-width', isBlocking ? 2.5 : 1.8)
                .attr('stroke-dasharray', type === 'escalates_to' ? '6,4' : (isBlocking ? '4,3' : 'none'))
                .attr('marker-end', 'url(#gov-arrow-' + markerType + ')')
                .attr('opacity', 0.75);

            // Edge label at midpoint
            var mid = points[Math.floor(points.length / 2)];
            var labelText = type === 'feeds_into' ? 'feeds' : 'escalates';
            if (isBlocking) labelText += ' (blocking)';

            edgeGroup.append('text')
                .attr('x', mid[0])
                .attr('y', mid[1] - 8)
                .attr('text-anchor', 'middle')
                .attr('font-size', 10)
                .attr('fill', isBlocking ? BLOCKING_COLOR : (EDGE_COLORS[type] ? EDGE_COLORS[type].label : '#78716c'))
                .attr('font-family', 'var(--font-mono)')
                .text(labelText);
        });

        // Draw nodes
        var nodeGroup = g.selectAll('.gov-graph-node')
            .data(nodes)
            .join('g')
            .attr('class', 'gov-graph-node')
            .attr('transform', function(d) { return 'translate(' + d.x + ',' + d.y + ')'; })
            .style('cursor', 'pointer')
            .on('click', function(event, d) {
                window.location = '/tor/' + d.id;
            })
            .on('mouseover', function(event, d) {
                d3.select(this).select('rect').attr('stroke', NODE_HOVER_STROKE).attr('stroke-width', 2);
                highlightConnected(d.id);
            })
            .on('mouseout', function() {
                d3.select(this).select('rect').attr('stroke', NODE_STROKE).attr('stroke-width', 1);
                unhighlight();
            });

        // Node rectangle
        nodeGroup.append('rect')
            .attr('x', -nodeW / 2)
            .attr('y', -nodeH / 2)
            .attr('width', nodeW)
            .attr('height', nodeH)
            .attr('rx', 8)
            .attr('ry', 8)
            .attr('fill', NODE_FILL)
            .attr('stroke', NODE_STROKE)
            .attr('stroke-width', 1);

        // Status dot
        nodeGroup.append('circle')
            .attr('cx', -nodeW / 2 + 14)
            .attr('cy', -nodeH / 2 + 16)
            .attr('r', 4)
            .attr('fill', function(d) { return STATUS_COLORS[d.status] || '#78716c'; });

        // Label text
        nodeGroup.append('text')
            .attr('x', 0)
            .attr('y', -4)
            .attr('text-anchor', 'middle')
            .attr('font-size', 12.5)
            .attr('font-weight', 600)
            .attr('fill', 'var(--text)')
            .attr('font-family', 'var(--font-body)')
            .each(function(d) {
                var label = d.label;
                if (label.length > 20) label = label.substring(0, 18) + '\u2026';
                d3.select(this).text(label);
            });

        // Cadence badge
        nodeGroup.append('text')
            .attr('x', 0)
            .attr('y', 14)
            .attr('text-anchor', 'middle')
            .attr('font-size', 10)
            .attr('fill', 'var(--text-muted)')
            .attr('font-family', 'var(--font-mono)')
            .text(function(d) {
                var parts = [];
                if (d.cadence) parts.push(CADENCE_LABELS[d.cadence] || d.cadence);
                if (d.cadence_day) parts.push(d.cadence_day.charAt(0).toUpperCase() + d.cadence_day.slice(1, 3));
                if (d.cadence_time) parts.push(d.cadence_time);
                return parts.join(' \u00b7 ') || '';
            });

        // Highlight connected nodes on hover
        var edgeIndex = {};
        edges.forEach(function(e) {
            if (!edgeIndex[e.source]) edgeIndex[e.source] = [];
            if (!edgeIndex[e.target]) edgeIndex[e.target] = [];
            edgeIndex[e.source].push(e.target);
            edgeIndex[e.target].push(e.source);
        });

        function highlightConnected(id) {
            var connected = new Set();
            connected.add(id);
            (edgeIndex[id] || []).forEach(function(nid) { connected.add(nid); });
            nodeGroup.attr('opacity', function(n) { return connected.has(n.id) ? 1 : 0.25; });
            edgeGroup.selectAll('path').attr('opacity', function() { return 0.3; });
            edgeGroup.selectAll('text').attr('opacity', function() { return 0.3; });
            // Re-highlight connected edges
            edges.forEach(function(e, i) {
                if (e.source === id || e.target === id) {
                    edgeGroup.selectAll('path').filter(function(d, j) { return j === i; }).attr('opacity', 1);
                    edgeGroup.selectAll('text').filter(function(d, j) { return j === i; }).attr('opacity', 1);
                }
            });
        }

        function unhighlight() {
            nodeGroup.attr('opacity', 1);
            edgeGroup.selectAll('path').attr('opacity', 0.75);
            edgeGroup.selectAll('text').attr('opacity', 1);
        }

        // Fit to view after rendering
        setTimeout(function() { toolkit.fitToView(false); }, 50);
    }
})();
