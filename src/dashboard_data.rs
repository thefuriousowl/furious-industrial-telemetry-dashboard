//! One dashboard refresh reads one coherent SQLite snapshot on the DB worker.
use crate::{
    config::{Aggregation, DashboardConfig, SignalType},
    db::DbReader,
    error::DbError,
    schema::RuntimeSchema,
    telemetry::{MetricPoint, TrendBucket},
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
pub(crate) struct DashboardRead {
    pub start: String,
    pub end: String,
    pub previous_start: String,
    pub previous_end: String,
    pub bucket: TrendBucket,
    pub group: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct DashboardSnapshot {
    pub latest_time: Option<String>,
    pub groups: Vec<String>,
    pub selected_group: Option<String>,
    pub metrics: Vec<MetricData>,
    pub ranking: Option<RankingData>,
    pub configuration: DashboardConfig,
}

#[derive(Serialize)]
pub(crate) struct MetricData {
    pub label: String,
    pub unit: String,
    pub kind: SignalType,
    pub aggregation: Aggregation,
    pub global: bool,
    pub group_count: usize,
    pub partial: bool,
    pub summary: Option<f64>,
    pub previous: Option<f64>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub peak: Option<(String, f64)>,
    pub points: Vec<MetricPoint>,
}

#[derive(Serialize)]
pub(crate) struct RankingData {
    pub label: String,
    pub unit: String,
    pub counter: bool,
    pub rows: Vec<(String, Option<f64>)>,
}

pub(crate) fn groups(schema: &RuntimeSchema, config: &DashboardConfig) -> Vec<String> {
    schema
        .signals()
        .iter()
        .filter(|signal| !config.is_global(signal.label()))
        .map(|signal| signal.group().to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn read_dashboard(
    reader: &DbReader,
    schema: &RuntimeSchema,
    config: &DashboardConfig,
    query: &DashboardRead,
) -> Result<DashboardSnapshot, DbError> {
    reader.with_read_snapshot(|reader| read_snapshot(reader, schema, config, query))
}

fn read_snapshot(
    reader: &DbReader,
    schema: &RuntimeSchema,
    config: &DashboardConfig,
    query: &DashboardRead,
) -> Result<DashboardSnapshot, DbError> {
    // The HMI owns the schema and may replace its database while we run.
    // Validate cached identifiers so SQLite cannot treat a missing quoted column as text.
    let columns = reader.get_columns_metadata(schema.table_name())?;
    let missing = std::iter::once(schema.time_column_name())
        .chain(schema.signals().iter().map(|signal| signal.column()))
        .filter(|name| !columns.iter().any(|column| column.name() == *name))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(DbError::ColumnNotFound(missing));
    }
    let available_groups = groups(schema, config);
    if let Some(group) = &query.group {
        if !available_groups.contains(group) {
            return Err(DbError::InvalidConfig(format!("Unknown group: {group}")));
        }
    }
    let labels = config
        .kpis()
        .iter()
        .map(|widget| widget.label())
        .chain(config.trends().iter().map(|widget| widget.label()))
        .chain(config.group_summary_label().iter().map(String::as_str))
        .collect::<BTreeSet<_>>();
    let mut metrics = Vec::new();
    let mut ranking = None;
    for label in labels {
        let all = schema
            .signals()
            .iter()
            .filter(|signal| signal.label() == label)
            .collect::<Vec<_>>();
        let first = all
            .first()
            .ok_or_else(|| DbError::InvalidConfig(format!("No configured signals for {label}")))?;
        if all.iter().any(|signal| {
            signal.unit() != first.unit() || signal.signal_type() != first.signal_type()
        }) {
            return Err(DbError::InvalidConfig(format!(
                "Mixed units or types for {label}"
            )));
        }
        let global = config.is_global(label);
        let selected = all
            .iter()
            .copied()
            .filter(|signal| {
                global
                    || query
                        .group
                        .as_ref()
                        .is_none_or(|group| signal.group() == group)
            })
            .collect::<Vec<_>>();
        let kind = first.signal_type().clone();
        let aggregation = if kind == SignalType::Counter {
            Aggregation::Sum
        } else {
            config.aggregation(label)
        };
        let mut metric = MetricData {
            label: label.into(),
            unit: first.unit().into(),
            kind: kind.clone(),
            aggregation,
            global,
            group_count: selected.len(),
            partial: false,
            summary: None,
            previous: None,
            minimum: None,
            maximum: None,
            peak: None,
            points: Vec::new(),
        };
        let is_ranking = config.group_summary_label().as_deref() == Some(label);
        let mut group_values = Vec::new();
        if kind == SignalType::Instantaneous {
            let columns = selected
                .iter()
                .map(|signal| signal.column().to_string())
                .collect::<Vec<_>>();
            let values = reader.read_instantaneous(
                schema.table_name(),
                schema.time_column_name(),
                &columns,
                aggregation,
                &query.start,
                &query.end,
                &query.bucket,
            )?;
            metric.summary = values.average;
            metric.minimum = values.minimum;
            metric.maximum = values.maximum;
            metric.peak = values.peak;
            metric.partial = values.partial;
            metric.points = values.points;
            if is_ranking {
                for signal in &selected {
                    group_values.push((
                        signal.group().to_string(),
                        reader.get_signal_summary(
                            schema.table_name(),
                            schema.time_column_name(),
                            signal.column(),
                            signal.signal_type(),
                            &query.start,
                            &query.end,
                        )?,
                    ));
                }
            }
        } else {
            let mut buckets: BTreeMap<String, Option<f64>> = BTreeMap::new();
            for signal in &selected {
                let value = reader.get_signal_summary(
                    schema.table_name(),
                    schema.time_column_name(),
                    signal.column(),
                    signal.signal_type(),
                    &query.start,
                    &query.end,
                )?;
                metric.partial |= value.is_none();
                metric.summary = add_optional(metric.summary, value);
                group_values.push((signal.group().to_string(), value));
                if !query.previous_start.is_empty() {
                    let previous = reader.get_signal_summary(
                        schema.table_name(),
                        schema.time_column_name(),
                        signal.column(),
                        signal.signal_type(),
                        &query.previous_start,
                        &query.previous_end,
                    )?;
                    metric.previous = add_optional(metric.previous, previous);
                }
                for (time, value) in reader.get_signal_trend(
                    schema.table_name(),
                    schema.time_column_name(),
                    signal.column(),
                    signal.signal_type(),
                    &query.bucket,
                    &query.start,
                    &query.end,
                )? {
                    let current = buckets.entry(time).or_default();
                    *current = add_optional(*current, value);
                }
            }
            metric.points = buckets
                .into_iter()
                .map(|(time, value)| MetricPoint {
                    time,
                    value,
                    minimum: value,
                    maximum: value,
                })
                .collect();
        }
        if kind == SignalType::Counter {
            metric.minimum = metric
                .points
                .iter()
                .filter_map(|point| point.value)
                .reduce(f64::min);
            metric.maximum = metric
                .points
                .iter()
                .filter_map(|point| point.value)
                .reduce(f64::max);
        }
        // Fill missing slots without turning absent readings into zeroes.
        let raw = metric
            .points
            .iter()
            .map(|point| (point.time.clone(), point.value))
            .collect();
        let filled = crate::api::fill_trend_buckets(&query.start, &query.end, &query.bucket, raw)
            .map_err(DbError::InvalidConfig)?;
        let mut raw_points = metric
            .points
            .into_iter()
            .map(|point| (point.time.clone(), point))
            .collect::<BTreeMap<_, _>>();
        metric.points = filled
            .into_iter()
            .map(|(time, value)| {
                raw_points.remove(&time).unwrap_or(MetricPoint {
                    time,
                    value,
                    minimum: None,
                    maximum: None,
                })
            })
            .collect();
        if is_ranking {
            group_values.sort_by(|a, b| {
                b.1.unwrap_or(f64::NEG_INFINITY)
                    .total_cmp(&a.1.unwrap_or(f64::NEG_INFINITY))
                    .then_with(|| a.0.cmp(&b.0))
            });
            ranking = Some(RankingData {
                label: label.into(),
                unit: metric.unit.clone(),
                counter: kind == SignalType::Counter,
                rows: group_values,
            });
        }
        metrics.push(metric);
    }
    Ok(DashboardSnapshot {
        latest_time: reader.get_latest_time(schema.table_name(), schema.time_column_name())?,
        groups: available_groups,
        selected_group: query.group.clone(),
        metrics,
        ranking,
        configuration: config.clone(),
    })
}

fn add_optional(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left + right),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}
