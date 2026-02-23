/**
 * Calendar — week and month view renderers.
 *
 * Globals provided: calendarWeekView(deps), calendarMonthView(deps)
 *   deps.el — function(tag, cls, text) → element
 *   deps.fmtDate, deps.addDays, deps.firstOfMonth, deps.torColor, deps.makePill
 *   deps.DAYS, deps.TODAY
 *
 * Returns { render(container, currentDate, cachedEvents) }
 */
function calendarWeekView(deps) {
    function render(container, currentDate, cachedEvents) {
        var el = deps.el;
        var grid = el('div', 'outlook-week-grid');

        // Header row
        grid.appendChild(el('div', 'outlook-time-header'));
        for (var col = 0; col < 7; col++) {
            var d = deps.addDays(currentDate, col);
            var hdr = el('div', 'outlook-day-header');
            if (deps.fmtDate(d) === deps.TODAY) hdr.classList.add('outlook-today');
            hdr.appendChild(el('span', 'outlook-day-name', deps.DAYS[col]));
            hdr.appendChild(el('span', 'outlook-day-num', String(d.getDate())));
            grid.appendChild(hdr);
        }

        // Time rows 07:00 - 19:00
        for (var hour = 7; hour < 19; hour++) {
            for (var half = 0; half < 2; half++) {
                var timeStr = String(hour).padStart(2, '0') + ':' + (half === 0 ? '00' : '30');
                var timeCell = el('div', 'outlook-time-label');
                if (half === 0) timeCell.textContent = timeStr;
                grid.appendChild(timeCell);

                for (var c2 = 0; c2 < 7; c2++) {
                    var cell = el('div', 'outlook-cell');
                    if (half === 0) cell.classList.add('outlook-cell-hour');
                    var dateStr = deps.fmtDate(deps.addDays(currentDate, c2));
                    if (dateStr === deps.TODAY) cell.classList.add('outlook-today-col');
                    cell.dataset.date = dateStr;
                    cell.dataset.time = timeStr;
                    grid.appendChild(cell);
                }
            }
        }

        container.appendChild(grid);

        // Place events
        cachedEvents.forEach(function(evt) {
            var parts = evt.start_time.split(':');
            var evtHour = parseInt(parts[0], 10);
            var evtMin = parseInt(parts[1], 10);
            if (evtHour < 7 || evtHour >= 19) return;

            var d = deps.parseDate(evt.date);
            var col = (d.getDay() + 6) % 7;
            var rowOffset = (evtHour - 7) * 2 + (evtMin >= 30 ? 1 : 0);

            var cells = grid.querySelectorAll('.outlook-cell');
            var idx = rowOffset * 7 + col;
            if (idx >= cells.length) return;

            var p = deps.makePill(evt, false);
            p.pill.style.height = (p.slots * 100) + '%';
            cells[idx].appendChild(p.pill);
        });
    }

    return { render: render };
}

function calendarMonthView(deps) {
    function render(container, currentDate, cachedEvents) {
        var el = deps.el;
        var grid = el('div', 'outlook-month-grid');

        for (var i = 0; i < 7; i++) {
            grid.appendChild(el('div', 'outlook-month-header', deps.DAYS[i]));
        }

        var first = deps.firstOfMonth(currentDate);
        var startCol = (first.getDay() + 6) % 7;
        var lastDay = new Date(first.getFullYear(), first.getMonth() + 1, 0).getDate();

        var eventsByDate = {};
        cachedEvents.forEach(function(e) {
            if (!eventsByDate[e.date]) eventsByDate[e.date] = [];
            eventsByDate[e.date].push(e);
        });

        for (var b = 0; b < startCol; b++) {
            grid.appendChild(el('div', 'outlook-month-cell outlook-month-blank'));
        }

        for (var day = 1; day <= lastDay; day++) {
            var d = new Date(first.getFullYear(), first.getMonth(), day);
            var ds = deps.fmtDate(d);
            var cell = el('div', 'outlook-month-cell');
            if (ds === deps.TODAY) cell.classList.add('outlook-today');
            cell.appendChild(el('span', 'outlook-month-num', String(day)));

            var dayEvts = eventsByDate[ds] || [];
            dayEvts.forEach(function(evt) {
                var c = deps.torColor(evt.tor_id);
                var dot = document.createElement('a');
                dot.href = '/tor/' + evt.tor_id;
                dot.className = 'outlook-month-event';
                dot.style.backgroundColor = c.bg;
                dot.style.borderLeft = '2px solid ' + c.border;
                dot.style.color = c.text;
                dot.textContent = evt.start_time + ' ' + evt.tor_label;
                dot.title = evt.tor_label + ' \u00b7 ' + evt.start_time + ' \u00b7 ' + evt.duration_minutes + 'min';
                cell.appendChild(dot);
            });

            grid.appendChild(cell);
        }

        var total = startCol + lastDay;
        var trailing = (7 - (total % 7)) % 7;
        for (var t = 0; t < trailing; t++) {
            grid.appendChild(el('div', 'outlook-month-cell outlook-month-blank'));
        }

        container.appendChild(grid);
    }

    return { render: render };
}
