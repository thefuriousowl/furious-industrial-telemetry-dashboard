use crate::{error::DbError, schema::ColumnMetaData};

pub(crate) const LIST_AVAILABLE_TABLE: &str = "SELECT name FROM main.sqlite_schema WHERE type = 'table' AND name NOT GLOB 'sqlite_*' ORDER BY name";
pub(crate) const LIST_AVAILABLE_COLUMNS: &str =
    "SELECT name, type FROM pragma_table_info(?1) ORDER BY cid";

pub(crate) fn get_query_string(
    table_name: &str,
    expected_time_column_name: &str,
    columns_metadata: &Vec<ColumnMetaData>,
    selected_columns: &[String],
) -> Result<String, DbError> {
    let available_columns = columns_metadata
        .iter()
        .map(|metadata| metadata.name())
        .collect::<Vec<_>>();

    if selected_columns.is_empty() || selected_columns.iter().any(|s| s.is_empty()) {
        return Err(DbError::EmptySelectedColumns);
    }

    if !available_columns.contains(&expected_time_column_name) {
        return Err(DbError::ColumnNotFound(vec![
            expected_time_column_name.to_string(),
        ]));
    }

    let mismatch_columns = selected_columns
        .iter()
        .filter(|s| !available_columns.contains(&s.as_str()))
        .collect::<Vec<_>>();

    if mismatch_columns.len() > 0 {
        return Err(DbError::ColumnNotFound(
            mismatch_columns.iter().map(|col| col.to_string()).collect(),
        ));
    }

    let selected_columns_str = selected_columns
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(", ");

    Ok(format!(
        "SELECT \"{expected_time_column_name}\", {selected_columns_str} FROM \"{table_name}\" WHERE \"{expected_time_column_name}\" >= ?1 AND \"{expected_time_column_name}\" < ?2 ORDER BY \"{expected_time_column_name}\" ASC"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample_metadata() -> Vec<ColumnMetaData> {
        vec![
            ColumnMetaData::new("Time".to_string(), "datetime".to_string()),
            ColumnMetaData::new("Furnace_Flow".to_string(), "REAL".to_string()),
            ColumnMetaData::new("Furnace_Pressure".to_string(), "REAL".to_string()),
        ]
    }

    #[test]
    fn all_selected_column_exists_in_metadata() {
        let expected_sql = "SELECT \"Time\", \"Furnace_Flow\", \"Furnace_Pressure\" FROM \"DataLogger1\" WHERE \"Time\" >= ?1 AND \"Time\" < ?2 ORDER BY \"Time\" ASC";
        let valid_selected_columns = ["Furnace_Flow".to_string(), "Furnace_Pressure".to_string()];
        let result = get_query_string(
            "DataLogger1",
            "Time",
            &sample_metadata(),
            &valid_selected_columns,
        )
        .expect("should valid");
        assert_eq!(result, expected_sql)
    }

    #[test]
    fn reject_empty_selected_columns() {
        let empty_selected_columns = [];
        let result = get_query_string(
            "DataLogger1",
            "Time",
            &sample_metadata(),
            &empty_selected_columns,
        );
        assert!(result.is_err())
    }

    #[test]
    fn reject_empty_string_in_selected_columns() {
        let selected_columns_contains_empty_string = ["Furnace_Flow".to_string(), "".to_string()];
        let result = get_query_string(
            "DataLogger1",
            "Time",
            &sample_metadata(),
            &selected_columns_contains_empty_string,
        );
        assert!(result.is_err())
    }

    #[test]
    fn reject_column_name_not_in_metadata() {
        let selected_columns_contains_mismatch_column_name =
            ["Furnace_Flow".to_string(), "Nope".to_string()];
        let result = get_query_string(
            "DataLogger1",
            "Time",
            &sample_metadata(),
            &selected_columns_contains_mismatch_column_name,
        );
        assert!(result.is_err())
    }

    #[test]
    fn reject_missing_time_column() {
        let valid_selected_columns = ["Furnace_Flow".to_string(), "Furnace_Pressure".to_string()];
        let result = get_query_string(
            "DataLogger1",
            "Timeeeeeeeeeeeeeeeee",
            &sample_metadata(),
            &valid_selected_columns,
        );
        assert!(result.is_err())
    }
}
