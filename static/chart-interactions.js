/* One delegated interaction layer survives dashboard polling and keeps source times unchanged. */
const shortMonths = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const normalNumber = new Intl.NumberFormat('en-US', {minimumFractionDigits: 1, maximumFractionDigits: 2});
const smallNumber = new Intl.NumberFormat('en-US', {maximumSignificantDigits: 3});

export function reading(value) {
    if (value == null || String(value).trim() === '') return null;
    const number = Number(value);
    return Number.isFinite(number) ? number : null;
}

export function formatChartValue(value) {
    const number = reading(value);
    if (number === null) return 'No readings';
    return number !== 0 && Math.abs(number) < 0.01 ? smallNumber.format(number) : normalNumber.format(number);
}

export function formatBucketTime(value, bucket) {
    const match = /^(\d{4})-(\d{2})-(\d{2})(?: (\d{2}):\d{2}:\d{2})?$/.exec(value);
    if (!match) return value;
    const [, year, month, day, hour] = match;
    const date = new Date(0);
    date.setUTCFullYear(Number(year), Number(month) - 1, Number(day));
    if (date.getUTCFullYear() !== Number(year) || date.getUTCMonth() + 1 !== Number(month) || date.getUTCDate() !== Number(day)) return value;
    if (bucket === 'month') return new Intl.DateTimeFormat('en-GB', {month: 'long', year: 'numeric', timeZone: 'UTC'}).format(date);
    const label = `${Number(day)} ${shortMonths[Number(month) - 1]} ${year}`;
    if (bucket !== 'hour' || hour === undefined || Number(hour) > 23) return label;
    const end = String((Number(hour) + 1) % 24).padStart(2, '0');
    return `${label} · ${hour}:00–${end}:00${hour === '23' ? ' (+1 day)' : ''}`;
}

function initializeChartInteractions() {
    const tooltip = document.createElement('div');
    tooltip.id = 'chart-tooltip';
    tooltip.className = 'chart-tooltip';
    tooltip.setAttribute('role', 'tooltip');
    tooltip.setAttribute('aria-live', 'polite');
    tooltip.hidden = true;
    document.body.append(tooltip);
    let activeHit = null;
    let activeChart = null;
    let activeMark = null;
    let inputMode = null;

    function hide() {
        if (activeChart) {
            activeChart.classList.remove('is-inspecting');
            activeChart.removeAttribute('aria-describedby');
            activeChart.querySelectorAll('[data-chart-overlay]').forEach(node => node.remove());
        }
        activeHit?.classList.remove('is-active');
        activeMark?.classList.remove('is-active');
        tooltip.hidden = true;
        activeHit = activeChart = activeMark = inputMode = null;
    }

    function addRow(list, label, value, unit) {
        const row = document.createElement('div');
        const key = document.createElement('dt');
        key.textContent = label;
        const number = document.createElement('dd');
        number.textContent = reading(value) === null ? 'No readings' : `${formatChartValue(value)} ${unit || '(unit not set)'}`;
        row.append(key, number);
        list.append(row);
    }

    function drawOverlay(chart, hit) {
        const x = hit.dataset.x;
        const top = Number(hit.getAttribute('y'));
        const bottom = top + Number(hit.getAttribute('height'));
        const guide = document.createElementNS('http://www.w3.org/2000/svg', 'line');
        guide.dataset.chartOverlay = 'true';
        guide.setAttribute('class', 'chart-guide');
        for (const [key, value] of Object.entries({x1: x, x2: x, y1: top, y2: bottom})) guide.setAttribute(key, value);
        chart.append(guide);
        if (chart.dataset.chart === 'trend' && reading(hit.dataset.value) !== null) {
            const dot = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
            dot.dataset.chartOverlay = 'true';
            dot.setAttribute('class', 'chart-active-dot');
            dot.setAttribute('cx', x);
            dot.setAttribute('cy', hit.dataset.y);
            dot.setAttribute('r', '5');
            chart.append(dot);
        }
    }

    function position(hit, event) {
        const bounds = tooltip.getBoundingClientRect();
        const anchor = hit.getBoundingClientRect();
        const x = event?.clientX ?? anchor.x + anchor.width / 2;
        const y = event?.clientY ?? anchor.y + anchor.height / 2;
        let left = x + 16;
        let top = y + 16;
        if (left + bounds.width > window.innerWidth - 8) left = x - bounds.width - 16;
        if (top + bounds.height > window.innerHeight - 8) top = y - bounds.height - 16;
        tooltip.style.left = `${Math.max(8, Math.min(left, window.innerWidth - bounds.width - 8))}px`;
        tooltip.style.top = `${Math.max(8, Math.min(top, window.innerHeight - bounds.height - 8))}px`;
    }

    function show(hit, event, mode) {
        const chart = hit?.closest('svg[data-chart]');
        if (!chart || !hit.isConnected) return;
        if (hit !== activeHit) {
            hide();
            activeHit = hit;
            activeChart = chart;
            activeMark = chart.querySelector(`[data-chart-mark="${Number(hit.dataset.index)}"]`);
            chart.classList.add('is-inspecting');
            chart.setAttribute('aria-describedby', tooltip.id);
            hit.classList.add('is-active');
            activeMark?.classList.add('is-active');
            drawOverlay(chart, hit);
            tooltip.dataset.tone = chart.classList.contains('orange') || hit.dataset.peak === 'true' ? 'orange' : 'blue';
            const heading = document.createElement('div');
            heading.className = 'chart-tooltip-heading';
            const label = document.createElement('strong');
            label.textContent = chart.dataset.label;
            heading.append(label);
            if (hit.dataset.peak === 'true') {
                const badge = document.createElement('span');
                badge.className = 'chart-tooltip-badge';
                badge.textContent = 'Highest bucket';
                heading.append(badge);
            }
            const time = document.createElement('p');
            time.className = 'chart-tooltip-time';
            time.textContent = formatBucketTime(hit.dataset.time, chart.dataset.bucket);
            const list = document.createElement('dl');
            addRow(list, chart.dataset.kind === 'counter' ? 'Consumption' : 'Average', hit.dataset.value, chart.dataset.unit);
            if (chart.dataset.kind !== 'counter' && reading(hit.dataset.value) !== null) {
                if (reading(hit.dataset.minimum) !== null) addRow(list, 'Minimum', hit.dataset.minimum, chart.dataset.unit);
                if (reading(hit.dataset.maximum) !== null) addRow(list, 'Maximum', hit.dataset.maximum, chart.dataset.unit);
            }
            tooltip.replaceChildren(heading, time, list);
            tooltip.hidden = false;
        }
        inputMode = mode;
        chart.dataset.inspectionMode = mode;
        position(hit, event);
    }

    document.addEventListener('pointermove', event => {
        if (event.pointerType === 'touch') return;
        const hit = event.target.closest?.('.chart-hit');
        if (hit) show(hit, event, 'pointer');
        else if (inputMode === 'pointer') hide();
    });
    document.addEventListener('pointerdown', event => {
        const hit = event.target.closest?.('.chart-hit');
        if (hit) show(hit, event, event.pointerType === 'touch' ? 'touch' : 'pointer');
        else hide();
    });
    document.addEventListener('pointerout', event => {
        if (!event.relatedTarget && inputMode === 'pointer') hide();
    });
    document.addEventListener('focusin', event => {
        const chart = event.target.closest?.('svg[data-chart]');
        if (!chart) { hide(); return; }
        if (activeChart === chart && inputMode !== 'keyboard') return;
        const hits = [...chart.querySelectorAll('.chart-hit')];
        const first = hits.find(hit => reading(hit.dataset.value) !== null);
        if (first) show(first, null, 'keyboard');
    });
    document.addEventListener('focusout', event => {
        if (activeChart && !activeChart.contains(event.relatedTarget)) hide();
    });
    document.addEventListener('keydown', event => {
        if (event.key === 'Escape') { hide(); return; }
        const chart = event.target.closest?.('svg[data-chart]');
        if (!chart || !['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
        const hits = [...chart.querySelectorAll('.chart-hit')];
        if (!hits.length) return;
        event.preventDefault();
        let index = activeChart === chart ? hits.indexOf(activeHit) : -1;
        if (event.key === 'Home') index = 0;
        else if (event.key === 'End') index = hits.length - 1;
        else index = Math.max(0, Math.min(hits.length - 1, index + (event.key === 'ArrowRight' ? 1 : -1)));
        show(hits[index], null, 'keyboard');
    });
    document.addEventListener('scroll', hide, true);
    window.addEventListener('resize', hide);
    document.body.addEventListener('htmx:beforeSwap', hide);
}

if (typeof document !== 'undefined') initializeChartInteractions();
