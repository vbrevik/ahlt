/**
 * Data manager — import pipeline (drop zone, file upload, chunked import).
 *
 * Factory: dataManagerImport(deps)
 *   deps.csrfToken, deps.fetchWithTimeout, deps.chunkArray, deps.FETCH_TIMEOUT_MS, deps.CHUNK_SIZE
 *   deps.showLoading, deps.updateLoadingStatus, deps.displayResult
 *   deps.dropZone, deps.fileInput, deps.fileNameEl, deps.btnImport, deps.conflictMode
 *
 * Returns { reset() }
 */
function dataManagerImport(deps) {
    var fileQueue = [];

    // Drop zone handlers
    deps.dropZone.addEventListener('click', function() { deps.fileInput.click(); });
    deps.dropZone.addEventListener('keydown', function(e) {
        if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); deps.fileInput.click(); }
    });
    deps.dropZone.addEventListener('dragover', function(e) {
        e.preventDefault();
        deps.dropZone.classList.add('dm-drop-active');
    });
    deps.dropZone.addEventListener('dragleave', function() {
        deps.dropZone.classList.remove('dm-drop-active');
    });
    deps.dropZone.addEventListener('drop', function(e) {
        e.preventDefault();
        deps.dropZone.classList.remove('dm-drop-active');
        if (e.dataTransfer.files.length) selectFiles(e.dataTransfer.files);
    });
    deps.fileInput.addEventListener('change', function() {
        if (deps.fileInput.files.length) selectFiles(deps.fileInput.files);
    });

    function selectFiles(files) {
        fileQueue = Array.from(files);
        if (fileQueue.length === 1) {
            deps.fileNameEl.textContent = fileQueue[0].name;
        } else {
            deps.fileNameEl.textContent = fileQueue.length + ' file(s) selected';
        }
        deps.fileNameEl.hidden = false;
        deps.btnImport.disabled = false;
    }

    deps.btnImport.addEventListener('click', handleImport);

    function handleImport() {
        if (fileQueue.length === 0) return;
        deps.showLoading(true);
        deps.updateLoadingStatus('');

        var files = fileQueue.slice();
        var mode = deps.conflictMode.value;

        importFiles(files, mode).then(function() {
            fileQueue = [];
            deps.fileNameEl.hidden = true;
            deps.btnImport.disabled = true;
            deps.fileInput.value = '';
        }).catch(function(e) {
            if (e.isTimeout) alert(e.message);
            else alert('Import error: ' + e.message);
        }).finally(function() {
            deps.showLoading(false);
            deps.updateLoadingStatus('');
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
                try { parsed = JSON.parse(text); }
                catch (e) { alert('Invalid JSON in ' + file.name + ': ' + e.message); return processNextFile(); }

                // JSON-LD format: send as single request
                if (parsed['@context'] || parsed['@graph']) {
                    deps.updateLoadingStatus('File ' + fileNum + ' of ' + files.length + ' (JSON-LD)\u2026');
                    parsed['ahlt:conflict_mode'] = mode;
                    parsed['csrf_token'] = deps.csrfToken;
                    return deps.fetchWithTimeout('/api/data/import', {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify(parsed),
                    }, deps.FETCH_TIMEOUT_MS).then(function(resp) {
                        if (!resp.ok) return resp.text().then(function(t) { alert('Import failed (' + file.name + '): ' + t); });
                        return resp.json().then(function(result) { deps.displayResult(result); });
                    }).then(processNextFile);
                }

                // Native format: chunk entities, send relations only with last chunk
                var entities = Array.isArray(parsed.entities) ? parsed.entities : [];
                var relations = Array.isArray(parsed.relations) ? parsed.relations : [];
                var entityChunks = deps.chunkArray(entities, deps.CHUNK_SIZE);
                if (entityChunks.length === 0) entityChunks = [[]];
                var chunkIdx = 0;

                function processNextChunk() {
                    if (chunkIdx >= entityChunks.length) return Promise.resolve();
                    var chunk = entityChunks[chunkIdx];
                    var chunkNum = chunkIdx + 1;
                    var isLastChunk = chunkNum === entityChunks.length;
                    chunkIdx++;

                    deps.updateLoadingStatus('File ' + fileNum + ' of ' + files.length +
                        ', chunk ' + chunkNum + ' of ' + entityChunks.length + '\u2026');

                    var payload = {
                        conflict_mode: mode, entities: chunk,
                        relations: isLastChunk ? relations : [], csrf_token: deps.csrfToken,
                    };
                    return deps.fetchWithTimeout('/api/data/import', {
                        method: 'POST',
                        headers: { 'Content-Type': 'application/json' },
                        body: JSON.stringify(payload),
                    }, deps.FETCH_TIMEOUT_MS).then(function(resp) {
                        if (!resp.ok) return resp.text().then(function(t) { alert('Import failed (' + file.name + ', chunk ' + chunkNum + '): ' + t); });
                        return resp.json().then(function(result) { if (isLastChunk) deps.displayResult(result); });
                    }).then(processNextChunk);
                }

                return processNextChunk().then(processNextFile);
            });
        }

        return processNextFile();
    }

    return { reset: function() { fileQueue = []; } };
}
