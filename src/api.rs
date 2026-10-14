use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};

use crate::{
    DEFAULT_DASHBOARD_SUBTITLE, DEFAULT_DASHBOARD_TITLE,
    client::AppState,
    config::{AppConfig, SourceConfig},
    dto::{
        GroupSummary, GroupSummaryQuery, SignalSummaryQuery, SignalTrend, SignalTrendQuery,
        TelemetryQuery,
    },
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
            let mut app_config = AppConfig::new(
                app_state.db_path(),
                DEFAULT_DASHBOARD_TITLE,
                DEFAULT_DASHBOARD_SUBTITLE,
            );
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

pub(crate) async fn get_signal_trend(
    State(app_state): State<AppState>,
    Query(q): Query<SignalTrendQuery>,
) -> Result<Json<Vec<SignalTrend>>, (StatusCode, String)> {
    let result = app_state
        .db_cli()
        .get_signal_trend(
            q.group().into(),
            q.label().into(),
            q.start().into(),
            q.end().into(),
        )
        .await;
    match result {
        Ok(trend) => Ok(Json(
            trend
                .into_iter()
                .map(|pair| SignalTrend::from(pair))
                .collect::<Vec<SignalTrend>>(),
        )),
        Err(e) => Err((e.status_code(), e.to_string())),
    }
}

pub(crate) async fn dashboard(State(app_state): State<AppState>) -> axum::response::Html<String> {
    let title = app_state.dashboard_title();
    let subtitle = app_state.dashboard_subtitle();
    axum::response::Html(format!(
        r##"<html>
        <head>
            <script src="/static/htmx.min.js"></script>
            <style>
                .row {{ display: flex; align-items: center; gap: 8px; margin: 4px 0; }}
                .group {{ width: 60px; font-weight: bold; }}
                .bar-track {{ flex: 1; background: #eee; height: 16px; border-radius: 4px; overflow: hidden; }}
                .bar-fill {{ background: #2563eb; height: 100%; }}
                .value {{ width: 120px; text-align: right; }}
                .kpi {{ margin: 12px 0; }}
                .kpi-value {{ font-size: 2.5rem; font-weight: bold; }}
                .kpi-label {{ color: #666; }}
                .trend-chart {{ display: flex; align-items: flex-end; gap: 2px; height: 150px; border-bottom: 1px solid #ccc; margin: 12px 0; }}
                .trend-bar {{ flex: 1; background: #2563eb; min-height: 1px; }}
            </style>
        </head>
        <body>
            <p>{title}</p>
            <h1>{subtitle}</h1>
            <div hx-get="/dashboard/kpi?group=JA1&label=Flow&start=2026-08-01%2000:00:00&end=2026-09-01%2000:00:00" hx-trigger="load"></div>
            <div hx-get="/dashboard/kpi/total?label=Total&start=2026-08-01%2000:00:00&end=2026-09-01%2000:00:00" hx-trigger="load"></div>
            <div id="result" hx-get="/dashboard/fragment?label=Total&start=2026-08-01%2000:00:00&end=2026-09-01%2000:00:00" hx-trigger="load"></div>
            <div hx-get="/dashboard/trend?group=JA1&label=Total&start=2026-08-01%2000:00:00&end=2026-09-01%2000:00:00" hx-trigger="load"></div>
        </body>
    </html>"##
    ))
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

pub(crate) async fn kpi_fragment(
    State(app_state): State<AppState>,
    Query(q): Query<SignalSummaryQuery>,
) -> axum::response::Html<String> {
    let result = app_state
        .db_cli()
        .get_signal_summary(
            q.group().into(),
            q.label().into(),
            q.start().into(),
            q.end().into(),
        )
        .await;

    let html = match result {
        Ok(Some(v)) => format!(
            r#"<div class="kpi"><div class="kpi-value">{v:.1}</div><div class="kpi-label">{}</div></div>"#,
            q.label()
        ),
        Ok(None) => r#"<div class="kpi"><div class="kpi-value">No data</div></div>"#.to_string(),
        Err(e) => format!("<p>error: {e}</p>"),
    };

    axum::response::Html(html)
}

pub(crate) async fn total_kpi_fragment(
    State(app_state): State<AppState>,
    Query(q): Query<GroupSummaryQuery>,
) -> axum::response::Html<String> {
    let result = app_state
        .db_cli()
        .get_group_summary(q.label().into(), q.start().into(), q.end().into())
        .await;

    let html = match result {
        Ok(summaries) => {
            let total: f64 = summaries.iter().filter_map(|(_, v)| *v).sum();
            format!(
                r#"<div class="kpi"><div class="kpi-value">{total:.1}</div><div class="kpi-label">Total consumption</div></div>"#
            )
        }
        Err(e) => format!("<p>error: {e}</p>"),
    };

    axum::response::Html(html)
}

pub(crate) async fn trend_fragment(
    State(app_state): State<AppState>,
    Query(q): Query<SignalTrendQuery>,
) -> axum::response::Html<String> {
    let result = app_state
        .db_cli()
        .get_signal_trend(
            q.group().into(),
            q.label().into(),
            q.start().into(),
            q.end().into(),
        )
        .await;

    let html = match result {
        Ok(points) => {
            let max = points
                .iter()
                .filter_map(|(_, v)| *v)
                .fold(0.0_f64, f64::max);
            let bars = points
                .iter()
                .map(|(day, value)| match value {
                    Some(v) => {
                        let height_pct = if max > 0.0 { (v / max) * 100.0 } else { 0.0 };
                        format!(
                            r#"<div class="trend-bar" title="{day}: {v:.1}" style="height: {height_pct:.1}%"></div>"#
                        )
                    }
                    None => format!(
                        r#"<div class="trend-bar" title="{day}: no data" style="height: 0%"></div>"#
                    ),
                })
                .collect::<Vec<_>>()
                .join("");
            format!(r#"<div class="trend-chart">{bars}</div>"#)
        }
        Err(e) => format!("<p>error: {e}</p>"),
    };

    axum::response::Html(html)
}
