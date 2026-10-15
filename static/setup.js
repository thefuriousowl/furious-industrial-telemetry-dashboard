const form = document.querySelector("#setup-form");
const rows = document.querySelector("#signal-rows");
const timeColumn = document.querySelector("#time-column");
const message = document.querySelector("#setup-message");
const preview = document.querySelector("#config-preview");
const autoButton = document.querySelector("#auto-generate");
const previewButton = document.querySelector("#preview-button");
const databasePath = document.querySelector("#database-path");
const tableName = document.querySelector("#table-name");
const openButton = document.querySelector("#open-database");
const browser = document.querySelector("#database-browser");
const browsePath = document.querySelector("#browse-path");
const browserMessage = document.querySelector("#browser-message");
const parentButton = document.querySelector("#browse-up");
const saveButton = document.querySelector("#save-button");
const settings = document.querySelector("#setup-fields");
const rankingLabel = document.querySelector("#ranking-label");
const mainTrend = document.querySelector("#main-trend");
let baseConfig = null;
const metricRules = new Map();
const ruleRows = document.querySelector("#metric-rules");
let previewConfig = null;
let columns = [];
let inspectedPath = "";
let sourceRevision = 0;
let browseRevision = 0;
let parentPath = null;

async function requestJson(url, body) {
    const options = body === undefined ? {} : {
        method: "POST", headers: {"Content-Type": "application/json"}, body: JSON.stringify(body),
    };
    const response = await fetch(url, options);
    if (!response.ok) { throw new Error(await response.text()); }
    return response.json();
}

function clearColumns() {
    columns = [];
    rows.replaceChildren();
    timeColumn.replaceChildren();
    timeColumn.disabled = true;
    autoButton.disabled = true;
    previewButton.disabled = true;
    invalidatePreview();
}

function resetDatabase() {
    sourceRevision += 1;
    inspectedPath = "";
    tableName.replaceChildren(new Option("Open a database first", ""));
    tableName.disabled = true;
    openButton.disabled = false;
    clearColumns();
}

databasePath.addEventListener("input", () => {
    resetDatabase();
    showMessage("Path changed. Press Open database to read its tables.");
});

async function openDatabase(savedSource = null) {
    resetDatabase();
    const path = databasePath.value.trim();
    if (!path) { showMessage("Choose a database or enter its path.", true); return; }
    const revision = sourceRevision;
    openButton.disabled = true;
    showMessage("Reading database tables…");
    try {
        const tables = await requestJson("/api/setup/tables", {db_path: path});
        if (revision !== sourceRevision) { return; }
        if (!tables.length) { throw new Error("This database has no user tables."); }
        inspectedPath = path;
        tableName.replaceChildren(new Option("Choose a table", ""));
        for (const name of tables) { tableName.append(new Option(name, name)); }
        tableName.disabled = false;
        showMessage("Database inspected. Select a table to read its columns.");
        if (savedSource) {
            if (!tables.includes(savedSource.table_name)) { throw new Error("The configured table is missing. Choose another table."); }
            tableName.value = savedSource.table_name;
            await loadColumns(savedSource);
        }
    } catch (error) {
        if (revision === sourceRevision) { showMessage(error.message, true); }
    } finally {
        if (path === databasePath.value.trim()) { openButton.disabled = false; }
    }
}
openButton.addEventListener("click", () => openDatabase());

async function loadColumns(savedSource = null) {
    const revision = ++sourceRevision;
    clearColumns();
    const table = tableName.value;
    if (!table || !inspectedPath) { return; }
    showMessage("Reading columns…");
    try {
        const result = await requestJson("/api/setup/columns", {db_path: inspectedPath, table_name: table});
        if (revision !== sourceRevision) { return; }
        columns = result;
        renderColumns(savedSource);
        showMessage(`Loaded ${columns.length} columns. Choose the time column, then configure signals.`);
    } catch (error) {
        if (revision === sourceRevision) { showMessage(error.message, true); }
    }
}
tableName.addEventListener("change", () => loadColumns());

async function browseFolder(path) {
    const revision = ++browseRevision;
    browserMessage.textContent = "Loading folder…";
    parentButton.disabled = true;
    document.querySelector("#browser-entries").replaceChildren();
    try {
        const query = path ? `?${new URLSearchParams({path})}` : "";
        const listing = await requestJson(`/api/setup/files${query}`);
        if (revision !== browseRevision) { return; }
        browsePath.value = listing.path;
        parentPath = listing.parent;
        parentButton.disabled = !parentPath;
        const entries = document.querySelector("#browser-entries");
        for (const entry of listing.entries) {
            const item = document.createElement("li");
            const button = document.createElement("button");
            button.type = "button";
            button.textContent = `${entry.is_directory ? "Folder" : "File"}: ${entry.name}`;
            button.addEventListener("click", () => {
                if (entry.is_directory) { browseFolder(entry.path); return; }
                databasePath.value = entry.path;
                resetDatabase();
                showMessage("File selected. Press Open database to read its tables.");
                browser.close();
            });
            item.append(button);
            entries.append(item);
        }
        browserMessage.textContent = `${listing.entries.length} entries. Select a folder to enter it, or a database file.`;
    } catch (error) {
        if (revision === browseRevision) { browserMessage.textContent = error.message; }
    }
}

document.querySelector("#browse-database").addEventListener("click", () => {
    browser.showModal();
    browseFolder(browsePath.value);
});
document.querySelector("#close-browser").addEventListener("click", () => browser.close());
document.querySelector("#browse-go").addEventListener("click", () => browseFolder(browsePath.value));
parentButton.addEventListener("click", () => browseFolder(parentPath));
browsePath.addEventListener("keydown", (event) => {
    if (event.key === "Enter") { event.preventDefault(); browseFolder(browsePath.value); }
});

function field(row, name) { return row.querySelector(`[data-field="${name}"]`); }

function showMessage(text, error = false) {
    message.textContent = text;
    message.dataset.error = String(error);
}

function invalidatePreview() {
    previewConfig = null;
    saveButton.disabled = true;
    preview.textContent = "Settings changed. Press Preview config to update.";
}

function updateSignalType(row) {
    const peak = field(row, "kpi-peak");
    peak.disabled = row.dataset.timeColumn === "true" || field(row, "signal-type").value !== "Instantaneous";
    if (peak.disabled) { peak.checked = false; }
}

function updateWidgetChoices() {
    const oldRanking = rankingLabel.value;
    const oldMain = mainTrend.value;
    rankingLabel.replaceChildren(new Option("No ranking", ""));
    mainTrend.replaceChildren(new Option("First selected trend", ""));
    const labels = new Set();
    for (const row of rows.rows) {
        if (!field(row, "use").checked) { continue; }
        const group = field(row, "group").value.trim();
        const label = field(row, "label").value.trim();
        if (!group || !label) { continue; }
        if (!labels.has(label)) { rankingLabel.append(new Option(label, label)); labels.add(label); }
        if (field(row, "trend").checked && ![...mainTrend.options].some(option => option.value === label)) {
            mainTrend.append(new Option(label, label));
        }
    }
    rankingLabel.value = labels.has(oldRanking) ? oldRanking : "";
    mainTrend.value = [...mainTrend.options].some((option) => option.value === oldMain) ? oldMain : "";
    renderMetricRules(labels);
}

function renderMetricRules(labels) {
    ruleRows.replaceChildren();
    for (const label of labels) {
        if (!metricRules.has(label)) {
            const dashboard = baseConfig.dashboard;
            metricRules.set(label, {
                aggregation: dashboard.aggregations?.[label] || "mean",
                global: dashboard.global_labels?.includes(label) || false,
                summaryTitle: dashboard.kpis.find(w => w.label === label && w.operation === "summary")?.title || "",
                peakTitle: dashboard.kpis.find(w => w.label === label && w.operation === "peak")?.title || "",
                trendTitle: dashboard.trends.find(w => w.label === label)?.title || "",
            });
        }
        const rule = metricRules.get(label);
        const row = document.createElement("tr");
        const name = document.createElement("th"); name.scope = "row"; name.textContent = label; row.append(name);
        const counter = [...rows.rows].some(r => field(r, "use").checked && field(r, "label").value.trim() === label && field(r, "signal-type").value === "Counter");
        for (const key of ["aggregation", "global", "summaryTitle", "peakTitle", "trendTitle"]) {
            const cell = document.createElement("td");
            const control = document.createElement(key === "aggregation" ? "select" : "input");
            const names = {aggregation: "Across groups", global: "Site-wide", summaryTitle: "Summary title", peakTitle: "Peak title", trendTitle: "Trend title"};
            control.setAttribute("aria-label", `${names[key]} for ${label}`);
            if (key === "aggregation") {
                control.append(new Option("Mean", "mean"), new Option("Sum", "sum"));
                if (counter) rule.aggregation = "sum";
                control.value = rule.aggregation; control.disabled = counter;
            } else if (key === "global") { control.type = "checkbox"; control.checked = rule.global; }
            else { control.value = rule[key]; control.placeholder = "Automatic title"; }
            control.addEventListener("input", () => { rule[key] = key === "global" ? control.checked : control.value; invalidatePreview(); });
            cell.append(control); row.append(cell);
        }
        ruleRows.append(row);
    }
}
document.querySelector("#signal-search").addEventListener("input", event => {
    const query = event.target.value.toLowerCase();
    for (const row of rows.rows) {
        row.hidden = ![row.dataset.column, field(row, "group").value, field(row, "label").value].join(" ").toLowerCase().includes(query);
    }
});

function updateTimeColumn() {
    for (const row of rows.rows) {
        const isTime = row.dataset.column === timeColumn.value;
        row.dataset.timeColumn = String(isTime);
        row.querySelectorAll("input, select").forEach((input) => { input.disabled = isTime; });
        if (isTime) { field(row, "use").checked = false; }
        updateSignalType(row);
    }
}

function renderColumns(savedSource = null) {
    timeColumn.append(new Option("Choose the time column", ""));
    for (const column of columns) {
        timeColumn.append(new Option(column.name, column.name));
        const row = document.querySelector("#signal-row").content.firstElementChild.cloneNode(true);
        row.dataset.column = column.name;
        field(row, "column").textContent = column.name;
        field(row, "declared-type").textContent = column.declare_type;
        for (const name of ["use", "group", "label", "unit", "signal-type", "kpi-summary", "kpi-peak", "trend"]) {
            const title = {use: "Use", group: "Group", label: "Label", unit: "Unit", "signal-type": "Type", "kpi-summary": "Summary KPI", "kpi-peak": "Peak KPI", trend: "Trend"}[name];
            field(row, name).setAttribute("aria-label", `${title} for ${column.name}`);
        }
        const signal = savedSource?.signals.find((signal) => signal.column === column.name);
        if (signal) {
            field(row, "use").checked = true;
            for (const name of ["group", "label", "unit"]) { field(row, name).value = signal[name] || ""; }
            field(row, "signal-type").value = signal.signal_type;
            const matches = (widget) => widget.label === signal.label;
            field(row, "kpi-summary").checked = baseConfig.dashboard.kpis.some((widget) => matches(widget) && widget.operation === "summary");
            field(row, "kpi-peak").checked = baseConfig.dashboard.kpis.some((widget) => matches(widget) && widget.operation === "peak");
            field(row, "trend").checked = baseConfig.dashboard.trends.some(matches);
        }
        rows.append(row);
    }
    if (savedSource && columns.some((column) => column.name === savedSource.time_column_name)) {
        timeColumn.value = savedSource.time_column_name;
    } else if (columns.some((column) => column.name === "Time")) { timeColumn.value = "Time"; }
    updateTimeColumn();
    timeColumn.disabled = false;
    autoButton.disabled = false;
    previewButton.disabled = false;
    updateWidgetChoices();
    if (savedSource) {
        rankingLabel.value = baseConfig.dashboard.group_summary_label || "";
        const first = baseConfig.dashboard.trends[0];
        if (first) { mainTrend.value = first.label; }
    }
}

autoButton.addEventListener("click", () => {
    let matched = 0;
    for (const row of rows.rows) {
        const suggestion = columns.find((column) => column.name === row.dataset.column)?.suggestion;
        if (!suggestion || row.dataset.timeColumn === "true") { continue; }
        for (const [name, value] of [["group", suggestion.group], ["label", suggestion.label], ["signal-type", suggestion.signal_type]]) {
            if (!field(row, name).value.trim()) { field(row, name).value = value; }
        }
        field(row, "use").checked = true;
        if (!field(row, "unit").value.trim()) {
            const similar = baseConfig.source?.signals.find(signal => signal.label === field(row, "label").value && signal.signal_type === field(row, "signal-type").value);
            field(row, "unit").value = similar?.unit || "";
        }
        updateSignalType(row);
        matched += 1;
    }
    updateWidgetChoices();
    invalidatePreview();
    showMessage(`Matched ${matched} columns. Review names, fill units and configure other columns manually.`);
});

form.addEventListener("input", (event) => {
    if (event.target.closest("#signal-rows tr")) { updateWidgetChoices(); }
    invalidatePreview();
});
form.addEventListener("change", (event) => {
    if (event.target === timeColumn) { updateTimeColumn(); }
    const row = event.target.closest("#signal-rows tr");
    if (row) {
        updateSignalType(row);
        const widgetField = event.target.dataset.field;
        if (["kpi-summary", "kpi-peak", "trend"].includes(widgetField)) {
            const label = field(row, "label").value.trim();
            for (const sibling of rows.rows) {
                if (field(sibling, "label").value.trim() !== label) continue;
                const control = field(sibling, widgetField);
                if (!control.disabled) control.checked = event.target.checked;
            }
        }
    }
    if (row || event.target === timeColumn) { updateWidgetChoices(); }
    invalidatePreview();
});

form.addEventListener("submit", (event) => {
    event.preventDefault();
    try {
        if (!baseConfig) { throw new Error("Configuration has not loaded. Reload this page."); }
        const dbPath = document.querySelector("#database-path").value.trim();
        if (!inspectedPath || inspectedPath !== dbPath || !tableName.value || !columns.length) {
            throw new Error("Open the database and choose a table first.");
        }
        if (!timeColumn.value) { throw new Error("Choose the time column."); }
        const signals = [];
        const kpis = [];
        const trends = [];
        const seen = new Set();
        for (const row of rows.rows) {
            if (!field(row, "use").checked) { continue; }
            const group = field(row, "group").value.trim();
            const label = field(row, "label").value.trim();
            const signalType = field(row, "signal-type").value;
            if (!group || !label || !signalType) {
                throw new Error(`${row.dataset.column}: fill group, label and type.`);
            }
            const key = JSON.stringify([group, label]);
            if (seen.has(key)) { throw new Error(`Duplicate group and label: ${group} / ${label}`); }
            seen.add(key);
            signals.push({column: row.dataset.column, group, label, unit: field(row, "unit").value.trim(), signal_type: signalType});
            if (field(row, "kpi-peak").checked && signalType !== "Instantaneous") {
                throw new Error(`${row.dataset.column}: Peak requires Instantaneous type.`);
            }
            if (field(row, "kpi-summary").checked) { kpis.push({group, label, operation: "summary"}); }
            if (field(row, "kpi-peak").checked) { kpis.push({group, label, operation: "peak"}); }
            if (field(row, "trend").checked) { trends.push({group, label}); }
        }
        if (!signals.length) { throw new Error("Select at least one signal column."); }
        // One widget per label and operation; its group is only the schema anchor.
        for (const widgets of [kpis, trends]) {
            const keys = new Set();
            for (let i = 0; i < widgets.length;) {
                const widget = widgets[i];
                const key = JSON.stringify([widget.label, widget.operation]);
                if (keys.has(key)) { widgets.splice(i, 1); continue; }
                keys.add(key);
                const rule = metricRules.get(widget.label);
                const title = rule?.[widget.operation === "peak" ? "peakTitle" : widget.operation === "summary" ? "summaryTitle" : "trendTitle"];
                if (title?.trim()) { widget.title = title.trim(); }
                i += 1;
            }
        }
        // Keep saved widget order when editing; newly selected widgets are appended.
        for (const [widgets, saved] of [[kpis, baseConfig.dashboard.kpis], [trends, baseConfig.dashboard.trends]]) {
            const position = (widget) => {
                const index = saved.findIndex((old) => old.label === widget.label && old.operation === widget.operation);
                return index < 0 ? saved.length : index;
            };
            widgets.sort((a, b) => position(a) - position(b));
        }
        const selectedMain = trends.findIndex((trend) => trend.label === mainTrend.value);
        if (selectedMain > 0) { trends.unshift(trends.splice(selectedMain, 1)[0]); }
        const config = {
            db_path: dbPath,
            listen_addr: baseConfig.listen_addr,
            dashboard_title: document.querySelector("#dashboard-title").value.trim(),
            dashboard_subtitle: document.querySelector("#dashboard-subtitle").value.trim(),
            source: {table_name: document.querySelector("#table-name").value, time_column_name: timeColumn.value, signals},
            dashboard: {
                kpis, trends, group_summary_label: rankingLabel.value || null,
                aggregations: Object.fromEntries([...new Set(signals.map(s => s.label))].map(label => [label, signals.find(s => s.label === label).signal_type === "Counter" ? "sum" : metricRules.get(label)?.aggregation || "mean"])),
                global_labels: [...new Set(signals.map(s => s.label))].filter(label => metricRules.get(label)?.global),
            },
        };
        previewConfig = config;
        saveButton.disabled = false;
        preview.textContent = JSON.stringify(config, null, 2);
        showMessage(`Preview generated for ${signals.length} signals. No configuration was saved.`);
    } catch (error) {
        previewConfig = null;
        saveButton.disabled = true;
        preview.textContent = "Fix the settings above, then preview again.";
        showMessage(error.message, true);
    }
});


saveButton.addEventListener("click", async () => {
    if (!previewConfig) { showMessage("Preview the current settings before saving.", true); return; }
    settings.disabled = true;
    showMessage("Validating and saving configuration…");
    try {
        const result = await requestJson("/api/setup/config", previewConfig);
        showMessage("Configuration saved. Opening dashboard…");
        window.location.assign(result.redirect);
    } catch (error) {
        showMessage(`Unable to save: ${error.message}`, true);
        settings.disabled = false;
    }
});

async function initialize() {
    try {
        baseConfig = await requestJson("/api/setup/config");
        baseConfig.dashboard ||= {};
        baseConfig.dashboard.kpis ||= [];
        baseConfig.dashboard.trends ||= [];
        document.querySelector("#dashboard-title").value = baseConfig.dashboard_title || "";
        document.querySelector("#dashboard-subtitle").value = baseConfig.dashboard_subtitle || "";
        databasePath.value = baseConfig.db_path;
        if (baseConfig.db_path) { await openDatabase(baseConfig.source); }
        else { showMessage("No configuration yet. Choose a database, open it, then select a table."); }
        settings.disabled = false;
    } catch (error) {
        showMessage(`Unable to load configuration: ${error.message}. Reload this page to retry.`, true);
    }
}
initialize();
