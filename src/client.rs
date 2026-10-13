use std::sync::mpsc;

use tokio::sync::oneshot;

use crate::{
    config::SourceConfig,
    request::DbRequest,
    schema::{ColumnMetaData, RuntimeSchema},
    telemetry::TelemetryRow,
};

#[derive(Debug, Clone)]
pub(crate) struct DbClient {
    sender: mpsc::Sender<DbRequest>,
}
impl DbClient {
    pub(crate) fn new(sender: mpsc::Sender<DbRequest>) -> Self {
        Self { sender }
    }
    pub(crate) async fn open_database(&self, path: &str) -> Result<(), String> {
        let (reply_sender, reply_receiver) = oneshot::channel::<Result<(), String>>();

        self.sender
            .send(DbRequest::OpenDatabase {
                path: path.to_string(),
                reply: reply_sender,
            })
            .map_err(|e| e.to_string())?;

        reply_receiver.await.map_err(|e| e.to_string())?
    }

    pub(crate) async fn list_table(&self) -> Result<Vec<String>, String> {
        let (reply_sender, reply_receiver) = oneshot::channel::<Result<Vec<String>, String>>();

        self.sender
            .send(DbRequest::ListTables {
                reply: reply_sender,
            })
            .map_err(|e| e.to_string())?;
        reply_receiver.await.map_err(|e| e.to_string())?
    }

    pub(crate) async fn get_available_columns(
        &self,
        table_name: &str,
    ) -> Result<Vec<ColumnMetaData>, String> {
        let (reply_sender, reply_receiver) =
            oneshot::channel::<Result<Vec<ColumnMetaData>, String>>();
        self.sender
            .send(DbRequest::GetColumns {
                table_name: table_name.to_string(),
                reply: reply_sender,
            })
            .map_err(|e| e.to_string())?;
        reply_receiver.await.map_err(|e| e.to_string())?
    }

    pub(crate) async fn configure_source(
        &self,
        config: SourceConfig,
    ) -> Result<RuntimeSchema, String> {
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.sender
            .send(DbRequest::ConfigureSource {
                config,
                reply: reply_sender,
            })
            .map_err(|e| e.to_string())?;
        reply_receiver.await.map_err(|e| e.to_string())?
    }

    pub(crate) async fn get_source(
        &self
    ) -> Result<RuntimeSchema, String> {
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.sender.send(DbRequest::GetSource { reply: reply_sender }).map_err(|e| e.to_string())?;
        reply_receiver.await.map_err(|e| e.to_string())?
    }

    pub(crate) async fn get_telemetry(
        &self,
        selected_columns: Vec<String>,
        start_datetime: String,
        end_datetime: String,
    ) -> Result<Vec<TelemetryRow>, String> {
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.sender
            .send(DbRequest::ReadRange {
                selected_columns,
                start_datetime,
                end_datetime,
                reply: reply_sender,
            })
            .map_err(|e| e.to_string())?;
        reply_receiver.await.map_err(|e| e.to_string())?
    }
}
