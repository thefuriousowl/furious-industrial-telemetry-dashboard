SYNTHETIC COMPRESSED-AIR DEMO

All names, measurements, operating profiles and totals were invented.
No customer database, project file, customer area names or sensor export
was used to generate this dataset. The schema is a generic wide table,
not a claim to reproduce a particular industrial runtime's database.

Coverage: 2024-01-01 00:00:00 through 2026-01-01 00:00:00 inclusive.
Cadence: every 5 minutes; 210,529 rows, including one closing boundary row.
Time convention: UTC, stored as TEXT in YYYY-MM-DD HH:MM:SS format.
Use the same format and timezone for query bounds. Queries compare stored
timestamps directly; no timezone conversion is applied to these readings.

DataLogger1 contains Time plus 21 numeric signals: four per meter and
one system dew point. Sensor columns allow NULL. No extra user tables.
Time is a primary key and has an index for range queries.

The generated database uses WAL mode. SQLite creates Database.db-wal and
Database.db-shm and keeps them after the generator closes its connection.
All committed data is checkpointed into Database.db before it is hashed;
an empty WAL at the end of generation is expected. The application can
open this database read-only. A later read/write SQLite client may remove
the sidecars when its last connection closes.

Flow_m3h is an interval-held flow applying from its timestamp to the next
sample. Total_m3 is a cumulative counter at the timestamp. Thus:
total(t+5min) - total(t) = flow(t) / 12, within rounding tolerance.
The closing row's instantaneous readings carry forward the previous
sample; it exists to supply the closing counter and is not a new interval.
All flows/volumes use a consistent illustrative m³ basis. No conversion
between normal volume and actual volume is implied.

2025 monthly plant totals (m³):
200000,210000,230000,220000,240000,250000,
260000,250000,240000,230000,220000,210000.
Annual totals: 2024 = 2,880,000; 2025 = 2,760,000 m³ (down 4.1667%).
Every 2024 monthly target is the corresponding 2025 target times 24/23.
Load profiles have shift, weekend, seasonal and meter-specific variation.
Workshop has zero flow throughout August 2025; its counter stays constant.

Deliberate missing readings (UTC; start inclusive, end exclusive):
- Packaging_Flow_m3h: 2025-05-14 10:00 to 12:00.
- Assembly_B_Pressure_bar: 2025-09-09 14:00 to 16:00.
- Warehouse_Temperature_C: 2025-07-22 11:00 to 13:00.
- System_DewPoint_C: 2025-06-18 09:00 to 10:00.
Counters remain complete. NULL means missing, not zero. Dew point values
are synthetic like every other signal; no backend spoof is required.
No counter resets or timestamp gaps are included in this initial fixture.

Consumption must be derived from counter differences, not the sum of
counter readings. For exact period consumption, obtain the opening and
closing counters. The current telemetry endpoint excludes the end bound,
so request the closing timestamp separately or extend the requested end
by one sample and exclude that extra interval from averages/peaks.
For averaging flow with NULLs, explicitly define whether to use available
samples or counter-based interval consumption; do not silently use zero.
Five-minute data cannot represent true sub-five-minute instantaneous peaks.

Integration with the application:
- For a fresh checkout, copy config.example.json to config.json. It maps
  two columns from demo/Database.db to the display group Demo and includes
  dashboard widgets. Preserve an existing installation's config.json.
- Run cargo run --offline from the repository root, then open
  http://127.0.0.1:3000/dashboard?base_date=2025-09-05&period=daily
- Set db_path in local config.json to the actual Database.db path.
- demo-source.json is the POST /api/source payload, not AppConfig.
- It maps all 21 signals using different display names from the minimal
  config.example.json. When switching source, update dashboard groups and
  labels to match it as well. Relative database paths resolve from the
  server's working directory.
- Discovery: GET /api/tables and GET /api/tables/DataLogger1/columns.
- Configure source before GET /api/telemetry.
- Query parameters: columns (comma separated), start, end.
- The dashboard supports Daily, Weekly, Monthly and Yearly aggregation.
- The fixture includes the previous year for 2025. The first fixture year
  has no previous-year data, and January 2024 has no previous month.

Example raw query after configuring the source:
/api/telemetry?columns=Assembly_A_Flow_m3h,Assembly_A_Total_m3&start=2025-08-01%2000:00:00&end=2025-08-02%2000:00:00

Generate from the repository root using Python 3.12+:
python3 demo/generate_demo.py --output demo
On Windows, the equivalent command is:
py -3 demo/generate_demo.py --output demo
Python must provide Connection.setconfig and SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE.
No separate sqlite3 command-line program or third-party package is needed.
The fixed seed and formulas give repeatable measurements; byte-identical
SQLite files additionally depend on using the same SQLite/runtime version.
The generator refuses to overwrite an existing Database.db or its sidecars.
To try it without changing an existing demo, choose a new --output directory.

Track generate_demo.py, test_generate_demo.py, README.txt, demo-source.json
and config.example.json in Git. Database files, WAL/SHM sidecars and the
generated dataset-manifest.json are ignored and recreated locally.
Run the generator integration checks from the repository root with:
python3 -m unittest discover -s demo -p 'test_*.py'
