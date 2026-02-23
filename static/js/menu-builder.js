(function() {
    const form = document.querySelector('.matrix-form');
    const checkboxes = form.querySelectorAll('input[type="checkbox"]');
    const saveBtn = form.querySelector('.btn-primary');
    let hasChanges = false;

    // Store initial state
    const initialState = {};
    checkboxes.forEach(cb => {
        initialState[cb.name] = cb.checked;
    });

    // Detect changes
    checkboxes.forEach(cb => {
        cb.addEventListener('change', () => {
            hasChanges = false;
            checkboxes.forEach(c => {
                if (c.checked !== initialState[c.name]) {
                    hasChanges = true;
                }
            });
            saveBtn.textContent = hasChanges ? 'Save Changes *' : 'Save Changes';
            saveBtn.classList.toggle('has-changes', hasChanges);
        });
    });

    // Clear changes flag on form submit so beforeunload doesn't fire
    form.addEventListener('submit', () => { hasChanges = false; });

    // Warn on navigate away with unsaved changes
    window.addEventListener('beforeunload', (e) => {
        if (hasChanges) {
            e.preventDefault();
            e.returnValue = '';
        }
    });

    // Column toggle: click role header to toggle all in column
    const roleHeaders = document.querySelectorAll('.role-header');
    roleHeaders.forEach((header, colIndex) => {
        header.style.cursor = 'pointer';
        header.title = 'Click to toggle all permissions for this role';
        header.addEventListener('click', () => {
            const colCheckboxes = [];
            document.querySelectorAll('.matrix-perm-row').forEach(row => {
                const cells = row.querySelectorAll('.matrix-cell input[type="checkbox"]');
                if (cells[colIndex]) colCheckboxes.push(cells[colIndex]);
            });
            const allChecked = colCheckboxes.every(cb => cb.checked);
            colCheckboxes.forEach(cb => {
                cb.checked = !allChecked;
                cb.dispatchEvent(new Event('change'));
            });
        });
    });
})();
