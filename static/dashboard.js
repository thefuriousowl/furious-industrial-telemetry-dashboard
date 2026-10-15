function showLoadError(event) {
    const target = event.detail.elt;
    target.textContent = "Unable to load data. Refresh to retry.";
}

document.body.addEventListener("htmx:responseError", showLoadError);
document.body.addEventListener("htmx:sendError", showLoadError);

const dashboardNumberFormatter = new Intl.NumberFormat("en-US", {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
});

function formatDashboardNumbers() {
    document.querySelectorAll("[data-number]").forEach((element) => {
        const value = Number(element.dataset.number);
        if (Number.isFinite(value)) {
            element.textContent = dashboardNumberFormatter.format(value);
        }
    });
}

document.body.addEventListener("htmx:afterSwap", formatDashboardNumbers);
formatDashboardNumbers();
