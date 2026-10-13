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
    ConfigureSource {
        config: SourceConfig,
        reply: oneshot::Sender<Result<RuntimeSchema, String>>,
    },
    ReadRange {
        selected_columns: Vec<String>,
        start_datetime: String,
        end_datetime: String,
        reply: oneshot::Sender<Result<Vec<TelemetryRow>, String>>,
    },
    Shutdown,
}
