use std::collections::BTreeMap;

use crate::kind::{MetricKind, parse_metric};

#[derive(Debug, Clone)]
pub(crate) struct ColumnMetaData {
    name: String,
    declare_type: String,
}
impl ColumnMetaData {
    pub(crate) fn new(name: String, declare_type: String) -> Self {
        Self { name, declare_type }
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn declare_type(&self) -> &str {
        &self.declare_type
    }
}

#[derive(Debug)]
pub(crate) struct MeterSchema {
    group_name: String,
    flow: Option<String>,
    pressure: Option<String>,
    temperature: Option<String>,
    totalizer: Option<String>,
}
impl MeterSchema {
    pub(crate) fn new(group_name: String) -> Self {
        Self {
            group_name,
            flow: None,
            pressure: None,
            temperature: None,
            totalizer: None,
        }
    }

    pub(crate) fn add_metric(
        &mut self,
        kind: MetricKind,
        column_name: String,
        overwrite: bool,
    ) -> Result<(), String> {
        let target_field = match kind {
            MetricKind::Flow => &mut self.flow,
            MetricKind::Pressure => &mut self.pressure,
            MetricKind::Temperature => &mut self.temperature,
            MetricKind::Totalizer => &mut self.totalizer,
        };

        if target_field.is_some() {
            if !overwrite {
                return Err(format!(
                    "duplicate metric '{column_name}' for kind {:?}",
                    kind
                ));
            }

            println!("Overwriting metric '{column_name}' for kind {:?}", kind);
        }

        *target_field = Some(column_name);

        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct RuntimeSchema {
    table_name: String,
    time_column_name: String,
    meters: BTreeMap<String, MeterSchema>,
    standalone_fields: Vec<ColumnMetaData>,
}
impl RuntimeSchema {
    pub(crate) fn generate_from(
        table_name: String,
        columns_metadata: Vec<ColumnMetaData>,
        expected_time_column_name: String,
        ignore_columns: Vec<String>,
        meter_overwrite_policy: bool,
    ) -> Result<Self, String> {
        let time_column_exists = columns_metadata
            .iter()
            .any(|metadata| metadata.name() == expected_time_column_name);

        if !time_column_exists {
            return Err(format!(
                "expected time column name {expected_time_column_name} does not exists"
            ));
        };

        let mut meters = BTreeMap::<String, MeterSchema>::new();
        let mut standalone_fields = Vec::<ColumnMetaData>::new();

        for metadata in columns_metadata {
            if metadata.name() == expected_time_column_name
                || ignore_columns.contains(&metadata.name().to_string())
            {
                continue;
            }

            if let Some((group_name, kind)) = parse_metric(metadata.name()) {
                let group_name = group_name.to_string();
                let meter = meters
                    .entry(group_name.clone())
                    .or_insert_with(|| MeterSchema::new(group_name));

                meter
                    .add_metric(kind, metadata.name().to_string(), meter_overwrite_policy)
                    .map_err(|e| e.to_string())?;
            } else {
                standalone_fields.push(metadata);
            }
        }

        Ok(Self {
            table_name,
            time_column_name: expected_time_column_name,
            meters,
            standalone_fields,
        })
    }
}
