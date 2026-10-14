use axum::http::StatusCode;

#[derive(Debug)]
pub(crate) enum DbError {
    // 404 not found
    TableNotFound(String),
    ColumnNotFound(Vec<String>),
    SelectedColumnNotInSource(Vec<String>),
    // 409 conflict
    DuplicateColumn(Vec<String>),
    DuplicateGroupLabel(Vec<(String, String)>),
    EmptySignals,
    DatabaseNotOpen,
    SourceNotConfig,
    EmptySelectedColumns,
    // 500 internal error
    Sqlite(String),
    Io(String),
    Serde(String),
    Channel(String),
}
impl DbError {
    pub(crate) fn status_code(&self) -> axum::http::StatusCode {
        match self {
            // 404 not found
            Self::TableNotFound(_) => StatusCode::NOT_FOUND,
            Self::ColumnNotFound(_) => StatusCode::NOT_FOUND,
            Self::SelectedColumnNotInSource(_) => StatusCode::NOT_FOUND,
            // 409 conflict
            Self::DuplicateColumn(_) => StatusCode::CONFLICT,
            Self::DuplicateGroupLabel(_) => StatusCode::CONFLICT,
            Self::EmptySignals => StatusCode::CONFLICT,
            Self::DatabaseNotOpen => StatusCode::CONFLICT,
            Self::SourceNotConfig => StatusCode::CONFLICT,
            Self::EmptySelectedColumns => StatusCode::CONFLICT,
            // 500 internal error
            Self::Sqlite(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Serde(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Channel(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // 404 not found
            Self::TableNotFound(table_name) => write!(f, "table '{table_name}' not found"),
            Self::ColumnNotFound(column_names) => write!(f, "column '{column_names:?}' not found"),
            Self::SelectedColumnNotInSource(selected_columns) => write!(
                f,
                "selected column '{selected_columns:?}' not found in source config"
            ),
            // 409 conflict
            Self::DuplicateColumn(columns) => write!(f, "duplicated columns: {columns:?}"),
            Self::DuplicateGroupLabel(group_label) => {
                write!(f, "duplicated group label pair: {group_label:?}")
            }
            Self::EmptySignals => write!(f, "signal is empty"),
            Self::DatabaseNotOpen => write!(f, "database not open"),
            Self::SourceNotConfig => write!(f, "source not configured"),
            Self::EmptySelectedColumns => {
                write!(f, "empty selected_columns or empty column_name is detected")
            }
            // 500 internal error
            Self::Sqlite(sqlite_err) => write!(f, "sql error: {sqlite_err}"),
            Self::Io(io_err) => write!(f, "IO error: {io_err}"),
            Self::Serde(serde_err) => write!(f, "JSON conversion error: {serde_err}"),
            Self::Channel(channel_error) => write!(f, "transport channel error: {channel_error}"),
        }
    }
}
