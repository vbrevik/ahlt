function updateBulkToolbar() {
    const checkboxes = document.querySelectorAll('.users-row-checkbox');
    const selectedCount = Array.from(checkboxes).filter(cb => cb.checked).length;
    const toolbar = document.getElementById('users-bulk-toolbar');
    const countSpan = document.getElementById('users-selected-count');
    const pluralSpan = document.getElementById('users-plural');
    countSpan.textContent = selectedCount;
    pluralSpan.textContent = selectedCount === 1 ? '' : 's';
    toolbar.hidden = selectedCount === 0;
    const selectAll = document.getElementById('users-select-all');
    const total = checkboxes.length;
    selectAll.checked = selectedCount === total && total > 0;
    selectAll.indeterminate = selectedCount > 0 && selectedCount < total;
}

function toggleSelectAll() {
    const checked = document.getElementById('users-select-all').checked;
    document.querySelectorAll('.users-row-checkbox').forEach(cb => cb.checked = checked);
    updateBulkToolbar();
}

function clearSelection() {
    document.querySelectorAll('.users-row-checkbox, #users-select-all').forEach(cb => cb.checked = false);
    updateBulkToolbar();
}

function deleteUser(userId) {
    const row = document.querySelector('[data-user-id="' + userId + '"]');
    const nameEl = row ? row.querySelector('.user-name') : null;
    const displayName = nameEl ? nameEl.textContent.replace('\u{1F464} ', '').trim() : userId;
    if (confirm('Delete user "' + displayName + '"?\n\nThis action cannot be undone.')) {
        const form = document.createElement('form');
        form.method = 'POST';
        form.action = '/users/' + userId + '/delete';
        const csrf = document.createElement('input');
        csrf.type = 'hidden';
        csrf.name = 'csrf_token';
        // Read CSRF token from the existing bulk-delete form's hidden input
        csrf.value = document.querySelector('#users-bulk-delete-form input[name="csrf_token"]').value;
        form.appendChild(csrf);
        document.body.appendChild(form);
        form.submit();
    }
}

function confirmBulkDelete() {
    const checkboxes = document.querySelectorAll('.users-row-checkbox:checked');
    const count = checkboxes.length;
    if (count === 0) { alert('No users selected'); return; }
    if (confirm('Delete ' + count + ' user' + (count !== 1 ? 's' : '') + '?\n\nThis action cannot be undone.')) {
        const ids = Array.from(checkboxes).map(cb => cb.value);
        document.getElementById('users-bulk-delete-ids').value = JSON.stringify(ids);
        document.getElementById('users-bulk-delete-form').submit();
    }
}

updateBulkToolbar();

// Avatar initials
document.querySelectorAll('.user-avatar-circle[data-name]').forEach(function(el) {
    var name = el.getAttribute('data-name') || '';
    var initial = name.charAt(0).toUpperCase() || '?';
    el.textContent = initial;
    el.setAttribute('data-hue', String(initial.charCodeAt(0) % 8));
});
