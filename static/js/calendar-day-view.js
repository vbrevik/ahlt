/**
 * Calendar — day view renderer with overlap algorithm.
 *
 * Globals provided: calendarDayView(deps)
 *   deps.el — function(tag, cls, text) → element
 *   deps.fmtDate, deps.torColor, deps.makePill
 *   deps.TODAY — today's date string
 *
 * Returns { render(container, currentDate, cachedEvents) }
 */
function calendarDayView(deps) {
    var DAY_SLOT_HEIGHT = 28; // px per 30-min slot

    /**
     * Compute overlap columns for events with startMin/endMin fields.
     * Returns array of { evt, col, totalCols } where col is 0-based column index.
     */
    function computeOverlapColumns(events) {
        if (!events.length) return [];

        events.sort(function(a, b) {
            return a.startMin - b.startMin || b.endMin - a.endMin;
        });

        var placed = [];
        events.forEach(function(item) {
            var usedCols = {};
            placed.forEach(function(p) {
                if (p.startMin < item.endMin && p.endMin > item.startMin) usedCols[p.col] = true;
            });
            var col = 0;
            while (usedCols[col]) col++;
            item.col = col;
            placed.push({ startMin: item.startMin, endMin: item.endMin, col: col });
        });

        // Connected-components for totalCols — track by event index, not column
        events.forEach(function(item, idx) {
            var inGroup = {};
            inGroup[idx] = true;
            var changed = true;
            while (changed) {
                changed = false;
                events.forEach(function(other, oi) {
                    if (inGroup[oi]) return;
                    events.forEach(function(member, mi) {
                        if (!inGroup[mi]) return;
                        if (member.startMin < other.endMin && member.endMin > other.startMin) {
                            inGroup[oi] = true;
                            changed = true;
                        }
                    });
                });
            }
            var groupCols = {};
            Object.keys(inGroup).forEach(function(k) { groupCols[events[k].col] = true; });
            item.totalCols = Object.keys(groupCols).length;
        });

        return events;
    }

    function render(container, currentDate, cachedEvents) {
        var el = deps.el;
        var grid = el('div', 'outlook-day-grid');
        var dateStr = deps.fmtDate(currentDate);
        var FIRST_HOUR = 7, LAST_HOUR = 19;

        for (var hour = FIRST_HOUR; hour < LAST_HOUR; hour++) {
            for (var half = 0; half < 2; half++) {
                var timeStr = String(hour).padStart(2, '0') + ':' + (half === 0 ? '00' : '30');
                var row = el('div', 'outlook-day-row');
                if (half === 0) row.classList.add('outlook-day-row-hour');
                var label = el('div', 'outlook-time-label');
                if (half === 0) label.textContent = timeStr;
                row.appendChild(label);
                var cell = el('div', 'outlook-day-cell');
                if (dateStr === deps.TODAY) cell.classList.add('outlook-today-col');
                row.appendChild(cell);
                grid.appendChild(row);
            }
        }
        container.appendChild(grid);

        var dayEvents = [];
        cachedEvents.forEach(function(evt) {
            if (evt.date !== dateStr) return;
            var parts = evt.start_time.split(':');
            var evtHour = parseInt(parts[0], 10);
            var evtMinute = parseInt(parts[1], 10);
            if (evtHour < FIRST_HOUR || evtHour >= LAST_HOUR) return;
            var startMin = (evtHour - FIRST_HOUR) * 60 + evtMinute;
            var endMin = startMin + (evt.duration_minutes || 30);
            dayEvents.push({ evt: evt, startMin: startMin, endMin: endMin });
        });

        if (!dayEvents.length) return;
        computeOverlapColumns(dayEvents);

        var dayCells = grid.querySelectorAll('.outlook-day-cell');
        dayEvents.forEach(function(item) {
            var evt = item.evt;
            var slotIndex = Math.floor(item.startMin / 30);
            if (slotIndex >= dayCells.length) return;
            var targetCell = dayCells[slotIndex];
            var p = deps.makePill(evt, true);
            p.pill.classList.add('outlook-event-day');

            var offsetInSlot = item.startMin % 30;
            var topPx = Math.round(offsetInSlot * DAY_SLOT_HEIGHT / 30);
            var heightPx = Math.round((item.endMin - item.startMin) * DAY_SLOT_HEIGHT / 30) - 2;
            p.pill.style.height = Math.max(heightPx, 20) + 'px';

            if (item.totalCols > 1) {
                p.pill.style.position = 'absolute';
                p.pill.style.top = topPx + 'px';
                var widthPct = 100 / item.totalCols;
                p.pill.style.left = (item.col * widthPct) + '%';
                p.pill.style.width = widthPct + '%';
                p.pill.style.boxSizing = 'border-box';
                p.pill.style.zIndex = String(item.col + 1);
            } else {
                if (topPx > 0) p.pill.style.marginTop = topPx + 'px';
            }

            var dur = el('span', 'outlook-event-dur', evt.duration_minutes + 'min');
            p.pill.appendChild(document.createTextNode(' '));
            p.pill.appendChild(dur);
            targetCell.appendChild(p.pill);
        });
    }

    return { render: render };
}
