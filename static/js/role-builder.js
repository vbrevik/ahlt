var debounceTimer = null;

/* ── Accordion Toggle ── */
document.querySelectorAll('.rb-group__header').forEach(function(header) {
    header.addEventListener('click', function() {
        var group = this.closest('.rb-group');
        var body = group.querySelector('.rb-group__body');
        var isExpanded = group.classList.contains('rb-group--expanded');

        if (isExpanded) {
            body.style.maxHeight = '0';
            group.classList.remove('rb-group--expanded');
            this.setAttribute('aria-expanded', 'false');
        } else {
            group.classList.add('rb-group--expanded');
            body.style.maxHeight = body.scrollHeight + 'px';
            this.setAttribute('aria-expanded', 'true');
        }
    });
});

/* ── Select All Toggle ── */
document.querySelectorAll('.select-all-checkbox').forEach(function(checkbox) {
    checkbox.addEventListener('change', function() {
        var group = this.dataset.group;
        var checked = this.checked;
        document.querySelectorAll('.permission-item[data-group="' + group + '"]').forEach(function(item) {
            item.checked = checked;
        });
        onPermissionChange();
    });
});

/* ── Individual Permission Change ── */
document.querySelectorAll('.permission-item').forEach(function(checkbox) {
    checkbox.addEventListener('change', function() {
        var group = this.dataset.group;
        var groupCheckboxes = document.querySelectorAll('.permission-item[data-group="' + group + '"]');
        var allChecked = Array.from(groupCheckboxes).every(function(cb) { return cb.checked; });
        var selectAllCheckbox = document.querySelector('.select-all-checkbox[data-group="' + group + '"]');
        if (selectAllCheckbox) selectAllCheckbox.checked = allChecked;
        onPermissionChange();
    });
});

/* ── Helpers ── */
function getSelectedIds() {
    return Array.from(document.querySelectorAll('.permission-item:checked'))
        .map(function(cb) { return parseInt(cb.value); });
}

function updateGroupState(groupEl) {
    var checkboxes = groupEl.querySelectorAll('.permission-item');
    var checkedCount = groupEl.querySelectorAll('.permission-item:checked').length;
    var total = checkboxes.length;

    var badge = groupEl.querySelector('.rb-group__badge');
    if (badge) badge.textContent = checkedCount + '/' + total;

    if (checkedCount > 0) {
        groupEl.classList.add('rb-group--has-selected');
    } else {
        groupEl.classList.remove('rb-group--has-selected');
    }
}

function updateAllGroupStates() {
    document.querySelectorAll('.rb-group').forEach(updateGroupState);
}

/* ── Permission Change Handler ── */
function onPermissionChange() {
    var selectedIds = getSelectedIds();
    document.getElementById('permission-ids-input').value = JSON.stringify(selectedIds);

    var submitBtn = document.getElementById('submit-btn');
    submitBtn.disabled = selectedIds.length === 0;

    var countEl = document.getElementById('perm-count');
    countEl.textContent = selectedIds.length > 0 ? selectedIds.length + ' selected' : '';

    updateAllGroupStates();

    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(fetchPreview, 200);
}

/* ── Preview Fetch ── */
async function fetchPreview() {
    var selectedIds = getSelectedIds();

    if (selectedIds.length === 0) {
        document.getElementById('preview-empty').style.display = 'block';
        document.getElementById('preview-loading').style.display = 'none';
        document.getElementById('preview-summary').textContent = '';
        clearPreview();
        return;
    }

    document.getElementById('preview-empty').style.display = 'none';
    document.getElementById('preview-loading').style.display = 'block';

    try {
        var response = await fetch('/roles/builder/preview', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ permission_ids: selectedIds })
        });

        if (!response.ok) throw new Error('Preview failed');

        var data = await response.json();
        displayPreview(data);
    } catch (error) {
        document.getElementById('preview-loading').style.display = 'none';
        document.getElementById('preview-empty').textContent = 'Error loading preview';
        document.getElementById('preview-empty').style.display = 'block';
    }
}

function clearPreview() {
    var sidebar = document.getElementById('mock-sidebar');
    sidebar.querySelectorAll('.rb-mock-module').forEach(function(el) { el.remove(); });
}

function el(tag, cls, text) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
}

function displayPreview(data) {
    document.getElementById('preview-loading').style.display = 'none';
    document.getElementById('preview-empty').style.display = 'none';
    clearPreview();

    document.getElementById('preview-summary').textContent =
        data.count + ' menu item' + (data.count !== 1 ? 's' : '') + ' accessible';

    var byModule = {};
    data.items.forEach(function(item) {
        if (!byModule[item.module_name]) byModule[item.module_name] = [];
        byModule[item.module_name].push(item);
    });

    var sidebar = document.getElementById('mock-sidebar');
    Object.keys(byModule).sort().forEach(function(moduleName) {
        var group = el('div', 'rb-mock-module');
        group.appendChild(el('div', 'rb-mock-module-name', moduleName));
        byModule[moduleName].forEach(function(item) {
            var a = el('a', 'rb-mock-link', item.label);
            a.href = '#';
            a.addEventListener('click', function(e) { e.preventDefault(); });
            group.appendChild(a);
        });
        sidebar.appendChild(group);
    });
}

/* ── Initialization ── */

// Set initial state (handles edit mode with pre-checked permissions)
onPermissionChange();

// Sync "Select All" checkboxes on load
document.querySelectorAll('.select-all-checkbox').forEach(function(selectAll) {
    var group = selectAll.dataset.group;
    var groupCheckboxes = document.querySelectorAll('.permission-item[data-group="' + group + '"]');
    var allChecked = groupCheckboxes.length > 0 && Array.from(groupCheckboxes).every(function(cb) { return cb.checked; });
    selectAll.checked = allChecked;
});

// Auto-expand groups with checked permissions (edit mode)
document.querySelectorAll('.rb-group').forEach(function(group) {
    var hasChecked = group.querySelector('.permission-item:checked');
    if (hasChecked) {
        var body = group.querySelector('.rb-group__body');
        group.classList.add('rb-group--expanded');
        body.style.maxHeight = body.scrollHeight + 'px';
        group.querySelector('.rb-group__header').setAttribute('aria-expanded', 'true');
    }
});
