use std::collections::BTreeMap;

#[derive(Debug)]
pub(crate) struct TelemetryRow {
    time: String,
    values: BTreeMap<String, Option<f64>>,
}
impl TelemetryRow {
    pub(crate) fn from(time: String, values: BTreeMap<String, Option<f64>>) -> Self {
        Self { time, values }
    }
}
