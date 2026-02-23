/**
 * Calendar outlook — orchestrator (colors, navigation, pill creation, fetch, confirm).
 * Depends on: calendar-day-view.js, calendar-views.js
 */
(function() {
    var container = document.getElementById('outlook-container');
    var titleEl = document.getElementById('outlook-title');
    var loadingEl = document.getElementById('outlook-loading');

    var TOR_COLORS = [
        { bg: '#dbeafe', border: '#3b82f6', text: '#1e40af' },
        { bg: '#dcfce7', border: '#22c55e', text: '#166534' },
        { bg: '#fef3c7', border: '#f59e0b', text: '#92400e' },
        { bg: '#fce7f3', border: '#ec4899', text: '#9d174d' },
        { bg: '#e0e7ff', border: '#6366f1', text: '#3730a3' },
        { bg: '#f3e8ff', border: '#a855f7', text: '#6b21a8' },
        { bg: '#ccfbf1', border: '#14b8a6', text: '#115e59' },
        { bg: '#fee2e2', border: '#ef4444', text: '#991b1b' }
    ];
    var torColorMap = {};
    var nextColorIdx = 0;
    function torColor(torId) {
        if (!torColorMap[torId]) {
            torColorMap[torId] = TOR_COLORS[nextColorIdx % TOR_COLORS.length];
            nextColorIdx++;
        }
        return torColorMap[torId];
    }

    function el(tag, cls, text) {
        var e = document.createElement(tag);
        if (cls) e.className = cls;
        if (text) e.textContent = text;
        return e;
    }

    var initData = JSON.parse(document.getElementById('outlook-init-data').textContent);
    var TODAY = initData.today;
    var currentView = 'week';
    var currentDate = parseDate(initData.week_start);
    var cachedEvents = initData.events;
    cachedEvents.forEach(function(e) { torColor(e.tor_id); });

    var FETCH_TIMEOUT_MS = 30000;
    function fetchWithTimeout(url, options, ms) {
        var controller = new AbortController();
        var timer = setTimeout(function() { controller.abort(); }, ms);
        var opts = Object.assign({}, options || {}, { signal: controller.signal });
        return fetch(url, opts).finally(function() { clearTimeout(timer); });
    }

    var DAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
    var MONTHS = ['January', 'February', 'March', 'April', 'May', 'June',
                  'July', 'August', 'September', 'October', 'November', 'December'];

    function parseDate(s) { var p = s.split('-'); return new Date(+p[0], +p[1] - 1, +p[2]); }
    function fmtDate(d) {
        return d.getFullYear() + '-' + String(d.getMonth() + 1).padStart(2, '0') + '-' + String(d.getDate()).padStart(2, '0');
    }
    function addDays(d, n) { var r = new Date(d); r.setDate(r.getDate() + n); return r; }
    function mondayOf(d) {
        var r = new Date(d); var day = r.getDay();
        r.setDate(r.getDate() + (day === 0 ? -6 : 1 - day)); return r;
    }
    function firstOfMonth(d) { return new Date(d.getFullYear(), d.getMonth(), 1); }

    function makePill(evt, showLoc) {
        var c = torColor(evt.tor_id);
        var slots = Math.max(1, Math.ceil(evt.duration_minutes / 30));
        var pill = document.createElement('div');
        pill.className = 'outlook-event';
        if (evt.meeting_status) pill.classList.add('outlook-event--' + evt.meeting_status);
        pill.style.backgroundColor = c.bg;
        pill.style.borderLeft = '3px solid ' + c.border;
        pill.style.color = c.text;
        pill.title = evt.tor_label + ' \u00b7 ' + evt.start_time + ' \u00b7 ' + evt.duration_minutes + 'min' +
            (evt.location ? ' \u00b7 ' + evt.location : '');

        var content = document.createElement('div');
        content.className = 'outlook-event-content';
        var link = document.createElement('a');
        link.href = '/tor/' + evt.tor_id;
        link.className = 'outlook-event-link';
        link.appendChild(el('span', 'outlook-event-time', evt.start_time));
        link.appendChild(document.createTextNode(' '));
        link.appendChild(el('span', 'outlook-event-label', evt.tor_label));
        if (showLoc && evt.location) {
            link.appendChild(document.createTextNode(' '));
            link.appendChild(el('span', 'outlook-event-loc', evt.location));
        }
        content.appendChild(link);
        pill.appendChild(content);

        if (evt.date > TODAY && evt.meeting_status !== 'confirmed') {
            var badge = document.createElement('button');
            badge.type = 'button';
            badge.className = 'outlook-event-confirm-badge';
            badge.textContent = '\u2713';
            badge.title = 'Confirm this meeting';
            badge.addEventListener('click', function(e) {
                e.preventDefault(); e.stopPropagation();
                confirmMeetingAjax(evt, pill, badge);
            });
            pill.appendChild(badge);
        }
        return { pill: pill, slots: slots };
    }

    function confirmMeetingAjax(evt, pill, badge) {
        var csrfToken = document.getElementById('csrf_token').value;
        badge.classList.add('outlook-event-confirm-badge--loading');
        badge.textContent = '\u22ef';
        var body = 'csrf_token=' + encodeURIComponent(csrfToken) +
                   '&meeting_date=' + encodeURIComponent(evt.date) +
                   '&tor_name=' + encodeURIComponent(evt.tor_label);
        if (evt.meeting_id) body += '&meeting_id=' + encodeURIComponent(evt.meeting_id);

        fetchWithTimeout('/api/tor/' + evt.tor_id + '/meetings/confirm-calendar', {
            method: 'POST',
            headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
            body: body, credentials: 'same-origin'
        }, FETCH_TIMEOUT_MS)
        .then(function(r) { if (!r.ok) throw new Error('Server error ' + r.status); return r.json(); })
        .then(function(data) {
            if (!data.ok) throw new Error(data.error || 'Unknown error');
            pill.classList.remove('outlook-event--projected');
            pill.classList.add('outlook-event--confirmed');
            if (badge.parentNode) badge.parentNode.removeChild(badge);
        })
        .catch(function(err) {
            badge.classList.remove('outlook-event-confirm-badge--loading');
            badge.textContent = '\u2713';
            if (err.name === 'AbortError') badge.title = 'Request timed out. Click to retry.';
            console.error('Failed to confirm meeting:', err);
        });
    }

    // Shared deps for view renderers
    var viewDeps = {
        el: el, fmtDate: fmtDate, addDays: addDays, parseDate: parseDate,
        firstOfMonth: firstOfMonth, torColor: torColor, makePill: makePill,
        DAYS: DAYS, TODAY: TODAY
    };
    var dayView = calendarDayView(viewDeps);
    var weekView = calendarWeekView(viewDeps);
    var monthView = calendarMonthView(viewDeps);

    // Tab switching
    document.querySelectorAll('.outlook-tab').forEach(function(tab) {
        tab.addEventListener('click', function() {
            document.querySelectorAll('.outlook-tab').forEach(function(t) { t.classList.remove('active'); });
            tab.classList.add('active');
            currentView = tab.dataset.view;
            if (currentView === 'week') currentDate = mondayOf(currentDate);
            else if (currentView === 'month') currentDate = firstOfMonth(currentDate);
            fetchAndRender();
        });
    });

    // Navigation
    document.getElementById('outlook-prev').addEventListener('click', function() {
        if (currentView === 'day') currentDate = addDays(currentDate, -1);
        else if (currentView === 'week') currentDate = addDays(currentDate, -7);
        else currentDate = new Date(currentDate.getFullYear(), currentDate.getMonth() - 1, 1);
        fetchAndRender();
    });
    document.getElementById('outlook-next').addEventListener('click', function() {
        if (currentView === 'day') currentDate = addDays(currentDate, 1);
        else if (currentView === 'week') currentDate = addDays(currentDate, 7);
        else currentDate = new Date(currentDate.getFullYear(), currentDate.getMonth() + 1, 1);
        fetchAndRender();
    });
    document.getElementById('outlook-today').addEventListener('click', function() {
        currentDate = parseDate(TODAY);
        if (currentView === 'week') currentDate = mondayOf(currentDate);
        else if (currentView === 'month') currentDate = firstOfMonth(currentDate);
        fetchAndRender();
    });

    function fetchAndRender() {
        var start, end;
        if (currentView === 'day') { start = fmtDate(currentDate); end = start; }
        else if (currentView === 'week') { start = fmtDate(currentDate); end = fmtDate(addDays(currentDate, 6)); }
        else { start = fmtDate(firstOfMonth(currentDate)); end = fmtDate(new Date(currentDate.getFullYear(), currentDate.getMonth() + 1, 0)); }

        loadingEl.style.display = '';
        fetchWithTimeout('/api/tor/calendar?start=' + start + '&end=' + end, null, FETCH_TIMEOUT_MS)
            .then(function(r) { return r.json(); })
            .then(function(events) {
                loadingEl.style.display = 'none';
                events.forEach(function(e) { torColor(e.tor_id); });
                cachedEvents = events;
                render();
            })
            .catch(function(err) {
                loadingEl.style.display = 'none';
                if (err.name === 'AbortError') container.textContent = 'Request timed out. Please try again.';
                else { container.textContent = 'Failed to load calendar data.'; console.error('Calendar fetch error:', err); }
            });
    }

    function updateTitle() {
        if (currentView === 'day') {
            titleEl.textContent = DAYS[(currentDate.getDay() + 6) % 7] + ' ' +
                currentDate.getDate() + ' ' + MONTHS[currentDate.getMonth()] + ' ' + currentDate.getFullYear();
        } else if (currentView === 'week') {
            var end = addDays(currentDate, 6);
            if (currentDate.getMonth() === end.getMonth()) {
                titleEl.textContent = currentDate.getDate() + '\u2013' + end.getDate() + ' ' +
                    MONTHS[currentDate.getMonth()] + ' ' + currentDate.getFullYear();
            } else {
                titleEl.textContent = currentDate.getDate() + ' ' + MONTHS[currentDate.getMonth()].substring(0, 3) +
                    ' \u2013 ' + end.getDate() + ' ' + MONTHS[end.getMonth()].substring(0, 3) + ' ' + end.getFullYear();
            }
        } else {
            titleEl.textContent = MONTHS[currentDate.getMonth()] + ' ' + currentDate.getFullYear();
        }
    }

    function render() {
        updateTitle();
        while (container.firstChild) container.removeChild(container.firstChild);
        if (currentView === 'week') weekView.render(container, currentDate, cachedEvents);
        else if (currentView === 'day') dayView.render(container, currentDate, cachedEvents);
        else monthView.render(container, currentDate, cachedEvents);
    }

    render();
})();
