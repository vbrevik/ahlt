// Auto-submit Add Role dropdowns on change + Menu preview
(function() {
    var FETCH_TIMEOUT_MS = 10000;

    // Auto-submit Add Role dropdowns
    var selects = document.querySelectorAll('.form-control--sm');
    for (var i = 0; i < selects.length; i++) {
        selects[i].addEventListener('change', function() {
            if (this.value) {
                this.closest('form').submit();
            }
        });
    }

    // Menu preview
    var panel = document.getElementById('menu-preview');
    var content = document.getElementById('menu-preview-content');
    var closeBtn = document.getElementById('menu-preview-close');

    function el(tag, cls, text) {
        var node = document.createElement(tag);
        if (cls) node.className = cls;
        if (text) node.textContent = text;
        return node;
    }

    function loadMenuPreview(userId) {
        if (!panel || !content) return;

        panel.removeAttribute('hidden');
        content.textContent = 'Loading...';

        var controller = new AbortController();
        var timeout = setTimeout(function() { controller.abort(); }, FETCH_TIMEOUT_MS);

        fetch('/api/roles/preview?user_id=' + userId, { signal: controller.signal })
            .then(function(r) { return r.json(); })
            .then(function(data) {
                clearTimeout(timeout);
                content.textContent = '';

                var heading = el('h4', null, 'Effective Menu (' + data.permission_count + ' permissions)');
                content.appendChild(heading);

                if (data.menu_items.length === 0) {
                    content.appendChild(el('p', 'text-muted', 'No accessible menu items.'));
                    return;
                }

                var ul = el('ul', 'menu-preview__list');
                data.menu_items.forEach(function(item) {
                    var li = el('li', null, item.label);
                    if (item.type === 'sidebar') {
                        li.appendChild(el('small', null, ' (sidebar)'));
                    }
                    ul.appendChild(li);
                });
                content.appendChild(ul);
            })
            .catch(function(e) {
                clearTimeout(timeout);
                content.textContent = e.name === 'AbortError' ? 'Request timed out' : 'Error loading preview';
            });
    }

    if (closeBtn) {
        closeBtn.addEventListener('click', function() {
            panel.setAttribute('hidden', '');
        });
    }

    // Wire up preview buttons
    var previewBtns = document.querySelectorAll('.menu-preview-btn');
    for (var j = 0; j < previewBtns.length; j++) {
        previewBtns[j].addEventListener('click', function() {
            loadMenuPreview(this.getAttribute('data-user-id'));
        });
    }
})();
