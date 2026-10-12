use crate::schema::ColumnMetaData;

pub(crate) const LIST_AVAILABLE_TABLE: &str =
    "SELECT name FROM main.sqlite_schema WHERE type = 'table' AND name NOT GLOB 'sqlite_*' ORDER BY name";
pub(crate) const LIST_AVAILABLE_COLUMNS: &str =
    "SELECT name, type FROM pragma_table_info(?1) ORDER BY cid";

pub(crate) fn get_query_string(
    table_name: &str,
    expected_time_column_name: &str,
    columns_metadata: &Vec<ColumnMetaData>,
    selected_columns: &[String]
) -> Result<String, String> {
    let available_columns = columns_metadata
        .iter()
        .map(|metadata| metadata.name())
        .collect::<Vec<_>>();

    if selected_columns.is_empty() || selected_columns.iter().any(|s| s.is_empty()) {
        return Err(
            format!(
                "parse query string failed: empty selected_columns or empty column_name is detected"
            )
        );
    }

    if !available_columns.contains(&expected_time_column_name) {
        return Err(
            format!(
                "expected time column name '{expected_time_column_name}' not found in table '{table_name}'"
            )
        );
    }

    let mismatch_columns = selected_columns
        .iter()
        .filter(|s| !available_columns.contains(&s.as_str()))
        .collect::<Vec<_>>();

    if mismatch_columns.len() > 0 {
        return Err(format!("error mismatch columns not allowed. found: {:#?}", mismatch_columns));
    }

    let selected_columns_str = selected_columns
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(", ");

    Ok(
        format!(
            "SELECT \"{expected_time_column_name}\", {selected_columns_str} FROM \"{table_name}\" WHERE \"{expected_time_column_name}\" >= ?1 AND \"{expected_time_column_name}\" < ?2 ORDER BY \"{expected_time_column_name}\" ASC"
        )
    )
}
