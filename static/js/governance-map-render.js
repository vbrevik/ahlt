/**
 * Governance map — graph rendering (dagre layout, nodes, edges, highlighting).
 *
 * Factory: governanceMapRender(deps)
 *   deps.g — D3 <g> selection for drawing
 *   deps.toolkit — graphToolkit instance
 *   deps.width, deps.height — viewport dimensions
 *
 * Returns { render(data) }
 */
function governanceMapRender(deps) {
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
    var STATUS_COLORS = { active: '#059669', draft: '#d97706', archived: '#78716c' };

    function render(data) {
        var g = deps.g;
        var nodes = data.nodes;
        var edges = data.edges;

        deps.statEl.textContent = nodes.length + ' ToRs \u00b7 ' + edges.length + ' dependencies';

        if (!nodes.length) {
            g.append('text')
                .attr('x', deps.width / 2).attr('y', deps.height / 2)
                .attr('text-anchor', 'middle').attr('fill', 'var(--text-muted)')
                .attr('font-size', 14).text('No ToRs to display');
            return;
        }

        // Dagre layout
        var dagreGraph = new dagre.graphlib.Graph();
        dagreGraph.setGraph({ rankdir: 'LR', nodesep: 40, ranksep: 100, marginx: 40, marginy: 40 });
        dagreGraph.setDefaultEdgeLabel(function() { return {}; });

        var nodeW = 170, nodeH = 56;
        nodes.forEach(function(n) { dagreGraph.setNode(String(n.id), { width: nodeW, height: nodeH }); });
        edges.forEach(function(e) { dagreGraph.setEdge(String(e.source), String(e.target)); });
        dagre.layout(dagreGraph);

        var nodeMap = {};
        nodes.forEach(function(n) {
            var pos = dagreGraph.node(String(n.id));
            n.x = pos.x; n.y = pos.y; nodeMap[n.id] = n;
        });

        // Edges
        var edgeGroup = g.append('g').attr('class', 'gov-graph-edges');
        var line = d3.line().curve(d3.curveBasis);

        edges.forEach(function(e) {
            var dagreEdge = dagreGraph.edge(String(e.source), String(e.target));
            var points = dagreEdge.points.map(function(p) { return [p.x, p.y]; });
            var isBlocking = e.is_blocking;
            var type = e.relation_type;
            var edgeColor = isBlocking ? BLOCKING_COLOR : (EDGE_COLORS[type] ? EDGE_COLORS[type].stroke : '#78716c');
            var markerType = isBlocking ? 'blocking' : type;

            edgeGroup.append('path')
                .attr('d', line(points)).attr('fill', 'none')
                .attr('stroke', edgeColor)
                .attr('stroke-width', isBlocking ? 2.5 : 1.8)
                .attr('stroke-dasharray', type === 'escalates_to' ? '6,4' : (isBlocking ? '4,3' : 'none'))
                .attr('marker-end', 'url(#gov-arrow-' + markerType + ')').attr('opacity', 0.75);

            var mid = points[Math.floor(points.length / 2)];
            var labelText = type === 'feeds_into' ? 'feeds' : 'escalates';
            if (isBlocking) labelText += ' (blocking)';
            edgeGroup.append('text')
                .attr('x', mid[0]).attr('y', mid[1] - 8)
                .attr('text-anchor', 'middle').attr('font-size', 10)
                .attr('fill', isBlocking ? BLOCKING_COLOR : (EDGE_COLORS[type] ? EDGE_COLORS[type].label : '#78716c'))
                .attr('font-family', 'var(--font-mono)').text(labelText);
        });

        // Nodes
        var nodeGroup = g.selectAll('.gov-graph-node')
            .data(nodes).join('g')
            .attr('class', 'gov-graph-node')
            .attr('transform', function(d) { return 'translate(' + d.x + ',' + d.y + ')'; })
            .style('cursor', 'pointer')
            .on('click', function(ev, d) { window.location = '/tor/' + d.id; })
            .on('mouseover', function(ev, d) {
                d3.select(this).select('rect').attr('stroke', NODE_HOVER_STROKE).attr('stroke-width', 2);
                highlightConnected(d.id);
            })
            .on('mouseout', function() {
                d3.select(this).select('rect').attr('stroke', NODE_STROKE).attr('stroke-width', 1);
                unhighlight();
            });

        nodeGroup.append('rect')
            .attr('x', -nodeW / 2).attr('y', -nodeH / 2)
            .attr('width', nodeW).attr('height', nodeH)
            .attr('rx', 8).attr('ry', 8)
            .attr('fill', NODE_FILL).attr('stroke', NODE_STROKE).attr('stroke-width', 1);
        nodeGroup.append('circle')
            .attr('cx', -nodeW / 2 + 14).attr('cy', -nodeH / 2 + 16).attr('r', 4)
            .attr('fill', function(d) { return STATUS_COLORS[d.status] || '#78716c'; });
        nodeGroup.append('text')
            .attr('x', 0).attr('y', -4).attr('text-anchor', 'middle')
            .attr('font-size', 12.5).attr('font-weight', 600)
            .attr('fill', 'var(--text)').attr('font-family', 'var(--font-body)')
            .each(function(d) {
                var label = d.label;
                if (label.length > 20) label = label.substring(0, 18) + '\u2026';
                d3.select(this).text(label);
            });
        nodeGroup.append('text')
            .attr('x', 0).attr('y', 14).attr('text-anchor', 'middle')
            .attr('font-size', 10).attr('fill', 'var(--text-muted)')
            .attr('font-family', 'var(--font-mono)')
            .text(function(d) {
                var parts = [];
                if (d.cadence) parts.push(CADENCE_LABELS[d.cadence] || d.cadence);
                if (d.cadence_day) parts.push(d.cadence_day.charAt(0).toUpperCase() + d.cadence_day.slice(1, 3));
                if (d.cadence_time) parts.push(d.cadence_time);
                return parts.join(' \u00b7 ') || '';
            });

        // Highlight connected on hover
        var edgeIndex = {};
        edges.forEach(function(e) {
            if (!edgeIndex[e.source]) edgeIndex[e.source] = [];
            if (!edgeIndex[e.target]) edgeIndex[e.target] = [];
            edgeIndex[e.source].push(e.target);
            edgeIndex[e.target].push(e.source);
        });

        function highlightConnected(id) {
            var connected = new Set([id]);
            (edgeIndex[id] || []).forEach(function(nid) { connected.add(nid); });
            nodeGroup.attr('opacity', function(n) { return connected.has(n.id) ? 1 : 0.25; });
            edgeGroup.selectAll('path').attr('opacity', 0.3);
            edgeGroup.selectAll('text').attr('opacity', 0.3);
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

        setTimeout(function() { deps.toolkit.fitToView(false); }, 50);
    }

    return { render: render };
}
