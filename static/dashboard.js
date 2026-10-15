function showLoadError(event) {
    const target = event.detail.elt;
    target.textContent = "Unable to load data. Refresh to retry.";
}

document.body.addEventListener("htmx:responseError", showLoadError);
document.body.addEventListener("htmx:sendError", showLoadError);