use std::{process::ExitCode, sync::mpsc, thread};

use axum::{Router, routing::get};

use crate::{
    api::{
        fragment, get_group_summary, get_signal_peak, get_signal_summary, get_signal_trend,
        get_source, kpi_fragment, list_available_columns, list_table, peak_kpi_fragment, source,
        telemetry, total_kpi_fragment, trend_fragment,
    },
    client::{AppState, DbClient},
    config::AppConfig,
    request::DbRequest,
    worker::DbWorker,
};

mod api;
mod charts;
mod client;
mod config;
mod dashboard;
mod dashboard_data;
mod db;
mod dto;
mod error;
mod request;
mod schema;
mod setup;
mod sql_command;
mod telemetry;
mod worker;

const CONFIG_PATH: &str = "config.json";
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

    let app_config = match AppConfig::load_or_setup(CONFIG_PATH).await {
        Ok(config) => config,
        Err(e) => {
            eprintln!("failed to read app config: {e}");
            return ExitCode::FAILURE;
        }
    };
    let app_state = AppState::from(
        app_config.clone(),
        CONFIG_PATH,
        DbClient::new(request_sender),
    );
    if app_config.source().is_some() {
        if let Err(e) = app_state
            .db_cli()
            .apply_config(app_config.clone(), None)
            .await
        {
            eprintln!("failed to activate config: {e}. Open /setup to correct it.");
        }
    } else {
        println!("No source configured. Open /setup to configure telemetry.");
    }
    let app = Router::new()
        .route("/", get(setup::home))
        .route(
            "/api/setup/config",
            get(setup::current_config).post(setup::save_config),
        )
        .route_service(
            "/setup",
            tower_http::services::ServeFile::new("static/setup.html"),
        )
        .route("/api/setup/files", get(setup::files))
        .route("/api/setup/tables", axum::routing::post(setup::tables))
        .route("/api/setup/columns", axum::routing::post(setup::columns))
        .route("/api/tables", get(list_table))
        .route("/api/tables/{table}/columns", get(list_available_columns))
        .route("/api/source", get(get_source).post(source))
        .route("/api/telemetry", get(telemetry))
        .route("/api/signal_summary", get(get_signal_summary))
        .route("/api/group_summary", get(get_group_summary))
        .route("/api/signal_trend", get(get_signal_trend))
        .route("/api/signal_peak", get(get_signal_peak))
        .route("/dashboard", get(dashboard::page))
        .route("/dashboard/data", get(dashboard::fragment))
        .route("/api/dashboard", get(dashboard::data))
        .route("/dashboard/fragment", get(fragment))
        .route("/dashboard/kpi", get(kpi_fragment))
        .route("/dashboard/trend", get(trend_fragment))
        .route("/dashboard/kpi/total", get(total_kpi_fragment))
        .route("/dashboard/kpi/peak", get(peak_kpi_fragment))
        .nest_service("/static", tower_http::services::ServeDir::new("static"))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(app_config.listen_addr())
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
