/* One refresh swaps a complete reading set. Failed requests retain the last good snapshot. */
htmx.config.timeout = 30000;
const refreshButton = document.querySelector('#refresh-dashboard');
const status = document.querySelector('#refresh-status');
const footer = document.querySelector('.dashboard-footer');
let lastLoaded = new Date();
let openTables = new Set();
const clock = date => date.toLocaleTimeString('en-GB', {hour12: false});
function success() {
    if (!status) return;
    status.textContent = `Updated ${clock(lastLoaded)} · refreshes every 60 seconds`;
    footer.dataset.error = 'false';
    document.querySelector('#dashboard-data').dataset.loadedAt = lastLoaded.toISOString();
}
function failed() {
    if (!status) return;
    status.textContent = `Update failed · showing readings loaded at ${clock(lastLoaded)} · retrying automatically`;
    footer.dataset.error = 'true';
}
refreshButton?.addEventListener('click', () => htmx.trigger(document.body, 'refresh'));
document.body.addEventListener('htmx:beforeRequest', event => {
    if (event.detail.elt.id !== 'dashboard-data') return;
    openTables = new Set([...document.querySelectorAll('details.chart-data[open]')].map(el => el.dataset.key));
    refreshButton.disabled = true;
    status.textContent = 'Updating readings…';
});
document.body.addEventListener('htmx:afterSwap', event => {
    if (event.detail.elt.id !== 'dashboard-data') return;
    document.querySelectorAll('details.chart-data').forEach(el => { el.open = openTables.has(el.dataset.key); });
    lastLoaded = new Date();
    success();
});
for (const name of ['htmx:responseError', 'htmx:sendError', 'htmx:timeout']) {
    document.body.addEventListener(name, failed);
}
document.body.addEventListener('htmx:afterRequest', () => { if(refreshButton) refreshButton.disabled = false; });
success();
