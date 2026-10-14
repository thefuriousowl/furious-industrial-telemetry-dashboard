use serde::Serialize;

use crate::config::SignalConfig;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ColumnMetaData {
    name: String,
    declare_type: String,
}
impl ColumnMetaData {
    pub(crate) fn new(name: String, declare_type: String) -> Self {
        Self { name, declare_type }
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn declare_type(&self) -> &str {
        &self.declare_type
    }
}

// this Clone derive as worker keeps one copy as the active source in DbState,
//and sends another copy back to the caller through the reply channel.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct RuntimeSchema {
    table_name: String,
    time_column_name: String,
    signals: Vec<SignalConfig>,
}
impl RuntimeSchema {
    pub(crate) fn generate_from(
        table_name: String,
        columns_metadata: Vec<ColumnMetaData>,
        expected_time_column_name: String,
        signals: Vec<SignalConfig>,
    ) -> Result<Self, String> {
        let time_column_exists = columns_metadata
            .iter()
            .any(|col| col.name() == expected_time_column_name);

        if !time_column_exists {
            return Err(format!(
                "expected time column name {expected_time_column_name} does not exists"
            ));
        }

        let mut seen_columns = std::collections::HashSet::new();
        let mut seen_group_label = std::collections::HashSet::new();

        let mut duplicated_columns = vec![];
        let mut duplicated_group_label = vec![];

        for signal in &signals {
            if !seen_columns.insert(signal.column()) {
                duplicated_columns.push(signal.column());
            }
            if !seen_group_label.insert((signal.group(), signal.label())) {
                duplicated_group_label.push((signal.group(), signal.label()));
            }
        }

        if !duplicated_columns.is_empty() {
            return Err(format!("duplicated columns found: {duplicated_columns:?}"));
        }
        if !duplicated_group_label.is_empty() {
            return Err(format!(
                "duplicated group-label pair found: {duplicated_group_label:?}"
            ));
        }

        if signals.is_empty() {
            return Err(format!("signals must not be empty"));
        }
        let mismatch_expected_columns = signals
            .iter()
            .filter_map(|signal| {
                let is_exists = columns_metadata
                    .iter()
                    .any(|metadata| metadata.name() == signal.column());

                if !is_exists {
                    Some(signal.column())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        if !mismatch_expected_columns.is_empty() {
            return Err(format!(
                "expected columns {mismatch_expected_columns:?} does not exists"
            ));
        }
        Ok(Self {
            table_name,
            time_column_name: expected_time_column_name,
            signals,
        })
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
