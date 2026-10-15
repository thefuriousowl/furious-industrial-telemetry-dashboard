use serde::{Deserialize, Serialize};

use crate::error::DbError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum SignalType {
    Instantaneous,
    Counter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SignalConfig {
    column: String,
    group: String,
    label: String,
    #[serde(default)]
    unit: String,
    signal_type: SignalType,
}
impl SignalConfig {
    pub(crate) fn new(
        column: &str,
        group: &str,
        label: &str,
        unit: &str,
        signal_type: SignalType,
    ) -> Self {
        Self {
            column: column.to_string(),
            group: group.to_string(),
            label: label.to_string(),
            unit: unit.to_string(),
            signal_type,
        }
    }

    pub(crate) fn column(&self) -> &str {
        &self.column
    }

    pub(crate) fn group(&self) -> &str {
        &self.group
    }

    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    pub(crate) fn unit(&self) -> &str {
        &self.unit
    }

    pub(crate) fn signal_type(&self) -> &SignalType {
        &self.signal_type
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct SourceConfig {
    table_name: String,
    time_column_name: String,
    signals: Vec<SignalConfig>,
}
impl SourceConfig {
    pub(crate) fn new(
        table_name: &str,
        time_column_name: &str,
        signals: Vec<SignalConfig>,
    ) -> Self {
        Self {
            table_name: table_name.to_string(),
            time_column_name: time_column_name.to_string(),
            signals,
        }
    }
    pub(crate) fn table_name(&self) -> &str {
        &self.table_name
    }
    pub(crate) fn time_column_name(&self) -> &str {
        &self.time_column_name
    }

    pub(crate) fn signals(&self) -> &[SignalConfig] {
        &self.signals
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum KpiOperation {
    Summary,
    Peak,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct KpiConfig {
    group: String,
    label: String,
    operation: KpiOperation,
}
impl KpiConfig {
    pub(crate) fn group(&self) -> &str {
        &self.group
    }

    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    pub(crate) fn operation(&self) -> &KpiOperation {
        &self.operation
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TrendConfig {
    group: String,
    label: String,
}
impl TrendConfig {
    pub(crate) fn group(&self) -> &str {
        &self.group
    }

    pub(crate) fn label(&self) -> &str {
        &self.label
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct DashboardConfig {
    #[serde(default)]
    kpis: Vec<KpiConfig>,
    #[serde(default)]
    trends: Vec<TrendConfig>,
    #[serde(default)]
    group_summary_label: Option<String>,
}
impl DashboardConfig {
    pub(crate) fn trends(&self) -> &[TrendConfig] {
        &self.trends
    }

    pub(crate) fn kpis(&self) -> &[KpiConfig] {
        &self.kpis
    }

    pub(crate) fn group_summary_label(&self) -> &Option<String> {
        &self.group_summary_label
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct AppConfig {
    db_path: String,
    #[serde(default)]
    dashboard_title: String,
    #[serde(default)]
    dashboard_subtitle: String,
    #[serde(default)]
    dashboard: DashboardConfig,
    source: Option<SourceConfig>,
}
impl AppConfig {
    pub(crate) fn new(db_path: &str, dashboard_title: &str, dashboard_subtitle: &str) -> Self {
        Self {
            dashboard_title: dashboard_title.to_string(),
            dashboard_subtitle: dashboard_subtitle.to_string(),
            db_path: db_path.to_string(),
            dashboard: DashboardConfig::default(),
            source: None,
        }
    }

    pub(crate) fn dashboard_title(&self) -> &str {
        &self.dashboard_title
    }

    pub(crate) fn dashboard_subtitle(&self) -> &str {
        &self.dashboard_subtitle
    }

    pub(crate) fn validate_dashboard(&self) -> Result<(), String> {
        let dashboard = &self.dashboard;
        if dashboard.kpis.is_empty()
            && dashboard.trends.is_empty()
            && dashboard.group_summary_label.is_none()
        {
            return Ok(());
        }

        let source = match &self.source {
            Some(src) => src,
            None => {
                return Err("Dashboard requires a configured source".to_string());
            }
        };

        for kpi in &dashboard.kpis {
            let signal = match source
                .signals
                .iter()
                .find(|signal| signal.group == kpi.group && signal.label == kpi.label)
            {
                Some(signal) => signal,
                None => {
                    return Err(format!(
                        "KPI signal not found: group={}, label={}",
                        kpi.group, kpi.label
                    ));
                }
            };

            if matches!(kpi.operation, KpiOperation::Peak)
                && matches!(signal.signal_type, SignalType::Counter)
            {
                return Err(format!(
                    "Peak is unsupported for counter: group={}, label={}",
                    kpi.group, kpi.label
                ));
            }
        }

        for trend in &dashboard.trends {
            let exists = source
                .signals
                .iter()
                .any(|signal| signal.group == trend.group && signal.label == trend.label);

            if !exists {
                return Err(format!(
                    "Trend signal not found: group={}, label={}",
                    trend.group, trend.label
                ));
            }
        }

        if let Some(label) = &dashboard.group_summary_label {
            let exists = source.signals.iter().any(|signal| signal.label == *label);

            if !exists {
                return Err(format!("Group summary label not found: {label}"));
            }
        }

        Ok(())
    }

    pub(crate) fn db_path(&self) -> &str {
        &self.db_path
    }

    pub(crate) fn source(&self) -> &Option<SourceConfig> {
        &self.source
    }

    pub(crate) fn dashboard(&self) -> &DashboardConfig {
        &self.dashboard
    }

    pub(crate) fn configure_source(&mut self, new_source: SourceConfig) {
        self.source = Some(new_source);
    }

    pub(crate) async fn save_to_file(&self, path: &str) -> Result<(), DbError> {
        let json = serde_json::to_string_pretty(self).map_err(|e| DbError::Serde(e.to_string()))?;
        tokio::fs::write(path, json)
            .await
            .map_err(|e| DbError::Io(e.to_string()))
    }

    pub(crate) async fn load_from_file(path: &str) -> Result<Self, DbError> {
        let json = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| DbError::Io(e.to_string()))?;
        serde_json::from_str::<AppConfig>(&json).map_err(|e| DbError::Serde(e.to_string()))
    }
}
