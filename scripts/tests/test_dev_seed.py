"""Tests for scripts/dev-seed.py (todo #040) — run with `just test-scripts`."""

import hashlib
import importlib.util
import sqlite3
import tempfile
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "dev_seed", Path(__file__).resolve().parents[1] / "dev-seed.py"
)
dev_seed = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(dev_seed)


def make_installed_database(folder):
    """A database shaped like the installed application's: data, a sync membership, a daily fetch."""
    folder.mkdir(parents=True)
    connection = sqlite3.connect(folder / "portfolio")
    connection.executescript(
        """
        CREATE TABLE accounts (id TEXT PRIMARY KEY, name TEXT NOT NULL);
        INSERT INTO accounts VALUES ('a1', 'Brokerage');
        CREATE TABLE sync_device (id INTEGER PRIMARY KEY, device_id TEXT NOT NULL);
        INSERT INTO sync_device VALUES (1, 'PHIL02');
        CREATE TABLE scheduled_fetch_configuration (id INTEGER PRIMARY KEY, enabled INTEGER NOT NULL);
        INSERT INTO scheduled_fetch_configuration VALUES (1, 1);
        """
    )
    connection.commit()
    connection.close()


def fingerprint(folder):
    """Name, content hash and modification time of every file in a folder."""
    return {
        path.name: (hashlib.sha256(path.read_bytes()).hexdigest(), path.stat().st_mtime_ns)
        for path in sorted(folder.iterdir())
    }


class SeedTest(unittest.TestCase):
    def setUp(self):
        self.base = Path(self.enterContext(tempfile.TemporaryDirectory()))
        self.installed = self.base / "com.folioneer.desktop"
        self.development = self.base / "com.folioneer.desktop.dev"
        make_installed_database(self.installed)

    def query(self, sql):
        connection = sqlite3.connect(self.development / "portfolio")
        try:
            return connection.execute(sql).fetchall()
        finally:
            connection.close()

    def test_the_copy_holds_the_portfolio_and_the_source_is_untouched(self):
        before = fingerprint(self.installed)

        dev_seed.seed(self.installed, self.development, replace=False)

        self.assertEqual(self.query("SELECT name FROM accounts"), [("Brokerage",)])
        self.assertEqual(fingerprint(self.installed), before)

    def test_writes_still_in_the_write_ahead_log_reach_the_copy(self):
        running = sqlite3.connect(self.installed / "portfolio")
        self.addCleanup(running.close)
        running.execute("PRAGMA journal_mode = WAL")
        running.execute("INSERT INTO accounts VALUES ('a2', 'Life insurance')")
        running.commit()
        self.assertTrue((self.installed / "portfolio-wal").stat().st_size > 0)
        before = fingerprint(self.installed)

        dev_seed.seed(self.installed, self.development, replace=False)

        self.assertEqual(
            self.query("SELECT name FROM accounts ORDER BY id"),
            [("Brokerage",), ("Life insurance",)],
        )
        self.assertEqual(fingerprint(self.installed), before)

    def test_the_copy_is_no_longer_a_member_of_the_sync_folder(self):
        dev_seed.seed(self.installed, self.development, replace=False)

        self.assertEqual(self.query("SELECT COUNT(*) FROM sync_device"), [(0,)])

    def test_the_copy_has_the_daily_fetch_disabled(self):
        dev_seed.seed(self.installed, self.development, replace=False)

        self.assertEqual(
            self.query("SELECT enabled FROM scheduled_fetch_configuration"), [(0,)]
        )

    def test_an_existing_development_database_is_kept_unless_replace_is_asked(self):
        dev_seed.seed(self.installed, self.development, replace=False)
        connection = sqlite3.connect(self.development / "portfolio")
        connection.execute("UPDATE accounts SET name = 'Edited in development'")
        connection.commit()
        connection.close()

        with self.assertRaises(dev_seed.SeedRefused):
            dev_seed.seed(self.installed, self.development, replace=False)
        self.assertEqual(self.query("SELECT name FROM accounts"), [("Edited in development",)])

        dev_seed.seed(self.installed, self.development, replace=True)
        self.assertEqual(self.query("SELECT name FROM accounts"), [("Brokerage",)])

    def test_a_copy_that_cannot_be_detached_is_not_left_behind(self):
        (self.installed / "portfolio").write_bytes(b"not a database")
        before = fingerprint(self.installed)

        with self.assertRaises(sqlite3.Error):
            dev_seed.seed(self.installed, self.development, replace=False)

        self.assertEqual(list(self.development.iterdir()), [])
        self.assertEqual(fingerprint(self.installed), before)

    def test_a_missing_installed_database_is_refused(self):
        with self.assertRaises(dev_seed.SeedRefused):
            dev_seed.seed(self.base / "nowhere", self.development, replace=False)

    def test_the_two_folders_sit_side_by_side_under_the_platform_data_folder(self):
        installed, development = dev_seed.folders(Path("/data"))

        self.assertEqual(installed, Path("/data/com.folioneer.desktop"))
        self.assertEqual(development, Path("/data/com.folioneer.desktop.dev"))


if __name__ == "__main__":
    unittest.main()
