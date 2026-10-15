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
            SignalType::Counter => {
                let steps = counter_steps_sql(table_name, expected_time_column_name, column);
                format!("{steps} SELECT SUM(consumption) FROM counter_steps")
            }
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
        let bucket_map = trend_bucket_sql(bucket, expected_time_column_name);
        let sql = match signal_type {
            SignalType::Instantaneous => format!(
                r#"SELECT {bucket_map}, AVG("{column}")
            FROM "{table_name}"
            WHERE "{expected_time_column_name}" >= ?1
            AND "{expected_time_column_name}" < ?2
            GROUP BY {bucket_map}
            ORDER BY {bucket_map} ASC"#
            ),
            SignalType::Counter => {
                let steps = counter_steps_sql(table_name, expected_time_column_name, column);
                let bucket_map = trend_bucket_sql(bucket, "sample_time");
                format!(
                    r#"{steps}
                    SELECT {bucket_map}, SUM(consumption)
                    FROM counter_steps
                    GROUP BY {bucket_map}
                    ORDER BY {bucket_map} ASC"#
                )
            }
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

fn counter_steps_sql(table: &str, time_column: &str, column: &str) -> String {
    let table = table.replace('"', "\"\"");
    let time_column = time_column.replace('"', "\"\"");
    let column = column.replace('"', "\"\"");
    format!(
        r#"WITH range_samples AS (
        SELECT "{time_column}" AS sample_time, "{column}" AS reading
        FROM "{table}"
        WHERE "{time_column}" >= ?1 AND "{time_column}" < ?2
    ), previous_sample AS (
        SELECT "{time_column}" AS sample_time, "{column}" AS reading
        FROM "{table}"
        WHERE "{time_column}" < ?1 AND "{column}" IS NOT NULL
        ORDER BY "{time_column}" DESC LIMIT 1
    ), valid_samples AS (
        SELECT sample_time, reading FROM previous_sample
        UNION ALL
        SELECT sample_time, reading FROM range_samples WHERE reading IS NOT NULL
    ), counter_readings AS (
        SELECT sample_time, reading,
            LAG(reading) OVER (ORDER BY sample_time) AS previous_reading
        FROM valid_samples
    ), counter_steps AS (
        SELECT sample_time,
            CASE
                WHEN previous_reading IS NULL THEN NULL
                WHEN reading >= previous_reading THEN reading - previous_reading
                ELSE reading
            END AS consumption
        FROM counter_readings
        WHERE sample_time >= ?1 AND sample_time < ?2
        UNION ALL
        SELECT sample_time, NULL FROM range_samples WHERE reading IS NULL
    )"#
    )
}
fn trend_bucket_sql(bucket: &TrendBucket, time_column: &str) -> String {
    let time_column = time_column.replace('"', "\"\"");
    match bucket {
        TrendBucket::Hour => format!(r#"strftime('%Y-%m-%d %H:00:00', "{time_column}")"#),
        TrendBucket::Day => format!(r#"date("{time_column}")"#),
        TrendBucket::Month => format!(r#"strftime('%Y-%m-01', "{time_column}")"#),
    }
}

#[cfg(test)]
mod counter_review {
    use super::*;
    fn reader(rows: &[(&str, Option<f64>)]) -> DbReader {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE samples(Time TEXT, Counter REAL);")
            .unwrap();
        for (time, value) in rows {
            conn.execute(
                "INSERT INTO samples VALUES (?1,?2)",
                rusqlite::params![time, value],
            )
            .unwrap();
        }
        DbReader { conn }
    }
    fn summary(db: &DbReader, start: &str, end: &str) -> Option<f64> {
        db.get_signal_summary(
            "samples",
            "Time",
            "Counter",
            &SignalType::Counter,
            start,
            end,
        )
        .unwrap()
    }
    fn trend(
        db: &DbReader,
        bucket: TrendBucket,
        start: &str,
        end: &str,
    ) -> Vec<(String, Option<f64>)> {
        db.get_signal_trend(
            "samples",
            "Time",
            "Counter",
            &SignalType::Counter,
            &bucket,
            start,
            end,
        )
        .unwrap()
    }
    const START: &str = "2026-09-05 00:00:00";
    const END: &str = "2026-09-06 00:00:00";
    #[test]
    fn preserves_hour_boundary_and_prior_baseline() {
        let db = reader(&[
            ("2026-09-04 23:59:00", Some(100.0)),
            ("2026-09-05 00:00:00", Some(104.0)),
            ("2026-09-05 01:00:00", Some(109.0)),
        ]);
        assert_eq!(summary(&db, START, END), Some(9.0));
        let points = trend(&db, TrendBucket::Hour, START, END);
        assert_eq!(
            points.iter().map(|p| p.1).collect::<Vec<_>>(),
            vec![Some(4.0), Some(5.0)]
        );
    }
    #[test]
    fn reset_observed_at_zero() {
        let db = reader(&[
            ("2026-09-04 23:59:00", Some(100.0)),
            ("2026-09-05 00:00:00", Some(110.0)),
            ("2026-09-05 00:01:00", Some(0.0)),
            ("2026-09-05 00:02:00", Some(5.0)),
        ]);
        assert_eq!(summary(&db, START, END), Some(15.0));
    }
    #[test]
    fn reset_between_samples() {
        let db = reader(&[
            ("2026-09-04 23:59:00", Some(100.0)),
            ("2026-09-05 00:00:00", Some(110.0)),
            ("2026-09-05 00:01:00", Some(2.0)),
        ]);
        assert_eq!(summary(&db, START, END), Some(12.0));
    }
    #[test]
    fn constant_is_zero() {
        let db = reader(&[
            ("2026-09-04 23:59:00", Some(100.0)),
            ("2026-09-05 00:00:00", Some(100.0)),
        ]);
        assert_eq!(summary(&db, START, END), Some(0.0));
    }
    #[test]
    fn absent_baseline_and_single_sample_is_unknown() {
        let db = reader(&[("2026-09-05 00:00:00", Some(100.0))]);
        assert_eq!(summary(&db, START, END), None);
    }
    #[test]
    fn missing_readings_keep_empty_bucket() {
        let db = reader(&[
            ("2026-09-04 23:59:00", Some(100.0)),
            ("2026-09-05 00:00:00", None),
            ("2026-09-05 01:00:00", Some(109.0)),
        ]);
        assert_eq!(summary(&db, START, END), Some(9.0));
        let points = trend(&db, TrendBucket::Hour, START, END);
        assert_eq!(
            points.iter().map(|p| p.1).collect::<Vec<_>>(),
            vec![None, Some(9.0)]
        );
    }
    #[test]
    fn adjacent_periods_add_up() {
        let db = reader(&[
            ("2026-09-04 23:59:00", Some(100.0)),
            ("2026-09-05 00:00:00", Some(104.0)),
            ("2026-09-05 23:59:00", Some(110.0)),
            ("2026-09-06 00:00:00", Some(112.0)),
            ("2026-09-06 01:00:00", Some(115.0)),
        ]);
        assert_eq!(
            summary(&db, START, "2026-09-07 00:00:00"),
            Some(
                summary(&db, START, END).unwrap()
                    + summary(&db, END, "2026-09-07 00:00:00").unwrap()
            )
        );
    }
    #[test]
    fn empty_range_is_unknown() {
        assert_eq!(summary(&reader(&[]), START, END), None);
    }
}
