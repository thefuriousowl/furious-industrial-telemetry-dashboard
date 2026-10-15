use std::{collections::BTreeMap, path::PathBuf};

use rusqlite::{Connection, OpenFlags};

use crate::{
    config::{Aggregation, SignalType, SourceConfig},
    error::DbError,
    schema::{ColumnMetaData, RuntimeSchema},
    sql_command::{LIST_AVAILABLE_COLUMNS, LIST_AVAILABLE_TABLE, get_query_string},
    telemetry::{InstantaneousData, MetricPoint, TelemetryRow, TrendBucket},
};

pub(crate) struct DbReader {
    conn: Connection,
}

impl DbReader {
    pub(crate) fn with_read_snapshot<T>(
        &self,
        read: impl FnOnce(&Self) -> Result<T, DbError>,
    ) -> Result<T, DbError> {
        let transaction = self
            .conn
            .unchecked_transaction()
            .map_err(|e| DbError::Sqlite(e.to_string()))?;
        let result = read(self)?;
        transaction
            .commit()
            .map_err(|e| DbError::Sqlite(e.to_string()))?;
        Ok(result)
    }
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

    pub(crate) fn get_latest_time(
        &self,
        table_name: &str,
        time_column_name: &str,
    ) -> Result<Option<String>, DbError> {
        let columns = self.get_columns_metadata(table_name)?;
        if !columns
            .iter()
            .any(|column| column.name() == time_column_name)
        {
            return Err(DbError::ColumnNotFound(vec![time_column_name.to_string()]));
        }
        let table = table_name.replace('"', "\"\"");
        let time_column = time_column_name.replace('"', "\"\"");

        let sql = format!(r#"SELECT MAX("{time_column}") FROM "{table}""#);

        self.conn
            .query_row(&sql, [], |row| row.get(0))
            .map_err(|e| DbError::Sqlite(e.to_string()))
    }

    // Combine readings from the same sample first. In particular, plant peak flow
    // is MAX(sum at a timestamp), rather than the sum of unrelated meter peaks.
    pub(crate) fn read_instantaneous(
        &self,
        table: &str,
        time_column: &str,
        columns: &[String],
        aggregation: Aggregation,
        start: &str,
        end: &str,
        bucket: &TrendBucket,
    ) -> Result<InstantaneousData, DbError> {
        if columns.is_empty() {
            return Ok(InstantaneousData::default());
        }
        let table = quote_identifier(table);
        let time_column = quote_identifier(time_column);
        let columns = columns
            .iter()
            .map(|column| quote_identifier(column))
            .collect::<Vec<_>>();
        let sum = columns
            .iter()
            .map(|column| format!("COALESCE({column}, 0.0)"))
            .collect::<Vec<_>>()
            .join(" + ");
        let count = columns
            .iter()
            .map(|column| format!("({column} IS NOT NULL)"))
            .collect::<Vec<_>>()
            .join(" + ");
        let value = match aggregation {
            Aggregation::Sum => format!("CASE WHEN ({count}) > 0 THEN ({sum}) END"),
            Aggregation::Mean => format!("({sum}) / NULLIF(1.0 * ({count}), 0)"),
        };
        let (low, high) = if aggregation == Aggregation::Sum || columns.len() == 1 {
            (value.clone(), value.clone())
        } else {
            // Replacing a NULL by an actual member value preserves scalar min/max.
            let fallback = format!("COALESCE({})", columns.join(", "));
            let members = columns
                .iter()
                .map(|column| format!("COALESCE({column}, {fallback})"))
                .collect::<Vec<_>>()
                .join(", ");
            (format!("MIN({members})"), format!("MAX({members})"))
        };
        let samples = format!(
            r#"WITH samples AS (
            SELECT {time_column} AS sample_time, {value} AS value,
                {low} AS low, {high} AS high, ({count}) AS present
            FROM main.{table} WHERE {time_column} >= ?1 AND {time_column} < ?2
        )"#
        );
        let mut result = self.conn.query_row(
            &format!("{samples} SELECT AVG(value), MIN(low), MAX(high), COALESCE(MAX(present < {}), 0) FROM samples", columns.len()),
            [start, end], |row| Ok(InstantaneousData {
                average: row.get(0)?, minimum: row.get(1)?, maximum: row.get(2)?,
                partial: row.get(3)?, ..Default::default()
            }),
        ).map_err(|e| DbError::Sqlite(e.to_string()))?;
        let mut statement = self.conn.prepare(&format!("{samples} SELECT sample_time, value FROM samples WHERE value IS NOT NULL ORDER BY value DESC, sample_time LIMIT 1"))
            .map_err(|e| DbError::Sqlite(e.to_string()))?;
        let mut rows = statement
            .query([start, end])
            .map_err(|e| DbError::Sqlite(e.to_string()))?;
        if let Some(row) = rows.next().map_err(|e| DbError::Sqlite(e.to_string()))? {
            result.peak = Some((
                row.get(0).map_err(|e| DbError::Sqlite(e.to_string()))?,
                row.get(1).map_err(|e| DbError::Sqlite(e.to_string()))?,
            ));
        }
        let bucket = trend_bucket_sql(bucket, "sample_time");
        let mut statement = self.conn.prepare(&format!("{samples} SELECT {bucket}, AVG(value), MIN(low), MAX(high) FROM samples GROUP BY {bucket} ORDER BY {bucket}"))
            .map_err(|e| DbError::Sqlite(e.to_string()))?;
        result.points = statement
            .query_map([start, end], |row| {
                Ok(MetricPoint {
                    time: row.get(0)?,
                    value: row.get(1)?,
                    minimum: row.get(2)?,
                    maximum: row.get(3)?,
                })
            })
            .map_err(|e| DbError::Sqlite(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| DbError::Sqlite(e.to_string()))?;
        Ok(result)
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

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn counter_steps_sql(table: &str, time_column: &str, column: &str) -> String {
    let table = table.replace('"', "\"\"");
    let time_column = time_column.replace('"', "\"\"");
    let column = column.replace('"', "\"\"");
    format!(
        r#"WITH range_samples AS (
        SELECT "{time_column}" AS sample_time, "{column}" AS reading
        FROM main."{table}"
        WHERE "{time_column}" >= ?1 AND "{time_column}" < ?2
    ), previous_sample AS (
        SELECT "{time_column}" AS sample_time, "{column}" AS reading
        FROM main."{table}"
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

#[cfg(test)]
mod aggregate_review {
    use super::*;
    #[test]
    fn source_table_names_can_match_internal_query_names() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE samples(Time TEXT, value REAL); INSERT INTO samples VALUES ('2026-01-01 00:00:00',42);
            CREATE TABLE range_samples(Time TEXT, value REAL); INSERT INTO range_samples VALUES ('2026-01-01 00:00:00',10),('2026-01-01 01:00:00',15);").unwrap();
        let db = DbReader { conn };
        let data = db
            .read_instantaneous(
                "samples",
                "Time",
                &["value".into()],
                Aggregation::Mean,
                "2026-01-01 00:00:00",
                "2026-01-02 00:00:00",
                &TrendBucket::Day,
            )
            .unwrap();
        assert_eq!(data.average, Some(42.));
        assert_eq!(
            db.get_signal_summary(
                "range_samples",
                "Time",
                "value",
                &SignalType::Counter,
                "2026-01-01 00:00:00",
                "2026-01-02 00:00:00"
            )
            .unwrap(),
            Some(5.)
        );
    }
    #[test]
    fn readonly_wal_snapshot_is_consistent_during_a_writer_commit() {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("telemetry-snapshot-{}-{id}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("test.db");
        let writer = Connection::open(&path).unwrap();
        writer.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE samples(Time TEXT); INSERT INTO samples VALUES ('2026-01-01 00:00:00');").unwrap();
        let reader = DbReader::open(path.to_str().unwrap()).unwrap();
        assert!(
            reader
                .conn
                .execute("INSERT INTO samples VALUES ('not allowed')", [])
                .is_err()
        );
        reader
            .with_read_snapshot(|reader| {
                let before = reader.get_latest_time("samples", "Time")?;
                writer
                    .execute("INSERT INTO samples VALUES ('2026-01-02 00:00:00')", [])
                    .unwrap();
                assert_eq!(reader.get_latest_time("samples", "Time")?, before);
                Ok(())
            })
            .unwrap();
        assert_eq!(
            reader.get_latest_time("samples", "Time").unwrap(),
            Some("2026-01-02 00:00:00".into())
        );
        drop(reader);
        drop(writer);
        std::fs::remove_dir_all(directory).unwrap();
    }
    fn reader() -> DbReader {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE readings(Time TEXT, a REAL, b REAL);
            INSERT INTO readings VALUES
            ('2026-01-01 00:00:00',100,0),
            ('2026-01-01 00:30:00',0,80),
            ('2026-01-01 01:00:00',NULL,20),
            ('2026-01-01 02:00:00',NULL,NULL),
            ('2026-01-02 00:00:00',1000,1000);",
        )
        .unwrap();
        DbReader { conn }
    }
    fn read(db: &DbReader, aggregation: Aggregation) -> InstantaneousData {
        db.read_instantaneous(
            "readings",
            "Time",
            &["a".into(), "b".into()],
            aggregation,
            "2026-01-01 00:00:00",
            "2026-01-02 00:00:00",
            &TrendBucket::Hour,
        )
        .unwrap()
    }
    #[test]
    fn peak_is_simultaneous_total_and_range_is_half_open() {
        let data = read(&reader(), Aggregation::Sum);
        assert_eq!(data.peak, Some(("2026-01-01 00:00:00".into(), 100.)));
        assert_eq!(data.average, Some(200. / 3.));
        assert_eq!(data.points[0].value, Some(90.));
        assert_eq!(data.points[1].value, Some(20.));
        assert_eq!(data.points[2].value, None);
        assert!(data.partial);
    }
    #[test]
    fn mean_uses_present_members_and_band_keeps_real_extremes() {
        let data = read(&reader(), Aggregation::Mean);
        assert_eq!(data.average, Some(110. / 3.));
        assert_eq!(data.minimum, Some(0.));
        assert_eq!(data.maximum, Some(100.));
        assert_eq!(data.points[0].value, Some(45.));
        assert_eq!(data.points[0].minimum, Some(0.));
        assert_eq!(data.points[0].maximum, Some(100.));
        assert_eq!(data.points[1].value, Some(20.));
        assert_eq!(data.points[2].value, None);
    }
    #[test]
    fn one_column_and_no_selected_columns_are_supported() {
        let db = reader();
        let one = db
            .read_instantaneous(
                "readings",
                "Time",
                &["a".into()],
                Aggregation::Mean,
                "2026-01-01 00:00:00",
                "2026-01-02 00:00:00",
                &TrendBucket::Day,
            )
            .unwrap();
        assert_eq!(one.average, Some(50.));
        let empty = db
            .read_instantaneous(
                "readings",
                "Time",
                &[],
                Aggregation::Sum,
                "a",
                "b",
                &TrendBucket::Day,
            )
            .unwrap();
        assert_eq!(empty.average, None);
    }
    #[test]
    fn latest_time_rejects_missing_identifier_instead_of_sqlite_string_literal() {
        let db = reader();
        assert_eq!(
            db.get_latest_time("readings", "Time").unwrap(),
            Some("2026-01-02 00:00:00".into())
        );
        assert!(matches!(
            db.get_latest_time("readings", "missing"),
            Err(DbError::ColumnNotFound(_))
        ));
    }
    #[test]
    fn read_snapshot_rolls_back_on_error_and_accepts_next_request() {
        let db = reader();
        let result: Result<(), DbError> =
            db.with_read_snapshot(|_| Err(DbError::UnsupportedOperation));
        assert!(result.is_err());
        assert!(
            db.with_read_snapshot(|reader| reader.get_latest_time("readings", "Time"))
                .is_ok()
        );
    }
    #[test]
    fn dashboard_combines_reset_counters_and_keeps_global_signal_under_filter() {
        use crate::{
            config::AppConfig,
            dashboard_data::{DashboardRead, read_dashboard},
        };
        let db = reader();
        db.conn
            .execute_batch(
                "ALTER TABLE readings ADD COLUMN ca REAL; ALTER TABLE readings ADD COLUMN cb REAL;
            UPDATE readings SET ca=10, cb=100 WHERE Time='2026-01-01 00:00:00';
            UPDATE readings SET ca=15, cb=110 WHERE Time='2026-01-01 00:30:00';
            UPDATE readings SET ca=0, cb=115 WHERE Time='2026-01-01 01:00:00';
            UPDATE readings SET ca=4, cb=120 WHERE Time='2026-01-01 02:00:00';",
            )
            .unwrap();
        let config:AppConfig=serde_json::from_value(serde_json::json!({
            "db_path":"unused","source":{"table_name":"readings","time_column_name":"Time","signals":[
                {"column":"ca","group":"Meter A","label":"Usage","unit":"u","signal_type":"Counter"},
                {"column":"cb","group":"Meter B","label":"Usage","unit":"u","signal_type":"Counter"},
                {"column":"a","group":"Ambient","label":"Environment","unit":"C","signal_type":"Instantaneous"}
            ]},"dashboard":{"kpis":[{"group":"Ambient","label":"Environment","operation":"summary"}],
                "trends":[{"group":"Meter A","label":"Usage"}],"group_summary_label":"Usage","global_labels":["Environment"]}
        })).unwrap();
        config.validate_setup().unwrap();
        let schema = db
            .discover_schema(config.source().as_ref().unwrap().clone())
            .unwrap();
        let mut query = DashboardRead {
            start: "2026-01-01 00:00:00".into(),
            end: "2026-01-02 00:00:00".into(),
            previous_start: String::new(),
            previous_end: String::new(),
            bucket: TrendBucket::Hour,
            group: None,
        };
        let all = read_dashboard(&db, &schema, config.dashboard(), &query).unwrap();
        assert_eq!(all.groups, vec!["Meter A", "Meter B"]);
        let usage = all.metrics.iter().find(|m| m.label == "Usage").unwrap();
        assert_eq!(usage.summary, Some(29.)); // A: 5+0+4, B:10+5+5; first readings have no baseline.
        assert_eq!(usage.points.len(), 24);
        assert_eq!(usage.points[0].value, Some(15.));
        assert_eq!(usage.points[3].value, None);
        query.group = Some("Meter A".into());
        let one = read_dashboard(&db, &schema, config.dashboard(), &query).unwrap();
        assert_eq!(
            one.metrics
                .iter()
                .find(|m| m.label == "Usage")
                .unwrap()
                .summary,
            Some(9.)
        );
        assert_eq!(
            one.metrics
                .iter()
                .find(|m| m.label == "Environment")
                .unwrap()
                .summary,
            Some(50.)
        );
        query.group = Some("Unknown".into());
        assert!(read_dashboard(&db, &schema, config.dashboard(), &query).is_err());
    }
}
