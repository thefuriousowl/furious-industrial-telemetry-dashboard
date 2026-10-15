use std::sync::mpsc;

use tokio::sync::oneshot;

use crate::{
    config::{AppConfig, SourceConfig},
    error::DbError,
    request::DbRequest,
    schema::{ColumnMetaData, RuntimeSchema},
    telemetry::{TelemetryRow, TrendBucket},
};

#[derive(Debug, Clone)]
pub(crate) struct AppState {
    initial_config: AppConfig,
    config_path: String,
    db_cli: DbClient,
}
impl AppState {
    pub(crate) fn from(config: AppConfig, path: &str, db_cli: DbClient) -> Self {
        Self {
            initial_config: config,
            config_path: path.to_string(),
            db_cli,
        }
    }
    pub(crate) fn config_path(&self) -> &str {
        &self.config_path
    }
    pub(crate) fn db_cli(&self) -> &DbClient {
        &self.db_cli
    }
    pub(crate) fn listen_addr(&self) -> std::net::SocketAddr {
        self.initial_config.listen_addr()
    }
    pub(crate) async fn app_config(&self) -> Result<AppConfig, DbError> {
        Ok(self
            .db_cli
            .get_app_config()
            .await?
            .unwrap_or_else(|| self.initial_config.clone()))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DbClient {
    sender: mpsc::Sender<DbRequest>,
}
impl DbClient {
    pub(crate) async fn read_dashboard(
        &self,
        query: crate::dashboard_data::DashboardRead,
    ) -> Result<crate::dashboard_data::DashboardSnapshot, DbError> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(DbRequest::ReadDashboard { query, reply })
            .map_err(|e| DbError::Channel(e.to_string()))?;
        response
            .await
            .map_err(|e| DbError::Channel(e.to_string()))?
    }
    pub(crate) async fn get_latest_time(&self) -> Result<Option<String>, DbError> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(DbRequest::GetLatestTime { reply })
            .map_err(|e| DbError::Channel(e.to_string()))?;
        response
            .await
            .map_err(|e| DbError::Channel(e.to_string()))?
    }
    pub(crate) fn new(sender: mpsc::Sender<DbRequest>) -> Self {
        Self { sender }
    }
    pub(crate) async fn apply_config(
        &self,
        config: AppConfig,
        persist_to: Option<&str>,
    ) -> Result<RuntimeSchema, DbError> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(DbRequest::ApplyConfig {
                config,
                persist_to: persist_to.map(str::to_owned),
                reply,
            })
            .map_err(|e| DbError::Channel(e.to_string()))?;
        response
            .await
            .map_err(|e| DbError::Channel(e.to_string()))?
    }

    pub(crate) async fn get_app_config(&self) -> Result<Option<AppConfig>, DbError> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(DbRequest::GetAppConfig { reply })
            .map_err(|e| DbError::Channel(e.to_string()))?;
        response.await.map_err(|e| DbError::Channel(e.to_string()))
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

    pub(crate) async fn inspect_tables(&self, path: &str) -> Result<Vec<String>, DbError> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(DbRequest::InspectTables {
                path: path.to_string(),
                reply,
            })
            .map_err(|e| DbError::Channel(e.to_string()))?;
        response
            .await
            .map_err(|e| DbError::Channel(e.to_string()))?
    }

    pub(crate) async fn inspect_columns(
        &self,
        path: &str,
        table_name: &str,
    ) -> Result<Vec<ColumnMetaData>, DbError> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(DbRequest::InspectColumns {
                path: path.to_string(),
                table_name: table_name.to_string(),
                reply,
            })
            .map_err(|e| DbError::Channel(e.to_string()))?;
        response
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
        bucket: TrendBucket,
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
                bucket,
                reply: reply_sender,
            })
            .map_err(|e| DbError::Channel(e.to_string()))?;
        reply_receiver
            .await
            .map_err(|e| DbError::Channel(e.to_string()))?
    }

    pub(crate) async fn get_signal_peak(
        &self,
        group: String,
        label: String,
        start_datetime: String,
        end_datetime: String,
    ) -> Result<Option<f64>, DbError> {
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.sender
            .send(DbRequest::GetSignalPeak {
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
