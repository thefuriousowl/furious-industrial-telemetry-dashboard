#!/usr/bin/env python3
"""Create an entirely synthetic, reproducible compressed-air demo.

Usage: python3 generate_demo.py --output demo
Requires Python 3.12+ and its standard library only, with SQLite's
SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE support. Produces Database.db, its WAL
sidecars, source configuration, documentation and a manifest.
The output directory must not already contain Database.db or its sidecars.
"""

import argparse
import calendar
import hashlib
import json
import math
import random
import sqlite3
from datetime import datetime, timedelta
from pathlib import Path

SEED = 20241014
STEP = timedelta(minutes=5)
METERS = [
    ("Assembly_A", "Assembly Area A", 0.34),
    ("Assembly_B", "Assembly Area B", 0.27),
    ("Packaging", "Packaging Area", 0.21),
    ("Warehouse", "Warehouse", 0.13),
    ("Workshop", "Workshop", 0.05),
]
MONTHLY_2025 = [200000, 210000, 230000, 220000, 240000, 250000,
                260000, 250000, 240000, 230000, 220000, 210000]
METRICS = [
    ("Flow_m3h", "Flow (m³/h)", "Instantaneous"),
    ("Total_m3", "Total (m³)", "Counter"),
    ("Pressure_bar", "Pressure (bar)", "Instantaneous"),
    ("Temperature_C", "Air temperature (°C)", "Instantaneous"),
]
NOTES = """SYNTHETIC COMPRESSED-AIR DEMO

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
  all 21 signals from demo/Database.db across five synthetic meter groups
  and a site-wide sensor, with their units and dashboard widgets. Preserve an existing installation's config.json.
- Run cargo run --offline from the repository root, then open
  http://127.0.0.1:3000/dashboard?base_date=2025-09-05&period=daily
- Set db_path in local config.json to the actual Database.db path.
- demo-source.json is the POST /api/source payload, not AppConfig.
- It maps all 21 signals using different display names from
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
"""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("demo"))
    args = parser.parse_args()
    if (not hasattr(sqlite3.Connection, "setconfig")
            or not hasattr(sqlite3, "SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE")):
        parser.error("Python 3.12+ with SQLite SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE support is required")
    out = args.output
    out.mkdir(parents=True, exist_ok=True)
    db_path = out / "Database.db"
    for suffix in ("", "-wal", "-shm", "-journal"):
        existing = Path(str(db_path) + suffix)
        if existing.exists():
            parser.error(f"Refusing to overwrite {existing}")

    columns = [f"{meter}_{suffix}" for meter, _, _ in METERS for suffix, _, _ in METRICS]
    columns.append("System_DewPoint_C")
    connection = sqlite3.connect(db_path)
    # Keep SQLite's own sidecars after closing; explicitly checkpoint below.
    connection.setconfig(sqlite3.SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, True)
    journal_mode = connection.execute("PRAGMA journal_mode=WAL").fetchone()[0]
    if journal_mode != "wal":
        connection.close()
        raise RuntimeError(f"Could not enable WAL mode: {journal_mode}")
    connection.execute("CREATE TABLE DataLogger1 (Time TEXT NOT NULL PRIMARY KEY, "
                       + ", ".join(f'"{name}" REAL' for name in columns) + ")")
    insert = "INSERT INTO DataLogger1 VALUES (" + ",".join("?" for _ in range(22)) + ")"
    counters = [100000.0, 80000.0, 60000.0, 40000.0, 20000.0]
    count = 0
    summary = []
    previous_values = None

    with connection:
        for year in (2024, 2025):
            for month in range(1, 13):
                rng = random.Random(SEED + year * 100 + month)
                begin = datetime(year, month, 1)
                samples = calendar.monthrange(year, month)[1] * 288
                flows = []
                for index in range(samples):
                    t = begin + index * STEP
                    hour = t.hour + t.minute / 60
                    shift = 1.3 if 7 <= hour < 19 else 0.48
                    weekend = 0.42 if t.weekday() == 6 else 0.72 if t.weekday() == 5 else 1.0
                    daily = 1 + 0.09 * math.sin(2 * math.pi * (t.day / 7))
                    row = []
                    for j, (_, _, weight) in enumerate(METERS):
                        variation = 1 + 0.055 * math.sin(hour * math.pi / 12 + j)
                        noise = rng.uniform(0.97, 1.03)
                        flow = 300 * weight * shift * weekend * daily * variation * noise
                        if j == 4 and year == 2025 and month == 8:
                            flow = 0.0
                        row.append(flow)
                    flows.append(row)

                target = MONTHLY_2025[month - 1] * (24 / 23 if year == 2024 else 1)
                scale = target / (sum(sum(row) for row in flows) / 12)
                flows = [[round(value * scale, 6) for value in row] for row in flows]
                residual = target - sum(sum(row) for row in flows) / 12
                flows[-1][0] = round(flows[-1][0] + residual * 12, 6)
                opening = sum(counters)
                batch = []

                for index, flow_row in enumerate(flows):
                    t = begin + index * STEP
                    hour = t.hour + t.minute / 60
                    total_flow = sum(flow_row)
                    ambient = 30.5 + 1.7 * math.sin(2 * math.pi * (month - 3) / 12)
                    ambient += 1.0 * math.sin(2 * math.pi * (hour - 8) / 24)
                    values = []
                    for j, (meter, _, _) in enumerate(METERS):
                        flow = flow_row[j]
                        pressure = round(6.0 - total_flow * 0.00075 + rng.uniform(-0.045, 0.045) - j * 0.025, 4)
                        temperature = round(ambient + 1.8 + j * 0.18 + rng.uniform(-0.15, 0.15), 4)
                        visible_flow = flow
                        if meter == "Packaging" and t.date().isoformat() == "2025-05-14" and 10 <= hour < 12:
                            visible_flow = None
                        if meter == "Assembly_B" and t.date().isoformat() == "2025-09-09" and 14 <= hour < 16:
                            pressure = None
                        if meter == "Warehouse" and t.date().isoformat() == "2025-07-22" and 11 <= hour < 13:
                            temperature = None
                        values.extend([visible_flow, round(counters[j], 6), pressure, temperature])
                        counters[j] += flow / 12
                    dew = round(5.2 + 0.38 * math.sin(2 * math.pi * month / 12)
                                + 0.18 * math.sin(2 * math.pi * hour / 24) + rng.uniform(-0.06, 0.06), 4)
                    if t.date().isoformat() == "2025-06-18" and 9 <= hour < 10:
                        dew = None
                    values.append(dew)
                    batch.append([t.strftime("%Y-%m-%d %H:%M:%S"), *values])
                    previous_values = values
                connection.executemany(insert, batch)
                count += samples
                summary.append({"month": f"{year}-{month:02}", "consumption_m3": round(sum(counters) - opening, 6)})

        closing = list(previous_values)
        for j in range(5):
            closing[j * 4 + 1] = round(counters[j], 6)
        connection.execute(insert, ["2026-01-01 00:00:00", *closing])
        count += 1

    integrity = connection.execute("PRAGMA integrity_check").fetchone()[0]
    checkpoint = connection.execute("PRAGMA wal_checkpoint(TRUNCATE)").fetchone()
    connection.close()
    if integrity != "ok":
        raise RuntimeError(f"Generated database integrity check failed: {integrity}")
    if checkpoint != (0, 0, 0):
        raise RuntimeError(f"Could not finish the WAL checkpoint: {checkpoint}")
    for suffix in ("-wal", "-shm"):
        sidecar = Path(str(db_path) + suffix)
        if not sidecar.is_file():
            raise RuntimeError(f"SQLite did not retain {sidecar}")
    signals = [{"column": f"{meter}_{suffix}", "group": label,
                "label": metric_label, "signal_type": kind}
               for meter, label, _ in METERS for suffix, metric_label, kind in METRICS]
    signals.append({"column": "System_DewPoint_C", "group": "Compressed Air System",
                    "label": "Pressure dew point (°C)", "signal_type": "Instantaneous"})
    source = {"table_name": "DataLogger1", "time_column_name": "Time", "signals": signals}
    (out / "demo-source.json").write_text(json.dumps(source, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    (out / "README.txt").write_text(NOTES, encoding="utf-8")
    report = {"synthetic": True, "seed": SEED, "rows": count, "columns": 22,
              "start": "2024-01-01 00:00:00", "closing_boundary": "2026-01-01 00:00:00",
              "timezone": "UTC", "cadence_minutes": 5, "integrity_check": integrity,
              "journal_mode": journal_mode,
              "sha256": hashlib.sha256(db_path.read_bytes()).hexdigest(),
              "sqlite_version": sqlite3.sqlite_version, "monthly_totals": summary}
    (out / "dataset-manifest.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"database": str(db_path), "rows": count, "bytes": db_path.stat().st_size,
                      "journal_mode": journal_mode,
                      "wal": str(db_path) + "-wal", "shm": str(db_path) + "-shm",
                      "integrity_check": integrity, "sha256": report["sha256"]}, indent=2))


if __name__ == "__main__":
    main()
