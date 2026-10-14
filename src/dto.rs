use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct TelemetryQuery {
    columns: String,
    start: String,
    end: String,
}

impl TelemetryQuery {
    pub(crate) fn columns(&self) -> &String {
        &self.columns
    }
    pub(crate) fn start(&self) -> &String {
        &self.start
    }
    pub(crate) fn end(&self) -> &String {
        &self.end
    }
}

#[derive(Deserialize)]
pub(crate) struct SignalSummaryQuery {
    group: String,
    label: String,
    start: String,
    end: String,
}
impl SignalSummaryQuery {
    pub(crate) fn group(&self) -> &String {
        &self.group
    }
    pub(crate) fn label(&self) -> &String {
        &self.label
    }
    pub(crate) fn start(&self) -> &String {
        &self.start
    }
    pub(crate) fn end(&self) -> &String {
        &self.end
    }
}
