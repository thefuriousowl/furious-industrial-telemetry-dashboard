use std::sync::mpsc;

use tokio::sync::oneshot;

use crate::{
    config::{AppConfig, SourceConfig},
    error::DbError,
    request::DbRequest,
    schema::{ColumnMetaData, RuntimeSchema},
    telemetry::TelemetryRow,
};

#[derive(Debug, Clone)]
pub(crate) struct AppState {
    dashboard_title: String,
    dashboard_subtitle: String,
    db_path: String,
    config_path: String,
    db_cli: DbClient,
}
impl AppState {
    pub(crate) fn from(config: AppConfig, path: &str, db_cli: DbClient) -> Self {
        Self {
            dashboard_title: config.dashboard_title().to_string(),
            dashboard_subtitle: config.dashboard_subtitle().to_string(),
            db_path: config.db_path().to_string(),
            config_path: path.to_string(),
            db_cli,
        }
    }
    pub(crate) fn db_path(&self) -> &str {
        &self.db_path
    }
    pub(crate) fn config_path(&self) -> &str {
        &self.config_path
    }
    pub(crate) fn db_cli(&self) -> &DbClient {
        &self.db_cli
    }
    pub(crate) fn dashboard_title(&self) -> &str {
        &self.dashboard_title
    }
    pub(crate) fn dashboard_subtitle(&self) -> &str {
        &self.dashboard_subtitle
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DbClient {
    sender: mpsc::Sender<DbRequest>,
}
impl DbClient {
    pub(crate) fn new(sender: mpsc::Sender<DbRequest>) -> Self {
        Self { sender }
    }
    pub(crate) async fn open_database(&self, path: &str) -> Result<(), DbError> {
        let (reply_sender, reply_receiver) = oneshot::channel::<Result<(), DbError>>();

        self.sender
            .send(DbRequest::OpenDatabase {
                path: path.to_string(),
                reply: reply_sender,
            })
            .map_err(|e| DbError::Channel(e.to_string()))?;

        reply_receiver
            .await
            .map_err(|e| DbError::Channel(e.to_string()))?
    }

    pub(crate) async fn list_table(&self) -> Result<Vec<String>, DbError> {
        let (reply_sender, reply_receiver) = oneshot::channel::<Result<Vec<String>, DbError>>();

        self.sender
            .send(DbRequest::ListTables {
                reply: reply_sender,
            })
            .map_err(|e| DbError::Channel(e.to_string()))?;
        reply_receiver
            .await
            .map_err(|e| DbError::Channel(e.to_string()))?
    }

    pub(crate) async fn get_available_columns(
        &self,
        table_name: &str,
    ) -> Result<Vec<ColumnMetaData>, DbError> {
        let (reply_sender, reply_receiver) =
            oneshot::channel::<Result<Vec<ColumnMetaData>, DbError>>();
        self.sender
            .send(DbRequest::GetColumns {
                table_name: table_name.to_string(),
                reply: reply_sender,
            })
            .map_err(|e| DbError::Channel(e.to_string()))?;
        reply_receiver
            .await
            .map_err(|e| DbError::Channel(e.to_string()))?
    }

    pub(crate) async fn configure_source(
        &self,
        config: SourceConfig,
    ) -> Result<RuntimeSchema, DbError> {
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.sender
            .send(DbRequest::ConfigureSource {
                config,
                reply: reply_sender,
            })
            .map_err(|e| DbError::Channel(e.to_string()))?;
        reply_receiver
            .await
            .map_err(|e| DbError::Channel(e.to_string()))?
    }

    pub(crate) async fn get_source(&self) -> Result<RuntimeSchema, DbError> {
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.sender
            .send(DbRequest::GetSource {
                reply: reply_sender,
            })
            .map_err(|e| DbError::Channel(e.to_string()))?;
        reply_receiver
            .await
            .map_err(|e| DbError::Channel(e.to_string()))?
    }

    pub(crate) async fn get_telemetry(
        &self,
        selected_columns: Vec<String>,
        start_datetime: String,
        end_datetime: String,
    ) -> Result<Vec<TelemetryRow>, DbError> {
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.sender
            .send(DbRequest::ReadRange {
                selected_columns,
                start_datetime,
                end_datetime,
                reply: reply_sender,
            })
            .map_err(|e| DbError::Channel(e.to_string()))?;
        reply_receiver
            .await
            .map_err(|e| DbError::Channel(e.to_string()))?
    }

    pub(crate) async fn get_signal_summary(
        &self,
        group: String,
        label: String,
        start_datetime: String,
        end_datetime: String,
    ) -> Result<Option<f64>, DbError> {
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.sender
            .send(DbRequest::GetSignalSummary {
                group,
                label,
                start_datetime,
                end_datetime,
                reply: reply_sender,
            })
            .map_err(|e| DbError::Channel(e.to_string()))?;
        reply_receiver
            .await
            .map_err(|e| DbError::Channel(e.to_string()))?
    }

    pub(crate) async fn get_group_summary(
        &self,
        label: String,
        start_datetime: String,
        end_datetime: String,
    ) -> Result<Vec<(String, Option<f64>)>, DbError> {
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.sender
            .send(DbRequest::GetGroupSummary {
                label,
                start_datetime,
                end_datetime,
                reply: reply_sender,
            })
            .map_err(|e| DbError::Channel(e.to_string()))?;
        reply_receiver
            .await
            .map_err(|e| DbError::Channel(e.to_string()))?
    }

    pub(crate) async fn get_signal_trend(
        &self,
        group: String,
        label: String,
        start_datetime: String,
        end_datetime: String,
    ) -> Result<Vec<(String, Option<f64>)>, DbError> {
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.sender
            .send(DbRequest::GetSignalTrend {
                group,
                label,
                start_datetime,
                end_datetime,
                reply: reply_sender,
            })
            .map_err(|e| DbError::Channel(e.to_string()))?;
        reply_receiver
            .await
            .map_err(|e| DbError::Channel(e.to_string()))?
    }
}
