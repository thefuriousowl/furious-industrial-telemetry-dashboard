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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "lowercase")]
pub(crate) enum TrendBucket {
    Hour,
    Day,
    Month,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct MetricPoint {
    pub time: String,
    pub value: Option<f64>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub(crate) struct InstantaneousData {
    pub average: Option<f64>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub peak: Option<(String, f64)>,
    pub partial: bool,
    pub points: Vec<MetricPoint>,
}
