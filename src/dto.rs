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