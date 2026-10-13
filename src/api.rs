use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;

use crate::{
    client::DbClient,
    config::SourceConfig,
    schema::{self, ColumnMetaData, RuntimeSchema},
};

#[derive(Debug)]
struct ConfigureSourceRequest {
    db: State<DbClient>,
    req: Json<RuntimeSchema>,
}

pub(crate) async fn list_table(
    State(db): State<DbClient>,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let result = db.list_table().await;
    match result {
        Ok(tables) => Ok(Json(tables)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

pub(crate) async fn list_available_columns(
    State(db): State<DbClient>,
    Path(table): Path<String>,
) -> Result<Json<Vec<ColumnMetaData>>, (StatusCode, String)> {
    let result = db.get_available_columns(&table).await;
    match result {
        Ok(columns) => Ok(Json(columns)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

pub(crate) async fn source(
    State(db): State<DbClient>,
    Json(req): Json<SourceConfig>,
) -> Result<Json<RuntimeSchema>, (StatusCode, String)> {
    let result = db.configure_source(req).await;
    match result {
        Ok(schema) => Ok(Json(schema)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}
