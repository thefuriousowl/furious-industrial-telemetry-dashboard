use crate::{db::DbReader, request::DbRequest};
use std::sync::mpsc;

const DATABASE_NOT_IN_OPEN_STATE_ERROR: &str = "database is not open";
pub(crate) enum DbState {
    Closed,
    Open(DbReader),
}
pub(crate) struct DbWorker {
    state: DbState,
    receiver: mpsc::Receiver<DbRequest>,
}
impl DbWorker {
    pub(crate) fn new(receiver: mpsc::Receiver<DbRequest>) -> Self {
        Self {
            state: DbState::Closed,
            receiver,
        }
    }

    pub(crate) fn set_state(&mut self, state: DbState) {
        match state {
            DbState::Open(reader) => {
                self.state = DbState::Open(reader);
            }
            DbState::Closed => {
                self.state = DbState::Closed;
            }
        }
    }

    pub(crate) fn state(&self) -> &DbState {
        &self.state
    }

    pub(crate) fn run(mut self) {
        while let Ok(request) = self.receiver.recv() {
            match request {
                DbRequest::OpenDatabase { path, reply } => {
                    println!("received OpenDatabase");
                    let db_reader = match DbReader::open(&path) {
                        Ok(conn) => {
                            self.set_state(DbState::Open(conn));
                            Ok(())
                        }
                        Err(e) => Err(e),
                    };

                    let _ = reply.send(db_reader);
                }
                DbRequest::ListTables { reply } => {
                    println!("received ListTables");
                    let result = match self.state() {
                        DbState::Closed => Err(DATABASE_NOT_IN_OPEN_STATE_ERROR.to_string()),
                        DbState::Open(reader) => reader.list_table(),
                    };

                    let _ = reply.send(result);
                }
                DbRequest::GetColumns { table_name, reply } => {
                    println!("received GetColumns");
                    let result = match self.state() {
                        DbState::Closed => Err(DATABASE_NOT_IN_OPEN_STATE_ERROR.to_string()),
                        DbState::Open(reader) => reader.get_columns_metadata(&table_name),
                    };

                    let _ = reply.send(result);
                }
                DbRequest::GetRuntimeSchema {
                    config,
                    meter_overwrite_policy,
                    reply,
                } => {
                    println!("received GetRuntimeSchema");
                    let result = match self.state() {
                        DbState::Closed => Err(DATABASE_NOT_IN_OPEN_STATE_ERROR.to_string()),
                        DbState::Open(reader) => {
                            reader.discover_schema(config, meter_overwrite_policy)
                        }
                    };

                    let _ = reply.send(result);
                }
                DbRequest::ReadRange {
                    table_name,
                    selected_columns,
                    expected_time_column_name,
                    start_datetime,
                    end_datetime,
                    reply,
                } => {
                    println!("received ReadRange");
                    let result = match self.state() {
                        DbState::Closed => Err(DATABASE_NOT_IN_OPEN_STATE_ERROR.to_string()),
                        DbState::Open(reader) => reader.get_telemetry(
                            &table_name,
                            &expected_time_column_name,
                            &selected_columns,
                            start_datetime,
                            end_datetime,
                        ),
                    };

                    let _ = reply.send(result);
                }
                DbRequest::Shutdown => {
                    println!("received Shutdown");
                    break;
                }
            }
        }
    }
}
