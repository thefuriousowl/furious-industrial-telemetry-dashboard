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
    signal_type: SignalType,
}
impl SignalConfig {
    pub(crate) fn new(column: &str, group: &str, label: &str, signal_type: SignalType) -> Self {
        Self {
            column: column.to_string(),
            group: group.to_string(),
            label: label.to_string(),
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct AppConfig {
    db_path: String,
    #[serde(default)]
    dashboard_title: String,
    #[serde(default)]
    dashboard_subtitle: String,
    source: Option<SourceConfig>,
}
impl AppConfig {
    pub(crate) fn new(db_path: &str, dashboard_title: &str, dashboard_subtitle: &str) -> Self {
        Self {
            dashboard_title: dashboard_title.to_string(),
            dashboard_subtitle: dashboard_subtitle.to_string(),
            db_path: db_path.to_string(),
            source: None,
        }
    }

    pub(crate) fn dashboard_title(&self) -> &str {
        &self.dashboard_title
    }

    pub(crate) fn dashboard_subtitle(&self) -> &str {
        &self.dashboard_subtitle
    }

    pub(crate) fn db_path(&self) -> &str {
        &self.db_path
    }

    pub(crate) fn source(&self) -> &Option<SourceConfig> {
        &self.source
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
