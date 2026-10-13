use serde::{Deserialize, Serialize};

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

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SourceConfig {
    pub(crate) table_name: String,
    pub(crate) time_column_name: String,
    pub(crate) signals: Vec<SignalConfig>,
}
