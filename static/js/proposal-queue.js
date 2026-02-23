(function() {
    var selectAll = document.getElementById('select-all');
    if (!selectAll) return;
    selectAll.addEventListener('change', function() {
        var boxes = document.querySelectorAll('input[name="proposal_ids"]');
        for (var i = 0; i < boxes.length; i++) {
            boxes[i].checked = selectAll.checked;
        }
    });
})();
