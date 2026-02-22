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
    var focusNeighbors = new Set();

    // URL param for pre-filtering
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
        if (!activeTypes.has(l.source.entity_type) || !activeTypes.has(l.target.entity_type)) return false;
        if (!activeRelTypes.has(l.relation_type)) return false;
        if (focusNodeId !== null) {
            if (l.source.id !== focusNodeId && l.target.id !== focusNodeId) return false;
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
        linkGroup.attr('marker-end', arrowsVisible ? 'url(#arrow)' : null);
        statEl.textContent = visibleNodeIds.size + ' nodes \u00b7 ' + visibleEdges + ' edges';
        applySearch();
    }

    // Search highlighting
    function applySearch() {
        if (highlightGroup) highlightGroup.selectAll('*').remove();
        if (!searchQuery) {
            nodeGroup.attr('opacity', function(d) { return isNodeVisible(d) ? 1 : 0; });
            labelGroup.attr('opacity', function(d) { return isNodeVisible(d) ? 1 : 0; });
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
            if (!isNodeVisible(d)) return 0;
            return matchIds.has(d.id) ? 1 : 0.15;
        });
        labelGroup.attr('opacity', function(d) {
            if (!isNodeVisible(d)) return 0;
            return matchIds.has(d.id) ? 1 : 0.1;
        });
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
        var typeCounts = {};
        allNodes.forEach(function(n) {
            typeCounts[n.entity_type] = (typeCounts[n.entity_type] || 0) + 1;
        });
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
        removeFocusPill();
        var node = allNodes.find(function(n) { return n.id === nodeId; });
        if (!node) return;
        var pill = createEl('button', { 'class': 'focus-pill', id: 'focus-pill' });
        pill.appendChild(document.createTextNode('Focus: ' + (node.label || node.name) + ' '));
        var close = createEl('span', { 'class': 'focus-pill__close' }, '\u2715');
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
        var headerLabel = createEl('span', { 'class': 'context-menu__header-label' }, d.label || d.name);
        var headerBadge = createEl('span', { 'class': 'context-menu__header-badge' }, d.entity_type);
        headerBadge.style.color = typeColor(d.entity_type);
        header.appendChild(headerLabel);
        header.appendChild(headerBadge);
        menu.appendChild(header);
        menu.appendChild(createEl('div', { 'class': 'context-menu__divider' }));

        // Focus action
        var focusAction = createEl('div', { 'class': 'context-menu__action' });
        focusAction.appendChild(createEl('span', { 'class': 'context-menu__action-icon' }, '\u25C9'));
        focusAction.appendChild(document.createTextNode('Focus on this node'));
        focusAction.addEventListener('click', function() {
            dismissContextMenu();
            setFocus(d.id);
        });
        menu.appendChild(focusAction);

        // Detail action
        var detailAction = createEl('div', { 'class': 'context-menu__action' });
        detailAction.appendChild(createEl('span', { 'class': 'context-menu__action-icon' }, '\u2197'));
        detailAction.appendChild(document.createTextNode('Open full detail'));
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
            var groups = {};
            allConns.forEach(function(c) {
                if (!groups[c.relType]) groups[c.relType] = [];
                groups[c.relType].push(c);
            });
            Object.keys(groups).forEach(function(relType) {
                var items = groups[relType];
                var MAX_ITEMS = 5;
                var group = createEl('div', { 'class': 'context-menu__group' });
                if (items.length > MAX_ITEMS) group.classList.add('collapsed');

                var groupHeader = createEl('div', { 'class': 'context-menu__group-header' });
                groupHeader.appendChild(createEl('span', { 'class': 'context-menu__group-toggle' }, '\u25BE'));
                groupHeader.appendChild(document.createTextNode(' ' + relType + ' '));
                groupHeader.appendChild(createEl('span', { 'class': 'context-menu__group-count' }, '(' + items.length + ')'));
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
                    item.appendChild(nameSpan);
                    item.appendChild(createEl('span', { 'class': 'context-menu__item-arrow' }, '\u2192'));
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
                        items.slice(MAX_ITEMS).forEach(function(c) {
                            var item = createEl('div', { 'class': 'context-menu__item' });
                            var nameSpan = createEl('span', null, c.node.label || c.node.name);
                            nameSpan.style.color = typeColor(c.node.entity_type);
                            item.appendChild(nameSpan);
                            item.appendChild(createEl('span', { 'class': 'context-menu__item-arrow' }, '\u2192'));
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

        // Position relative to container
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
            .selectAll('line').data(allLinks).join('line')
            .attr('stroke', 'var(--border-strong)')
            .attr('stroke-width', 1.2)
            .attr('stroke-opacity', 0.6)
            .attr('marker-end', 'url(#arrow)');

        edgeLabelGroup = g.append('g').attr('class', 'edge-labels')
            .selectAll('text').data(allLinks).join('text')
            .text(function(d) { return d.relation_type; })
            .attr('font-size', 8)
            .attr('fill', 'var(--text-muted)')
            .attr('text-anchor', 'middle')
            .attr('dy', -4)
            .style('pointer-events', 'none')
            .style('font-family', 'var(--font-mono)');

        nodeGroup = g.append('g').attr('class', 'nodes')
            .selectAll('circle').data(allNodes).join('circle')
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
            .selectAll('text').data(allNodes).join('text')
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

        if (preFilterType) applyVisibility();
    }

    function highlightNode(d) {
        if (searchQuery) return;
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
        document.getElementById('detail-type').textContent = d.entity_type;
        document.getElementById('detail-type').style.color = typeColor(d.entity_type);
        document.getElementById('detail-label').textContent = d.label || d.name;
        document.getElementById('detail-meta').textContent = '#' + d.id + ' \u00b7 ' + d.name;

        var propsEl = document.getElementById('detail-props');
        while (propsEl.firstChild) propsEl.removeChild(propsEl.firstChild);
        if (d.properties && Object.keys(d.properties).length > 0) {
            propsEl.appendChild(createEl('span', { 'class': 'conn-heading' }, 'Properties'));
            Object.keys(d.properties).forEach(function(key) {
                var val = d.properties[key];
                if (key === 'password') val = '\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022';
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
                row.appendChild(document.createTextNode(' \u2192 '));
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
                row.appendChild(document.createTextNode(' \u2192 '));
                row.appendChild(createEl('code', null, l.relation_type));
                connEl.appendChild(row);
            });
        }
        if (!outgoing.length && !incoming.length) {
            connEl.appendChild(createEl('span', { 'class': 'conn-empty' }, 'No connections'));
        }

        var actionsEl = document.getElementById('detail-actions');
        while (actionsEl.firstChild) actionsEl.removeChild(actionsEl.firstChild);
        actionsEl.appendChild(createEl('a', { 'class': 'btn btn-sm', href: '/ontology/data/' + d.id }, 'Full detail'));
    }

    document.getElementById('btn-close-detail').addEventListener('click', function() {
        detail.style.display = 'none';
    });

    function dragStart(event, d) {
        if (!event.active) simulation.alphaTarget(0.3).restart();
        d.fx = d.x; d.fy = d.y;
    }
    function dragging(event, d) {
        d.fx = event.x; d.fy = event.y;
    }
    function dragEnd(event, d) {
        if (!event.active) simulation.alphaTarget(0);
        if (!locked) { d.fx = null; d.fy = null; }
    }
})();
