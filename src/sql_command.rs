pub(crate) const LIST_AVAILABLE_TABLE: &str = "SELECT name FROM main.sqlite_schema WHERE type = 'table' AND name NOT GLOB 'sqlite_*' ORDER BY name";
pub(crate) const LIST_AVAILABLE_COLUMNS: &str =
    "SELECT name, type FROM pragma_table_info(?1) ORDER BY cid";

pub(crate) fn get_query_string(table_name: &str, expected_time_column_name: &str) -> String {
    format!(
        "SELECT * FROM {table_name} WHERE {expected_time_column_name} >= ?1 AND {expected_time_column_name} < ?2 ORDER BY {expected_time_column_name} ASC"
    )
}
