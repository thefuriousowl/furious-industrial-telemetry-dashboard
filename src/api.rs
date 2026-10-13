use axum::{ Json, extract::{ Path, Query, State }, http::StatusCode };
use serde::Deserialize;

use crate::{
    client::DbClient,
    config::SourceConfig,
    dto::TelemetryQuery,
    schema::{ self, ColumnMetaData, RuntimeSchema },
    telemetry::TelemetryRow,
};

#[derive(Debug)]
struct ConfigureSourceRequest {
    db: State<DbClient>,
    req: Json<RuntimeSchema>,
}

pub(crate) async fn list_table(State(db): State<DbClient>) -> Result<
    Json<Vec<String>>,
    (StatusCode, String)
> {
    let result = db.list_table().await;
    match result {
        Ok(tables) => Ok(Json(tables)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

pub(crate) async fn list_available_columns(
    State(db): State<DbClient>,
    Path(table): Path<String>
) -> Result<Json<Vec<ColumnMetaData>>, (StatusCode, String)> {
    let result = db.get_available_columns(&table).await;
    match result {
        Ok(columns) => Ok(Json(columns)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

pub(crate) async fn source(
    State(db): State<DbClient>,
    Json(req): Json<SourceConfig>
) -> Result<Json<RuntimeSchema>, (StatusCode, String)> {
    let result = db.configure_source(req).await;
    match result {
        Ok(schema) => Ok(Json(schema)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

pub(crate) async fn telemetry(State(db): State<DbClient>, Query(q): Query<TelemetryQuery>) -> Result<
    Json<Vec<TelemetryRow>>,
    (StatusCode, String)
> {
    let columns_array = q.columns().split(",").map(|s| s.trim().to_string()).collect::<Vec<String>>();

    let result = db.get_telemetry(columns_array, q.start().clone(), q.end().clone()).await;
    match result {
        Ok(telemetry) => Ok(Json(telemetry)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }

}
