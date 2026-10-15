Just another simple sqlite reader and simple analytic dashboard.
The application opens telemetry databases read-only.
The demo contains entirely synthetic data covering 2024–2025.

Prerequisite:
- Rust toolchain
- Python 3.12+

Create demo sqlite telemetry db:

```bash
python3 demo/generate_demo.py --output demo
```

Configure the demo:

Copy `config.example.json` to `config.json` before starting the server.
For an existing installation, preserve its current `config.json`.

Alternatively, start without `config.json` and open `http://127.0.0.1:3000/`.
The application opens setup automatically and creates the config only when you save.
Browse selects a SQLite file on the computer running the server. Open the database,
choose its table and time column, and configure signal groups, labels, units and types.
Auto-generate suggests settings for names ending in `_Flow`, `_Pressure`, `_Temp`
or `_Total`; other column names can be configured manually.
Select Summary/Peak KPIs, trends, the main trend and an optional ranking label.
Preview config, then choose **Save and open dashboard** to apply it immediately.
Use **Configure** to edit the saved configuration later.

Save validates the new source and widgets before replacing the config file and
active database. A failed validation or file write retains the active configuration.

Start demo server:

```bash
cargo run
```

Open in a browser:

```text
http://127.0.0.1:3000/dashboard?base_date=2025-09-05&period=daily
```

Run test:
``` bash
cargo test --bin furious-industrial-telemetry-dashboard
python3 -m unittest discover -s demo -p 'test_*.py'
```

project is still in development phase

LICENSE MIT