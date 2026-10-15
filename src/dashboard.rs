//! The dashboard presents a single worker-owned, read-only data snapshot.
use crate::{
    client::AppState,
    config::{KpiOperation, SignalType},
    dashboard_data::{DashboardRead, DashboardSnapshot, MetricData},
    dto::{DashboardQuery, Period},
    telemetry::TrendBucket,
};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use std::{collections::BTreeSet, fmt::Write};
use time::Date;
type WebError = (StatusCode, String);
struct Context {
    date: Date,
    start: Date,
    end: Date,
    period: Period,
    read: DashboardRead,
}
impl Context {
    async fn new(state: &AppState, q: &DashboardQuery) -> Result<Self, WebError> {
        let fallback = if q.base_date().is_none() {
            state
                .db_cli()
                .get_latest_time()
                .await
                .map_err(db_error)?
                .and_then(|s| s.get(..10).map(str::to_owned))
        } else {
            None
        };
        let text = q.base_date().as_deref().or(fallback.as_deref());
        let date = match text {
            Some(text) => Date::parse(
                text,
                time::macros::format_description!("[year]-[month]-[day]"),
            )
            .map_err(|_| {
                (
                    StatusCode::BAD_REQUEST,
                    "Choose a valid date (YYYY-MM-DD).".into(),
                )
            })?,
            None => time::OffsetDateTime::now_utc().date(),
        };
        let period = q.period().unwrap_or(Period::Monthly);
        let (start, end) = crate::api::dashboard_period_range(date, &Some(period))
            .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
        let previous = start
            .previous_day()
            .and_then(|day| crate::api::dashboard_period_range(day, &Some(period)).ok());
        let bucket = match period {
            Period::Daily => TrendBucket::Hour,
            Period::Weekly | Period::Monthly => TrendBucket::Day,
            Period::Yearly => TrendBucket::Month,
        };
        Ok(Self {
            date,
            start,
            end,
            period,
            read: DashboardRead {
                start: format!("{start} 00:00:00"),
                end: format!("{end} 00:00:00"),
                previous_start: previous
                    .map(|v| format!("{} 00:00:00", v.0))
                    .unwrap_or_default(),
                previous_end: previous
                    .map(|v| format!("{} 00:00:00", v.1))
                    .unwrap_or_default(),
                bucket,
                group: q.group().map(str::to_owned),
            },
        })
    }
    fn query(&self, date: Date, period: Period) -> String {
        serde_urlencoded::to_string([
            ("base_date", date.to_string()),
            ("period", period_name(period).to_lowercase()),
            ("group", self.read.group.clone().unwrap_or_default()),
        ])
        .expect("string URL fields")
    }
    fn range_title(&self) -> String {
        const MONTHS: [&str; 12] = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        match self.period {
            Period::Monthly => format!(
                "{} {}",
                MONTHS[self.start.month() as usize - 1],
                self.start.year()
            ),
            Period::Yearly => self.start.year().to_string(),
            Period::Daily => format!(
                "{} {} {}",
                self.start.day(),
                MONTHS[self.start.month() as usize - 1],
                self.start.year()
            ),
            Period::Weekly => format!(
                "{} – {}",
                self.start,
                self.end.previous_day().unwrap_or(self.start)
            ),
        }
    }
}
fn db_error(e: crate::error::DbError) -> WebError {
    (e.status_code(), e.to_string())
}
fn period_name(p: Period) -> &'static str {
    match p {
        Period::Daily => "Daily",
        Period::Weekly => "Weekly",
        Period::Monthly => "Monthly",
        Period::Yearly => "Yearly",
    }
}
pub(crate) async fn page(
    State(state): State<AppState>,
    Query(q): Query<DashboardQuery>,
) -> Result<Response, WebError> {
    let Some(config) = state.db_cli().get_app_config().await.map_err(db_error)? else {
        return Ok(Redirect::to("/setup").into_response());
    };
    let context = Context::new(&state, &q).await?;
    let snapshot = state
        .db_cli()
        .read_dashboard(context.read.clone())
        .await
        .map_err(db_error)?;
    let organization = if config.dashboard_title().is_empty() {
        "Industrial telemetry"
    } else {
        config.dashboard_title()
    };
    let title = if config.dashboard_subtitle().is_empty() {
        "Telemetry overview"
    } else {
        config.dashboard_subtitle()
    };
    let mut tabs = String::new();
    for period in [
        Period::Daily,
        Period::Weekly,
        Period::Monthly,
        Period::Yearly,
    ] {
        write!(
            tabs,
            "<a href=\"/dashboard?{}\" {}>{}</a>",
            escape(&context.query(context.date, period)),
            if std::mem::discriminant(&period) == std::mem::discriminant(&context.period) {
                "aria-current=\"page\""
            } else {
                ""
            },
            period_name(period)
        )
        .unwrap();
    }
    let mut options = "<option value=\"\">All groups</option>".to_string();
    for group in &snapshot.groups {
        write!(
            options,
            "<option value=\"{}\" {}>{}</option>",
            escape(group),
            if Some(group) == snapshot.selected_group.as_ref() {
                "selected"
            } else {
                ""
            },
            escape(group)
        )
        .unwrap();
    }
    let previous = context
        .start
        .previous_day()
        .and_then(|day| crate::api::dashboard_period_range(day, &Some(context.period)).ok())
        .map(|v| v.0);
    let navigation = |date: Option<Date>, symbol: &str, label: &str| -> String {
        match date {
            Some(date) => format!(
                "<a class=\"range-arrow\" aria-label=\"{label}\" href=\"/dashboard?{}\">{symbol}</a>",
                escape(&context.query(date, context.period))
            ),
            None => format!("<span class=\"range-arrow\" aria-disabled=\"true\">{symbol}</span>"),
        }
    };
    let contents = render_snapshot(&snapshot, &context);
    let fragment_url = escape(&format!(
        "/dashboard/data?{}",
        context.query(context.date, context.period)
    ));
    let html = format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>{title} · Telemetry</title><link rel="stylesheet" href="/static/dashboard.css?v=8"><script src="/static/htmx.min.js" defer></script><script src="/static/dashboard.js?v=7" defer></script><script type="module" src="/static/chart-interactions.js?v=1"></script><script type="module" src="/static/dashboard-export.js?v=2"></script></head>
<body><main class="dashboard-shell" data-report-period="{period}" data-report-start="{start}" data-report-group="{report_group}"><header class="dashboard-header"><div><p class="organization">{organization}</p><h1>{title}</h1></div><div class="header-actions"><a class="configure-link" href="/setup">Configure <span aria-hidden="true">↗</span></a><nav class="period-nav" aria-label="Period">{tabs}</nav></div></header>
<div class="dashboard-toolbar"><form class="scope-form" action="/dashboard" method="get"><input type="hidden" name="period" value="{period}"><label>View<select name="group" aria-label="Group">{options}</select></label><label>Date<input type="date" name="base_date" value="{date}" required></label><button type="submit" class="apply-button">Apply</button></form><div class="range-nav">{previous}<span class="range-title">{range_title}</span>{next}</div><div class="dashboard-actions"><button type="button" id="refresh-dashboard" class="refresh-button"><span aria-hidden="true">↻</span> Refresh</button><details class="export-menu" id="export-menu"><summary>↓ Export</summary><div class="export-options"><button type="button" data-export="png">Image (.png)</button><button type="button" data-export="pdf">PDF (.pdf)</button></div></details></div></div><p id="export-status" role="status" aria-live="polite" hidden></p>
<div id="dashboard-data" hx-get="{fragment_url}" hx-trigger="every 60s, refresh from:body" hx-swap="innerHTML" aria-live="polite" aria-atomic="false">{contents}</div>
<footer class="dashboard-footer"><div><span class="live-dot" aria-hidden="true"></span><span id="refresh-status" role="status">Loaded · updates every 60 seconds</span></div><span>Source: {table} · source time as recorded</span></footer></main></body></html>"#,
        title = escape(title),
        organization = escape(organization),
        period = period_name(context.period).to_lowercase(),
        date = context.date,
        start = context.start,
        report_group = escape(context.read.group.as_deref().unwrap_or("all-groups")),
        previous = navigation(previous, "‹", "Previous period"),
        next = navigation(Some(context.end), "›", "Next period"),
        range_title = context.range_title(),
        table = escape(
            config
                .source()
                .as_ref()
                .map(|s| s.table_name())
                .unwrap_or("")
        )
    );
    Ok(Html(html).into_response())
}
pub(crate) async fn fragment(
    State(state): State<AppState>,
    Query(q): Query<DashboardQuery>,
) -> Result<Html<String>, WebError> {
    let context = Context::new(&state, &q).await?;
    let snapshot = state
        .db_cli()
        .read_dashboard(context.read.clone())
        .await
        .map_err(db_error)?;
    Ok(Html(render_snapshot(&snapshot, &context)))
}
pub(crate) async fn data(
    State(state): State<AppState>,
    Query(q): Query<DashboardQuery>,
) -> Result<Json<DashboardSnapshot>, WebError> {
    let context = Context::new(&state, &q).await?;
    Ok(Json(
        state
            .db_cli()
            .read_dashboard(context.read)
            .await
            .map_err(db_error)?,
    ))
}
fn render_snapshot(snapshot: &DashboardSnapshot, ctx: &Context) -> String {
    let config = &snapshot.configuration;
    let metric = |label: &str| snapshot.metrics.iter().find(|m| m.label == label);
    let mut cards = String::new();
    let mut seen = BTreeSet::new();
    // Consumption takes the first, wider cell, as in the supplied design.
    if let Some(label) = config.group_summary_label().as_deref() {
        if let Some(data) = metric(label) {
            let title = if data.kind == SignalType::Counter {
                "Total consumption".into()
            } else {
                format!("Average {}", label.to_lowercase())
            };
            cards.push_str(&kpi(data, &title, false, true));
            seen.insert((label.to_string(), false));
        }
    }
    for widget in config.kpis() {
        let peak = matches!(widget.operation(), KpiOperation::Peak);
        if !seen.insert((widget.label().to_string(), peak)) {
            continue;
        }
        if let Some(data) = metric(widget.label()) {
            let fallback = if peak {
                format!("Peak {}", data.label.to_lowercase())
            } else if data.kind == SignalType::Counter {
                format!("{} consumption", data.label)
            } else {
                format!("Average {}", data.label.to_lowercase())
            };
            cards.push_str(&kpi(data, widget.title().unwrap_or(&fallback), peak, false));
        }
    }
    let scope = snapshot
        .selected_group
        .as_deref()
        .map(escape)
        .unwrap_or_else(|| format!("All groups · {} meters", snapshot.groups.len()));
    let mut out = format!(
        "<div class=\"snapshot-meta\"><span>{scope}</span><span>Latest source reading: <strong>{}</strong></span></div>",
        escape(
            snapshot
                .latest_time
                .as_deref()
                .map(|s| s.get(..19).unwrap_or(s))
                .unwrap_or("No readings")
        )
    );
    if cards.is_empty() {
        out.push_str("<section class=\"empty-state panel\"><h2>Your data source is ready</h2><p>Choose KPIs and trends in configuration to build your dashboard.</p><a href=\"/setup\">Configure widgets →</a></section>");
    } else {
        write!(out,"<section class=\"kpi-strip\" aria-label=\"Key performance indicators\">{cards}</section>").unwrap();
    }
    let mut trend_labels = BTreeSet::new();
    let trends = config
        .trends()
        .iter()
        .filter(|t| trend_labels.insert(t.label()))
        .collect::<Vec<_>>();
    if let Some(main) = trends
        .first()
        .and_then(|t| metric(t.label()).map(|m| (*t, m)))
    {
        let title = main.0.title().map(str::to_owned).unwrap_or_else(|| {
            if main.1.kind == SignalType::Counter {
                "Consumption trend".into()
            } else {
                format!("{} trend", main.1.label)
            }
        });
        out.push_str("<section class=\"chart-row\">");
        write!(out,"<article class=\"panel main-chart\"><div class=\"panel-heading\"><div><h2>{}</h2><p>{} · {}</p></div><span class=\"unit-badge\">{}</span></div><div class=\"chart-legend\"><span><i class=\"legend-bar\"></i>{}</span><span><i class=\"legend-peak\"></i>Highest bucket</span><span><i class=\"legend-dash\"></i>Bucket average</span></div><div class=\"plot-scroll\">{}</div><p class=\"mobile-chart-hint\">{axis_caption} · Swipe the chart to see all buckets →</p>{}<p class=\"chart-caption\">{} through {} · {} buckets</p></article>",escape(&title),scope,ctx.range_title(),escape(&main.1.unit),if main.1.kind==SignalType::Counter{"Consumption"}else{"Average"},crate::charts::chart(main.1,&ctx.read.bucket,true,false),data_table(main.1),ctx.start,ctx.end.previous_day().unwrap_or(ctx.start),match ctx.read.bucket {TrendBucket::Hour=>"Hourly",TrendBucket::Day=>"Daily",TrendBucket::Month=>"Monthly"},axis_caption=match ctx.read.bucket {TrendBucket::Hour=>"Time of day (hour)",TrendBucket::Day=>"Date",TrendBucket::Month=>"Month"}).unwrap();
        if let Some(ranking) = &snapshot.ranking {
            let total: Option<f64> = ranking.rows.iter().filter_map(|r| r.1).reduce(|a, b| a + b);
            let denominator = total.unwrap_or(0.);
            write!(out,"<article class=\"panel ranking-panel\"><div class=\"panel-heading\"><div><h2>{} by group</h2><p>{}</p></div><span class=\"unit-badge\">{}</span></div><div class=\"ranking-total\"><span>{}</span><strong>{}</strong></div><div class=\"ranking-axes\"><span>Group</span><span>Value ({}) · Share (%)</span></div><div class=\"ranking-list\">",escape(&ranking.label),if ranking.counter{"Consumption share in selected period"}else{"Period average per group"},escape(&ranking.unit),if ranking.counter{"Total"}else{"Combined average"},number(total),escape(&ranking.unit)).unwrap();
            for (index, (group, value)) in ranking.rows.iter().enumerate() {
                let share = value
                    .filter(|_| denominator > 0.)
                    .map(|v| v / denominator * 100.);
                let selected = if snapshot.selected_group.as_deref() == Some(group) {
                    " selected"
                } else {
                    ""
                };
                let url = serde_urlencoded::to_string([
                    ("base_date", ctx.date.to_string()),
                    ("period", period_name(ctx.period).to_lowercase()),
                    ("group", group.clone()),
                ])
                .unwrap();
                write!(out,"<a class=\"ranking-item{selected}\" href=\"/dashboard?{}\" aria-label=\"View {}\"><span class=\"rank-number\">{:02}</span><span class=\"rank-name\">{}</span><span class=\"rank-value\">{}<small>{}</small></span><span class=\"rank-track\"><span style=\"width:{:.3}%\"></span></span></a>",escape(&url),escape(group),index+1,escape(group),number(*value),share.map(|v|format!("{v:.1}%")).unwrap_or_else(||"—".into()),share.unwrap_or(0.).clamp(0.,100.)).unwrap();
            }
            out.push_str("</div><p class=\"chart-caption\">Bar length: share of total (%) · select a group to explore</p></article>");
        }
        out.push_str("</section>");
    } else if let Some(ranking) = &snapshot.ranking {
        // A ranking-only configuration remains useful without requiring a trend widget.
        out.push_str("<section class=\"panel\"><h2>By group</h2><table class=\"data-table\"><thead><tr><th>Group</th><th>Value</th></tr></thead><tbody>");
        for (group, value) in &ranking.rows {
            write!(
                out,
                "<tr><td>{}</td><td>{} {}</td></tr>",
                escape(group),
                number(*value),
                escape(&ranking.unit)
            )
            .unwrap();
        }
        out.push_str("</tbody></table></section>");
    }
    if trends.len() > 1 {
        out.push_str("<section class=\"detail-charts\" aria-label=\"Signal trends\">");
        for (index, widget) in trends.iter().skip(1).enumerate() {
            if let Some(data) = metric(widget.label()) {
                let title = widget
                    .title()
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("{} trend", data.label));
                let description = if data.kind == SignalType::Counter {
                    "Counter consumption"
                } else {
                    match data.aggregation {
                        crate::config::Aggregation::Sum => "Sum across meters",
                        crate::config::Aggregation::Mean => "Mean across meters",
                    }
                };
                write!(out, r#"<article class="panel detail-chart">
                    <div class="panel-heading"><h2>{title}</h2><span class="unit-badge">{unit}</span></div>
                    <p class="detail-description">{scope} · {description}</p>
                    <div class="detail-stats"><span>{value_label} <strong>{value}</strong></span><span>Range <strong>{low} – {high}</strong></span></div>
                    {chart}<p class="detail-footnote">Line: bucket average · band: reading range</p>{table}</article>"#,
                    title=escape(&title), unit=escape(&data.unit), scope=if data.global { "Site-wide" } else { &scope },
                    value_label=if data.kind==SignalType::Counter {"Consumption"}else{"Average"},
                    value=number(data.summary), low=number(data.minimum), high=number(data.maximum),
                    chart=crate::charts::chart(data,&ctx.read.bucket,false,index%3==2),table=data_table(data)).unwrap();
            }
        }
        out.push_str("</section>");
    }
    if snapshot.metrics.iter().any(|m| m.partial)
        && snapshot.metrics.iter().any(|m| m.summary.is_some())
    {
        out.push_str("<p class=\"data-note\">Some meters have missing readings. Available values are shown; gaps are kept in charts.</p>");
    }
    out
}
fn kpi(data: &MetricData, title: &str, peak: bool, featured: bool) -> String {
    let value = if peak {
        data.peak.as_ref().map(|v| v.1)
    } else {
        data.summary
    };
    let caption = if peak {
        data.peak
            .as_ref()
            .map(|v| format!("Recorded {}", escape(v.0.get(..19).unwrap_or(&v.0))))
            .unwrap_or_else(|| "No readings in this period".into())
    } else if featured && data.kind == SignalType::Counter {
        match (data.summary, data.previous) {
            (Some(current), Some(previous)) if previous > 0. => {
                let change = (current - previous) / previous * 100.;
                let change = if change.abs() < 0.05 { 0.0 } else { change };
                format!(
                    "<span class=\"comparison\">{}{:.1}%</span> vs previous period",
                    if change >= 0. { "+" } else { "" },
                    change
                )
            }
            _ => "No previous-period comparison".into(),
        }
    } else if data.global {
        "Site-wide measurement".into()
    } else {
        format!(
            "{} · {} meter{}",
            if data.aggregation == crate::config::Aggregation::Sum {
                "Combined"
            } else {
                "Mean"
            },
            data.group_count,
            if data.group_count == 1 { "" } else { "s" }
        )
    };
    format!(
        "<article class=\"kpi{}\"><p class=\"kpi-label\">{}</p><p class=\"kpi-reading\"><strong>{}</strong><span>{}</span></p><p class=\"kpi-caption\">{caption}</p></article>",
        if featured { " featured" } else { "" },
        escape(title),
        number(value),
        escape(&data.unit)
    )
}
fn data_table(data: &MetricData) -> String {
    let mut out = format!(
        "<details class=\"chart-data\" data-key=\"{}\"><summary>View data</summary><div class=\"table-scroll\"><table class=\"data-table\"><thead><tr><th>Bucket start</th><th>Value ({})</th></tr></thead><tbody>",
        escape(&data.label),
        escape(&data.unit)
    );
    for point in &data.points {
        write!(
            out,
            "<tr><td>{}</td><td>{}</td></tr>",
            escape(&point.time),
            number(point.value)
        )
        .unwrap();
    }
    out.push_str("</tbody></table></div></details>");
    out
}
pub(crate) fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
pub(crate) fn number(value: Option<f64>) -> String {
    let Some(value) = value.filter(|v| v.is_finite()) else {
        return "—".into();
    };
    let fixed = format!("{:.1}", value.abs());
    let (whole, fraction) = fixed.split_once('.').unwrap();
    let mut result = String::new();
    if value < 0. {
        result.push('-');
    }
    for (i, ch) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    format!("{result}.{fraction}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formats_unknown_zero_negative_and_large_values() {
        assert_eq!(number(None), "—");
        assert_eq!(number(Some(0.)), "0.0");
        assert_eq!(number(Some(-12345.67)), "-12,345.7");
        assert_eq!(number(Some(f64::INFINITY)), "—");
    }
    #[test]
    fn escapes_names_in_html_attributes_and_text() {
        assert_eq!(escape("\"<meter>&'"), "&quot;&lt;meter&gt;&amp;&#39;");
    }
}
