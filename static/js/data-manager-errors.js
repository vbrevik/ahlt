/**
 * Data manager — error display, mitigation actions, and inline editor.
 *
 * Factory: dataManagerErrors(deps)
 *   deps.csrfToken, deps.fetchWithTimeout, deps.FETCH_TIMEOUT_MS
 *   deps.showLoading, deps.updateCounts, deps.cumulativeCounts
 *   deps.errorSection, deps.errorTbody
 *   deps.editorOverlay, deps.editorJson, deps.editorError
 *
 * Returns { displayResult(result), getPendingErrors() }
 */
function dataManagerErrors(deps) {
    var pendingErrors = [];
    var editingIndex = -1;

    function describeItem(item) {
        if (item.entity_type && item.name) return item.entity_type + ':' + item.name;
        if (item.relation_type) return item.relation_type + ' (' + item.source + ' -> ' + item.target + ')';
        return JSON.stringify(item).substring(0, 80);
    }

    function displayResult(result) {
        deps.cumulativeCounts.created += result.created;
        deps.cumulativeCounts.updated += result.updated;
        deps.cumulativeCounts.skipped += result.skipped;
        deps.cumulativeCounts.errors += result.errors.length;
        deps.updateCounts();
        document.getElementById('import-result').hidden = false;

        if (result.errors.length > 0) {
            pendingErrors = result.errors;
            displayErrors();
        } else {
            deps.errorSection.hidden = true;
            pendingErrors = [];
        }
    }

    function displayErrors() {
        while (deps.errorTbody.firstChild) deps.errorTbody.removeChild(deps.errorTbody.firstChild);
        deps.errorSection.hidden = pendingErrors.length === 0;

        pendingErrors.forEach(function(err, idx) {
            var tr = document.createElement('tr');
            var tdItem = document.createElement('td');
            var code = document.createElement('code');
            code.textContent = describeItem(err.item);
            tdItem.appendChild(code);
            tr.appendChild(tdItem);

            var tdReason = document.createElement('td');
            tdReason.textContent = err.reason;
            tr.appendChild(tdReason);

            var tdActions = document.createElement('td');
            tdActions.className = 'dm-error-btn-group';
            var btnEdit = document.createElement('button');
            btnEdit.className = 'btn btn-sm'; btnEdit.textContent = 'Edit';
            btnEdit.dataset.action = 'edit'; btnEdit.dataset.idx = idx;
            var btnForce = document.createElement('button');
            btnForce.className = 'btn btn-sm btn-primary'; btnForce.textContent = 'Force upsert';
            btnForce.dataset.action = 'force'; btnForce.dataset.idx = idx;
            var btnSkip = document.createElement('button');
            btnSkip.className = 'btn btn-sm btn-danger'; btnSkip.textContent = 'Skip';
            btnSkip.dataset.action = 'skip'; btnSkip.dataset.idx = idx;

            tdActions.appendChild(btnEdit);
            tdActions.appendChild(document.createTextNode(' '));
            tdActions.appendChild(btnForce);
            tdActions.appendChild(document.createTextNode(' '));
            tdActions.appendChild(btnSkip);
            tr.appendChild(tdActions);
            deps.errorTbody.appendChild(tr);
        });
    }

    deps.errorTbody.addEventListener('click', function(e) {
        var btn = e.target.closest('button[data-action]');
        if (!btn) return;
        var idx = parseInt(btn.dataset.idx);
        var action = btn.dataset.action;
        if (action === 'skip') { pendingErrors.splice(idx, 1); displayErrors(); }
        else if (action === 'edit') openEditor(idx);
        else if (action === 'force') forceUpsertItem(idx);
    });

    document.getElementById('btn-skip-all').addEventListener('click', function() {
        pendingErrors = []; displayErrors();
    });

    document.getElementById('btn-retry-all').addEventListener('click', retryAll);

    function retryAll() {
        if (pendingErrors.length === 0) return;
        deps.showLoading(true);
        var mode = document.getElementById('retry-conflict-mode').value;
        var entities = [], relations = [];
        pendingErrors.forEach(function(err) {
            if (err.item.relation_type) relations.push(err.item);
            else entities.push(err.item);
        });
        var payload = { conflict_mode: mode, entities: entities, relations: relations, csrf_token: deps.csrfToken };
        deps.fetchWithTimeout('/api/data/import', {
            method: 'POST', headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload),
        }, deps.FETCH_TIMEOUT_MS).then(function(resp) {
            if (resp.ok) return resp.json().then(function(result) { displayResult(result); });
            else return resp.text().then(function(t) { alert('Retry failed: ' + t); });
        }).catch(function(e) {
            alert(e.isTimeout ? e.message : 'Retry error: ' + e.message);
        }).finally(function() { deps.showLoading(false); });
    }

    function forceUpsertItem(idx) {
        deps.showLoading(true);
        var err = pendingErrors[idx];
        var isRelation = !!err.item.relation_type;
        var payload = {
            conflict_mode: 'upsert',
            entities: isRelation ? [] : [err.item], relations: isRelation ? [err.item] : [],
            csrf_token: deps.csrfToken,
        };
        deps.fetchWithTimeout('/api/data/import', {
            method: 'POST', headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload),
        }, deps.FETCH_TIMEOUT_MS).then(function(resp) {
            if (resp.ok) return resp.json().then(function(result) {
                deps.cumulativeCounts.created += result.created;
                deps.cumulativeCounts.updated += result.updated;
                deps.cumulativeCounts.skipped += result.skipped;
                if (result.errors.length === 0) pendingErrors.splice(idx, 1);
                else pendingErrors[idx] = result.errors[0];
                deps.updateCounts(); displayErrors();
            });
            else return resp.text().then(function(t) { alert('Force upsert failed: ' + t); });
        }).catch(function(e) {
            alert(e.isTimeout ? e.message : 'Error: ' + e.message);
        }).finally(function() { deps.showLoading(false); });
    }

    // Inline editor
    function openEditor(idx) {
        editingIndex = idx;
        deps.editorJson.value = JSON.stringify(pendingErrors[idx].item, null, 2);
        deps.editorError.hidden = true;
        deps.editorOverlay.hidden = false;
    }

    function closeEditor() { deps.editorOverlay.hidden = true; editingIndex = -1; }
    document.getElementById('editor-close').addEventListener('click', closeEditor);
    document.getElementById('editor-cancel').addEventListener('click', closeEditor);

    document.getElementById('editor-save').addEventListener('click', function() {
        var parsed;
        try { parsed = JSON.parse(deps.editorJson.value); }
        catch (e) { deps.editorError.textContent = 'Invalid JSON: ' + e.message; deps.editorError.hidden = false; return; }

        var isRelation = !!parsed.relation_type;
        var payload = {
            conflict_mode: document.getElementById('retry-conflict-mode').value,
            entities: isRelation ? [] : [parsed], relations: isRelation ? [parsed] : [],
            csrf_token: deps.csrfToken,
        };
        deps.showLoading(true);
        deps.fetchWithTimeout('/api/data/import', {
            method: 'POST', headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload),
        }, deps.FETCH_TIMEOUT_MS).then(function(resp) {
            if (resp.ok) return resp.json().then(function(result) {
                deps.cumulativeCounts.created += result.created;
                deps.cumulativeCounts.updated += result.updated;
                deps.cumulativeCounts.skipped += result.skipped;
                if (result.errors.length === 0) pendingErrors.splice(editingIndex, 1);
                else pendingErrors[editingIndex] = result.errors[0];
                deps.updateCounts(); displayErrors(); closeEditor();
            });
            else return resp.text().then(function(t) {
                deps.editorError.textContent = 'Import failed: ' + t; deps.editorError.hidden = false;
            });
        }).catch(function(e) {
            deps.editorError.textContent = e.isTimeout ? e.message : 'Error: ' + e.message;
            deps.editorError.hidden = false;
        }).finally(function() { deps.showLoading(false); });
    });

    return { displayResult: displayResult, getPendingErrors: function() { return pendingErrors; } };
}
