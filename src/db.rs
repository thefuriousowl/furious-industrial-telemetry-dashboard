use std::{collections::BTreeMap, path::PathBuf};

use rusqlite::{Connection, OpenFlags};

use crate::{
    config::{SignalType, SourceConfig},
    error::DbError,
    schema::{ColumnMetaData, RuntimeSchema},
    sql_command::{LIST_AVAILABLE_COLUMNS, LIST_AVAILABLE_TABLE, get_query_string},
    telemetry::{TelemetryRow, TrendBucket},
};

pub(crate) struct DbReader {
    conn: Connection,
}

impl DbReader {
    pub(crate) fn open(path: &str) -> Result<Self, DbError> {
        let db_path = PathBuf::from(path);
        let conn = match Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
            Ok(conn) => conn,
            Err(e) => {
                return Err(DbError::Sqlite(e.to_string()));
            }
        };

        Ok(Self { conn })
    }

    pub(crate) fn list_table(&self) -> Result<Vec<String>, DbError> {
        let mut stmt = self
            .conn
            .prepare(LIST_AVAILABLE_TABLE)
            .map_err(|e| DbError::Sqlite(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| DbError::Sqlite(e.to_string()))?;
        let mut tables = Vec::new();
        for r in rows {
            tables.push(r.map_err(|e| DbError::Sqlite(e.to_string()))?);
        }

        Ok(tables)
    }

    pub(crate) fn get_columns_metadata(
        &self,
        table_name: &str,
    ) -> Result<Vec<ColumnMetaData>, DbError> {
        let mut stmt = self
            .conn
            .prepare(LIST_AVAILABLE_COLUMNS)
            .map_err(|e| DbError::Sqlite(e.to_string()))?;
        let rows = stmt
            .query_map([table_name], |r| {
                Ok(ColumnMetaData::new(r.get(0)?, r.get(1)?))
            })
            .map_err(|e| DbError::Sqlite(e.to_string()))?;

        let columns_metadata = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| DbError::Sqlite(e.to_string()))?;

        if columns_metadata.is_empty() {
            return Err(DbError::TableNotFound(table_name.to_string()));
        }

        Ok(columns_metadata)
    }

    pub(crate) fn discover_schema(&self, config: SourceConfig) -> Result<RuntimeSchema, DbError> {
        let current_column_metadata = self.get_columns_metadata(&config.table_name())?;

        RuntimeSchema::generate_from(
            config.table_name().to_string(),
            current_column_metadata,
            config.time_column_name().to_string(),
            config.signals().to_vec(),
        )
    }

    pub(crate) fn get_telemetry(
        &self,
        table_name: &str,
        expected_time_column_name: &str,
        selected_columns: &[String],
        start: String,
        end: String,
    ) -> Result<Vec<TelemetryRow>, DbError> {
        let columns_metadata = self.get_columns_metadata(table_name)?;
        let sql = get_query_string(
            table_name,
            expected_time_column_name,
            &columns_metadata,
            selected_columns,
        )?;
        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|e| DbError::Sqlite(e.to_string()))?;

        let mut rows = stmt
            .query([start, end])
            .map_err(|e| DbError::Sqlite(e.to_string()))?;

        let mut output = Vec::<TelemetryRow>::new();

        while let Some(r) = rows.next().map_err(|e| DbError::Sqlite(e.to_string()))? {
            let time: String = r
                .get(expected_time_column_name)
                .map_err(|e| DbError::Sqlite(e.to_string()))?;

            let mut values = BTreeMap::new();

            for col_name in selected_columns {
                let value: Option<f64> = r
                    .get(col_name.as_str())
                    .map_err(|e| DbError::Sqlite(e.to_string()))?;

                values.insert(col_name.to_string(), value);
            }

            output.push(TelemetryRow::from(time, values));
        }

        Ok(output)
    }

    pub(crate) fn get_signal_summary(
        &self,
        table_name: &str,
        expected_time_column_name: &str,
        column: &str,
        signal_type: &SignalType,
        start: &str,
        end: &str,
    ) -> Result<Option<f64>, DbError> {
        let sql = match signal_type {
            SignalType::Instantaneous => format!(
                "SELECT AVG(\"{column}\") FROM \"{table_name}\" WHERE \"{expected_time_column_name}\" >= ?1 AND \"{expected_time_column_name}\" <?2"
            ),
            SignalType::Counter => format!(
                "SELECT MAX(\"{column}\") - MIN(\"{column}\") FROM \"{table_name}\" WHERE \"{expected_time_column_name}\" >= ?1 AND \"{expected_time_column_name}\" < ?2"
            ),
        };

        self.conn
            .query_row(&sql, [start, end], |r| r.get(0))
            .map_err(|e| DbError::Sqlite(e.to_string()))
    }

    pub(crate) fn get_signal_trend(
        &self,
        table_name: &str,
        expected_time_column_name: &str,
        column: &str,
        signal_type: &SignalType,
        bucket: &TrendBucket,
        start: &str,
        end: &str,
    ) -> Result<Vec<(String, Option<f64>)>, DbError> {
        let bucket_map = match bucket {
            TrendBucket::Hour => {
                format!(r#"strftime('%Y-%m-%d %H:00:00', "{expected_time_column_name}")"#)
            }
            TrendBucket::Day => format!(r#"date("{expected_time_column_name}")"#),
            TrendBucket::Month => format!(r#"strftime('%Y-%m-01', "{expected_time_column_name}")"#),
        };
        let sql = match signal_type {
            SignalType::Instantaneous => format!(
                r#"SELECT {bucket_map}, AVG("{column}")
            FROM "{table_name}"
            WHERE "{expected_time_column_name}" >= ?1
            AND "{expected_time_column_name}" < ?2
            GROUP BY {bucket_map}
            ORDER BY {bucket_map} ASC"#
            ),
            SignalType::Counter => format!(
                r#"SELECT {bucket_map}, MAX("{column}") - MIN("{column}")
            FROM "{table_name}"
            WHERE "{expected_time_column_name}" >= ?1
            AND "{expected_time_column_name}" < ?2
            GROUP BY {bucket_map}
            ORDER BY {bucket_map} ASC"#
            ),
        };
        let mut stmt = self
            .conn
            .prepare(&sql)
            .map_err(|e| DbError::Sqlite(e.to_string()))?;
        let rows = stmt
            .query_map([start, end], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| DbError::Sqlite(e.to_string()))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| DbError::Sqlite(e.to_string()))
    }

    pub(crate) fn get_signal_peak(
        &self,
        table_name: &str,
        expected_time_column_name: &str,
        column: &str,
        signal_type: &SignalType,
        start: &str,
        end: &str,
    ) -> Result<Option<f64>, DbError> {
        let sql = match signal_type {
            SignalType::Instantaneous => format!(
                "SELECT MAX(\"{column}\") FROM \"{table_name}\" WHERE \"{expected_time_column_name}\" >= ?1 AND \"{expected_time_column_name}\" <?2"
            ),
            SignalType::Counter => {
                return Err(DbError::UnsupportedOperation);
            } // coming soon
        };

        self.conn
            .query_row(&sql, [start, end], |r| r.get(0))
            .map_err(|e| DbError::Sqlite(e.to_string()))
    }
}
