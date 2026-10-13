use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SourceConfig {
    pub(crate) table_name: String,
    pub(crate) time_column_name: String,
    pub(crate) ignored_columns: Vec<String>,
    #[serde(default)]
    pub(crate) meter_overwrite_policy: bool,
}
