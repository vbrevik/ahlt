(function() {
    var picker = document.getElementById('col-picker');
    var btn = document.getElementById('col-picker-btn');
    var list = document.getElementById('col-picker-list');
    if (!picker || !btn || !list) return;

    btn.addEventListener('click', function(e) {
        e.stopPropagation();
        picker.hidden = !picker.hidden;
    });
    document.addEventListener('click', function(e) {
        if (!picker.contains(e.target) && !btn.contains(e.target)) {
            picker.hidden = true;
        }
    });

    function getColumnOrder() {
        return Array.from(list.querySelectorAll('.col-picker__item')).map(function(item) {
            return {
                key: item.dataset.key,
                visible: item.querySelector('.col-picker__check').checked
            };
        });
    }

    function saveColumns(setGlobal) {
        var cols = getColumnOrder();
        var visibleKeys = cols.filter(function(c) { return c.visible; }).map(function(c) { return c.key; }).join(',');
        document.getElementById('col-picker-columns-input').value = visibleKeys;
        document.getElementById('col-picker-set-global-input').value = setGlobal ? 'true' : 'false';
        document.getElementById('col-picker-redirect-input').value = window.location.href;
        document.getElementById('col-picker-form').submit();
    }

    list.addEventListener('change', function(e) {
        if (e.target.classList.contains('col-picker__check')) {
            saveColumns(false);
        }
    });

    var globalBtn = document.getElementById('col-picker-set-global');
    if (globalBtn) {
        globalBtn.addEventListener('click', function() { saveColumns(true); });
    }

    // Drag-and-drop reordering
    var dragSrc = null;
    list.addEventListener('dragstart', function(e) {
        dragSrc = e.target.closest('.col-picker__item');
        if (dragSrc) e.dataTransfer.effectAllowed = 'move';
    });
    list.addEventListener('dragover', function(e) {
        e.preventDefault();
        var target = e.target.closest('.col-picker__item');
        if (target && target !== dragSrc) {
            var rect = target.getBoundingClientRect();
            var after = e.clientY > rect.top + rect.height / 2;
            list.insertBefore(dragSrc, after ? target.nextSibling : target);
        }
    });
    list.addEventListener('dragend', function() {
        dragSrc = null;
        saveColumns(false);
    });
})();
