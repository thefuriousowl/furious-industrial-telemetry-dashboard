use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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

#[derive(Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub(crate) enum TrendBucket {
    Hour,
    Day,
    Month,
}
