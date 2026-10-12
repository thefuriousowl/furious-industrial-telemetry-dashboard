#[derive(Debug)]
pub(crate) enum MetricKind {
    Flow,
    Pressure,
    Temperature,
    Totalizer,
}

pub(crate) fn parse_metric(column_name: &str) -> Option<(&str, MetricKind)> {
    [
        ("_Flow", MetricKind::Flow),
        ("_Pressure", MetricKind::Pressure),
        ("_Temp", MetricKind::Temperature),
        ("_Total", MetricKind::Totalizer),
    ]
    .into_iter()
    .find_map(|(suffix, kind)| {
        let group_name = column_name.strip_suffix(suffix)?;

        if group_name.is_empty() {
            return None;
        }

        Some((group_name, kind))
    })
}
