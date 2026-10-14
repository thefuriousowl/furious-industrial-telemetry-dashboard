use tokio::sync::oneshot;

use crate::{
    config::SourceConfig,
    error::DbError,
    schema::{ColumnMetaData, RuntimeSchema},
    telemetry::TelemetryRow,
};
pub(crate) enum DbRequest {
    OpenDatabase {
        path: String,
        reply: oneshot::Sender<Result<(), DbError>>,
    },
    ListTables {
        reply: oneshot::Sender<Result<Vec<String>, DbError>>,
    },
    GetColumns {
        table_name: String,
        reply: oneshot::Sender<Result<Vec<ColumnMetaData>, DbError>>,
    },
    ConfigureSource {
        config: SourceConfig,
        reply: oneshot::Sender<Result<RuntimeSchema, DbError>>,
    },
    GetSource {
        reply: oneshot::Sender<Result<RuntimeSchema, DbError>>,
    },
    ReadRange {
        selected_columns: Vec<String>,
        start_datetime: String,
        end_datetime: String,
        reply: oneshot::Sender<Result<Vec<TelemetryRow>, DbError>>,
    },
    GetSignalSummary {
        group: String,
        label: String,
        start_datetime: String,
        end_datetime: String,
        reply: oneshot::Sender<Result<Option<f64>, DbError>>,
    },
    Shutdown,
}
