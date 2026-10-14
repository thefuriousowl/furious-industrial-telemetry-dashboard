use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};

use crate::{
    client::AppState,
    config::{AppConfig, SourceConfig},
    dto::{GroupSummary, GroupSummaryQuery, SignalSummaryQuery, TelemetryQuery},
    schema::{ColumnMetaData, RuntimeSchema},
    telemetry::TelemetryRow,
};

pub(crate) async fn list_table(
    State(app_state): State<AppState>,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let result = app_state.db_cli().list_table().await;
    match result {
        Ok(tables) => Ok(Json(tables)),
        Err(e) => Err((e.status_code(), e.to_string())),
    }
}

pub(crate) async fn list_available_columns(
    State(app_state): State<AppState>,
    Path(table): Path<String>,
) -> Result<Json<Vec<ColumnMetaData>>, (StatusCode, String)> {
    let result = app_state.db_cli().get_available_columns(&table).await;
    match result {
        Ok(columns) => Ok(Json(columns)),
        Err(e) => Err((e.status_code(), e.to_string())),
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
        Err(e) => Err((e.status_code(), e.to_string())),
    }
}

pub(crate) async fn get_source(
    State(app_state): State<AppState>,
) -> Result<Json<RuntimeSchema>, (StatusCode, String)> {
    let result = app_state.db_cli().get_source().await;
    match result {
        Ok(schema) => Ok(Json(schema)),
        Err(e) => Err((e.status_code(), e.to_string())),
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
        Err(e) => Err((e.status_code(), e.to_string())),
    }
}

pub(crate) async fn get_signal_summary(
    State(app_state): State<AppState>,
    Query(q): Query<SignalSummaryQuery>,
) -> Result<Json<Option<f64>>, (StatusCode, String)> {
    let result = app_state
        .db_cli()
        .get_signal_summary(
            q.group().into(),
            q.label().into(),
            q.start().into(),
            q.end().into(),
        )
        .await;
    match result {
        Ok(summary) => Ok(Json(summary)),
        Err(e) => Err((e.status_code(), e.to_string())),
    }
}

pub(crate) async fn get_group_summary(
    State(app_state): State<AppState>,
    Query(q): Query<GroupSummaryQuery>,
) -> Result<Json<Vec<GroupSummary>>, (StatusCode, String)> {
    let result = app_state
        .db_cli()
        .get_group_summary(q.label().into(), q.start().into(), q.end().into())
        .await;
    match result {
        Ok(summary) => Ok(Json(
            summary
                .into_iter()
                .map(|pair| GroupSummary::from(pair))
                .collect::<Vec<GroupSummary>>(),
        )),
        Err(e) => Err((e.status_code(), e.to_string())),
    }
}

pub(crate) async fn dashboard() -> axum::response::Html<&'static str> {
    axum::response::Html(
        r##"<html>
        <head>
            <script src="/static/htmx.min.js"></script>
            <style>
                .row { display: flex; align-items: center; gap: 8px; margin: 4px 0; }
                .group { width: 60px; font-weight: bold; }
                .bar-track { flex: 1; background: #eee; height: 16px; border-radius: 4px; overflow: hidden; }
                .bar-fill { background: #2563eb; height: 100%; }
                .value { width: 120px; text-align: right; }
            </style>
        </head>
        <body>
            <h1>hello dashboard</h1>
            <button hx-get="/dashboard/fragment" hx-target="#result">Load</button>
            <div id="result" hx-get="/dashboard/fragment?label=Flow&start=2026-08-01%2000:00:00&end=2026-09-01%2000:00:00" hx-trigger="load"></div>
        </body>
    </html>"##,
    )
}

pub(crate) async fn fragment(
    State(app_state): State<AppState>,
    Query(q): Query<GroupSummaryQuery>,
) -> axum::response::Html<String> {
    let result = app_state
        .db_cli()
        .get_group_summary(q.label().into(), q.start().into(), q.end().into())
        .await;

    let html = match result {
        Ok(mut summaries) => {
            summaries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            let total: f64 = summaries.iter().filter_map(|(_, v)| *v).sum();
            let header = format!("<p><strong>Total:</strong> {total:.1}</p>");
            let rows = summaries
                .iter()
                .map(|(group, value)| {
                    match value {
                        Some(v) => {
                            let pct = if total > 0.0 { (v / total) * 100.0 } else { 0.0 };
                            format!(
                                r#"<div class="row"><span class="group">{group}</span><div class="bar-track"><div class="bar-fill" style="width: {pct:.1}%"></div></div><span class="value">{v:.1} ({pct:.1}%)</span></div>"#
                            )
                        }
                        None =>
                            format!(
                                r#"<div class="row"><span class="group">{group}</span><span class="value">No flow</span></div>"#
                            ),
                    }
                })
                .collect::<Vec<_>>()
                .join("");

            format!("{header}<div>{rows}</div>")
        }
        Err(e) => format!("<p>error: {e}</p>"),
    };

    axum::response::Html(html)
}
