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