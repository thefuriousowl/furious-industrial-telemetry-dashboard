//! Small, accessible SVG charts rendered from real bucketed readings.
use crate::{
    config::SignalType,
    dashboard::{escape, number},
    dashboard_data::MetricData,
    telemetry::TrendBucket,
};
use std::fmt::Write;

pub(crate) fn chart(metric: &MetricData, bucket: &TrendBucket, bars: bool, orange: bool) -> String {
    let (width, height, left, top, bottom) = if bars {
        (900., 440., 60., 42., 58.)
    } else {
        (430., 220., 48., 38., 54.)
    };
    let right = if bars { 18. } else { 12. };
    let plot_w = width - left - right;
    let plot_h = height - top - bottom;
    let values: Vec<f64> = metric
        .points
        .iter()
        .flat_map(|p| [p.value, p.minimum, p.maximum])
        .flatten()
        .filter(|v| v.is_finite())
        .collect();
    let mut out = format!(
        "<svg class=\"telemetry-chart {}\" viewBox=\"0 0 {width} {height}\" role=\"{}\" {} data-chart=\"{}\" data-label=\"{}\" data-unit=\"{}\" data-kind=\"{}\" data-bucket=\"{}\" aria-label=\"{} {} chart, {}\"><title>{} — {}</title><desc>Hover or tap to inspect a bucket. With keyboard focus, use Left and Right arrows, Home and End. Escape dismisses details.</desc>",
        if orange { "orange" } else { "blue" },
        if values.is_empty() { "img" } else { "group" },
        if values.is_empty() {
            ""
        } else {
            "tabindex=\"0\""
        },
        if bars { "bar" } else { "trend" },
        escape(&metric.label),
        escape(&metric.unit),
        if metric.kind == SignalType::Counter {
            "counter"
        } else {
            "instantaneous"
        },
        match bucket {
            TrendBucket::Hour => "hour",
            TrendBucket::Day => "day",
            TrendBucket::Month => "month",
        },
        escape(&metric.label),
        if bars && metric.kind == SignalType::Counter {
            "consumption"
        } else {
            "trend"
        },
        escape(&metric.unit),
        escape(&metric.label),
        escape(&metric.unit)
    );
    let y_title = y_axis_title(metric);
    let x_title = match bucket {
        TrendBucket::Hour => "Time of day (hour)",
        TrendBucket::Day => "Date",
        TrendBucket::Month => "Month",
    };
    write!(out, "<text class=\"axis-title y-axis-title\" x=\"{left}\" y=\"18\">{}</text><text class=\"axis-title x-axis-title\" x=\"{}\" y=\"{}\" text-anchor=\"middle\">{x_title}</text>",escape(&y_title),left+plot_w/2.,height-7.).unwrap();
    if values.is_empty() {
        write!(out, "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" class=\"empty-chart\">No readings in this period</text></svg>", width / 2., height / 2.).unwrap();
        return out;
    }
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let padding = ((max - min) * 0.16).max(max.abs() * 0.035).max(0.1);
    let lo = if bars { min.min(0.) } else { min - padding };
    let hi = if bars {
        max.max(0.) + padding
    } else {
        max + padding
    };
    let y = |value: f64| top + (hi - value) / (hi - lo) * plot_h;
    let n = metric.points.len().max(1);
    let x = |index: usize| left + (index as f64 + 0.5) * plot_w / n as f64;
    for i in 0..=4 {
        let value = lo + (hi - lo) * i as f64 / 4.;
        let py = y(value);
        write!(out, "<line class=\"grid-line\" x1=\"{left}\" x2=\"{}\" y1=\"{py:.2}\" y2=\"{py:.2}\"/><text class=\"axis-label\" x=\"{}\" y=\"{:.2}\" text-anchor=\"end\">{}</text>", width-right, left-10., py+4., axis_number(value)).unwrap();
    }
    if bars {
        let peak = metric
            .points
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.value.map(|v| (i, v)))
            .max_by(|a, b| a.1.total_cmp(&b.1));
        let bar_w = (plot_w / n as f64 * 0.65).max(1.);
        for (i, point) in metric.points.iter().enumerate() {
            if let Some(value) = point.value {
                let py = y(value).min(y(0.));
                let h = (y(value) - y(0.)).abs().max(1.);
                write!(out, "<rect data-chart-mark=\"{i}\" class=\"{}\" x=\"{:.2}\" y=\"{py:.2}\" width=\"{bar_w:.2}\" height=\"{h:.2}\" rx=\"2\"></rect>", if peak.is_some_and(|p| p.0==i) { "peak-bar" } else { "chart-bar" }, x(i)-bar_w/2.).unwrap();
            }
        }
        let valid: Vec<_> = metric.points.iter().filter_map(|p| p.value).collect();
        let average = valid.iter().sum::<f64>() / valid.len() as f64;
        let py = y(average);
        write!(out, "<line class=\"average-line\" x1=\"{left}\" x2=\"{}\" y1=\"{py:.2}\" y2=\"{py:.2}\"/><text class=\"average-label\" x=\"{}\" y=\"{:.2}\" text-anchor=\"end\">Avg {}</text>", width-right, width-right-4., py-7., number(Some(average))).unwrap();
    } else {
        // Each contiguous segment is separate: missing readings never become a zero or a connecting line.
        let mut segment = Vec::new();
        for i in 0..=n {
            if i < n && metric.points[i].value.is_some() {
                segment.push(i);
                continue;
            }
            if segment.is_empty() {
                continue;
            }
            let mut band = String::new();
            for (position, index) in segment.iter().enumerate() {
                let p = &metric.points[*index];
                write!(
                    band,
                    "{} {:.2},{:.2} ",
                    if position == 0 { "M" } else { "L" },
                    x(*index),
                    y(p.maximum.or(p.value).unwrap())
                )
                .unwrap();
            }
            for index in segment.iter().rev() {
                let p = &metric.points[*index];
                write!(
                    band,
                    "L {:.2},{:.2} ",
                    x(*index),
                    y(p.minimum.or(p.value).unwrap())
                )
                .unwrap();
            }
            write!(out, "<path class=\"range-band\" d=\"{band}Z\"/>").unwrap();
            let mut path = String::new();
            for (position, index) in segment.iter().enumerate() {
                let p = &metric.points[*index];
                write!(
                    path,
                    "{} {:.2},{:.2} ",
                    if position == 0 { "M" } else { "L" },
                    x(*index),
                    y(p.value.unwrap())
                )
                .unwrap();
                write!(out,"<circle data-chart-mark=\"{index}\" class=\"chart-point\" cx=\"{:.2}\" cy=\"{:.2}\" r=\"2.3\"></circle>", x(*index),y(p.value.unwrap())).unwrap();
            }
            write!(out, "<path class=\"trend-line\" d=\"{path}\"/>").unwrap();
            segment.clear();
        }
    }
    let peak_index = if bars {
        metric
            .points
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.value.map(|v| (i, v)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    } else {
        None
    };
    out.push_str("<g class=\"chart-hit-targets\">");
    for (i, point) in metric.points.iter().enumerate() {
        write!(out, r#"<rect class="chart-hit" data-index="{i}" data-time="{time}" data-value="{value}" data-minimum="{minimum}" data-maximum="{maximum}" data-peak="{peak}" data-x="{center:.3}" data-y="{value_y}" x="{slot_left:.3}" y="{top}" width="{slot_width:.3}" height="{plot_h:.3}"/>"#,
            time=escape(&point.time), value=attribute_number(point.value),
            minimum=attribute_number(point.minimum),maximum=attribute_number(point.maximum),
            peak=peak_index==Some(i),center=x(i),value_y=point.value.map(|value|format!("{:.3}",y(value))).unwrap_or_default(),
            slot_left=left+i as f64*plot_w/n as f64,slot_width=plot_w/n as f64).unwrap();
    }
    out.push_str("</g>");
    let every = if bars {
        (n as f64 / 16.).ceil() as usize
    } else {
        (n as f64 / 7.).ceil() as usize
    }
    .max(1);
    let mut ticks = (0..n).step_by(every).collect::<Vec<_>>();
    if let Some(last) = ticks.last_mut() {
        if *last != n - 1 {
            if n - 1 - *last >= (every as f64 * 0.75).ceil() as usize {
                ticks.push(n - 1);
            } else {
                *last = n - 1;
            }
        }
    }
    for i in ticks {
        let point = &metric.points[i];
        write!(
            out,
            "<text class=\"axis-label\" x=\"{:.2}\" y=\"{}\" text-anchor=\"middle\">{}</text>",
            x(i),
            height - 31.,
            escape(&bucket_label(&point.time, bucket))
        )
        .unwrap();
    }
    out.push_str("</svg>");
    out
}
fn attribute_number(value: Option<f64>) -> String {
    value
        .filter(|value| value.is_finite())
        .map(|value| value.to_string())
        .unwrap_or_default()
}
fn axis_number(value: f64) -> String {
    if value.abs() >= 1_000_000. {
        format!("{:.1}M", value / 1_000_000.)
    } else if value.abs() >= 1000. {
        format!("{:.1}k", value / 1000.)
    } else if value.abs() >= 100. {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}
fn y_axis_title(metric: &MetricData) -> String {
    let quantity = if metric.kind == SignalType::Counter {
        format!("{} consumption", metric.label)
    } else {
        format!("Average {}", metric.label)
    };
    if metric.unit.trim().is_empty() {
        format!("{quantity} (unit not set)")
    } else {
        format!("{quantity} ({})", metric.unit)
    }
}
fn month_label(value: &str) -> Option<&'static str> {
    value.get(5..7)?.parse::<usize>().ok().and_then(|i| {
        [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ]
        .get(i.wrapping_sub(1))
        .copied()
    })
}
pub(crate) fn bucket_label(value: &str, bucket: &TrendBucket) -> String {
    match bucket {
        TrendBucket::Hour => value.get(11..16).unwrap_or(value).into(),
        TrendBucket::Day => match (value.get(8..10), month_label(value)) {
            (Some(day), Some(month)) => format!("{day} {month}"),
            _ => value.into(),
        },
        TrendBucket::Month => month_label(value).unwrap_or(value).into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{Aggregation, SignalType},
        telemetry::MetricPoint,
    };
    fn metric(kind: SignalType, unit: &str) -> MetricData {
        MetricData {
            label: "Metric <A>".into(),
            unit: unit.into(),
            kind,
            aggregation: Aggregation::Mean,
            global: false,
            group_count: 1,
            partial: false,
            summary: None,
            previous: None,
            minimum: None,
            maximum: None,
            peak: None,
            points: vec![MetricPoint {
                time: "2026-09-30".into(),
                value: Some(1.),
                minimum: Some(0.),
                maximum: Some(2.),
            }],
        }
    }
    #[test]
    fn axes_describe_the_quantity_unit_and_time_bucket() {
        let metric = metric(SignalType::Instantaneous, "kW");
        for (bucket, title) in [
            (TrendBucket::Hour, "Time of day (hour)"),
            (TrendBucket::Day, "Date"),
            (TrendBucket::Month, "Month"),
        ] {
            let svg = chart(&metric, &bucket, false, false);
            assert!(svg.contains("Average Metric &lt;A&gt; (kW)"));
            assert!(svg.contains(&format!("text-anchor=\"middle\">{title}</text>")));
        }
    }
    #[test]
    fn empty_counter_chart_keeps_labels_and_does_not_invent_units() {
        let mut metric = metric(SignalType::Counter, "");
        metric.points.clear();
        let svg = chart(&metric, &TrendBucket::Day, true, false);
        assert!(svg.contains("Metric &lt;A&gt; consumption (unit not set)"));
        assert!(svg.contains("No readings in this period"));
        assert!(!svg.contains("chart-bar"));
    }
    #[test]
    fn hover_data_keeps_zero_missing_and_raw_range_distinct() {
        let mut metric = metric(SignalType::Instantaneous, "kW & kVA");
        metric.points = vec![
            MetricPoint {
                time: "2026-09-01".into(),
                value: None,
                minimum: None,
                maximum: None,
            },
            MetricPoint {
                time: "2026-09-02".into(),
                value: Some(0.),
                minimum: Some(-1.25),
                maximum: Some(2.5),
            },
        ];
        let svg = chart(&metric, &TrendBucket::Day, false, false);
        assert!(svg.contains("data-label=\"Metric &lt;A&gt;\""));
        assert!(svg.contains("data-unit=\"kW &amp; kVA\""));
        assert!(svg.contains("data-time=\"2026-09-01\" data-value=\"\""));
        assert!(svg.contains("data-value=\"0\" data-minimum=\"-1.25\" data-maximum=\"2.5\""));
        assert_eq!(svg.matches("class=\"chart-hit\"").count(), 2);
        assert!(svg.contains("tabindex=\"0\""));
    }
    #[test]
    fn bar_and_hover_layer_agree_on_the_highest_bucket() {
        let metric = metric(SignalType::Counter, "m³");
        let svg = chart(&metric, &TrendBucket::Day, true, false);
        assert!(svg.contains("data-chart-mark=\"0\" class=\"peak-bar\""));
        assert!(svg.contains("data-peak=\"true\""));
        assert!(svg.contains("data-kind=\"counter\""));
    }
    #[test]
    fn day_ticks_distinguish_month_boundaries() {
        assert_eq!(bucket_label("2026-09-30", &TrendBucket::Day), "30 Sep");
        assert_eq!(bucket_label("2026-10-01", &TrendBucket::Day), "01 Oct");
        assert_eq!(
            bucket_label("2026-09-30 13:00:00", &TrendBucket::Hour),
            "13:00"
        );
        assert_eq!(bucket_label("2026-12-01", &TrendBucket::Month), "Dec");
    }
}
