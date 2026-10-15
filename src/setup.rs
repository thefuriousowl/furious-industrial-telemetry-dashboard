use axum::Json;
use serde::Serialize;

use crate::{
    client::AppState,
    config::{AppConfig, SignalType},
};

pub(crate) async fn home(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<axum::response::Redirect, SetupError> {
    let active = state
        .db_cli()
        .get_app_config()
        .await
        .map_err(|e| (e.status_code(), e.to_string()))?;
    Ok(axum::response::Redirect::to(if active.is_some() {
        "/dashboard"
    } else {
        "/setup"
    }))
}

pub(crate) async fn current_config(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<AppConfig>, SetupError> {
    state
        .app_config()
        .await
        .map(Json)
        .map_err(|e| (e.status_code(), e.to_string()))
}

pub(crate) async fn save_config(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(config): Json<AppConfig>,
) -> Result<Json<serde_json::Value>, SetupError> {
    if config.listen_addr() != state.listen_addr() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "Keep the current listen address. Changing it requires a server restart.".into(),
        ));
    }
    state
        .db_cli()
        .apply_config(config, Some(state.config_path()))
        .await
        .map_err(|e| (e.status_code(), e.to_string()))?;
    Ok(Json(serde_json::json!({"redirect": "/dashboard"})))
}

#[derive(Serialize)]
pub(crate) struct SetupColumn {
    name: String,
    declare_type: String,
    suggestion: Option<Suggestion>,
}

#[derive(Serialize)]
struct Suggestion {
    group: String,
    label: String,
    signal_type: SignalType,
}

#[derive(serde::Deserialize)]
pub(crate) struct InspectInput {
    db_path: String,
}

#[derive(serde::Deserialize)]
pub(crate) struct ColumnsInput {
    db_path: String,
    table_name: String,
}

type SetupError = (axum::http::StatusCode, String);

pub(crate) async fn tables(
    axum::extract::State(state): axum::extract::State<crate::client::AppState>,
    Json(input): Json<InspectInput>,
) -> Result<Json<Vec<String>>, SetupError> {
    state
        .db_cli()
        .inspect_tables(&input.db_path)
        .await
        .map(Json)
        .map_err(|e| (e.status_code(), e.to_string()))
}

pub(crate) async fn columns(
    axum::extract::State(state): axum::extract::State<crate::client::AppState>,
    Json(input): Json<ColumnsInput>,
) -> Result<Json<Vec<SetupColumn>>, SetupError> {
    let columns = state
        .db_cli()
        .inspect_columns(&input.db_path, &input.table_name)
        .await
        .map_err(|e| (e.status_code(), e.to_string()))?;
    Ok(Json(
        columns
            .into_iter()
            .map(|column| {
                let suggestion = parse_metric(column.name()).map(|(group, kind)| {
                    let (label, signal_type) = match kind {
                        MetricKind::Flow => ("Flow", SignalType::Instantaneous),
                        MetricKind::Pressure => ("Pressure", SignalType::Instantaneous),
                        MetricKind::Temperature => ("Temperature", SignalType::Instantaneous),
                        MetricKind::Totalizer => ("Total", SignalType::Counter),
                    };
                    Suggestion {
                        group: group.to_string(),
                        label: label.to_string(),
                        signal_type,
                    }
                });
                SetupColumn {
                    name: column.name().to_string(),
                    declare_type: column.declare_type().to_string(),
                    suggestion,
                }
            })
            .collect(),
    ))
}

#[derive(serde::Deserialize)]
pub(crate) struct BrowseQuery {
    path: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct DirectoryListing {
    path: String,
    parent: Option<String>,
    entries: Vec<DirectoryEntry>,
}

#[derive(Serialize)]
struct DirectoryEntry {
    name: String,
    path: String,
    is_directory: bool,
}

pub(crate) async fn files(
    axum::extract::Query(query): axum::extract::Query<BrowseQuery>,
) -> Result<Json<DirectoryListing>, SetupError> {
    use axum::http::StatusCode;
    let path = tokio::fs::canonicalize(query.path.as_deref().unwrap_or("."))
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let mut directory = tokio::fs::read_dir(&path)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let mut entries = Vec::new();
    while let Some(entry) = directory
        .next_entry()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    {
        let kind = entry
            .file_type()
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        // Only direct directories and regular files; SQLite validates the selected file.
        if !kind.is_dir() && !kind.is_file() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some(entry_path) = entry.path().to_str().map(str::to_owned) else {
            continue;
        };
        entries.push(DirectoryEntry {
            name,
            path: entry_path,
            is_directory: kind.is_dir(),
        });
    }
    entries.sort_by(|a, b| {
        b.is_directory
            .cmp(&a.is_directory)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(Json(DirectoryListing {
        path: path.to_string_lossy().into_owned(),
        parent: path.parent().map(|p| p.to_string_lossy().into_owned()),
        entries,
    }))
}

#[derive(Debug, PartialEq)]
enum MetricKind {
    Flow,
    Pressure,
    Temperature,
    Totalizer,
}

// This suggests values for the editor; RuntimeSchema still uses user configuration.
fn parse_metric(column_name: &str) -> Option<(&str, MetricKind)> {
    [
        ("_Flow", MetricKind::Flow),
        ("_Pressure", MetricKind::Pressure),
        ("_Temp", MetricKind::Temperature),
        ("_Total", MetricKind::Totalizer),
    ]
    .into_iter()
    .find_map(|(suffix, kind)| {
        let group = column_name.strip_suffix(suffix)?;
        if group.is_empty() {
            None
        } else {
            Some((group, kind))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_known_metrics_and_keeps_group_underscores() {
        assert_eq!(
            parse_metric("Meter_A_Flow"),
            Some(("Meter_A", MetricKind::Flow))
        );
        assert_eq!(
            parse_metric("Meter_A_Pressure"),
            Some(("Meter_A", MetricKind::Pressure))
        );
        assert_eq!(
            parse_metric("Meter_A_Temp"),
            Some(("Meter_A", MetricKind::Temperature))
        );
        assert_eq!(
            parse_metric("Meter_A_Total"),
            Some(("Meter_A", MetricKind::Totalizer))
        );
    }

    #[test]
    fn leaves_unknown_columns_for_manual_configuration() {
        assert_eq!(parse_metric("Ambient_Humidity"), None);
        assert_eq!(parse_metric("Time"), None);
    }

    #[test]
    fn rejects_an_empty_group() {
        assert_eq!(parse_metric("_Flow"), None);
        assert_eq!(parse_metric(""), None);
    }
}
