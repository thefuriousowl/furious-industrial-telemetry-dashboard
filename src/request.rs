use tokio::sync::oneshot;

use crate::{
    config::SourceConfig,
    schema::{ColumnMetaData, RuntimeSchema},
    telemetry::TelemetryRow,
};
pub(crate) enum DbRequest {
    OpenDatabase {
        path: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
    ListTables {
        reply: oneshot::Sender<Result<Vec<String>, String>>,
    },
    GetColumns {
        table_name: String,
        reply: oneshot::Sender<Result<Vec<ColumnMetaData>, String>>,
    },
    GetRuntimeSchema {
        config: SourceConfig,
        meter_overwrite_policy: bool,
        reply: oneshot::Sender<Result<RuntimeSchema, String>>,
    },
    ReadRange {
        table_name: String,
        selected_columns: Vec<String>,
        expected_time_column_name: String,
        start_datetime: String,
        end_datetime: String,
        reply: oneshot::Sender<Result<Vec<TelemetryRow>, String>>,
    },
    Shutdown,
}
