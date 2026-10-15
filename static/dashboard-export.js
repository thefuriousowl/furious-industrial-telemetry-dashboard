/* Export the displayed reading set in a detached desktop layout, entirely on this computer. */
const REPORT_WIDTH = 1440;
const MAX_PIXELS = 16_000_000;
const MAX_SIDE = 16_000;
const libraryLoads = new Map();
let exporting = false;

export function exportFilename(period, start, group) {
    const safe = value => String(value || '').normalize('NFKC')
        .replace(/[^\p{L}\p{M}\p{N}_-]+/gu, '-').replace(/^-+|-+$/g, '').slice(0, 70);
    return `telemetry-${safe(period) || 'period'}-${safe(start) || 'date'}-${safe(group) || 'all-groups'}`;
}

export function captureDimensions(width, height) {
    if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) {
        throw new Error('The dashboard has no exportable content.');
    }
    const pixelRatio = Math.min(2, Math.sqrt(MAX_PIXELS / (width * height)), MAX_SIDE / width, MAX_SIDE / height);
    if (pixelRatio < 1) {
        throw new Error('This dashboard is too large to export. Reduce the number of widgets or groups.');
    }
    return {width, height, pixelRatio};
}

export function pdfPage(width, height) {
    return {width: width * 0.75, height: height * 0.75, orientation: height >= width ? 'portrait' : 'landscape'};
}

function loadLibrary(url) {
    if (!libraryLoads.has(url)) {
        const promise = new Promise((resolve, reject) => {
            const script = document.createElement('script');
            script.src = url;
            const timer = setTimeout(() => {
                script.remove();
                reject(new Error('The export component timed out. Please retry.'));
            }, 15_000);
            script.onload = () => { clearTimeout(timer); resolve(); };
            script.onerror = () => {
                clearTimeout(timer);
                script.remove();
                reject(new Error('The export component could not load. Please retry.'));
            };
            document.head.append(script);
        });
        libraryLoads.set(url, promise);
        promise.catch(() => libraryLoads.delete(url));
    }
    return libraryLoads.get(url);
}

function reportSnapshot() {
    const live = document.querySelector('.dashboard-shell');
    if (!live) throw new Error('No dashboard is available to export.');
    // Clone before any await: later polling cannot change the exported reading set.
    const report = live.cloneNode(true);
    // A report contains the chart data, without temporary hover/keyboard decorations.
    report.querySelectorAll('[data-chart-overlay], .chart-hit-targets').forEach(node => node.remove());
    report.querySelectorAll('.is-inspecting, [data-chart-mark].is-active').forEach(node => {
        node.classList.remove('is-inspecting', 'is-active');
        node.removeAttribute('aria-describedby');
    });
    const scope = live.querySelector('.snapshot-meta > span')?.textContent || '';
    const range = live.querySelector('.range-title')?.textContent || '';
    const context = document.createElement('div');
    context.className = 'report-context';
    for (const text of [scope, range, live.dataset.reportPeriod]) {
        const item = document.createElement('span');
        item.textContent = text;
        context.append(item);
    }
    report.querySelector('.dashboard-toolbar').replaceWith(context);
    report.querySelectorAll('.header-actions, .chart-data, .mobile-chart-hint, #export-status, script').forEach(node => node.remove());
    report.querySelectorAll('[hx-get], [hx-trigger], [hx-swap]').forEach(node => {
        for (const attribute of [...node.attributes]) {
            if (attribute.name.startsWith('hx-')) node.removeAttribute(attribute.name);
        }
    });
    report.querySelectorAll('.ranking-item').forEach(node => node.removeAttribute('href'));
    const rankingCaption = report.querySelector('.ranking-panel > .chart-caption');
    if (rankingCaption) rankingCaption.textContent = 'Bar length: share of total (%)';
    const footer = report.querySelector('.dashboard-footer');
    const source = footer.lastElementChild.cloneNode(true);
    const timestamp = document.createElement('span');
    const loadedAt = live.querySelector('#dashboard-data')?.dataset.loadedAt;
    timestamp.textContent = `Readings loaded: ${loadedAt ? new Date(loadedAt).toLocaleString('en-GB', {hour12: false}) : 'unknown'} · Exported: ${new Date().toLocaleString('en-GB', {hour12: false})}`;
    if (footer.dataset.error === 'true') {
        const note = document.createElement('p');
        note.className = 'data-note';
        note.textContent = 'The last refresh failed. This report contains the last successfully loaded readings.';
        footer.before(note);
    }
    footer.replaceChildren(timestamp, source);
    report.classList.add('export-report');
    return {
        report,
        filename: exportFilename(live.dataset.reportPeriod, live.dataset.reportStart, live.dataset.reportGroup),
        title: live.querySelector('h1')?.textContent || 'Telemetry dashboard',
    };
}

async function renderReport(report) {
    const stylesheet = document.querySelector('link[href*="dashboard.css"]').href;
    const response = await fetch(stylesheet, {signal: AbortSignal.timeout(15_000)});
    if (!response.ok) throw new Error('The report stylesheet could not load. Please retry.');
    const css = await response.text();
    const frame = document.createElement('iframe');
    frame.className = 'export-frame';
    frame.title = 'Preparing dashboard export';
    frame.setAttribute('aria-hidden', 'true');
    frame.tabIndex = -1;
    frame.style.width = `${REPORT_WIDTH}px`;
    // Establish a separate viewport so a mobile export still uses the complete desktop layout.
    let loadTimer;
    const ready = new Promise((resolve, reject) => {
        frame.onload = resolve;
        loadTimer = setTimeout(() => reject(new Error('The report could not be prepared. Please retry.')), 15_000);
    });
    frame.srcdoc = '<!doctype html><html><head><meta charset="utf-8"></head><body></body></html>';
    document.body.append(frame);
    try {
        await ready;
        clearTimeout(loadTimer);
        const doc = frame.contentDocument;
        const style = doc.createElement('style');
        style.textContent = css + '\n.export-report { width: 1440px; margin: 0; } .export-report .ranking-list { max-height: none; overflow: visible; } .export-report .chart-point { opacity: 1; }';
        doc.head.append(style);
        doc.body.append(doc.adoptNode(report));
        await doc.fonts.ready;
        const height = Math.ceil(report.getBoundingClientRect().height);
        frame.style.height = `${height}px`;
        // html-to-image deep-clones SVGs without walking their descendants.
        // Resolve their CSS here so bars, bands, text and units retain the on-screen appearance.
        const svgProperties = ['fill', 'fill-opacity', 'stroke', 'stroke-width', 'stroke-opacity',
            'stroke-dasharray', 'stroke-linecap', 'stroke-linejoin', 'opacity', 'font-family',
            'font-size', 'font-weight', 'font-style', 'letter-spacing', 'paint-order', 'text-anchor'];
        for (const node of report.querySelectorAll('svg, svg *')) {
            const computed = doc.defaultView.getComputedStyle(node);
            for (const property of svgProperties) node.style.setProperty(property, computed.getPropertyValue(property));
        }
        const dimensions = captureDimensions(REPORT_WIDTH, height);
        return await window.htmlToImage.toCanvas(report, {...dimensions, backgroundColor: '#f3f4f8', skipFonts: true});
    } finally {
        clearTimeout(loadTimer);
        frame.remove();
    }
}

function canvasBlob(canvas) {
    return new Promise((resolve, reject) => canvas.toBlob(blob => blob ? resolve(blob) : reject(new Error('The image could not be encoded.')), 'image/png'));
}

function download(blob, filename) {
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = filename;
    document.body.append(link);
    link.click();
    link.remove();
    // Allow the download to consume the blob before releasing it.
    setTimeout(() => URL.revokeObjectURL(url), 60_000);
}

async function exportDashboard(format) {
    if (exporting) return;
    const status = document.querySelector('#export-status');
    const buttons = [...document.querySelectorAll('[data-export]')];
    exporting = true;
    buttons.forEach(button => { button.disabled = true; });
    status.hidden = false;
    status.dataset.error = 'false';
    status.textContent = `Preparing ${format.toUpperCase()}…`;
    document.querySelector('#export-menu').open = false;
    let canvas;
    try {
        const {report, filename, title} = reportSnapshot();
        await loadLibrary('/static/vendor/html-to-image-1.11.13.min.js');
        if (format === 'pdf') await loadLibrary('/static/vendor/jspdf-4.2.1.min.js');
        canvas = await renderReport(report);
        let blob;
        if (format === 'png') {
            blob = await canvasBlob(canvas);
        } else {
            const page = pdfPage(REPORT_WIDTH, canvas.height * REPORT_WIDTH / canvas.width);
            const pdf = new window.jspdf.jsPDF({unit: 'pt', format: [page.width, page.height], orientation: page.orientation, compress: true});
            pdf.setProperties({title, subject: 'Dashboard snapshot', creator: 'Industrial telemetry'});
            // Lossless image keeps small axis labels and units sharp in the PDF.
            pdf.addImage(canvas, 'PNG', 0, 0, page.width, page.height, undefined, 'FAST');
            blob = pdf.output('blob');
        }
        download(blob, `${filename}.${format}`);
        status.textContent = `${format.toUpperCase()} download started.`;
    } catch (error) {
        status.dataset.error = 'true';
        status.textContent = `Export failed: ${error.message || 'Please retry.'}`;
    } finally {
        // Large backing buffers should not stay allocated after the download.
        if (canvas) { canvas.width = 0; canvas.height = 0; }
        buttons.forEach(button => { button.disabled = false; });
        exporting = false;
    }
}

if (typeof document !== 'undefined') {
    document.querySelectorAll('[data-export]').forEach(button => {
        button.addEventListener('click', () => exportDashboard(button.dataset.export));
    });
    document.addEventListener('click', event => {
        const menu = document.querySelector('#export-menu');
        if (menu && !menu.contains(event.target)) menu.open = false;
    });
    document.addEventListener('keydown', event => {
        if (event.key === 'Escape') document.querySelector('#export-menu').open = false;
    });
}
