use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub(crate) struct TelemetryRow {
    time: String,
    values: BTreeMap<String, Option<f64>>,
}
impl TelemetryRow {
    pub(crate) fn from(time: String, values: BTreeMap<String, Option<f64>>) -> Self {
        Self { time, values }
    }
}
