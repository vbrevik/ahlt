/**
 * Governance map — orchestrator (SVG setup, zoom, toolkit, fetch).
 * Depends on: graph-toolkit.js, governance-map-render.js
 */
(function() {
    var canvas = document.getElementById('gov-canvas');
    if (!canvas) return;
    var loading = document.getElementById('gov-loading');
    var statEl = document.getElementById('gov-stat');

    var width = canvas.clientWidth;
    var height = canvas.clientHeight || 463;

    var svg = d3.select('#gov-canvas')
        .append('svg')
        .attr('width', '100%')
        .attr('height', '100%')
        .attr('viewBox', [0, 0, width, height]);

    // Arrow markers for each edge type
    var EDGE_COLORS = {
        feeds_into:   { stroke: '#2563eb' },
        escalates_to: { stroke: '#d97706' }
    };
    var BLOCKING_COLOR = '#b91c1c';
    var defs = svg.append('defs');
    ['feeds_into', 'escalates_to', 'blocking'].forEach(function(type) {
        var color = type === 'blocking' ? BLOCKING_COLOR
            : (EDGE_COLORS[type] ? EDGE_COLORS[type].stroke : '#78716c');
        defs.append('marker')
            .attr('id', 'gov-arrow-' + type)
            .attr('viewBox', '0 -5 10 10')
            .attr('refX', 10).attr('refY', 0)
            .attr('markerWidth', 7).attr('markerHeight', 7)
            .attr('orient', 'auto')
            .append('path').attr('d', 'M0,-4L10,0L0,4').attr('fill', color);
    });

    var g = svg.append('g');
    var zoom = d3.zoom().scaleExtent([0.2, 5])
        .on('zoom', function(e) { g.attr('transform', e.transform); });
    svg.call(zoom);

    var toolkit = graphToolkit({
        svg: svg, zoomBehavior: zoom, width: width, height: height,
        getNodePositions: function() {
            var xs = [], ys = [];
            g.selectAll('.gov-graph-node').each(function(d) { xs.push(d.x); ys.push(d.y); });
            return { xs: xs, ys: ys };
        },
        fitPadding: { top: 80, right: 260, bottom: 140, left: 80 },
        maxScale: 1.5
    });
    toolkit.setupToolbar({ fit: 'gov-btn-fit', reset: 'gov-btn-reset', zoomIn: 'gov-btn-zoom-in', zoomOut: 'gov-btn-zoom-out' });
    toolkit.setupKeyboardShortcuts();

    var renderer = governanceMapRender({
        g: g, toolkit: toolkit, width: width, height: height, statEl: statEl
    });

    var FETCH_TIMEOUT_MS = 30000;
    fetch('/api/governance/graph', { signal: AbortSignal.timeout(FETCH_TIMEOUT_MS) })
        .then(function(r) { return r.json(); })
        .then(function(data) {
            loading.style.display = 'none';
            renderer.render(data);
        })
        .catch(function(e) {
            loading.style.display = 'none';
            canvas.textContent = (e.name === 'TimeoutError' || e.name === 'AbortError')
                ? 'Request timed out. Please refresh to retry.'
                : 'Failed to load graph data.';
        });
})();
