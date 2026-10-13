use std::{collections::BTreeMap, path::PathBuf};

use rusqlite::{Connection, OpenFlags};

use crate::{
    config::SourceConfig,
    schema::{ColumnMetaData, RuntimeSchema},
    sql_command::{LIST_AVAILABLE_COLUMNS, LIST_AVAILABLE_TABLE, get_query_string},
    telemetry::TelemetryRow,
};

pub(crate) struct DbReader {
    conn: Connection,
}

impl DbReader {
    pub(crate) fn open(path: &str) -> Result<Self, String> {
        let db_path = PathBuf::from(path);
        let conn = match Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
            Ok(conn) => conn,
            Err(e) => {
                return Err(format!("Error: {}", e));
            }
        };

        Ok(Self { conn })
    }

    pub(crate) fn list_table(&self) -> Result<Vec<String>, String> {
        let mut stmt = self
            .conn
            .prepare(LIST_AVAILABLE_TABLE)
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        let mut tables = Vec::new();
        for r in rows {
            tables.push(r.map_err(|e| e.to_string())?);
        }

        Ok(tables)
    }

    pub(crate) fn get_columns_metadata(
        &self,
        table_name: &str,
    ) -> Result<Vec<ColumnMetaData>, String> {
        let mut stmt = self
            .conn
            .prepare(LIST_AVAILABLE_COLUMNS)
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([table_name], |r| {
                Ok(ColumnMetaData::new(r.get(0)?, r.get(1)?))
            })
            .map_err(|e| e.to_string())?;

        let columns_metadata = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        if columns_metadata.is_empty() {
            return Err(format!("table '{}' not found", &table_name));
        }

        Ok(columns_metadata)
    }

    pub(crate) fn discover_schema(&self, config: SourceConfig) -> Result<RuntimeSchema, String> {
        let current_column_metadata = self
            .get_columns_metadata(&config.table_name)
            .map_err(|e| e.to_string())?;

        let time_column_exists = current_column_metadata
            .iter()
            .any(|metadata| metadata.name() == config.time_column_name);

        if !time_column_exists {
            return Err(format!(
                "Discover schema error: {} column does not exists",
                config.time_column_name
            ));
        }

        RuntimeSchema::generate_from(
            config.table_name,
            current_column_metadata,
            config.time_column_name,
            config.signals,
        )
        .map_err(|e| e.to_string())
    }

    pub(crate) fn get_telemetry(
        &self,
        table_name: &str,
        expected_time_column_name: &str,
        selected_columns: &[String],
        start: String,
        end: String,
    ) -> Result<Vec<TelemetryRow>, String> {
        let columns_metadata = self.get_columns_metadata(table_name)?;
        let sql = get_query_string(
            table_name,
            expected_time_column_name,
            &columns_metadata,
            selected_columns,
        )?;
        let mut stmt = self.conn.prepare(&sql).map_err(|e| e.to_string())?;

        let mut rows = stmt.query([start, end]).map_err(|e| e.to_string())?;

        let mut output = Vec::<TelemetryRow>::new();

        while let Some(r) = rows.next().map_err(|e| e.to_string())? {
            let time: String = r
                .get(expected_time_column_name)
                .map_err(|e| e.to_string())?;

            let mut values = BTreeMap::new();

            for col_name in selected_columns {
                let value: Option<f64> = r.get(col_name.as_str()).map_err(|e| e.to_string())?;

                values.insert(col_name.to_string(), value);
            }

            output.push(TelemetryRow::from(time, values));
        }

        Ok(output)
    }
}
