use serde::Serialize;

use crate::{config::SignalConfig, error::DbError};

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
    ) -> Result<Self, DbError> {
        let time_column_exists = columns_metadata
            .iter()
            .any(|col| col.name() == expected_time_column_name);

        if !time_column_exists {
            return Err(DbError::ColumnNotFound(vec![expected_time_column_name]));
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
            return Err(DbError::DuplicateColumn(
                duplicated_columns
                    .iter()
                    .map(|col| col.to_string())
                    .collect(),
            ));
        }
        if !duplicated_group_label.is_empty() {
            return Err(DbError::DuplicateGroupLabel(
                duplicated_group_label
                    .iter()
                    .map(|(grp, label)| (grp.to_string(), label.to_string()))
                    .collect(),
            ));
        }

        if signals.is_empty() {
            return Err(DbError::EmptySignals);
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
            return Err(DbError::ColumnNotFound(
                mismatch_expected_columns
                    .iter()
                    .map(|col| col.to_string())
                    .collect(),
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

#[cfg(test)]
mod tests {
    use crate::{
        config::{SignalConfig, SignalType},
        error::DbError,
        schema::{ColumnMetaData, RuntimeSchema},
    };
    fn sample_metadata() -> Vec<ColumnMetaData> {
        vec![
            ColumnMetaData::new("Time".to_string(), "datetime".to_string()),
            ColumnMetaData::new("Furnace_Flow".to_string(), "REAL".to_string()),
            ColumnMetaData::new("Furnace_Pressure".to_string(), "REAL".to_string()),
        ]
    }

    fn sample_signals() -> Vec<SignalConfig> {
        vec![
            SignalConfig::new("Furnace_Flow", "Furnace", "Flow", SignalType::Instantaneous),
            SignalConfig::new(
                "Furnace_Pressure",
                "Furnace",
                "Pressure",
                SignalType::Instantaneous,
            ),
        ]
    }

    #[test]
    fn rejects_expected_time_column_not_found() {
        let expected_time_column_name = String::from("datetime");
        let result = RuntimeSchema::generate_from(
            "DataLogger1".to_string(),
            sample_metadata(),
            expected_time_column_name,
            sample_signals(),
        );
        assert!(matches!(result, Err(DbError::ColumnNotFound(..))))
    }

    #[test]
    fn rejects_duplicate_signal_column_name() {
        let expected_time_column_name = String::from("Time");
        let mut duplicated_signals = sample_signals();
        duplicated_signals.push(SignalConfig::new(
            "Furnace_Flow",
            "Furnace",
            "Someshit",
            SignalType::Instantaneous,
        ));
        let result = RuntimeSchema::generate_from(
            "DataLogger1".to_string(),
            sample_metadata(),
            expected_time_column_name,
            duplicated_signals,
        );
        assert!(matches!(result, Err(DbError::DuplicateColumn(..))))
    }

    #[test]
    fn rejects_duplicate_group_label_pair() {
        let expected_time_column_name = String::from("Time");
        let mut duplicated_group_label_pair_signals = sample_signals();
        duplicated_group_label_pair_signals[1] = SignalConfig::new(
            "Furnace_Pressure",
            "Furnace",
            "Flow",
            SignalType::Instantaneous,
        );
        let result = RuntimeSchema::generate_from(
            "DataLogger1".to_string(),
            sample_metadata(),
            expected_time_column_name,
            duplicated_group_label_pair_signals,
        );
        assert!(matches!(result, Err(DbError::DuplicateGroupLabel(..))))
    }

    #[test]
    fn rejects_empty_signal() {
        let expected_time_column_name = String::from("Time");
        let empty_signals = vec![];
        let result = RuntimeSchema::generate_from(
            "DataLogger1".to_string(),
            sample_metadata(),
            expected_time_column_name,
            empty_signals,
        );

        assert!(matches!(result, Err(DbError::EmptySignals)))
    }

    #[test]
    fn rejects_signal_mismatch_from_metadata() {
        let expected_time_column_name = String::from("Time");
        let mut mismatch_signals = sample_signals();
        mismatch_signals.push(SignalConfig::new(
            "Mismatch_Signal",
            "Furnace",
            "Totalizer",
            SignalType::Instantaneous,
        ));
        let result = RuntimeSchema::generate_from(
            "DataLogger1".to_string(),
            sample_metadata(),
            expected_time_column_name,
            mismatch_signals,
        );

        assert!(matches!(result, Err(DbError::ColumnNotFound(..))))
    }

    #[test]
    fn accepts_valid() {
        let expected_time_column_name = String::from("Time");
        let result = RuntimeSchema::generate_from(
            "DataLogger1".to_string(),
            sample_metadata(),
            expected_time_column_name,
            sample_signals(),
        );
        assert!(matches!(result, Ok(..)))
    }
}
