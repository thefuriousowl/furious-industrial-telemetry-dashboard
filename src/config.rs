use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::error::DbError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
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
    pub(crate) fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TrendConfig {
    group: String,
    label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    title: Option<String>,
}
impl TrendConfig {
    pub(crate) fn group(&self) -> &str {
        &self.group
    }

    pub(crate) fn label(&self) -> &str {
        &self.label
    }
    pub(crate) fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Aggregation {
    Sum,
    #[default]
    Mean,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct DashboardConfig {
    #[serde(default)]
    kpis: Vec<KpiConfig>,
    #[serde(default)]
    trends: Vec<TrendConfig>,
    #[serde(default)]
    group_summary_label: Option<String>,
    #[serde(default)]
    aggregations: BTreeMap<String, Aggregation>,
    #[serde(default)]
    global_labels: Vec<String>,
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
    pub(crate) fn aggregation(&self, label: &str) -> Aggregation {
        self.aggregations.get(label).copied().unwrap_or_default()
    }
    pub(crate) fn is_global(&self, label: &str) -> bool {
        self.global_labels.iter().any(|item| item == label)
    }
}
fn default_listen_addr() -> std::net::SocketAddr {
    std::net::SocketAddr::from(([127, 0, 0, 1], 3000))
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct AppConfig {
    db_path: String,
    #[serde(default = "default_listen_addr")]
    listen_addr: std::net::SocketAddr,
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
            listen_addr: default_listen_addr(),
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
    pub(crate) fn listen_addr(&self) -> std::net::SocketAddr {
        self.listen_addr
    }

    pub(crate) fn validate_setup(&self) -> Result<(), String> {
        if self.db_path.trim().is_empty() {
            return Err("Choose a database".into());
        }
        let source = self.source.as_ref().ok_or("Configure a source")?;
        if source.table_name.trim().is_empty() || source.time_column_name.trim().is_empty() {
            return Err("Choose a table and time column".into());
        }
        for signal in &source.signals {
            if signal.column.trim().is_empty()
                || signal.group.trim().is_empty()
                || signal.label.trim().is_empty()
            {
                return Err("Each signal requires a column, group and label".into());
            }
            if signal.column == source.time_column_name {
                return Err("The time column cannot also be a signal".into());
            }
        }
        self.validate_dashboard()
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

        let labels = dashboard
            .kpis
            .iter()
            .map(|widget| widget.label.as_str())
            .chain(dashboard.trends.iter().map(|widget| widget.label.as_str()))
            .chain(dashboard.group_summary_label.iter().map(String::as_str))
            .chain(dashboard.aggregations.keys().map(String::as_str))
            .chain(dashboard.global_labels.iter().map(String::as_str));
        for label in labels {
            let signals = source
                .signals
                .iter()
                .filter(|signal| signal.label == label)
                .collect::<Vec<_>>();
            let first = signals
                .first()
                .ok_or_else(|| format!("Dashboard label not found: {label}"))?;
            if signals
                .iter()
                .any(|signal| signal.unit != first.unit || signal.signal_type != first.signal_type)
            {
                return Err(format!(
                    "Signals for '{label}' must use the same unit and signal type before combining groups"
                ));
            }
            if first.signal_type == SignalType::Counter
                && dashboard.aggregations.get(label) == Some(&Aggregation::Mean)
            {
                return Err(format!("Counter '{label}' must use sum aggregation"));
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

    // Called on the worker thread. Replace the config only after a complete write.
    pub(crate) fn save_to_file(&self, path: &str) -> Result<(), DbError> {
        use std::{
            fs::OpenOptions,
            io::Write,
            sync::atomic::{AtomicU64, Ordering},
        };
        static NEXT_FILE: AtomicU64 = AtomicU64::new(0);
        let json = serde_json::to_string_pretty(self).map_err(|e| DbError::Serde(e.to_string()))?;
        let target = std::path::Path::new(path);
        let name = target
            .file_name()
            .ok_or_else(|| DbError::Io("Invalid config path".into()))?;
        let (temporary, mut file) = loop {
            let mut temporary_name = name.to_os_string();
            temporary_name.push(format!(
                ".{}.{}.tmp",
                std::process::id(),
                NEXT_FILE.fetch_add(1, Ordering::Relaxed)
            ));
            let temporary = target.with_file_name(temporary_name);
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
            {
                Ok(file) => break (temporary, file),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(DbError::Io(e.to_string())),
            }
        };
        let result = (|| {
            file.write_all(json.as_bytes())?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&temporary, target)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result.map_err(|e| DbError::Io(e.to_string()))
    }

    pub(crate) async fn load_or_setup(path: &str) -> Result<Self, DbError> {
        let json = match tokio::fs::read_to_string(path).await {
            Ok(json) => json,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::new("", "Industrial telemetry", "Telemetry dashboard"));
            }
            Err(e) => return Err(DbError::Io(e.to_string())),
        };
        serde_json::from_str::<AppConfig>(&json).map_err(|e| DbError::Serde(e.to_string()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn example_config() -> AppConfig {
        serde_json::from_str(include_str!("../config.example.json"))
            .expect("Example config must parse")
    }

    #[test]
    fn accepts_example_dashboard() {
        assert!(example_config().validate_dashboard().is_ok());
    }

    #[test]
    fn accepts_unconfigured_dashboard() {
        let config = AppConfig::new("unused.db", "", "");
        assert!(config.validate_dashboard().is_ok());
    }

    #[test]
    fn rejects_dashboard_without_source() {
        let mut config = example_config();
        config.source = None;

        assert!(config.validate_dashboard().is_err());
    }

    #[test]
    fn rejects_unknown_kpi_signal() {
        let mut config = example_config();
        config.dashboard.kpis[0].label = "Missing".to_string();

        let error = config
            .validate_dashboard()
            .expect_err("Unknown KPI must be rejected");

        assert!(error.contains("KPI signal not found"));
    }

    #[test]
    fn rejects_unknown_trend_signal() {
        let mut config = example_config();
        config.dashboard.trends[0].group = "Missing".to_string();

        let error = config
            .validate_dashboard()
            .expect_err("Unknown trend must be rejected");

        assert!(error.contains("Trend signal not found"));
    }

    #[test]
    fn rejects_counter_peak() {
        let mut config = example_config();
        config.dashboard.kpis[0].label = "Total".to_string();
        config.dashboard.kpis[0].operation = KpiOperation::Peak;

        let error = config
            .validate_dashboard()
            .expect_err("Counter peak must be rejected");

        assert!(error.contains("Peak is unsupported for counter"));
    }

    #[test]
    fn rejects_unknown_group_summary_label() {
        let mut config = example_config();
        config.dashboard.group_summary_label = Some("Missing".to_string());

        let error = config
            .validate_dashboard()
            .expect_err("Unknown summary label must be rejected");

        assert!(error.contains("Group summary label not found"));
    }
}
#[cfg(test)]
mod listen_addr_tests {
    use super::*;

    fn example_json() -> serde_json::Value {
        serde_json::from_str(include_str!("../config.example.json")).unwrap()
    }

    #[test]
    fn missing_address_and_new_config_use_localhost() {
        let mut json = example_json();
        json.as_object_mut().unwrap().remove("listen_addr");
        let legacy: AppConfig = serde_json::from_value(json).unwrap();
        let expected: std::net::SocketAddr = "127.0.0.1:3000".parse().unwrap();
        assert_eq!(legacy.listen_addr(), expected);
        assert_eq!(AppConfig::new("unused.db", "", "").listen_addr(), expected);
    }

    #[test]
    fn custom_address_survives_serialization() {
        let mut json = example_json();
        json["listen_addr"] = serde_json::json!("127.0.0.1:3100");
        let config: AppConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.listen_addr(), "127.0.0.1:3100".parse().unwrap());
        assert_eq!(
            serde_json::to_value(config).unwrap()["listen_addr"],
            "127.0.0.1:3100"
        );
    }

    #[test]
    fn rejects_invalid_ip_or_port() {
        for address in ["invalid", "127.0.0.1:70000", "999.1.1.1:3000"] {
            let mut json = example_json();
            json["listen_addr"] = serde_json::json!(address);
            assert!(serde_json::from_value::<AppConfig>(json).is_err());
        }
    }
}

#[cfg(test)]
mod aggregation_validation_tests {
    use super::*;
    fn config() -> serde_json::Value {
        serde_json::json!({"db_path":"example.db","source":{"table_name":"readings","time_column_name":"Time","signals":[
            {"column":"a","group":"A","label":"Metric","unit":"u","signal_type":"Instantaneous"},
            {"column":"b","group":"B","label":"Metric","unit":"u","signal_type":"Instantaneous"}
        ]},"dashboard":{"kpis":[{"group":"A","label":"Metric","operation":"summary"}],"aggregations":{"Metric":"sum"}}})
    }
    #[test]
    fn rejects_incompatible_units_and_kinds_before_combining() {
        let mut value = config();
        value["source"]["signals"][1]["unit"] = "different".into();
        assert!(
            serde_json::from_value::<AppConfig>(value)
                .unwrap()
                .validate_setup()
                .is_err()
        );
        let mut value = config();
        value["source"]["signals"][1]["signal_type"] = "Counter".into();
        assert!(
            serde_json::from_value::<AppConfig>(value)
                .unwrap()
                .validate_setup()
                .is_err()
        );
    }
    #[test]
    fn rejects_unknown_global_label_and_mean_counter() {
        let mut value = config();
        value["dashboard"]["global_labels"] = serde_json::json!(["unknown"]);
        assert!(
            serde_json::from_value::<AppConfig>(value)
                .unwrap()
                .validate_setup()
                .is_err()
        );
        let mut value = config();
        for signal in value["source"]["signals"].as_array_mut().unwrap() {
            signal["signal_type"] = "Counter".into();
        }
        value["dashboard"]["aggregations"]["Metric"] = "mean".into();
        assert!(
            serde_json::from_value::<AppConfig>(value)
                .unwrap()
                .validate_setup()
                .is_err()
        );
    }
    #[test]
    fn aggregation_and_custom_titles_survive_save_serialization() {
        let mut value = config();
        value["dashboard"]["kpis"][0]["title"] = "Custom title".into();
        let config: AppConfig = serde_json::from_value(value).unwrap();
        config.validate_setup().unwrap();
        let roundtrip: AppConfig =
            serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(
            roundtrip.dashboard().aggregation("Metric"),
            Aggregation::Sum
        );
        assert_eq!(
            roundtrip.dashboard().kpis()[0].title(),
            Some("Custom title")
        );
    }
}
