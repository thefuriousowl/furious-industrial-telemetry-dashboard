use crate::{
    config::AppConfig, db::DbReader, error::DbError, request::DbRequest, schema::RuntimeSchema,
};
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
    app_config: Option<AppConfig>,
    receiver: mpsc::Receiver<DbRequest>,
}
impl DbWorker {
    pub(crate) fn new(receiver: mpsc::Receiver<DbRequest>) -> Self {
        Self {
            state: DbState::Closed,
            app_config: None,
            receiver,
        }
    }

    pub(crate) fn set_state(&mut self, state: DbState) {
        self.state = state;
    }

    pub(crate) fn state(&self) -> &DbState {
        &self.state
    }

    fn apply_config(
        &mut self,
        config: AppConfig,
        persist_to: Option<&str>,
    ) -> Result<RuntimeSchema, DbError> {
        config.validate_setup().map_err(DbError::InvalidConfig)?;
        let reader = DbReader::open(config.db_path())?;
        let source = config.source().as_ref().ok_or(DbError::SourceNotConfig)?;
        let schema = reader.discover_schema(source.clone())?;
        if let Some(path) = persist_to {
            config.save_to_file(path)?;
        }
        // All fallible work is complete. Publish the reader, schema and config together.
        self.state = DbState::Open {
            reader,
            source: Some(schema.clone()),
        };
        self.app_config = Some(config);
        Ok(schema)
    }

    pub(crate) fn run(mut self) {
        while let Ok(request) = self.receiver.recv() {
            match request {
                DbRequest::ReadDashboard { query, reply } => {
                    let result = match (&self.state, &self.app_config) {
                        (DbState::Closed, _) => Err(DbError::DatabaseNotOpen),
                        (
                            DbState::Open {
                                reader,
                                source: Some(source),
                            },
                            Some(config),
                        ) => crate::dashboard_data::read_dashboard(
                            reader,
                            source,
                            config.dashboard(),
                            &query,
                        ),
                        _ => Err(DbError::SourceNotConfig),
                    };
                    let _ = reply.send(result);
                }
                DbRequest::GetLatestTime { reply } => {
                    let result = match &self.state {
                        DbState::Closed => Err(DbError::DatabaseNotOpen),
                        DbState::Open {
                            reader,
                            source: Some(source),
                        } => reader.get_latest_time(source.table_name(), source.time_column_name()),
                        _ => Err(DbError::SourceNotConfig),
                    };
                    let _ = reply.send(result);
                }
                DbRequest::ApplyConfig {
                    config,
                    persist_to,
                    reply,
                } => {
                    let result = self.apply_config(config, persist_to.as_deref());
                    let _ = reply.send(result);
                }
                DbRequest::GetAppConfig { reply } => {
                    let _ = reply.send(self.app_config.clone());
                }
                DbRequest::OpenDatabase { path, reply } => {
                    println!("received OpenDatabase");
                    let db_reader = match DbReader::open(&path) {
                        Ok(reader) => {
                            self.app_config = None;
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
                DbRequest::InspectTables { path, reply } => {
                    let result = DbReader::open(&path).and_then(|reader| reader.list_table());
                    let _ = reply.send(result);
                }
                DbRequest::InspectColumns {
                    path,
                    table_name,
                    reply,
                } => {
                    let result = DbReader::open(&path)
                        .and_then(|reader| reader.get_columns_metadata(&table_name));
                    let _ = reply.send(result);
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
                    let mut updated_config = self.app_config.clone();
                    if let Some(app_config) = &mut updated_config {
                        app_config.configure_source(config.clone());
                        if let Err(error) = app_config.validate_setup() {
                            let _ = reply.send(Err(DbError::InvalidConfig(error)));
                            continue;
                        }
                    }
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

                    if result.is_ok() {
                        self.app_config = updated_config;
                    }
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
                    bucket,
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
                                    &bucket,
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
                DbRequest::GetSignalPeak {
                    group,
                    label,
                    start_datetime,
                    end_datetime,
                    reply,
                } => {
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
                                Some(signal) => reader.get_signal_peak(
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

#[cfg(test)]
mod setup_inspection_tests {
    use super::*;
    use crate::{
        client::DbClient,
        config::{SignalConfig, SignalType, SourceConfig},
    };

    #[tokio::test]
    async fn inspection_works_closed_and_preserves_an_active_source_after_success_and_failure() {
        let folder = std::env::temp_dir().join(format!(
            "telemetry-inspect-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&folder).unwrap();
        let a = folder.join("active.db");
        let b = folder.join("draft.db");
        for (path, sql) in [
            (&a, "CREATE TABLE active (Time TEXT, Meter_Flow REAL)"),
            (&b, "CREATE TABLE draft (Recorded TEXT, Reading REAL)"),
        ] {
            rusqlite::Connection::open(path)
                .unwrap()
                .execute_batch(sql)
                .unwrap();
        }
        let a = a.to_str().unwrap();
        let b = b.to_str().unwrap();
        let missing = folder.join("missing.db");
        let bad = folder.join("invalid.db");
        std::fs::write(&bad, "not SQLite").unwrap();
        let (sender, receiver) = mpsc::channel();
        let handle = std::thread::spawn(move || DbWorker::new(receiver).run());
        let client = DbClient::new(sender);
        assert_eq!(client.inspect_tables(b).await.unwrap(), vec!["draft"]);
        assert!(matches!(
            client.list_table().await,
            Err(DbError::DatabaseNotOpen)
        ));
        client.open_database(a).await.unwrap();
        client
            .configure_source(SourceConfig::new(
                "active",
                "Time",
                vec![SignalConfig::new(
                    "Meter_Flow",
                    "Meter",
                    "Flow",
                    "",
                    SignalType::Instantaneous,
                )],
            ))
            .await
            .unwrap();
        assert_eq!(client.inspect_tables(b).await.unwrap(), vec!["draft"]);
        let columns = client.inspect_columns(b, "draft").await.unwrap();
        assert_eq!(
            columns.iter().map(|c| c.name()).collect::<Vec<_>>(),
            vec!["Recorded", "Reading"]
        );
        assert!(client.inspect_columns(b, "absent").await.is_err());
        assert!(
            client
                .inspect_tables(missing.to_str().unwrap())
                .await
                .is_err()
        );
        assert!(!missing.exists());
        assert!(client.inspect_tables(bad.to_str().unwrap()).await.is_err());
        assert_eq!(client.get_source().await.unwrap().table_name(), "active");
        assert_eq!(client.list_table().await.unwrap(), vec!["active"]);
        drop(client);
        handle.join().unwrap();
        std::fs::remove_dir_all(folder).unwrap();
    }
}

#[cfg(test)]
mod config_save_tests {
    use super::*;
    use crate::client::DbClient;

    #[tokio::test]
    async fn save_publishes_config_and_reader_only_after_validation_and_persistence() {
        let folder = std::env::temp_dir().join(format!(
            "telemetry-save-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&folder).unwrap();
        let config_path = folder.join("config.json");
        let path = config_path.to_str().unwrap();
        let initial = AppConfig::load_or_setup(path).await.unwrap();
        assert!(initial.source().is_none());
        assert!(
            !config_path.exists(),
            "First launch must not create a placeholder config"
        );
        let make_config = |name: &str, value: f64| {
            let database = folder.join(format!("{name}.db"));
            let connection = rusqlite::Connection::open(&database).unwrap();
            connection
                .execute_batch("CREATE TABLE samples (Time TEXT, Meter_Flow REAL)")
                .unwrap();
            connection
                .execute(
                    "INSERT INTO samples VALUES ('2026-10-15 00:00:00', ?1)",
                    [value],
                )
                .unwrap();
            serde_json::from_value::<AppConfig>(serde_json::json!({
                "db_path": database, "dashboard_title": name, "dashboard_subtitle": "Flow",
                "source": {"table_name": "samples", "time_column_name": "Time", "signals": [
                    {"column": "Meter_Flow", "group": "Meter", "label": "Flow", "unit": "m³/h", "signal_type": "Instantaneous"}
                ]},
                "dashboard": {"kpis": [{"group": "Meter", "label": "Flow", "operation": "summary"}],
                              "trends": [{"group": "Meter", "label": "Flow"}]}
            })).unwrap()
        };
        let first = make_config("first", 12.0);
        let second = make_config("second", 99.0);
        let (sender, receiver) = mpsc::channel();
        let handle = std::thread::spawn(move || DbWorker::new(receiver).run());
        let client = DbClient::new(sender);
        assert!(client.get_app_config().await.unwrap().is_none());
        client
            .apply_config(first.clone(), Some(path))
            .await
            .unwrap();
        let saved = std::fs::read(&config_path).unwrap();
        let mut invalid_schema = serde_json::to_value(&second).unwrap();
        invalid_schema["source"]["signals"][0]["column"] = "Missing".into();
        let mut invalid_widget = serde_json::to_value(&second).unwrap();
        invalid_widget["dashboard"]["trends"][0]["label"] = "Missing".into();
        let mut invalid_name = serde_json::to_value(&second).unwrap();
        invalid_name["source"]["signals"][0]["group"] = " ".into();
        let mut missing_database = serde_json::to_value(&second).unwrap();
        missing_database["db_path"] = serde_json::to_value(folder.join("missing.db")).unwrap();
        let mut time_signal = serde_json::to_value(&second).unwrap();
        time_signal["source"]["signals"][0]["column"] = "Time".into();
        for config in [
            invalid_schema,
            invalid_widget,
            invalid_name,
            missing_database,
            time_signal,
        ] {
            assert!(
                client
                    .apply_config(serde_json::from_value(config).unwrap(), Some(path))
                    .await
                    .is_err()
            );
            assert_eq!(std::fs::read(&config_path).unwrap(), saved);
            assert_eq!(
                client
                    .get_app_config()
                    .await
                    .unwrap()
                    .unwrap()
                    .dashboard_title(),
                "first"
            );
        }
        assert!(!folder.join("missing.db").exists());
        let bad_directory = folder.join("missing-parent/config.json");
        let target_directory = folder.join("config-is-directory");
        std::fs::create_dir(&target_directory).unwrap();
        for target in [&bad_directory, &target_directory] {
            assert!(
                client
                    .apply_config(second.clone(), Some(target.to_str().unwrap()))
                    .await
                    .is_err()
            );
            assert_eq!(std::fs::read(&config_path).unwrap(), saved);
            assert_eq!(
                client
                    .get_signal_summary(
                        "Meter".into(),
                        "Flow".into(),
                        "2026-10-15 00:00:00".into(),
                        "2026-10-16 00:00:00".into()
                    )
                    .await
                    .unwrap(),
                Some(12.0)
            );
        }
        assert!(std::fs::read_dir(&folder).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
        client
            .apply_config(second.clone(), Some(path))
            .await
            .unwrap();
        assert_eq!(
            client
                .get_app_config()
                .await
                .unwrap()
                .unwrap()
                .dashboard_title(),
            "second"
        );
        assert_eq!(
            client
                .get_signal_summary(
                    "Meter".into(),
                    "Flow".into(),
                    "2026-10-15 00:00:00".into(),
                    "2026-10-16 00:00:00".into()
                )
                .await
                .unwrap(),
            Some(99.0)
        );
        drop(client);
        handle.join().unwrap();
        // Loading on the next launch restores the saved reader and configuration.
        let restored = AppConfig::load_or_setup(path).await.unwrap();
        let (_, receiver) = mpsc::channel();
        let mut restarted = DbWorker::new(receiver);
        restarted.apply_config(restored, None).unwrap();
        assert_eq!(
            restarted.app_config.as_ref().unwrap().dashboard_title(),
            "second"
        );
        drop(restarted);
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[tokio::test]
    async fn disconnected_save_caller_does_not_leave_disk_and_runtime_out_of_sync() {
        let folder = std::env::temp_dir().join(format!(
            "telemetry-cancel-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&folder).unwrap();
        let database = folder.join("data.db");
        rusqlite::Connection::open(&database)
            .unwrap()
            .execute_batch("CREATE TABLE samples (Time TEXT, Flow REAL)")
            .unwrap();
        let config: AppConfig = serde_json::from_value(serde_json::json!({
            "db_path": database, "source": {"table_name": "samples", "time_column_name": "Time", "signals": [
                {"column": "Flow", "group": "Meter", "label": "Flow", "signal_type": "Instantaneous"}
            ]}
        })).unwrap();
        let config_path = folder.join("config.json");
        let (sender, receiver) = mpsc::channel();
        let (reply, response) = tokio::sync::oneshot::channel();
        drop(response);
        sender
            .send(DbRequest::ApplyConfig {
                config: config.clone(),
                persist_to: Some(config_path.to_str().unwrap().into()),
                reply,
            })
            .unwrap();
        let handle = std::thread::spawn(move || DbWorker::new(receiver).run());
        let client = DbClient::new(sender);
        assert_eq!(
            serde_json::to_value(client.get_app_config().await.unwrap().unwrap()).unwrap(),
            serde_json::to_value(&config).unwrap()
        );
        assert!(config_path.exists());
        drop(client);
        handle.join().unwrap();
        std::fs::remove_dir_all(folder).unwrap();
    }
}
