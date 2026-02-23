/**
 * Ontology graph — detail panel (right sidebar).
 *
 * Factory: ontologyGraphDetailPanel(deps)
 *   deps.detailEl   — #graph-detail DOM element
 *   deps.typeColor  — function(entityType) → color string
 *   deps.createEl   — function(tag, attrs, text) → element
 *   deps.centerOnNode — function(node, animate)
 *
 * Returns { show(node, links), hide() }
 */
function ontologyGraphDetailPanel(deps) {
    var detail = deps.detailEl;
    var typeColor = deps.typeColor;
    var createEl = deps.createEl;

    document.getElementById('btn-close-detail').addEventListener('click', function() {
        detail.style.display = 'none';
    });

    function hide() { detail.style.display = 'none'; }

    function show(d, links) {
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

        function addConnRow(parent, l, isOutgoing) {
            var row = createEl('div', { 'class': 'conn-row' });
            if (isOutgoing) {
                row.appendChild(createEl('code', null, l.relation_type));
                row.appendChild(document.createTextNode(' \u2192 '));
                var name = createEl('span', null, l.target.label || l.target.name);
                name.style.color = typeColor(l.target.entity_type);
                name.style.cursor = 'pointer';
                name.addEventListener('click', function() {
                    deps.centerOnNode(l.target, true);
                    show(l.target, links);
                });
                row.appendChild(name);
            } else {
                var name = createEl('span', null, l.source.label || l.source.name);
                name.style.color = typeColor(l.source.entity_type);
                name.style.cursor = 'pointer';
                name.addEventListener('click', function() {
                    deps.centerOnNode(l.source, true);
                    show(l.source, links);
                });
                row.appendChild(name);
                row.appendChild(document.createTextNode(' \u2192 '));
                row.appendChild(createEl('code', null, l.relation_type));
            }
            parent.appendChild(row);
        }

        if (outgoing.length) {
            connEl.appendChild(createEl('span', { 'class': 'conn-heading' }, 'Outgoing'));
            outgoing.forEach(function(l) { addConnRow(connEl, l, true); });
        }
        if (incoming.length) {
            connEl.appendChild(createEl('span', { 'class': 'conn-heading' }, 'Incoming'));
            incoming.forEach(function(l) { addConnRow(connEl, l, false); });
        }
        if (!outgoing.length && !incoming.length) {
            connEl.appendChild(createEl('span', { 'class': 'conn-empty' }, 'No connections'));
        }

        var actionsEl = document.getElementById('detail-actions');
        while (actionsEl.firstChild) actionsEl.removeChild(actionsEl.firstChild);
        actionsEl.appendChild(createEl('a', { 'class': 'btn btn-sm', href: '/ontology/data/' + d.id }, 'Full detail'));
    }

    return { show: show, hide: hide };
}
