(function() {
    // --- Attendance ---
    var ATTENDANCE_STATUS = ['present', 'absent', 'excused'];

    function makeAttendanceRow(item, canEdit) {
        var name = item.name || '';
        var status = item.status || 'present';
        var delegationTo = item.delegation_to || '';

        var tr = document.createElement('tr');

        var nameTd = document.createElement('td');
        var nameInput = document.createElement('input');
        nameInput.type = 'text';
        nameInput.className = 'input input--sm';
        nameInput.placeholder = 'Name';
        nameInput.value = name;
        nameInput.readOnly = !canEdit;
        nameTd.appendChild(nameInput);
        tr.appendChild(nameTd);

        var statusTd = document.createElement('td');
        var statusSel = document.createElement('select');
        statusSel.className = 'input input--sm';
        statusSel.disabled = !canEdit;
        ATTENDANCE_STATUS.forEach(function(opt) {
            var o = document.createElement('option');
            o.value = opt;
            o.textContent = opt;
            if (opt === status) { o.selected = true; }
            statusSel.appendChild(o);
        });
        statusTd.appendChild(statusSel);
        tr.appendChild(statusTd);

        var delegTd = document.createElement('td');
        var delegInput = document.createElement('input');
        delegInput.type = 'text';
        delegInput.className = 'input input--sm';
        delegInput.placeholder = 'Delegation to (optional)';
        delegInput.value = delegationTo;
        delegInput.readOnly = !canEdit;
        delegTd.appendChild(delegInput);
        tr.appendChild(delegTd);

        if (canEdit) {
            var actionTd = document.createElement('td');
            var removeBtn = document.createElement('button');
            removeBtn.type = 'button';
            removeBtn.className = 'btn btn-sm btn-danger';
            removeBtn.textContent = '\u00d7';
            removeBtn.addEventListener('click', function() { tr.remove(); });
            actionTd.appendChild(removeBtn);
            tr.appendChild(actionTd);
        }

        return tr;
    }

    createDynamicTable({
        tableBodyId: 'attendance-body',
        dataId: 'attendance-data',
        addBtnId: 'add-attendance-row',
        saveBtnId: 'save-attendance',
        hiddenInputId: 'attendance-json',
        makeRow: makeAttendanceRow,
        serializeRow: function(tr) {
            var inputs = tr.querySelectorAll('input, select');
            var name = inputs[0].value.trim();
            if (!name) return null;
            return {
                name: name,
                status: inputs[1].value,
                delegation_to: inputs[2].value.trim()
            };
        }
    });

    // --- Action Items ---
    var ACTION_STATUS = ['open', 'in_progress', 'done'];

    function makeActionItemRow(item, canEdit) {
        var description = item.description || '';
        var responsible = item.responsible || '';
        var dueDate = item.due_date || '';
        var status = item.status || 'open';

        var tr = document.createElement('tr');

        var descTd = document.createElement('td');
        var descInput = document.createElement('input');
        descInput.type = 'text';
        descInput.className = 'input input--sm';
        descInput.placeholder = 'Description';
        descInput.value = description;
        descInput.readOnly = !canEdit;
        descTd.appendChild(descInput);
        tr.appendChild(descTd);

        var respTd = document.createElement('td');
        var respInput = document.createElement('input');
        respInput.type = 'text';
        respInput.className = 'input input--sm';
        respInput.placeholder = 'Responsible';
        respInput.value = responsible;
        respInput.readOnly = !canEdit;
        respTd.appendChild(respInput);
        tr.appendChild(respTd);

        var dateTd = document.createElement('td');
        var dateInput = document.createElement('input');
        dateInput.type = 'date';
        dateInput.className = 'input input--sm';
        dateInput.value = dueDate;
        dateInput.readOnly = !canEdit;
        dateTd.appendChild(dateInput);
        tr.appendChild(dateTd);

        var statusTd = document.createElement('td');
        var statusSel = document.createElement('select');
        statusSel.className = 'input input--sm';
        statusSel.disabled = !canEdit;
        ACTION_STATUS.forEach(function(opt) {
            var o = document.createElement('option');
            o.value = opt;
            o.textContent = opt;
            if (opt === status) { o.selected = true; }
            statusSel.appendChild(o);
        });
        statusTd.appendChild(statusSel);
        tr.appendChild(statusTd);

        if (canEdit) {
            var actionTd = document.createElement('td');
            var removeBtn = document.createElement('button');
            removeBtn.type = 'button';
            removeBtn.className = 'btn btn-sm btn-danger';
            removeBtn.textContent = '\u00d7';
            removeBtn.addEventListener('click', function() { tr.remove(); });
            actionTd.appendChild(removeBtn);
            tr.appendChild(actionTd);
        }

        return tr;
    }

    createDynamicTable({
        tableBodyId: 'action-items-body',
        dataId: 'action-items-data',
        addBtnId: 'add-action-item-row',
        saveBtnId: 'save-action-items',
        hiddenInputId: 'action-items-json',
        makeRow: makeActionItemRow,
        serializeRow: function(tr) {
            var inputs = tr.querySelectorAll('input, select');
            var description = inputs[0].value.trim();
            if (!description) return null;
            return {
                description: description,
                responsible: inputs[1].value.trim(),
                due_date: inputs[2].value.trim(),
                status: inputs[3].value
            };
        }
    });
})();
