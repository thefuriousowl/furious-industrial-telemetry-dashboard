use serde::{Deserialize, Serialize};

use crate::telemetry::TrendBucket;

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

#[derive(Deserialize)]
pub(crate) struct SingleSignalQuery {
    group: String,
    label: String,
    start: String,
    end: String,
}
impl SingleSignalQuery {
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

#[derive(Deserialize)]
pub(crate) struct GroupSummaryQuery {
    label: String,
    start: String,
    end: String,
}
impl GroupSummaryQuery {
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
#[derive(Serialize)]
pub(crate) struct GroupSummary {
    group: String,
    value: Option<f64>,
}
impl GroupSummary {
    pub(crate) fn from(pair: (String, Option<f64>)) -> Self {
        Self {
            group: pair.0,
            value: pair.1,
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct SignalTrendQuery {
    group: String,
    label: String,
    bucket: TrendBucket,
    start: String,
    end: String,
}
impl SignalTrendQuery {
    pub(crate) fn group(&self) -> &String {
        &self.group
    }
    pub(crate) fn label(&self) -> &String {
        &self.label
    }
    pub(crate) fn bucket(&self) -> &TrendBucket {
        &self.bucket
    }
    pub(crate) fn start(&self) -> &String {
        &self.start
    }
    pub(crate) fn end(&self) -> &String {
        &self.end
    }
}
#[derive(Serialize)]
pub(crate) struct SignalTrend {
    timestamp: String,
    value: Option<f64>,
}
impl SignalTrend {
    pub(crate) fn from(pair: (String, Option<f64>)) -> Self {
        Self {
            timestamp: pair.0,
            value: pair.1,
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct SignalPeakQuery {
    group: String,
    label: String,
    start: String,
    end: String,
}
impl SignalPeakQuery {
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

#[derive(Deserialize)]
pub(crate) struct DashboardQuery {
    period: Option<Period>,
    base_date: Option<String>,
    group: Option<String>,
}
impl DashboardQuery {
    pub(crate) fn group(&self) -> Option<&str> {
        self.group.as_deref().filter(|s| !s.is_empty())
    }
    pub(crate) fn period(&self) -> &Option<Period> {
        &self.period
    }

    pub(crate) fn base_date(&self) -> &Option<String> {
        &self.base_date
    }
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Period {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}
