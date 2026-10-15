use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};

use crate::{
    client::AppState,
    config::{AppConfig, KpiOperation, SourceConfig},
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
    let result = app_state.db_cli().configure_source(req.clone()).await;
    match result {
        Ok(schema) => {
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
            println!("missing date. fallback to '2026-08-03'");
            "2026-08-03"
        }
    };

    let format = time::macros::format_description!("[year]-[month]-[day]");

    let parsed_base_date = match time::Date::parse(base_date, format) {
        Ok(date) => date,
        Err(e) => {
            return Err((StatusCode::BAD_REQUEST, e.to_string()));
        }
    };

    let (start, end) = match q.period() {
        Some(Period::Daily) | None => {
            let end_date = match parsed_base_date.next_day() {
                Some(date) => date,
                None => {
                    return Err((StatusCode::BAD_REQUEST, "Date is out of range".to_string()));
                }
            };
            let start_datetime = format!("{parsed_base_date} 00:00:00");
            let end_datetime = format!("{end_date} 00:00:00");

            (
                start_datetime.replace(' ', "%20"),
                end_datetime.replace(' ', "%20"),
            )
        }
        Some(Period::Weekly) => {
            let days_from_monday = parsed_base_date.weekday().number_days_from_monday();
            let days_to_subtract = time::Duration::days(i64::from(days_from_monday));
            let start_date = match parsed_base_date.checked_sub(days_to_subtract) {
                Some(date) => date,
                None => {
                    return Err((StatusCode::BAD_REQUEST, "Date is out of range".to_string()));
                }
            };
            let end_date = match start_date.checked_add(time::Duration::days(7)) {
                Some(date) => date,
                None => {
                    return Err((StatusCode::BAD_REQUEST, "Date is out of range".to_string()));
                }
            };
            let start_datetime = format!("{start_date} 00:00:00");
            let end_datetime = format!("{end_date} 00:00:00");
            (
                start_datetime.replace(' ', "%20"),
                end_datetime.replace(' ', "%20"),
            )
        }
        Some(Period::Monthly) => {
            let year = parsed_base_date.year();
            let month = parsed_base_date.month();
            let start_date = match time::Date::from_calendar_date(year, month, 1) {
                Ok(date) => date,
                Err(e) => {
                    return Err((StatusCode::BAD_REQUEST, e.to_string()));
                }
            };
            let next_month = month.next();
            let next_year = if month == time::Month::December {
                year + 1
            } else {
                year
            };

            let end_date = match time::Date::from_calendar_date(next_year, next_month, 1) {
                Ok(date) => date,
                Err(e) => {
                    return Err((StatusCode::BAD_REQUEST, e.to_string()));
                }
            };
            let start_datetime = format!("{start_date} 00:00:00");
            let end_datetime = format!("{end_date} 00:00:00");

            (
                start_datetime.replace(' ', "%20"),
                end_datetime.replace(' ', "%20"),
            )
        }
        Some(Period::Yearly) => {
            let year = parsed_base_date.year();
            let start_date = match time::Date::from_calendar_date(year, time::Month::January, 1) {
                Ok(date) => date,
                Err(e) => {
                    return Err((StatusCode::BAD_REQUEST, e.to_string()));
                }
            };
            let end_date = match time::Date::from_calendar_date(year + 1, time::Month::January, 1) {
                Ok(date) => date,
                Err(e) => {
                    return Err((StatusCode::BAD_REQUEST, e.to_string()));
                }
            };
            let start_datetime = format!("{start_date} 00:00:00");
            let end_datetime = format!("{end_date} 00:00:00");

            (
                start_datetime.replace(' ', "%20"),
                end_datetime.replace(' ', "%20"),
            )
        }
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
                   <link rel="stylesheet" href="/static/dashboard.css">
                   <script src="/static/htmx.min.js" defer></script>
                   <script src="/static/dashboard.js" defer></script>
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
                <p><strong>Total:</strong> {total:.1} {unit}</p>"#
            );
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
                                r#"<div class="row">
                                    <span class="group">{group}</span>
                                    <span class="value">No data</span>
                                </div>"#
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
                    {v:.1} <span class="kpi-unit">{unit}</span>
                </div>
                <div class="kpi-label">{label}</div>
            </div>"#
        ),
        Ok(None) => format!(
            r#"<div class="kpi">
                <div class="kpi-value">No data</div>
                <div class="kpi-label">{label}</div>
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
                {v:.1} <span class="kpi-unit">{unit}</span>
            </div>
            <div class="kpi-label">Peak {label}</div>
        </div>"#
        ),
        Ok(None) => format!(
            r#"<div class="kpi">
            <div class="kpi-value">No data</div>
            <div class="kpi-label">Peak {label}</div>
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
                {total:.1} <span class="kpi-unit">{unit}</span>
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
            let has_values = points.iter().any(|(_, value)| value.is_some());

            if !has_values {
                return axum::response::Html(format!(
                    r#"<h3>{group} / {label}</h3>
                        <p>Unit: {unit}</p>
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
                .map(|(day, value)| match value {
                    Some(v) => {
                        let value_pct = ((v - axis_min) / axis_range) * 100.0;
                        let bottom_pct = value_pct.min(zero_pct);
                        let height_pct = (value_pct - zero_pct).abs();

                        format!(
                            r#"<div class="trend-slot" title="{day}: {v:.1}">
                        <div class="trend-bar"
                             style="bottom: {bottom_pct:.4}%; height: {height_pct:.4}%">
                        </div>
                    </div>"#
                        )
                    }
                    None => format!(r#"<div class="trend-slot" title="{day}: no data"></div>"#),
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
        <p>Unit: {unit}</p>

        <div class="trend-plot">
            <div class="trend-axis">
                <span>{axis_max:.1}</span>
                <span>{axis_mid:.1}</span>
                <span>{axis_min:.1}</span>
            </div>

            <div class="trend-chart">
                <div class="trend-zero"
                     style="bottom: {zero_pct:.4}%">
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
                    <th scope="col">Value {unit}</th>
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
