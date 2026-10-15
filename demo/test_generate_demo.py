"""Integration checks for the generated SQLite demo and retained WAL files."""

from contextlib import closing
import hashlib
import json
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import unittest


@unittest.skipUnless(
    hasattr(sqlite3.Connection, "setconfig")
    and hasattr(sqlite3, "SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE"),
    "Python 3.12+ with SQLite NO_CKPT_ON_CLOSE support is required",
)
class GenerateDemoTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory(prefix="furious-demo-test-")
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.output = Path(cls.temporary.name) / "demo"
        cls.script = Path(__file__).with_name("generate_demo.py")
        subprocess.run(
            [sys.executable, str(cls.script), "--output", str(cls.output)],
            check=True, capture_output=True, text=True,
        )
        cls.database = cls.output / "Database.db"

    def test_readonly_database_and_retained_sidecars(self):
        for suffix in ("-wal", "-shm"):
            self.assertTrue(Path(str(self.database) + suffix).is_file())
        with closing(sqlite3.connect(self.database.as_uri() + "?mode=ro", uri=True)) as db:
            self.assertEqual(db.execute("PRAGMA journal_mode").fetchone()[0], "wal")
            self.assertEqual(db.execute("PRAGMA integrity_check").fetchone()[0], "ok")
            self.assertEqual(
                db.execute("SELECT COUNT(*), MIN(Time), MAX(Time) FROM DataLogger1").fetchone(),
                (210529, "2024-01-01 00:00:00", "2026-01-01 00:00:00"),
            )
        # Check the checkpointed main file without consulting WAL sidecars.
        with closing(sqlite3.connect(self.database.as_uri() + "?immutable=1", uri=True)) as db:
            self.assertEqual(db.execute("SELECT COUNT(*) FROM DataLogger1").fetchone()[0], 210529)
        for suffix in ("-wal", "-shm"):
            self.assertTrue(Path(str(self.database) + suffix).is_file())

    def test_manifest_checksum_and_source_columns(self):
        manifest = json.loads((self.output / "dataset-manifest.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["sha256"], hashlib.sha256(self.database.read_bytes()).hexdigest())
        self.assertEqual(manifest["journal_mode"], "wal")
        self.assertEqual(manifest["rows"], 210529)
        self.assertEqual(len(manifest["monthly_totals"]), 24)
        source = json.loads((self.output / "demo-source.json").read_text(encoding="utf-8"))
        with closing(sqlite3.connect(self.database.as_uri() + "?mode=ro", uri=True)) as db:
            columns = {row[1] for row in db.execute('PRAGMA table_info("DataLogger1")')}
        self.assertIn(source["time_column_name"], columns)
        self.assertTrue(all(signal["column"] in columns for signal in source["signals"]))

    def test_existing_database_or_orphaned_sidecars_are_not_overwritten(self):
        before = hashlib.sha256(self.database.read_bytes()).hexdigest()
        result = subprocess.run(
            [sys.executable, str(self.script), "--output", str(self.output)],
            capture_output=True, text=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Refusing to overwrite", result.stderr)
        self.assertEqual(hashlib.sha256(self.database.read_bytes()).hexdigest(), before)

        for suffix in ("-wal", "-shm", "-journal"):
            output = Path(self.temporary.name) / suffix.removeprefix("-")
            output.mkdir()
            sidecar = output / ("Database.db" + suffix)
            sidecar.write_bytes(b"existing sidecar")
            result = subprocess.run(
                [sys.executable, str(self.script), "--output", str(output)],
                capture_output=True, text=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Refusing to overwrite", result.stderr)
            self.assertEqual(sidecar.read_bytes(), b"existing sidecar")
            self.assertFalse((output / "Database.db").exists())


if __name__ == "__main__":
    unittest.main()
