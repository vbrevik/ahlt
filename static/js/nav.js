document.addEventListener('click', function(e) {
    document.querySelectorAll('.user-dropdown.open').forEach(function(d) {
        if (!d.contains(e.target)) d.classList.remove('open');
    });
});

// WebSocket for real-time warning notifications
(function() {
    var proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    var ws = null;
    var retryDelay = 1000;

    function connect() {
        ws = new WebSocket(proto + '//' + location.host + '/ws/notifications');

        ws.onopen = function() {
            retryDelay = 1000;
        };

        ws.onmessage = function(evt) {
            try {
                var data = JSON.parse(evt.data);
                if (data.type === 'count_update' || data.type === 'new_warning') {
                    updateBadge(data.unread_count);
                }
                if (data.type === 'new_warning') {
                    showToast(data.severity, data.title);
                }
            } catch(e) {}
        };

        ws.onclose = function() {
            setTimeout(function() {
                retryDelay = Math.min(retryDelay * 2, 30000);
                connect();
            }, retryDelay);
        };
    }

    function updateBadge(count) {
        // Avatar badge
        var avatarBadge = document.querySelector('.avatar-badge');
        if (count > 0) {
            if (!avatarBadge) {
                avatarBadge = document.createElement('span');
                avatarBadge.className = 'avatar-badge';
                document.querySelector('.avatar').appendChild(avatarBadge);
            }
            avatarBadge.textContent = count;
        } else if (avatarBadge) {
            avatarBadge.remove();
        }
        // Dropdown badge
        var dropBadge = document.querySelector('.dropdown-item .badge-count');
        if (count > 0) {
            if (dropBadge) dropBadge.textContent = count;
        } else if (dropBadge) {
            dropBadge.remove();
        }
    }

    function showToast(severity, title) {
        var toast = document.createElement('div');
        toast.className = 'toast toast-' + severity;
        toast.textContent = title;
        var container = document.getElementById('toast-container');
        if (!container) {
            container = document.createElement('div');
            container.id = 'toast-container';
            document.body.appendChild(container);
        }
        container.appendChild(toast);
        setTimeout(function() { toast.classList.add('show'); }, 10);
        setTimeout(function() {
            toast.classList.remove('show');
            setTimeout(function() { toast.remove(); }, 300);
        }, 5000);
    }

    connect();
})();
