/**
 * Data manager — orchestrator + export pipeline + shared helpers.
 * Depends on: data-manager-import.js, data-manager-errors.js
 */
(function() {
    var csrfEl = document.querySelector('input[name="csrf_token"]');
    var csrfToken = csrfEl ? csrfEl.value : '';
    var FETCH_TIMEOUT_MS = 60000;
    var CHUNK_SIZE = 100;

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
        }).finally(function() { clearTimeout(timeoutId); });
    }

    function chunkArray(arr, size) {
        var chunks = [];
        for (var i = 0; i < arr.length; i += size) chunks.push(arr.slice(i, i + size));
        return chunks;
    }

    var cumulativeCounts = { created: 0, updated: 0, skipped: 0, errors: 0 };
    var loadingEl = document.getElementById('loading');

    function showLoading(show) { loadingEl.hidden = !show; }
    function updateLoadingStatus(msg) { document.getElementById('loading-status').textContent = msg; }
    function updateCounts() {
        document.getElementById('count-created').textContent = cumulativeCounts.created;
        document.getElementById('count-updated').textContent = cumulativeCounts.updated;
        document.getElementById('count-skipped').textContent = cumulativeCounts.skipped;
        document.getElementById('count-errors').textContent = cumulativeCounts.errors;
    }

    // Initialize error handler sub-module
    var errors = dataManagerErrors({
        csrfToken: csrfToken, fetchWithTimeout: fetchWithTimeout, FETCH_TIMEOUT_MS: FETCH_TIMEOUT_MS,
        showLoading: showLoading, updateCounts: updateCounts, cumulativeCounts: cumulativeCounts,
        errorSection: document.getElementById('error-section'),
        errorTbody: document.getElementById('error-tbody'),
        editorOverlay: document.getElementById('editor-overlay'),
        editorJson: document.getElementById('editor-json'),
        editorError: document.getElementById('editor-error')
    });

    // Initialize import sub-module
    dataManagerImport({
        csrfToken: csrfToken, fetchWithTimeout: fetchWithTimeout,
        chunkArray: chunkArray, FETCH_TIMEOUT_MS: FETCH_TIMEOUT_MS, CHUNK_SIZE: CHUNK_SIZE,
        showLoading: showLoading, updateLoadingStatus: updateLoadingStatus,
        displayResult: errors.displayResult,
        dropZone: document.getElementById('drop-zone'),
        fileInput: document.getElementById('import-file'),
        fileNameEl: document.getElementById('file-name'),
        btnImport: document.getElementById('btn-import'),
        conflictMode: document.getElementById('conflict-mode')
    });

    // Export pipeline
    var typeAll = document.getElementById('type-all');
    var btnExport = document.getElementById('btn-export');

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

    btnExport.addEventListener('click', function() {
        showLoading(true);
        var format = document.querySelector('input[name="export-format"]:checked').value;
        var checkedTypes = [];
        var checked = document.querySelectorAll('.type-filter:checked');
        for (var i = 0; i < checked.length; i++) checkedTypes.push(checked[i].value);

        var params = new URLSearchParams();
        params.set('format', format);
        if (checkedTypes.length > 0) params.set('types', checkedTypes.join(','));

        fetchWithTimeout('/api/data/export?' + params.toString(), {}, FETCH_TIMEOUT_MS).then(function(resp) {
            if (!resp.ok) return resp.text().then(function(t) { alert('Export failed: ' + t); });
            var ext = format === 'jsonld' ? 'jsonld' : format === 'sql' ? 'sql' : 'json';
            return resp.blob().then(function(blob) {
                var url = URL.createObjectURL(blob);
                var a = document.createElement('a');
                a.href = url; a.download = 'ahlt-export.' + ext;
                document.body.appendChild(a); a.click(); a.remove();
                URL.revokeObjectURL(url);
            });
        }).catch(function(e) {
            alert(e.isTimeout ? e.message : 'Export error: ' + e.message);
        }).finally(function() { showLoading(false); });
    });
})();
