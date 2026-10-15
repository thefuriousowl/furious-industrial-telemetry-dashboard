use tokio::sync::oneshot;

use crate::{
    config::{AppConfig, SourceConfig},
    error::DbError,
    schema::{ColumnMetaData, RuntimeSchema},
    telemetry::{TelemetryRow, TrendBucket},
};
pub(crate) enum DbRequest {
    ReadDashboard {
        query: crate::dashboard_data::DashboardRead,
        reply: oneshot::Sender<Result<crate::dashboard_data::DashboardSnapshot, DbError>>,
    },
    GetLatestTime {
        reply: oneshot::Sender<Result<Option<String>, DbError>>,
    },
    ApplyConfig {
        config: AppConfig,
        persist_to: Option<String>,
        reply: oneshot::Sender<Result<RuntimeSchema, DbError>>,
    },
    GetAppConfig {
        reply: oneshot::Sender<Option<AppConfig>>,
    },
    OpenDatabase {
        path: String,
        reply: oneshot::Sender<Result<(), DbError>>,
    },
    InspectTables {
        path: String,
        reply: oneshot::Sender<Result<Vec<String>, DbError>>,
    },
    InspectColumns {
        path: String,
        table_name: String,
        reply: oneshot::Sender<Result<Vec<ColumnMetaData>, DbError>>,
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
    GetGroupSummary {
        label: String,
        start_datetime: String,
        end_datetime: String,
        reply: oneshot::Sender<Result<Vec<(String, Option<f64>)>, DbError>>,
    },
    GetSignalTrend {
        group: String,
        label: String,
        start_datetime: String,
        end_datetime: String,
        bucket: TrendBucket,
        reply: oneshot::Sender<Result<Vec<(String, Option<f64>)>, DbError>>,
    },
    GetSignalPeak {
        group: String,
        label: String,
        start_datetime: String,
        end_datetime: String,
        reply: oneshot::Sender<Result<Option<f64>, DbError>>,
    },
    Shutdown,
}
