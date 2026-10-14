use std::{process::ExitCode, sync::mpsc, thread};

use axum::{Router, routing::get};

use crate::{
    api::{get_source, list_available_columns, list_table, source, telemetry},
    client::{AppState, DbClient},
    config::AppConfig,
    request::DbRequest,
    worker::DbWorker,
};

mod api;
mod client;
mod config;
mod db;
mod dto;
mod error;
mod request;
mod schema;
mod sql_command;
mod telemetry;
mod worker;

const CONFIG_PATH: &str = "config.json";
const DEFAULT_DB_PATH: &str = "local-data/Database.db";
#[tokio::main]
async fn main() -> ExitCode {
    // Create request channel
    let (request_sender, request_receiver) = mpsc::channel::<DbRequest>();

    // Create DB state-machine runner
    let worker = DbWorker::new(request_receiver);

    // Start DB worker thread
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

    let app_config = match std::path::Path::new(CONFIG_PATH).exists() {
        true => {
            let app_config = match AppConfig::load_from_file(CONFIG_PATH).await {
                Ok(cfg) => cfg,
                Err(e) => {
                    eprintln!("failed to load app config file: {e}");
                    return ExitCode::FAILURE;
                }
            };
            app_config
        }
        false => {
            let default_config = AppConfig::new(DEFAULT_DB_PATH);
            if let Err(e) = default_config.save_to_file(CONFIG_PATH).await {
                eprintln!("failed to save config to file: {e}");
            }
            default_config
        }
    };
    let app_state = AppState::from(
        app_config.clone(),
        CONFIG_PATH,
        DbClient::new(request_sender),
    );

    let open_db_result = app_state.db_cli().open_database(app_state.db_path()).await;

    match open_db_result {
        Ok(()) => println!("database opened"),
        Err(e) => eprintln!("failed to open database: {e}"),
    }

    if let Some(source_config) = app_config.clone().source() {
        match app_state
            .db_cli()
            .configure_source(source_config.clone())
            .await
        {
            Ok(_schema) => {
                println!("source configured");
            }
            Err(e) => eprintln!("failed to configure source {e}"),
        }
    }
    let app = Router::new()
        .route("/api/tables", get(list_table))
        .route("/api/tables/{table}/columns", get(list_available_columns))
        .route("/api/source", get(get_source).post(source))
        .route("/api/telemetry", get(telemetry))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    // --------------------------------------------------
    // Wait for worker thread to finish
    // --------------------------------------------------
    match worker_handle.join() {
        Ok(()) => ExitCode::SUCCESS,

        Err(_) => {
            eprintln!("db worker panicked");
            ExitCode::FAILURE
        }
    }
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.expect("grateful shutdown")
}
