/**
 * Ontology graph — right-click context menu.
 *
 * Factory: ontologyGraphContextMenu(deps)
 *   deps.container — .graph-container DOM element
 *   deps.typeColor — function(entityType) → color string
 *   deps.createEl  — function(tag, attrs, text) → element
 *   deps.centerOnNode — function(node, animate)
 *   deps.showDetail — function(node, links)
 *   deps.setFocus  — function(nodeId)
 *   deps.allLinks  — function() → current links array
 *
 * Returns { show(event, d), dismiss() }
 */
function ontologyGraphContextMenu(deps) {
    var container = deps.container;
    var typeColor = deps.typeColor;
    var createEl = deps.createEl;
    var contextMenuEl = null;

    function dismiss() {
        if (contextMenuEl) { contextMenuEl.remove(); contextMenuEl = null; }
    }

    document.addEventListener('click', function(e) {
        if (contextMenuEl && !contextMenuEl.contains(e.target)) dismiss();
    });

    function show(event, d) {
        event.preventDefault();
        dismiss();
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
            dismiss();
            deps.setFocus(d.id);
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
        var allLinks = deps.allLinks();
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
                groupHeader.addEventListener('click', function() { group.classList.toggle('collapsed'); });
                group.appendChild(groupHeader);

                var itemsContainer = createEl('div', { 'class': 'context-menu__group-items' });
                function addItem(c) {
                    var item = createEl('div', { 'class': 'context-menu__item' });
                    var nameSpan = createEl('span', null, c.node.label || c.node.name);
                    nameSpan.style.color = typeColor(c.node.entity_type);
                    item.appendChild(nameSpan);
                    item.appendChild(createEl('span', { 'class': 'context-menu__item-arrow' }, '\u2192'));
                    item.addEventListener('click', function() {
                        dismiss();
                        deps.centerOnNode(c.node, true);
                        deps.showDetail(c.node, allLinks);
                    });
                    itemsContainer.appendChild(item);
                }
                items.slice(0, MAX_ITEMS).forEach(addItem);
                if (items.length > MAX_ITEMS) {
                    var overflow = createEl('div', { 'class': 'context-menu__overflow' }, '...' + (items.length - MAX_ITEMS) + ' more');
                    overflow.style.cursor = 'pointer';
                    overflow.addEventListener('click', function() {
                        group.classList.remove('collapsed');
                        items.slice(MAX_ITEMS).forEach(addItem);
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
            if (menuRect.right > rect.right - 10) menu.style.left = (x - menuRect.width) + 'px';
            if (menuRect.bottom > rect.bottom - 10) menu.style.top = (y - menuRect.height) + 'px';
        });
    }

    return { show: show, dismiss: dismiss };
}
