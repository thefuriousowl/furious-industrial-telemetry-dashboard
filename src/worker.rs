use crate::{db::DbReader, error::DbError, request::DbRequest, schema::RuntimeSchema};
use std::sync::mpsc;

pub(crate) enum DbState {
    Closed,
    Open {
        reader: DbReader,
        source: Option<RuntimeSchema>,
    },
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
        self.state = state;
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
                        Ok(reader) => {
                            self.set_state(DbState::Open {
                                reader,
                                source: None,
                            });
                            Ok(())
                        }
                        Err(e) => Err(e),
                    };

                    let _ = reply.send(db_reader);
                }
                DbRequest::ListTables { reply } => {
                    println!("received ListTables");
                    let result = match self.state() {
                        DbState::Closed => Err(DbError::DatabaseNotOpen),
                        DbState::Open { reader, .. } => reader.list_table(),
                    };

                    let _ = reply.send(result);
                }
                DbRequest::GetColumns { table_name, reply } => {
                    println!("received GetColumns");
                    let result = match self.state() {
                        DbState::Closed => Err(DbError::DatabaseNotOpen),
                        DbState::Open { reader, .. } => reader.get_columns_metadata(&table_name),
                    };

                    let _ = reply.send(result);
                }
                DbRequest::ConfigureSource { config, reply } => {
                    println!("received ConfigureSource");
                    let result = match &mut self.state {
                        DbState::Closed => Err(DbError::DatabaseNotOpen),
                        DbState::Open { reader, source } => {
                            match reader.discover_schema(config) {
                                Ok(schema) => {
                                    *source = Some(schema.clone()); // one copy stays in state, original goes to reply
                                    Ok(schema)
                                }
                                Err(e) => Err(e),
                            }
                        }
                    };

                    let _ = reply.send(result);
                }
                DbRequest::GetSource { reply } => {
                    println!("received GetSource");
                    let result = match self.state() {
                        DbState::Closed => Err(DbError::DatabaseNotOpen),
                        DbState::Open { source: None, .. } => Err(DbError::SourceNotConfig),
                        DbState::Open {
                            source: Some(src), ..
                        } => Ok(src.clone()),
                    };
                    let _ = reply.send(result);
                }
                DbRequest::ReadRange {
                    selected_columns,
                    start_datetime,
                    end_datetime,
                    reply,
                } => {
                    println!("received ReadRange");
                    let result = match self.state() {
                        DbState::Closed => Err(DbError::DatabaseNotOpen),
                        DbState::Open { source: None, .. } => Err(DbError::SourceNotConfig),
                        DbState::Open {
                            reader,
                            source: Some(src),
                        } => {
                            let mismatch_source_selected_columns = selected_columns
                                .iter()
                                .filter_map(|selected_column| {
                                    match src
                                        .signals()
                                        .iter()
                                        .any(|signal| signal.column() == selected_column)
                                    {
                                        true => None,
                                        false => Some(selected_column),
                                    }
                                })
                                .collect::<Vec<_>>();

                            if !mismatch_source_selected_columns.is_empty() {
                                Err(DbError::SelectedColumnNotInSource(
                                    mismatch_source_selected_columns
                                        .iter()
                                        .map(|col| col.to_string())
                                        .collect(),
                                ))
                            } else {
                                reader.get_telemetry(
                                    src.table_name(),
                                    src.time_column_name(),
                                    &selected_columns,
                                    start_datetime,
                                    end_datetime,
                                )
                            }
                        }
                    };
                    let _ = reply.send(result);
                }
                DbRequest::GetSignalSummary {
                    group,
                    label,
                    start_datetime,
                    end_datetime,
                    reply,
                } => {
                    println!("received GetSignalSummary");
                    let result = match self.state() {
                        DbState::Closed => Err(DbError::DatabaseNotOpen),
                        DbState::Open { source: None, .. } => Err(DbError::SourceNotConfig),
                        DbState::Open {
                            reader,
                            source: Some(src),
                        } => {
                            match src
                                .signals()
                                .iter()
                                .find(|s| s.group() == group && s.label() == label)
                            {
                                Some(signal) => reader.get_signal_summary(
                                    src.table_name(),
                                    src.time_column_name(),
                                    signal.column(),
                                    signal.signal_type(),
                                    &start_datetime,
                                    &end_datetime,
                                ),
                                None => Err(DbError::SelectedColumnNotInSource(vec![format!(
                                    "{group}/{label}"
                                )])),
                            }
                        }
                    };
                    let _ = reply.send(result);
                }
                DbRequest::GetGroupSummary {
                    label,
                    start_datetime,
                    end_datetime,
                    reply,
                } => {
                    println!("received GetGroupSummary");
                    let result = match self.state() {
                        DbState::Closed => Err(DbError::DatabaseNotOpen),
                        DbState::Open { source: None, .. } => Err(DbError::SourceNotConfig),
                        DbState::Open {
                            reader,
                            source: Some(src),
                        } => src
                            .signals()
                            .iter()
                            .filter(|s| s.label() == label)
                            .map(|signal| {
                                reader
                                    .get_signal_summary(
                                        src.table_name(),
                                        src.time_column_name(),
                                        signal.column(),
                                        signal.signal_type(),
                                        &start_datetime,
                                        &end_datetime,
                                    )
                                    .map(|value| (signal.group().to_string(), value))
                            })
                            .collect::<Result<Vec<_>, DbError>>(),
                    };
                    let _ = reply.send(result);
                }
                DbRequest::GetSignalTrend {
                    group,
                    label,
                    start_datetime,
                    end_datetime,
                    reply,
                } => {
                    println!("received GetSignalTrend");
                    let result = match self.state() {
                        DbState::Closed => Err(DbError::DatabaseNotOpen),
                        DbState::Open { source: None, .. } => Err(DbError::SourceNotConfig),
                        DbState::Open {
                            reader,
                            source: Some(src),
                        } => {
                            match src
                                .signals()
                                .iter()
                                .find(|s| s.group() == group && s.label() == label)
                            {
                                Some(signal) => reader.get_signal_trend(
                                    src.table_name(),
                                    src.time_column_name(),
                                    signal.column(),
                                    signal.signal_type(),
                                    &start_datetime,
                                    &end_datetime,
                                ),
                                None => Err(DbError::SelectedColumnNotInSource(vec![format!(
                                    "{group}/{label}"
                                )])),
                            }
                        }
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
