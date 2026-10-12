use std::{process::ExitCode, sync::mpsc, thread};

use tokio::sync::oneshot;

use crate::{
    client::DbClient,
    config::SourceConfig,
    request::DbRequest,
    schema::{ColumnMetaData, RuntimeSchema},
    worker::DbWorker,
};

mod client;
mod config;
mod db;
mod kind;
mod request;
mod schema;
mod sql_command;
mod telemetry;
mod worker;

fn main() -> ExitCode {
    // --------------------------------------------------
    // 1. Create request channel
    //
    // request_sender   -> ฝั่ง app/API ถือไว้ส่ง command
    // request_receiver -> ย้ายให้ DbWorker
    // --------------------------------------------------
    let (request_sender, request_receiver) = mpsc::channel::<DbRequest>();

    // --------------------------------------------------
    // 2. Create DB state-machine runner
    //
    // DbWorker::new() ควรเริ่มด้วย state = Closed
    // --------------------------------------------------
    let worker = DbWorker::new(request_receiver);

    // --------------------------------------------------
    // 3. Start DB worker thread
    // --------------------------------------------------
    let worker_handle =
        match thread::Builder::new()
            .name("db-worker".to_string())
            .spawn(move || {
                worker.run();
            }) {
            Ok(handle) => handle,

            Err(error) => {
                eprintln!("failed to start db worker: {error}");
                return ExitCode::FAILURE;
            }
        };

    // ==================================================
    // 4. MOCK APP / MOCK API
    //
    // ตอนนี้ main แกล้งเป็น caller ไปก่อน
    //
    // TODO:
    // ส่ง DbRequest ต่าง ๆ จากตรงนี้
    //
    // ตัวอย่าง flow ในอนาคต:
    //
    // OpenDatabase
    // ListTables
    // GetColumns
    // DiscoverSchema
    // ReadRange
    //
    // เมื่อมี Axum จริง block นี้จะหายไป
    // แล้ว request_sender จะถูกส่งให้ AppState/DbClient แทน
    // ==================================================

    // TODO: send commands here
    // --------------------------------------------------
    // Open DB worker
    // --------------------------------------------------
    let config = SourceConfig {
        table_name: "DataLogger1".to_string(),
        time_column_name: "Time".to_string(),
        ignored_columns: vec!["Id".to_string()],
    };
    let db_cli = DbClient::new(request_sender.clone());
    let open_db_result = db_cli.open_database("local-data/Database.db");

    match open_db_result {
        Ok(()) => println!("database opened"),
        Err(e) => eprintln!("failed to open database: {e}"),
    }

    // --------------------------------------------------
    // ListTables worker
    // --------------------------------------------------
    let mut main_tables = Vec::new();

    let list_table_result = db_cli.list_table();
    match list_table_result {
        Ok(tables) => {
            println!("list table complete");
            println!("result: {:?}", tables);
            main_tables.extend_from_slice(&tables);
        }
        Err(e) => {
            println!("list table failed");
            eprintln!("{}", e.to_string());
        }
    }

    // --------------------------------------------------
    // GetColumns worker
    // --------------------------------------------------
    let mut main_columns_metadata = Vec::new();
    let get_columns_result = db_cli.get_available_columns(&main_tables[0]);

    match get_columns_result {
        Ok(metadata) => {
            main_columns_metadata = metadata;
        }
        Err(e) => {
            eprintln!("Get column metadata error: {e}");
        }
    }

    println!("ColumnMetaData: {:#?}", main_columns_metadata);

    // --------------------------------------------------
    // DiscoverSchema worker
    // --------------------------------------------------

    let meter_overwrite_policy = false;
    let expected_time_column_name = String::from("Time");
    let get_schema_result = db_cli.get_schema(config, meter_overwrite_policy);
    let mut main_runtime_schema: Option<RuntimeSchema> = None;
    match get_schema_result {
        Ok(schema) => {
            main_runtime_schema = Some(schema);
        }
        Err(e) => {
            eprintln!("Discover schema error: {e}");
        }
    }

    println!("Current RuntimeSchema: {:#?}", main_runtime_schema);

    // --------------------------------------------------
    // ReadRange worker
    // --------------------------------------------------
    let start = "2026-07-01 00:00:00.000000";
    let end = "2026-07-02 00:00:00.000000";
    let selected_columns = vec![
        main_columns_metadata[2].name().to_string(),
        main_columns_metadata[3].name().to_string(),
    ];

    let get_telemetry_result = db_cli.get_telemetry(
        &main_tables[0].clone(),
        selected_columns,
        &expected_time_column_name,
        start.to_string(),
        end.to_string(),
    );

    let mut telemetry_rows = Vec::new();
    match get_telemetry_result {
        Ok(telemetry) => {
            telemetry_rows = telemetry;
        }
        Err(e) => {
            eprintln!("Read range error: {e}");
        }
    }

    println!("ReadRange result: {:#?}", telemetry_rows);

    // --------------------------------------------------
    // 5. Shutdown worker
    // --------------------------------------------------
    if request_sender.send(DbRequest::Shutdown).is_err() {
        eprintln!("db worker stopped before shutdown request");
    }

    // --------------------------------------------------
    // 6. Wait for worker thread to finish
    // --------------------------------------------------
    match worker_handle.join() {
        Ok(()) => ExitCode::SUCCESS,

        Err(_) => {
            eprintln!("db worker panicked");
            ExitCode::FAILURE
        }
    }
}
