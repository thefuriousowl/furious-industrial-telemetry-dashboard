use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};

use crate::{
    client::AppState,
    config::{AppConfig, SourceConfig},
    dto::TelemetryQuery,
    schema::{ColumnMetaData, RuntimeSchema},
    telemetry::TelemetryRow,
};

pub(crate) async fn list_table(
    State(app_state): State<AppState>,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let result = app_state.db_cli().list_table().await;
    match result {
        Ok(tables) => Ok(Json(tables)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

pub(crate) async fn list_available_columns(
    State(app_state): State<AppState>,
    Path(table): Path<String>,
) -> Result<Json<Vec<ColumnMetaData>>, (StatusCode, String)> {
    let result = app_state.db_cli().get_available_columns(&table).await;
    match result {
        Ok(columns) => Ok(Json(columns)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

pub(crate) async fn source(
    State(app_state): State<AppState>,
    Json(req): Json<SourceConfig>,
) -> Result<Json<RuntimeSchema>, (StatusCode, String)> {
    let result = app_state.db_cli().configure_source(req.clone()).await;
    match result {
        Ok(schema) => {
            let mut app_config = AppConfig::new(app_state.db_path());
            app_config.configure_source(req);
            if let Err(e) = app_config.save_to_file(app_state.config_path()).await {
                eprintln!("failed to save config to disk: {e}");
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Source is active, but configuration could not be saved".to_string(),
                ));
            }

            Ok(Json(schema))
        }
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

pub(crate) async fn get_source(
    State(app_state): State<AppState>,
) -> Result<Json<RuntimeSchema>, (StatusCode, String)> {
    let result = app_state.db_cli().get_source().await;
    match result {
        Ok(schema) => Ok(Json(schema)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

pub(crate) async fn telemetry(
    State(app_state): State<AppState>,
    Query(q): Query<TelemetryQuery>,
) -> Result<Json<Vec<TelemetryRow>>, (StatusCode, String)> {
    let columns_array = q
        .columns()
        .split(",")
        .map(|s| s.trim().to_string())
        .collect::<Vec<String>>();

    let result = app_state
        .db_cli()
        .get_telemetry(columns_array, q.start().clone(), q.end().clone())
        .await;
    match result {
        Ok(telemetry) => Ok(Json(telemetry)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}
