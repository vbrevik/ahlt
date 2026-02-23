(function() {
    // Read CSRF token from the page's hidden input (rendered server-side)
    var csrfEl = document.querySelector('input[name="csrf_token"]');
    var csrfToken = csrfEl ? csrfEl.value : '';
    var FETCH_TIMEOUT_MS = 60000;
    var CHUNK_SIZE = 100;

    // Helper function for fetch with timeout using AbortController
    function fetchWithTimeout(url, options, ms) {
        var controller = new AbortController();
        var timeoutId = setTimeout(function() { controller.abort(); }, ms);

        if (!options) options = {};
        options.signal = controller.signal;

        return fetch(url, options).catch(function(e) {
            if (e.name === 'AbortError') {
                var timeoutErr = new Error('Request timed out. The server may be busy.');
                timeoutErr.isTimeout = true;
                throw timeoutErr;
            }
            throw e;
        }).finally(function() {
            clearTimeout(timeoutId);
        });
    }
    var pendingErrors = [];
    var cumulativeCounts = { created: 0, updated: 0, skipped: 0, errors: 0 };

    function chunkArray(arr, size) {
        var chunks = [];
        for (var i = 0; i < arr.length; i += size) {
            chunks.push(arr.slice(i, i + size));
        }
        return chunks;
    }

    function updateLoadingStatus(msg) {
        document.getElementById('loading-status').textContent = msg;
    }

    // DOM refs
    var dropZone = document.getElementById('drop-zone');
    var fileInput = document.getElementById('import-file');
    var fileNameEl = document.getElementById('file-name');
    var btnImport = document.getElementById('btn-import');
    var btnExport = document.getElementById('btn-export');
    var conflictMode = document.getElementById('conflict-mode');
    var resultSection = document.getElementById('import-result');
    var errorSection = document.getElementById('error-section');
    var errorTbody = document.getElementById('error-tbody');
    var loadingEl = document.getElementById('loading');
    var editorOverlay = document.getElementById('editor-overlay');
    var editorJson = document.getElementById('editor-json');
    var editorError = document.getElementById('editor-error');
    var typeAll = document.getElementById('type-all');
    var fileQueue = [];
    var editingIndex = -1;

    // ── Drop Zone ──────────────────────────────────────────────

    dropZone.addEventListener('click', function() { fileInput.click(); });
    dropZone.addEventListener('keydown', function(e) {
        if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); fileInput.click(); }
    });

    dropZone.addEventListener('dragover', function(e) {
        e.preventDefault();
        dropZone.classList.add('dm-drop-active');
    });
    dropZone.addEventListener('dragleave', function() {
        dropZone.classList.remove('dm-drop-active');
    });
    dropZone.addEventListener('drop', function(e) {
        e.preventDefault();
        dropZone.classList.remove('dm-drop-active');
        if (e.dataTransfer.files.length) selectFiles(e.dataTransfer.files);
    });
    fileInput.addEventListener('change', function() {
        if (fileInput.files.length) selectFiles(fileInput.files);
    });

    function selectFiles(files) {
        fileQueue = Array.from(files);
        if (fileQueue.length === 1) {
            fileNameEl.textContent = fileQueue[0].name;
        } else {
            fileNameEl.textContent = fileQueue.length + ' file(s) selected';
        }
        fileNameEl.hidden = false;
        btnImport.disabled = false;
    }

    // ── Import ─────────────────────────────────────────────────

    btnImport.addEventListener('click', handleImport);

    function handleImport() {
        if (fileQueue.length === 0) return;
        showLoading(true);
        updateLoadingStatus('');

        var files = fileQueue.slice();
        var mode = conflictMode.value;

        importFiles(files, mode).then(function() {
            // Reset queue after successful import
            fileQueue = [];
            fileNameEl.hidden = true;
            btnImport.disabled = true;
            fileInput.value = '';
        }).catch(function(e) {
            if (e.isTimeout) {
                alert(e.message);
            } else {
                alert('Import error: ' + e.message);
            }
        }).finally(function() {
            showLoading(false);
            updateLoadingStatus('');
        });
    }

    function importFiles(files, mode) {
        var fileIdx = 0;

        function processNextFile() {
            if (fileIdx >= files.length) return Promise.resolve();

            var file = files[fileIdx];
            var fileNum = fileIdx + 1;
            fileIdx++;

            return file.text().then(function(text) {
                var parsed;
                try {
                    parsed = JSON.parse(text);
                } catch (e) {
                    alert('Invalid JSON in ' + file.name + ': ' + e.message);
                    return processNextFile();
                }

                // JSON-LD format: send as single request (no chunking)
                if (parsed['@context'] || parsed['@graph']) {
                    updateLoadingStatus('File ' + fileNum + ' of ' + files.length + ' (JSON-LD)\u2026');
                    parsed['ahlt:conflict_mode'] = mode;
                    parsed['csrf_token'] = csrfToken;
                    return fetchWithTimeout('/api/data/import', {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify(parsed),
                    }, FETCH_TIMEOUT_MS).then(function(resp) {
                        if (!resp.ok) {
                            return resp.text().then(function(t) {
                                alert('Import failed (' + file.name + '): ' + t);
                            });
                        }
                        return resp.json().then(function(result) {
                            cumulativeCounts.created += result.created;
                            cumulativeCounts.updated += result.updated;
                            cumulativeCounts.skipped += result.skipped;
                            displayResult(result);
                        });
                    }).then(processNextFile);
                }

                // Native format: chunk entities, send relations only with last chunk
                var entities = Array.isArray(parsed.entities) ? parsed.entities : [];
                var relations = Array.isArray(parsed.relations) ? parsed.relations : [];
                var entityChunks = chunkArray(entities, CHUNK_SIZE);
                if (entityChunks.length === 0) entityChunks = [[]];

                var chunkIdx = 0;

                function processNextChunk() {
                    if (chunkIdx >= entityChunks.length) return Promise.resolve();

                    var chunk = entityChunks[chunkIdx];
                    var chunkNum = chunkIdx + 1;
                    var isLastChunk = chunkNum === entityChunks.length;
                    chunkIdx++;

                    updateLoadingStatus('File ' + fileNum + ' of ' + files.length +
                        ', chunk ' + chunkNum + ' of ' + entityChunks.length + '\u2026');

                    var payload = {
                        conflict_mode: mode,
                        entities: chunk,
                        relations: isLastChunk ? relations : [],
                        csrf_token: csrfToken,
                    };

                    return fetchWithTimeout('/api/data/import', {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify(payload),
                    }, FETCH_TIMEOUT_MS).then(function(resp) {
                        if (!resp.ok) {
                            return resp.text().then(function(t) {
                                alert('Import failed (' + file.name + ', chunk ' + chunkNum + '): ' + t);
                            });
                        }
                        return resp.json().then(function(result) {
                            cumulativeCounts.created += result.created;
                            cumulativeCounts.updated += result.updated;
                            cumulativeCounts.skipped += result.skipped;
                            if (isLastChunk) displayResult(result);
                        });
                    }).then(processNextChunk);
                }

                return processNextChunk().then(processNextFile);
            });
        }

        return processNextFile();
    }

    function displayResult(result) {
        document.getElementById('count-created').textContent = cumulativeCounts.created;
        document.getElementById('count-updated').textContent = cumulativeCounts.updated;
        document.getElementById('count-skipped').textContent = cumulativeCounts.skipped;
        cumulativeCounts.errors += result.errors.length;
        document.getElementById('count-errors').textContent = cumulativeCounts.errors;
        resultSection.hidden = false;

        if (result.errors.length > 0) {
            pendingErrors = result.errors;
            displayErrors();
        } else {
            errorSection.hidden = true;
            pendingErrors = [];
        }
    }

    // ── Error Display & Mitigation ─────────────────────────────

    function displayErrors() {
        // Clear existing rows safely
        while (errorTbody.firstChild) {
            errorTbody.removeChild(errorTbody.firstChild);
        }
        errorSection.hidden = pendingErrors.length === 0;

        pendingErrors.forEach(function(err, idx) {
            var tr = document.createElement('tr');

            // Item cell
            var tdItem = document.createElement('td');
            var code = document.createElement('code');
            code.textContent = describeItem(err.item);
            tdItem.appendChild(code);
            tr.appendChild(tdItem);

            // Reason cell
            var tdReason = document.createElement('td');
            tdReason.textContent = err.reason;
            tr.appendChild(tdReason);

            // Actions cell
            var tdActions = document.createElement('td');
            tdActions.className = 'dm-error-btn-group';

            var btnEdit = document.createElement('button');
            btnEdit.className = 'btn btn-sm';
            btnEdit.textContent = 'Edit';
            btnEdit.dataset.action = 'edit';
            btnEdit.dataset.idx = idx;

            var btnForce = document.createElement('button');
            btnForce.className = 'btn btn-sm btn-primary';
            btnForce.textContent = 'Force upsert';
            btnForce.dataset.action = 'force';
            btnForce.dataset.idx = idx;

            var btnSkip = document.createElement('button');
            btnSkip.className = 'btn btn-sm btn-danger';
            btnSkip.textContent = 'Skip';
            btnSkip.dataset.action = 'skip';
            btnSkip.dataset.idx = idx;

            tdActions.appendChild(btnEdit);
            tdActions.appendChild(document.createTextNode(' '));
            tdActions.appendChild(btnForce);
            tdActions.appendChild(document.createTextNode(' '));
            tdActions.appendChild(btnSkip);
            tr.appendChild(tdActions);

            errorTbody.appendChild(tr);
        });
    }

    errorTbody.addEventListener('click', function(e) {
        var btn = e.target.closest('button[data-action]');
        if (!btn) return;
        var idx = parseInt(btn.dataset.idx);
        var action = btn.dataset.action;
        if (action === 'skip') skipItem(idx);
        else if (action === 'edit') openEditor(idx);
        else if (action === 'force') forceUpsertItem(idx);
    });

    function skipItem(idx) {
        pendingErrors.splice(idx, 1);
        displayErrors();
    }

    document.getElementById('btn-skip-all').addEventListener('click', function() {
        pendingErrors = [];
        displayErrors();
    });

    document.getElementById('btn-retry-all').addEventListener('click', retryAll);

    function retryAll() {
        if (pendingErrors.length === 0) return;
        showLoading(true);

        var mode = document.getElementById('retry-conflict-mode').value;
        var entities = [];
        var relations = [];

        pendingErrors.forEach(function(err) {
            if (err.item.relation_type) {
                relations.push(err.item);
            } else {
                entities.push(err.item);
            }
        });

        var payload = { conflict_mode: mode, entities: entities, relations: relations, csrf_token: csrfToken };
        fetchWithTimeout('/api/data/import', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload),
        }, FETCH_TIMEOUT_MS).then(function(resp) {
            if (resp.ok) {
                return resp.json().then(function(result) {
                    cumulativeCounts.created += result.created;
                    cumulativeCounts.updated += result.updated;
                    cumulativeCounts.skipped += result.skipped;
                    displayResult(result);
                });
            } else {
                return resp.text().then(function(t) { alert('Retry failed: ' + t); });
            }
        }).catch(function(e) {
            if (e.isTimeout) {
                alert(e.message);
            } else {
                alert('Retry error: ' + e.message);
            }
        }).finally(function() {
            showLoading(false);
        });
    }

    function forceUpsertItem(idx) {
        showLoading(true);
        var err = pendingErrors[idx];
        var isRelation = !!err.item.relation_type;
        var payload = {
            conflict_mode: 'upsert',
            entities: isRelation ? [] : [err.item],
            relations: isRelation ? [err.item] : [],
            csrf_token: csrfToken,
        };

        fetchWithTimeout('/api/data/import', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload),
        }, FETCH_TIMEOUT_MS).then(function(resp) {
            if (resp.ok) {
                return resp.json().then(function(result) {
                    cumulativeCounts.created += result.created;
                    cumulativeCounts.updated += result.updated;
                    cumulativeCounts.skipped += result.skipped;
                    if (result.errors.length === 0) {
                        pendingErrors.splice(idx, 1);
                    } else {
                        pendingErrors[idx] = result.errors[0];
                    }
                    updateCounts();
                    displayErrors();
                });
            } else {
                return resp.text().then(function(t) { alert('Force upsert failed: ' + t); });
            }
        }).catch(function(e) {
            if (e.isTimeout) {
                alert(e.message);
            } else {
                alert('Error: ' + e.message);
            }
        }).finally(function() {
            showLoading(false);
        });
    }

    // ── Inline Editor ──────────────────────────────────────────

    function openEditor(idx) {
        editingIndex = idx;
        editorJson.value = JSON.stringify(pendingErrors[idx].item, null, 2);
        editorError.hidden = true;
        editorOverlay.hidden = false;
    }

    document.getElementById('editor-close').addEventListener('click', closeEditor);
    document.getElementById('editor-cancel').addEventListener('click', closeEditor);

    function closeEditor() {
        editorOverlay.hidden = true;
        editingIndex = -1;
    }

    document.getElementById('editor-save').addEventListener('click', function() {
        var parsed;
        try {
            parsed = JSON.parse(editorJson.value);
        } catch (e) {
            editorError.textContent = 'Invalid JSON: ' + e.message;
            editorError.hidden = false;
            return;
        }

        var isRelation = !!parsed.relation_type;
        var payload = {
            conflict_mode: document.getElementById('retry-conflict-mode').value,
            entities: isRelation ? [] : [parsed],
            relations: isRelation ? [parsed] : [],
            csrf_token: csrfToken,
        };

        showLoading(true);
        fetchWithTimeout('/api/data/import', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(payload),
        }, FETCH_TIMEOUT_MS).then(function(resp) {
            if (resp.ok) {
                return resp.json().then(function(result) {
                    cumulativeCounts.created += result.created;
                    cumulativeCounts.updated += result.updated;
                    cumulativeCounts.skipped += result.skipped;
                    if (result.errors.length === 0) {
                        pendingErrors.splice(editingIndex, 1);
                    } else {
                        pendingErrors[editingIndex] = result.errors[0];
                    }
                    updateCounts();
                    displayErrors();
                    closeEditor();
                });
            } else {
                return resp.text().then(function(t) {
                    editorError.textContent = 'Import failed: ' + t;
                    editorError.hidden = false;
                });
            }
        }).catch(function(e) {
            if (e.isTimeout) {
                editorError.textContent = e.message;
            } else {
                editorError.textContent = 'Error: ' + e.message;
            }
            editorError.hidden = false;
        }).finally(function() {
            showLoading(false);
        });
    });

    // ── Export ──────────────────────────────────────────────────

    typeAll.addEventListener('change', function() {
        var filters = document.querySelectorAll('.type-filter');
        for (var i = 0; i < filters.length; i++) filters[i].checked = false;
    });
    var typeFilters = document.querySelectorAll('.type-filter');
    for (var i = 0; i < typeFilters.length; i++) {
        typeFilters[i].addEventListener('change', function() {
            var anyChecked = document.querySelectorAll('.type-filter:checked').length > 0;
            typeAll.checked = !anyChecked;
        });
    }

    btnExport.addEventListener('click', handleExport);

    function handleExport() {
        showLoading(true);

        var format = document.querySelector('input[name="export-format"]:checked').value;
        var checkedTypes = [];
        var checked = document.querySelectorAll('.type-filter:checked');
        for (var i = 0; i < checked.length; i++) checkedTypes.push(checked[i].value);

        var params = new URLSearchParams();
        params.set('format', format);
        if (checkedTypes.length > 0) params.set('types', checkedTypes.join(','));

        fetchWithTimeout('/api/data/export?' + params.toString(), {}, FETCH_TIMEOUT_MS).then(function(resp) {
            if (!resp.ok) {
                return resp.text().then(function(t) { alert('Export failed: ' + t); });
            }

            var ext = format === 'jsonld' ? 'jsonld' : format === 'sql' ? 'sql' : 'json';
            return resp.blob().then(function(blob) {
                var url = URL.createObjectURL(blob);
                var a = document.createElement('a');
                a.href = url;
                a.download = 'ahlt-export.' + ext;
                document.body.appendChild(a);
                a.click();
                a.remove();
                URL.revokeObjectURL(url);
            });
        }).catch(function(e) {
            if (e.isTimeout) {
                alert(e.message);
            } else {
                alert('Export error: ' + e.message);
            }
        }).finally(function() {
            showLoading(false);
        });
    }

    // ── Helpers ─────────────────────────────────────────────────

    function showLoading(show) {
        loadingEl.hidden = !show;
    }

    function updateCounts() {
        document.getElementById('count-created').textContent = cumulativeCounts.created;
        document.getElementById('count-updated').textContent = cumulativeCounts.updated;
        document.getElementById('count-skipped').textContent = cumulativeCounts.skipped;
        document.getElementById('count-errors').textContent = cumulativeCounts.errors;
    }

    function describeItem(item) {
        if (item.entity_type && item.name) return item.entity_type + ':' + item.name;
        if (item.relation_type) return item.relation_type + ' (' + item.source + ' -> ' + item.target + ')';
        return JSON.stringify(item).substring(0, 80);
    }
})();
