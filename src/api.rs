use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};

use crate::{
    client::AppState,
    config::{AppConfig, KpiOperation, SignalType, SourceConfig},
    dto::{
        DashboardQuery, GroupSummary, GroupSummaryQuery, Period, SignalSummaryQuery, SignalTrend,
        SignalTrendQuery, SingleSignalQuery, TelemetryQuery,
    },
    schema::{ColumnMetaData, RuntimeSchema},
    telemetry::{TelemetryRow, TrendBucket},
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
    let mut app_config = match AppConfig::load_from_file(app_state.config_path()).await {
        Ok(config) => config,
        Err(e) => {
            return Err((e.status_code(), e.to_string()));
        }
    };
    app_config.configure_source(req.clone());

    if let Err(error) = app_config.validate_dashboard() {
        return Err((StatusCode::BAD_REQUEST, error));
    }

    let result = app_state.db_cli().configure_source(req).await;
    match result {
        Ok(schema) => {
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
            q.bucket().clone(),
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

pub(crate) async fn get_signal_peak(
    State(app_state): State<AppState>,
    Query(q): Query<SingleSignalQuery>,
) -> Result<Json<Option<f64>>, (StatusCode, String)> {
    let result = app_state
        .db_cli()
        .get_signal_peak(
            q.group().into(),
            q.label().into(),
            q.start().into(),
            q.end().into(),
        )
        .await;
    match result {
        Ok(peak) => Ok(Json(peak)),
        Err(e) => Err((e.status_code(), e.to_string())),
    }
}

pub(crate) async fn dashboard(
    State(app_state): State<AppState>,
    Query(q): Query<DashboardQuery>,
) -> Result<axum::response::Html<String>, (StatusCode, String)> {
    let title = app_state.dashboard_title();
    let subtitle = app_state.dashboard_subtitle();
    let base_date = match q.base_date() {
        Some(date) => date.as_str(),
        None => {
            println!("missing date. fallback to today");
            &time::OffsetDateTime::now_utc().date().to_string()
        }
    };

    let format = time::macros::format_description!("[year]-[month]-[day]");

    let parsed_base_date = match time::Date::parse(base_date, format) {
        Ok(date) => date,
        Err(e) => {
            return Err((StatusCode::BAD_REQUEST, e.to_string()));
        }
    };

    let (start_date, end_date) = dashboard_period_range(parsed_base_date, q.period())
        .map_err(|error| (StatusCode::BAD_REQUEST, error))?;

    let start = format!("{start_date}%2000:00:00");
    let end = format!("{end_date}%2000:00:00");

    let period_name = match q.period() {
        Some(Period::Daily) | None => "daily",
        Some(Period::Weekly) => "weekly",
        Some(Period::Monthly) => "monthly",
        Some(Period::Yearly) => "yearly",
    };

    let previous_date = start_date
        .previous_day()
        .and_then(|date| dashboard_period_range(date, q.period()).ok())
        .map(|(start, _)| start);
    let next_date = dashboard_period_range(end_date, q.period())
        .ok()
        .map(|(start, _)| start);

    let previous_link = match previous_date {
        Some(date) => format!(
            r#"<a href="/dashboard?base_date={date}&amp;period={period_name}">Previous</a>"#
        ),
        None => r#"<span aria-disabled="true">Previous</span>"#.to_string(),
    };
    let next_link = match next_date {
        Some(date) => {
            format!(r#"<a href="/dashboard?base_date={date}&amp;period={period_name}">Next</a>"#)
        }
        None => r#"<span aria-disabled="true">Next</span>"#.to_string(),
    };

    let bucket = match q.period() {
        Some(Period::Daily) | None => "hour",
        Some(Period::Weekly) | Some(Period::Monthly) => "day",
        Some(Period::Yearly) => "month",
    };
    let display_start = start.replace("%20", " ");
    let display_end = end.replace("%20", " ");
    let (daily_class, weekly_class, monthly_class, yearly_class) = match q.period() {
        Some(Period::Daily) | None => ("active", "", "", ""),
        Some(Period::Weekly) => ("", "active", "", ""),
        Some(Period::Monthly) => ("", "", "active", ""),
        Some(Period::Yearly) => ("", "", "", "active"),
    };

    let mut main_trend = r#"<div class="chart-col"><p>No trend configured</p></div>"#.to_string();

    let mut detail_trends = String::new();

    for (index, trend) in app_state.dashboard().trends().iter().enumerate() {
        let query = match serde_urlencoded::to_string([
            ("group", trend.group()),
            ("label", trend.label()),
            ("start", display_start.as_str()),
            ("end", display_end.as_str()),
            ("bucket", bucket),
        ]) {
            Ok(query) => query,
            Err(e) => {
                return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
            }
        };

        let query = escape_html(&query);
        let label = escape_html(trend.label());

        let card = format!(
            r#"<div class="chart-col"
                hx-get="/dashboard/trend?{query}"
                hx-trigger="load">
            <p>{label} — Loading...</p>
        </div>"#
        );

        if index == 0 {
            main_trend = card;
        } else {
            detail_trends.push_str(&card);
        }
    }
    let mut kpis_html = String::new();

    for kpi in app_state.dashboard().kpis() {
        let path = match kpi.operation() {
            KpiOperation::Summary => "/dashboard/kpi",
            KpiOperation::Peak => "/dashboard/kpi/peak",
        };

        let query = match serde_urlencoded::to_string([
            ("group", kpi.group()),
            ("label", kpi.label()),
            ("start", display_start.as_str()),
            ("end", display_end.as_str()),
        ]) {
            Ok(query) => query,
            Err(e) => {
                return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
            }
        };

        let query = escape_html(&query);
        let label = escape_html(kpi.label());

        kpis_html.push_str(&format!(
            r#"<div hx-get="{path}?{query}" hx-trigger="load">
            <p>{label} — Loading...</p>
        </div>"#
        ));
    }

    let mut ranking_html = String::new();

    if let Some(label) = app_state.dashboard().group_summary_label() {
        let query = match serde_urlencoded::to_string([
            ("label", label.as_str()),
            ("start", display_start.as_str()),
            ("end", display_end.as_str()),
        ]) {
            Ok(query) => query,
            Err(e) => {
                return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
            }
        };

        let query = escape_html(&query);

        kpis_html.push_str(&format!(
            r#"<div hx-get="/dashboard/kpi/total?{query}" hx-trigger="load">
            <p>Total — Loading...</p>
        </div>"#
        ));

        ranking_html = format!(
            r#"<div class="chart-col" id="result"
                hx-get="/dashboard/fragment?{query}"
                hx-trigger="load">
            <p>Ranking — Loading...</p>
        </div>"#
        );
    }

    Ok(axum::response::Html(format!(
        r##"<html>

       <head>
           <meta charset="utf-8">
           <meta name="viewport" content="width=device-width, initial-scale=1">
           <link rel="stylesheet" href="/static/dashboard.css?v=3">
           <script src="/static/htmx.min.js" defer></script>
           <script src="/static/dashboard.js?v=2" defer></script>
       </head>

       <body>
           <p>{title}</p>
           <h1>{subtitle}</h1>
           <form action="/dashboard" method="get">
               <label>
                   Date:
                   <input type="date" name="base_date" value="{parsed_base_date}" required>
               </label>

               <div class="period-nav">
                   <button type="submit" name="period" value="daily" class="{daily_class}">Daily</button>

                   <button type="submit" name="period" value="weekly" class="{weekly_class}">Weekly</button>

                   <button type="submit" name="period" value="monthly" class="{monthly_class}">Monthly</button>

                   <button type="submit" name="period" value="yearly" class="{yearly_class}">Yearly</button>
               </div>
               <nav class="range-nav" aria-label="Period navigation">
                   {previous_link}
                   {next_link}
               </nav>
           </form>
           <p>Data from {display_start} to before {display_end}</p>
           <div class="kpi-row">
               {kpis_html}
           </div>

           <div class="chart-row">
               {main_trend}
               {ranking_html}
           </div>

           <div class="detail-charts">
               {detail_trends}
           </div>
       </body>

       </html>"##
    )))
}

pub(crate) async fn fragment(
    State(app_state): State<AppState>,
    Query(q): Query<GroupSummaryQuery>,
) -> axum::response::Html<String> {
    let schema = match app_state.db_cli().get_source().await {
        Ok(schema) => schema,
        Err(e) => {
            return axum::response::Html(format!("<p>error: {}</p>", escape_html(&e.to_string())));
        }
    };

    let first_signal = match schema.signals().iter().find(|s| s.label() == q.label()) {
        Some(signal) => signal,
        None => {
            return axum::response::Html("<p>No matching signals</p>".to_string());
        }
    };

    let common_unit = first_signal.unit();

    for signal in schema.signals() {
        if signal.label() == q.label() && signal.unit() != common_unit {
            return axum::response::Html(
                "<p>Cannot compare signals with different units</p>".to_string(),
            );
        }
    }

    let unit = escape_html(common_unit);
    let label = escape_html(q.label());
    let result = app_state
        .db_cli()
        .get_group_summary(q.label().into(), q.start().into(), q.end().into())
        .await;

    let html = match result {
        Ok(mut summaries) => {
            let has_values = summaries.iter().any(|(_, value)| value.is_some());

            if !has_values {
                return axum::response::Html(format!(
                    r#"<h3>{label} by group</h3>
                    <p>No data in this period</p>"#
                ));
            }
            summaries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            let total: f64 = summaries.iter().filter_map(|(_, v)| *v).sum();
            let header = format!(
                r#"<h3>{label} by group</h3>
   <p><strong>Total:</strong> <span data-number="{total}">{total:.1}</span> {unit}</p>"#
            );
            let rows = summaries
                .iter()
                .map(|(group, value)| match value {
                    Some(v) => {
                        let pct = if total > 0.0 {
                            (v / total) * 100.0
                        } else {
                            0.0
                        };
                        format!(
                            r#"<div class="row"><span class="group">{group}</span>
                                    <div class="bar-track">
                                        <div class="bar-fill" style="width: {pct:.1}%"></div>
                                    </div><span class="value">
                                        <span data-number="{v}">{v:.1}</span> ({pct:.1}%)
                                    </span>
                                </div>"#
                        )
                    }
                    None => format!(
                        r#"<div class="row">
                                    <span class="group">{group}</span>
                                    <span class="value">No data</span>
                                </div>"#
                    ),
                })
                .collect::<Vec<_>>()
                .join("");

            format!("{header}<div>{rows}</div>")
        }
        Err(e) => format!("<p>error: {e}</p>"),
    };

    axum::response::Html(html)
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub(crate) async fn kpi_fragment(
    State(app_state): State<AppState>,
    Query(q): Query<SignalSummaryQuery>,
) -> axum::response::Html<String> {
    // read signals metadata from source current source
    let schema = match app_state.db_cli().get_source().await {
        Ok(schema) => schema,
        Err(e) => {
            return axum::response::Html(format!("<p>error: {}</p>", escape_html(&e.to_string())));
        }
    };

    // find signal which match group and label in url
    let signal = match schema
        .signals()
        .iter()
        .find(|signal| signal.group() == q.group() && signal.label() == q.label())
    {
        Some(signal) => signal,
        None => {
            return axum::response::Html("<p>Signal not configured</p>".to_string());
        }
    };

    let label = escape_html(signal.label());
    let unit = escape_html(signal.unit());
    let group = escape_html(signal.group());
    let summary_kind = match signal.signal_type() {
        SignalType::Instantaneous => "Average",
        SignalType::Counter => "Consumption",
    };
    // read value
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
            r#"<div class="kpi">
                <div class="kpi-value">
                    <span data-number="{v}">{v:.1}</span>
                    <span class="kpi-unit">{unit}</span>
                </div>
                <div class="kpi-label">{group}/{label} · {summary_kind}</div>
            </div>"#
        ),
        Ok(None) => format!(
            r#"<div class="kpi">
                <div class="kpi-value">No data</div>
                <div class="kpi-label">{group}/{label} · {summary_kind}</div>
            </div>"#
        ),
        Err(e) => format!("<p>error: {}</p>", escape_html(&e.to_string())),
    };

    axum::response::Html(html)
}

pub(crate) async fn peak_kpi_fragment(
    State(app_state): State<AppState>,
    Query(q): Query<SingleSignalQuery>,
) -> axum::response::Html<String> {
    let schema = match app_state.db_cli().get_source().await {
        Ok(schema) => schema,
        Err(e) => {
            return axum::response::Html(format!("<p>error: {}</p>", escape_html(&e.to_string())));
        }
    };

    let signal = match schema
        .signals()
        .iter()
        .find(|signal| signal.group() == q.group() && signal.label() == q.label())
    {
        Some(signal) => signal,
        None => {
            return axum::response::Html("<p>Signal not configured</p>".to_string());
        }
    };
    let label = escape_html(signal.label());
    let unit = escape_html(signal.unit());
    let group = escape_html(signal.group());
    let result = app_state
        .db_cli()
        .get_signal_peak(
            q.group().into(),
            q.label().into(),
            q.start().into(),
            q.end().into(),
        )
        .await;

    let html = match result {
        Ok(Some(v)) => format!(
            r#"<div class="kpi">
                <div class="kpi-value">
                    <span data-number="{v}">{v:.1}</span>
                    <span class="kpi-unit">{unit}</span>
                </div>
                <div class="kpi-label">Peak {group}/{label}</div>
            </div>"#
        ),
        Ok(None) => format!(
            r#"<div class="kpi">
            <div class="kpi-value">No data</div>
            <div class="kpi-label">Peak {group}/{label}</div>
        </div>"#
        ),
        Err(e) => format!("<p>error: {}</p>", escape_html(&e.to_string())),
    };

    axum::response::Html(html)
}

pub(crate) async fn total_kpi_fragment(
    State(app_state): State<AppState>,
    Query(q): Query<GroupSummaryQuery>,
) -> axum::response::Html<String> {
    let schema = match app_state.db_cli().get_source().await {
        Ok(schema) => schema,
        Err(e) => {
            return axum::response::Html(format!("<p>error: {}</p>", escape_html(&e.to_string())));
        }
    };

    // find unit from the first matched signal-label
    let first_signal = match schema.signals().iter().find(|s| s.label() == q.label()) {
        Some(signal) => signal,
        None => {
            return axum::response::Html("<p>No matching signals</p>".to_string());
        }
    };

    // all sum element should have the same unit signal
    let common_unit = first_signal.unit();

    for signal in schema.signals() {
        if signal.label() == q.label() && signal.unit() != common_unit {
            return axum::response::Html(
                "<p>Cannot total signals with different units</p>".to_string(),
            );
        }
    }

    let unit = escape_html(common_unit);

    let result = app_state
        .db_cli()
        .get_group_summary(q.label().into(), q.start().into(), q.end().into())
        .await;

    let html = match result {
        Ok(summaries) => {
            let has_values = summaries.iter().any(|(_, value)| value.is_some());

            if !has_values {
                return axum::response::Html(
                    r#"<div class="kpi">
                        <div class="kpi-value">No data</div>
                        <div class="kpi-label">Total consumption</div>
                    </div>"#
                        .to_string(),
                );
            }

            let total: f64 = summaries.iter().filter_map(|(_, value)| *value).sum();

            format!(
                r#"<div class="kpi">
                    <div class="kpi-value">
                        <span data-number="{total}">{total:.1}</span>
                        <span class="kpi-unit">{unit}</span>
                    </div>
                    <div class="kpi-label">Total consumption</div>
                </div>"#
            )
        }
        Err(e) => format!("<p>error: {}</p>", escape_html(&e.to_string())),
    };

    axum::response::Html(html)
}

pub(crate) async fn trend_fragment(
    State(app_state): State<AppState>,
    Query(q): Query<SignalTrendQuery>,
) -> axum::response::Html<String> {
    let schema = match app_state.db_cli().get_source().await {
        Ok(schema) => schema,
        Err(e) => {
            return axum::response::Html(format!("<p>error: {}</p>", escape_html(&e.to_string())));
        }
    };

    let signal = match schema
        .signals()
        .iter()
        .find(|signal| signal.group() == q.group() && signal.label() == q.label())
    {
        Some(signal) => signal,
        None => {
            return axum::response::Html("<p>Signal not configured</p>".to_string());
        }
    };

    let group = escape_html(signal.group());
    let label = escape_html(signal.label());
    let unit = escape_html(signal.unit());
    let summary_kind = match signal.signal_type() {
        SignalType::Instantaneous => "Average",
        SignalType::Counter => "Consumption",
    };

    let bucket_label = match q.bucket() {
        TrendBucket::Hour => "hour",
        TrendBucket::Day => "day",
        TrendBucket::Month => "month",
    };
    let result = app_state
        .db_cli()
        .get_signal_trend(
            q.group().into(),
            q.label().into(),
            q.bucket().clone(),
            q.start().into(),
            q.end().into(),
        )
        .await;

    let html = match result {
        Ok(points) => {
            let points = match fill_trend_buckets(q.start(), q.end(), q.bucket(), points) {
                Ok(points) => points,
                Err(error) => {
                    return axum::response::Html(format!("<p>error: {}</p>", escape_html(&error)));
                }
            };

            let has_values = points.iter().any(|(_, value)| value.is_some());
            if !has_values {
                return axum::response::Html(format!(
                    r#"<h3>{group} / {label}</h3>
                        <p>{summary_kind} per {bucket_label} · Unit: {unit}</p>
                        <p>No data in this period</p>"#
                ));
            }
            let axis_min = points
                .iter()
                .filter_map(|(_, value)| *value)
                .fold(0.0_f64, f64::min);

            let max = points
                .iter()
                .filter_map(|(_, value)| *value)
                .fold(0.0_f64, f64::max);

            let axis_max = if max > axis_min { max } else { 1.0 };
            let axis_range = axis_max - axis_min;
            let axis_mid = (axis_max + axis_min) / 2.0;
            let zero_pct = ((0.0 - axis_min) / axis_range) * 100.0;

            let bars = points
                .iter()
                .map(|(day, value)| {
                    match value {
                        Some(v) => {
                            let value_pct = ((v - axis_min) / axis_range) * 100.0;
                            let bottom_pct = value_pct.min(zero_pct);
                            let height_pct = (value_pct - zero_pct).abs();

                            format!(
                                r#"<div class="trend-slot" title="{day}: {v:.1}">
                                    <div class="trend-bar" style="bottom: {bottom_pct:.4}%; height: {height_pct:.4}%">
                                    </div>
                                </div>"#
                            )
                        }
                        None => format!(r#"<div class="trend-slot" title="{day}: no data"></div>"#),
                    }
                })
                .collect::<Vec<_>>()
                .join("");
            let mut time_labels = String::new();

            let label_step = if points.len() > 12 { 4 } else { 1 };

            for (index, (day, _)) in points.iter().enumerate() {
                let text = if index % label_step == 0 {
                    match q.bucket() {
                        TrendBucket::Hour => day.get(11..16).unwrap_or(day),
                        TrendBucket::Day => day.get(5..10).unwrap_or(day),
                        TrendBucket::Month => day.get(5..7).unwrap_or(day),
                    }
                } else {
                    ""
                };

                let text = escape_html(text);
                time_labels.push_str(&format!("<span>{text}</span>"));
            }
            let mut table_rows = String::new();

            for (day, value) in &points {
                let day = escape_html(day);

                let value_text = match value {
                    Some(v) => format!("{v:.2}"),
                    None => "No data".to_string(),
                };

                table_rows.push_str(&format!("<tr><td>{day}</td><td>{value_text}</td></tr>"));
            }
            format!(
                r#"<h3>{group} / {label}</h3>
                <p>{summary_kind} per {bucket_label} · Unit: {unit}</p>

                <div class="trend-plot">
                    <div class="trend-axis">
                        <span>{axis_max:.1}</span>
                        <span>{axis_mid:.1}</span>
                        <span>{axis_min:.1}</span>
                    </div>

                    <div class="trend-chart">
                        <div class="trend-zero" style="bottom: {zero_pct:.4}%">
                        </div>
                        {bars}
                    </div>

                    <div class="trend-labels">{time_labels}</div>
                </div>
                <details class="trend-data">
                    <summary>View data</summary>
                    <div class="trend-table-wrap">
                        <table class="trend-table">
                            <thead>
                                <tr>
                                    <th scope="col">Time</th>
                                    <th scope="col">{summary_kind} ({unit})</th>
                                </tr>
                            </thead>
                            <tbody>{table_rows}</tbody>
                        </table>
                    </div>
                </details>
                "#
            )
        }
        Err(e) => format!("<p>error: {e}</p>"),
    };

    axum::response::Html(html)
}
fn fill_trend_buckets(
    start: &str,
    end: &str,
    bucket: &TrendBucket,
    points: Vec<(String, Option<f64>)>,
) -> Result<Vec<(String, Option<f64>)>, String> {
    let format = time::macros::format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
    let start = time::PrimitiveDateTime::parse(start, format).map_err(|e| e.to_string())?;
    let end = time::PrimitiveDateTime::parse(end, format).map_err(|e| e.to_string())?;

    if start >= end {
        return Err("Start must be before end".to_string());
    }

    let mut cursor = (match bucket {
        TrendBucket::Hour => start.date().with_hms(start.hour(), 0, 0),
        TrendBucket::Day => Ok(start.date().midnight()),
        TrendBucket::Month => start.date().replace_day(1).map(|d| d.midnight()),
    })
    .map_err(|e| e.to_string())?;

    let mut values: std::collections::BTreeMap<_, _> = points.into_iter().collect();
    let mut filled = Vec::new();

    while cursor < end {
        let key = match bucket {
            TrendBucket::Hour => format!("{} {:02}:00:00", cursor.date(), cursor.hour()),
            TrendBucket::Day | TrendBucket::Month => cursor.date().to_string(),
        };
        let value = match values.remove(&key) {
            Some(value) => value,
            None => None,
        };
        filled.push((key, value));

        let next = match bucket {
            TrendBucket::Hour => cursor.checked_add(time::Duration::hours(1)),
            TrendBucket::Day => cursor.checked_add(time::Duration::days(1)),
            TrendBucket::Month => {
                let month = cursor.month().next();
                let year = if cursor.month() == time::Month::December {
                    cursor.year() + 1
                } else {
                    cursor.year()
                };
                time::Date::from_calendar_date(year, month, 1)
                    .ok()
                    .map(|date| date.midnight())
            }
        };
        match next {
            Some(next) => {
                cursor = next;
            }
            None => {
                break;
            }
        }
    }

    Ok(filled)
}

fn dashboard_period_range(
    base_date: time::Date,
    period: &Option<Period>,
) -> Result<(time::Date, time::Date), String> {
    let out_of_range = || "Date is out of range".to_string();
    let start = match period {
        Some(Period::Daily) | None => base_date,
        Some(Period::Weekly) => base_date
            .checked_sub(time::Duration::days(i64::from(
                base_date.weekday().number_days_from_monday(),
            )))
            .ok_or_else(out_of_range)?,
        Some(Period::Monthly) => base_date.replace_day(1).map_err(|e| e.to_string())?,
        Some(Period::Yearly) => {
            time::Date::from_calendar_date(base_date.year(), time::Month::January, 1)
                .map_err(|e| e.to_string())?
        }
    };
    let end = match period {
        Some(Period::Daily) | None => start.next_day().ok_or_else(out_of_range)?,
        Some(Period::Weekly) => start
            .checked_add(time::Duration::days(7))
            .ok_or_else(out_of_range)?,
        Some(Period::Monthly) => {
            let next_year = if start.month() == time::Month::December {
                start.year() + 1
            } else {
                start.year()
            };
            time::Date::from_calendar_date(next_year, start.month().next(), 1)
                .map_err(|e| e.to_string())?
        }
        Some(Period::Yearly) => {
            time::Date::from_calendar_date(start.year() + 1, time::Month::January, 1)
                .map_err(|e| e.to_string())?
        }
    };
    Ok((start, end))
}

#[cfg(test)]
mod period_navigation_tests {
    use super::*;
    use time::macros::date;

    fn navigation(
        base: time::Date,
        period: &Option<Period>,
    ) -> (Option<time::Date>, Option<time::Date>) {
        let (start, end) = dashboard_period_range(base, period).unwrap();
        let previous = start
            .previous_day()
            .and_then(|d| dashboard_period_range(d, period).ok())
            .map(|(start, _)| start);
        let next = dashboard_period_range(end, period)
            .ok()
            .map(|(start, _)| start);
        (previous, next)
    }

    #[test]
    fn daily_navigation_crosses_leap_day_and_default_is_daily() {
        assert_eq!(
            navigation(date!(2024 - 03 - 01), &Some(Period::Daily)),
            (Some(date!(2024 - 02 - 29)), Some(date!(2024 - 03 - 02)),)
        );
        assert_eq!(
            dashboard_period_range(date!(2026 - 09 - 05), &None).unwrap(),
            (date!(2026 - 09 - 05), date!(2026 - 09 - 06),)
        );
    }

    #[test]
    fn weekly_navigation_uses_monday_and_crosses_year_boundary() {
        let period = Some(Period::Weekly);
        assert_eq!(
            dashboard_period_range(date!(2026 - 01 - 01), &period).unwrap(),
            (date!(2025 - 12 - 29), date!(2026 - 01 - 05),)
        );
        assert_eq!(
            navigation(date!(2026 - 01 - 01), &period),
            (Some(date!(2025 - 12 - 22)), Some(date!(2026 - 01 - 05)),)
        );
    }

    #[test]
    fn monthly_navigation_handles_31st_and_february() {
        let period = Some(Period::Monthly);
        assert_eq!(
            navigation(date!(2024 - 03 - 31), &period),
            (Some(date!(2024 - 02 - 01)), Some(date!(2024 - 04 - 01)),)
        );
        assert_eq!(
            dashboard_period_range(date!(2024 - 02 - 29), &period).unwrap(),
            (date!(2024 - 02 - 01), date!(2024 - 03 - 01),)
        );
        assert_eq!(
            navigation(date!(2026 - 12 - 31), &period).1,
            Some(date!(2027 - 01 - 01))
        );
    }

    #[test]
    fn yearly_navigation_handles_february_29() {
        assert_eq!(
            navigation(date!(2024 - 02 - 29), &Some(Period::Yearly)),
            (Some(date!(2023 - 01 - 01)), Some(date!(2025 - 01 - 01)),)
        );
    }

    #[test]
    fn navigation_disables_links_at_date_limits() {
        assert_eq!(navigation(time::Date::MIN, &Some(Period::Daily)).0, None);
        let before_max = time::Date::MAX.previous_day().unwrap();
        assert_eq!(navigation(before_max, &Some(Period::Daily)).1, None);
        assert!(dashboard_period_range(time::Date::MAX, &Some(Period::Daily)).is_err());
    }
}

#[cfg(test)]
mod trend_bucket_tests {
    use super::*;
    fn point(key: &str, value: Option<f64>) -> (String, Option<f64>) {
        (key.to_string(), value)
    }
    #[test]
    fn daily_fills_hours_and_preserves_zero_and_null() {
        let points = fill_trend_buckets(
            "2026-09-05 00:00:00",
            "2026-09-06 00:00:00",
            &TrendBucket::Hour,
            vec![
                point("2026-09-05 03:00:00", Some(0.0)),
                point("2026-09-05 08:00:00", Some(9.0)),
                point("2026-09-05 09:00:00", None),
            ],
        )
        .unwrap();
        assert_eq!(points.len(), 24);
        assert_eq!(points[0], point("2026-09-05 00:00:00", None));
        assert_eq!(points[3].1, Some(0.0));
        assert_eq!(points[8].1, Some(9.0));
        assert_eq!(points[9].1, None);
        assert_eq!(points[23].0, "2026-09-05 23:00:00");
    }
    #[test]
    fn yearly_fills_all_months_preserving_values() {
        let points = fill_trend_buckets(
            "2026-01-01 00:00:00",
            "2027-01-01 00:00:00",
            &TrendBucket::Month,
            vec![
                point("2026-09-01", Some(4.0)),
                point("2026-07-01", Some(7.0)),
                point("2026-08-01", Some(8.0)),
            ],
        )
        .unwrap();
        assert_eq!(points.len(), 12);
        assert_eq!(points[0], point("2026-01-01", None));
        assert_eq!(points[6].1, Some(7.0));
        assert_eq!(points[7].1, Some(8.0));
        assert_eq!(points[8].1, Some(4.0));
        assert_eq!(points[11], point("2026-12-01", None));
    }
    #[test]
    fn handles_leap_year_and_regular_february() {
        for (year, days) in [(2024, 29), (2026, 28)] {
            let p = fill_trend_buckets(
                &format!("{year}-02-01 00:00:00"),
                &format!("{year}-03-01 00:00:00"),
                &TrendBucket::Day,
                vec![],
            )
            .unwrap();
            assert_eq!(p.len(), days);
            assert!(p.iter().all(|(_, v)| v.is_none()));
        }
    }
    #[test]
    fn monthly_crosses_december_into_january() {
        let p = fill_trend_buckets(
            "2026-12-01 00:00:00",
            "2027-02-01 00:00:00",
            &TrendBucket::Month,
            vec![],
        )
        .unwrap();
        assert_eq!(
            p,
            vec![point("2026-12-01", None), point("2027-01-01", None)]
        );
    }
    #[test]
    fn floors_partial_start_and_excludes_end_boundary() {
        let p = fill_trend_buckets(
            "2026-09-05 03:30:00",
            "2026-09-05 05:00:00",
            &TrendBucket::Hour,
            vec![],
        )
        .unwrap();
        assert_eq!(
            p,
            vec![
                point("2026-09-05 03:00:00", None),
                point("2026-09-05 04:00:00", None)
            ]
        );
    }
    #[test]
    fn rejects_invalid_or_reversed_range() {
        assert!(
            fill_trend_buckets("bad", "2026-09-06 00:00:00", &TrendBucket::Hour, vec![]).is_err()
        );
        assert!(
            fill_trend_buckets(
                "2026-09-06 00:00:00",
                "2026-09-05 00:00:00",
                &TrendBucket::Hour,
                vec![]
            )
            .is_err()
        );
    }
}
