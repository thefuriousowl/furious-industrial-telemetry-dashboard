use std::sync::mpsc;

use tokio::sync::oneshot;

use crate::{
    config::{self, SourceConfig},
    request::DbRequest,
    schema::{ColumnMetaData, RuntimeSchema},
    telemetry::TelemetryRow,
};

pub(crate) struct DbClient {
    sender: mpsc::Sender<DbRequest>,
}
impl DbClient {
    pub(crate) fn new(sender: mpsc::Sender<DbRequest>) -> Self {
        Self { sender }
    }
    pub(crate) fn open_database(&self, path: &str) -> Result<(), String> {
        let (reply_sender, reply_receiver) = oneshot::channel::<Result<(), String>>();

        self.sender
            .send(DbRequest::OpenDatabase {
                path: path.to_string(),
                reply: reply_sender,
            })
            .map_err(|e| e.to_string())?;

        reply_receiver.blocking_recv().map_err(|e| e.to_string())?
    }

    pub(crate) fn list_table(&self) -> Result<Vec<String>, String> {
        let (reply_sender, reply_receiver) = oneshot::channel::<Result<Vec<String>, String>>();

        self.sender
            .send(DbRequest::ListTables {
                reply: reply_sender,
            })
            .map_err(|e| e.to_string())?;
        reply_receiver.blocking_recv().map_err(|e| e.to_string())?
    }

    pub(crate) fn get_available_columns(
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
        reply_receiver.blocking_recv().map_err(|e| e.to_string())?
    }

    pub(crate) fn get_schema(
        &self,
        config: SourceConfig,
        meter_overwrite_policy: bool,
    ) -> Result<RuntimeSchema, String> {
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.sender
            .send(DbRequest::GetRuntimeSchema {
                config,
                meter_overwrite_policy,
                reply: reply_sender,
            })
            .map_err(|e| e.to_string())?;
        reply_receiver.blocking_recv().map_err(|e| e.to_string())?
    }

    pub(crate) fn get_telemetry(
        &self,
        table_name: &str,
        selected_columns: Vec<String>,
        expected_time_column_name: &str,
        start_datetime: String,
        end_datetime: String,
    ) -> Result<Vec<TelemetryRow>, String> {
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.sender
            .send(DbRequest::ReadRange {
                table_name: table_name.to_string(),
                selected_columns,
                expected_time_column_name: expected_time_column_name.to_string(),
                start_datetime,
                end_datetime,
                reply: reply_sender,
            })
            .map_err(|e| e.to_string())?;
        reply_receiver.blocking_recv().map_err(|e| e.to_string())?
    }
}
