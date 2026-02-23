/**
 * Ontology schema graph — detail panel and context menu.
 *
 * Factory: schemaGraphPanels(deps)
 *   deps.typeColor, deps.createEl — from graphHelpers
 *   deps.allLinks — function() → current links array
 *   deps.containerEl — .graph-container element
 *   deps.detailEl — #graph-detail element
 *
 * Returns { showDetail(d), showContextMenu(event, d), dismissContextMenu() }
 */
function schemaGraphPanels(deps) {
    var typeColor = deps.typeColor;
    var createEl = deps.createEl;

    // Detail panel
    document.getElementById('btn-close-detail').addEventListener('click', function() {
        deps.detailEl.style.display = 'none';
    });

    function showDetail(d) {
        var detail = deps.detailEl;
        detail.style.display = '';
        var detailType = document.getElementById('detail-type');
        detailType.textContent = 'ENTITY TYPE';
        detailType.style.color = typeColor(d.id);
        document.getElementById('detail-label').textContent = d.label;
        document.getElementById('detail-meta').textContent = d.count + ' instance' + (d.count !== 1 ? 's' : '');

        var propsEl = document.getElementById('detail-props');
        while (propsEl.firstChild) propsEl.removeChild(propsEl.firstChild);
        if (d.property_keys && d.property_keys.length > 0) {
            propsEl.appendChild(createEl('span', { 'class': 'conn-heading' }, 'Properties'));
            d.property_keys.forEach(function(key) {
                var row = createEl('div', { 'class': 'conn-row' });
                row.appendChild(createEl('code', null, key));
                propsEl.appendChild(row);
            });
        }

        var connEl = document.getElementById('detail-connections');
        while (connEl.firstChild) connEl.removeChild(connEl.firstChild);
        var allLinks = deps.allLinks();
        var outgoing = allLinks.filter(function(l) { return l.source.id === d.id; });
        var incoming = allLinks.filter(function(l) { return l.target.id === d.id; });

        if (outgoing.length) {
            connEl.appendChild(createEl('span', { 'class': 'conn-heading' }, 'Outgoing Relations'));
            outgoing.forEach(function(l) {
                var row = createEl('div', { 'class': 'conn-row' });
                row.appendChild(createEl('code', null, l.relation_type));
                row.appendChild(document.createTextNode(' \u2192 '));
                var name = createEl('span', null, l.target.label);
                name.style.color = typeColor(l.target.id);
                name.style.fontWeight = '600';
                row.appendChild(name);
                row.appendChild(createEl('span', { 'class': 'conn-count' }, ' (' + l.count + ')'));
                connEl.appendChild(row);
            });
        }
        if (incoming.length) {
            connEl.appendChild(createEl('span', { 'class': 'conn-heading' }, 'Incoming Relations'));
            incoming.forEach(function(l) {
                var row = createEl('div', { 'class': 'conn-row' });
                var name = createEl('span', null, l.source.label);
                name.style.color = typeColor(l.source.id);
                name.style.fontWeight = '600';
                row.appendChild(name);
                row.appendChild(document.createTextNode(' \u2192 '));
                row.appendChild(createEl('code', null, l.relation_type));
                row.appendChild(createEl('span', { 'class': 'conn-count' }, ' (' + l.count + ')'));
                connEl.appendChild(row);
            });
        }
        if (!outgoing.length && !incoming.length) {
            connEl.appendChild(createEl('span', { 'class': 'conn-empty' }, 'No relations'));
        }
    }

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

        var header = createEl('div', { 'class': 'context-menu__header' });
        var headerLabel = createEl('span', { 'class': 'context-menu__header-label' }, d.label);
        var headerBadge = createEl('span', { 'class': 'context-menu__header-badge' }, d.count + ' instance' + (d.count !== 1 ? 's' : ''));
        headerBadge.style.color = typeColor(d.id);
        header.appendChild(headerLabel);
        header.appendChild(headerBadge);
        menu.appendChild(header);
        menu.appendChild(createEl('div', { 'class': 'context-menu__divider' }));

        var viewAction = createEl('div', { 'class': 'context-menu__action' });
        viewAction.appendChild(createEl('span', { 'class': 'context-menu__action-icon' }, '\u2192'));
        viewAction.appendChild(document.createTextNode('View instances'));
        viewAction.addEventListener('click', function() {
            window.location.href = '/ontology/data?type=' + encodeURIComponent(d.id);
        });
        menu.appendChild(viewAction);

        var allLinks = deps.allLinks();
        var relatedLinks = allLinks.filter(function(l) { return l.source.id === d.id || l.target.id === d.id; });
        if (relatedLinks.length) {
            menu.appendChild(createEl('div', { 'class': 'context-menu__divider' }));
            var group = createEl('div', { 'class': 'context-menu__group' });
            var groupHeader = createEl('div', { 'class': 'context-menu__group-header' });
            groupHeader.appendChild(createEl('span', { 'class': 'context-menu__group-toggle' }, '\u25BE'));
            groupHeader.appendChild(document.createTextNode(' Relation types'));
            groupHeader.addEventListener('click', function() { group.classList.toggle('collapsed'); });
            group.appendChild(groupHeader);
            var itemsContainer = createEl('div', { 'class': 'context-menu__group-items' });
            relatedLinks.forEach(function(l) {
                var item = createEl('div', { 'class': 'context-menu__item' });
                item.appendChild(createEl('span', null, l.relation_type + ' (' + l.count + ')'));
                itemsContainer.appendChild(item);
            });
            group.appendChild(itemsContainer);
            menu.appendChild(group);
        }

        var rect = deps.containerEl.getBoundingClientRect();
        var x = event.clientX - rect.left;
        var y = event.clientY - rect.top;
        menu.style.left = x + 'px';
        menu.style.top = y + 'px';
        deps.containerEl.appendChild(menu);
        contextMenuEl = menu;

        requestAnimationFrame(function() {
            var menuRect = menu.getBoundingClientRect();
            if (menuRect.right > rect.right - 10) menu.style.left = (x - menuRect.width) + 'px';
            if (menuRect.bottom > rect.bottom - 10) menu.style.top = (y - menuRect.height) + 'px';
        });
    }

    return { showDetail: showDetail, showContextMenu: showContextMenu, dismissContextMenu: dismissContextMenu };
}
