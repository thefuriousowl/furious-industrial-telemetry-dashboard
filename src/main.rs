use std::{process::ExitCode, sync::mpsc, thread};

use axum::{
    Router,
    routing::{get, post},
};

use crate::{
    api::{list_available_columns, list_table, source},
    client::DbClient,
    request::DbRequest,
    worker::DbWorker,
};

mod api;
mod client;
mod config;
mod db;
mod kind;
mod request;
mod schema;
mod sql_command;
mod telemetry;
mod worker;

#[tokio::main]
async fn main() -> ExitCode {
    // --------------------------------------------------
    // Create request channel
    // --------------------------------------------------
    let (request_sender, request_receiver) = mpsc::channel::<DbRequest>();

    // --------------------------------------------------
    // Create DB state-machine runner
    // --------------------------------------------------
    let worker = DbWorker::new(request_receiver);

    // --------------------------------------------------
    // Start DB worker thread
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

    let db_cli = DbClient::new(request_sender);

    let open_db_result = db_cli.open_database("local-data/Database.db").await;

    match open_db_result {
        Ok(()) => println!("database opened"),
        Err(e) => eprintln!("failed to open database: {e}"),
        // --------------------------------------------------
        // Start HTTP server
        // --------------------------------------------------
    }
    let app = Router::new()
        .route("/api/tables", get(list_table))
        .route("/api/tables/{table}/columns", get(list_available_columns))
        .route("/api/source", post(source))
        .with_state(db_cli);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();

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
