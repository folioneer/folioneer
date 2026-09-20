"""Tests for scripts/migration/from-vaultcompass.sh (todo #033) — run with `just test-scripts`."""

import hashlib
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "migration" / "from-vaultcompass.sh"
OLD = "com.phileggel.vault-compass"
NEW = "com.folioneer.desktop"


def digest(root: Path) -> dict[str, str]:
    """Relative path → SHA-256 of every file under `root`."""
    return {
        str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


class CarryOver(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.data_home = Path(self.tmp.name) / "share"
        self.config_home = Path(self.tmp.name) / "config"
        self.data_home.mkdir()
        self.config_home.mkdir()

    def tearDown(self):
        self.tmp.cleanup()

    def lay_out_vaultcompass(self):
        """The folders as the last VaultCompass release leaves them on Linux."""
        old = self.data_home / OLD
        for folder in ("logs", "localstorage", "storage/origin", "WebKitCache/Version 17", "CacheStorage"):
            (old / folder).mkdir(parents=True)
        (old / "portfolio").write_bytes(b"SQLite format 3\x00" + b"accounts" * 512)
        (old / "portfolio-wal").write_bytes(b"wal" * 1024)
        (old / "portfolio-shm").write_bytes(b"shm" * 64)
        (old / "logs/app.log").write_text("a very long log\n" * 100)
        (old / "localstorage/tauri_localhost_0.localstorage").write_bytes(b"theme=dark")
        (old / "storage/origin/data").write_bytes(b"lang=fr")
        (old / "WebKitCache/Version 17/blob").write_bytes(b"cache" * 100)
        (old / "CacheStorage/blob").write_bytes(b"cache")
        (old / "hsts-storage.sqlite").write_bytes(b"hsts")
        (self.config_home / OLD).mkdir()
        (self.config_home / OLD / ".window-state.json").write_text('{"main":{"width":1400}}')
        units = self.config_home / "systemd/user"
        units.mkdir(parents=True)
        (units / "vaultcompass-fetch.service").write_text("[Service]\n")
        (units / "vaultcompass-fetch.timer").write_text("[Timer]\n")
        return old

    def run_script(self):
        return subprocess.run(
            ["bash", str(SCRIPT), "--data-home", str(self.data_home), "--config-home", str(self.config_home),
             "--no-systemctl", "--no-running-check"],
            capture_output=True,
            text=True,
            env={**os.environ, "NO_COLOR": "1"},
            check=False,
        )

    def test_the_data_is_copied_identical_and_the_old_folder_is_left_untouched(self):
        old = self.lay_out_vaultcompass()
        before = digest(old)

        result = self.run_script()

        self.assertEqual(result.returncode, 0, result.stderr)
        new = self.data_home / NEW
        copied = digest(new)
        for name in ("portfolio", "portfolio-wal", "portfolio-shm",
                     "localstorage/tauri_localhost_0.localstorage", "storage/origin/data"):
            self.assertEqual(copied.get(name), before[name], name)
        self.assertEqual(digest(old), before, "the old folder must not change")
        self.assertEqual(
            (self.config_home / NEW / ".window-state.json").read_text(), '{"main":{"width":1400}}'
        )

    def test_logs_and_caches_stay_behind(self):
        self.lay_out_vaultcompass()

        self.assertEqual(self.run_script().returncode, 0)

        new = self.data_home / NEW
        for left in ("logs", "WebKitCache", "CacheStorage", "hsts-storage.sqlite"):
            self.assertFalse((new / left).exists(), left)

    def test_the_scheduler_files_registered_under_the_old_name_are_removed(self):
        self.lay_out_vaultcompass()

        self.assertEqual(self.run_script().returncode, 0)

        units = self.config_home / "systemd/user"
        self.assertFalse((units / "vaultcompass-fetch.service").exists())
        self.assertFalse((units / "vaultcompass-fetch.timer").exists())

    def test_an_existing_folioneer_database_is_never_overwritten(self):
        self.lay_out_vaultcompass()
        new = self.data_home / NEW
        new.mkdir()
        (new / "portfolio").write_bytes(b"already here")

        result = self.run_script()

        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((new / "portfolio").read_bytes(), b"already here")
        self.assertIn("already", result.stderr.lower())

    def test_nothing_to_carry_over_is_not_an_error(self):
        result = self.run_script()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((self.data_home / NEW).exists())


if __name__ == "__main__":
    unittest.main()
