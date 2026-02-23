/**
 * Shared helpers for graph visualizations.
 * Used by ontology-graph, ontology-schema-graph, and their sub-modules.
 *
 * Provides:
 *   graphHelpers.typeColor(entityType) — color for an entity type
 *   graphHelpers.createEl(tag, attrs, text) — safe DOM element factory
 *   graphHelpers.TYPE_COLORS — color map
 */
var graphHelpers = (function() {
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

    return {
        TYPE_COLORS: TYPE_COLORS,
        FALLBACK_COLOR: FALLBACK_COLOR,
        typeColor: typeColor,
        createEl: createEl
    };
})();
