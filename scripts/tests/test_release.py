"""Tests for scripts/release.py — run with `just test-scripts`."""

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

# release.py imports its sibling scripts (check.py) by name.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

SPEC = importlib.util.spec_from_file_location(
    "release", Path(__file__).resolve().parents[1] / "release.py"
)
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)


class LockfileVersion(unittest.TestCase):
    # TD-035 — a release leaves the lockfile stating the released version, and nothing else moves.
    def test_the_lockfile_states_the_released_version(self):
        with tempfile.TemporaryDirectory() as folder:
            lockfile = Path(folder) / "package-lock.json"
            lock = {
                "name": "folioneer",
                "version": "0.0.0",
                "lockfileVersion": 3,
                "requires": True,
                "packages": {
                    "": {"name": "folioneer", "version": "0.0.0", "dependencies": {"é": "1.0.0"}},
                    "node_modules/é": {"version": "1.0.0"},
                },
            }
            lockfile.write_text(json.dumps(lock, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

            release.set_lockfile_version(lockfile, "0.4.0")

            data = json.loads(lockfile.read_text(encoding="utf-8"))
            self.assertEqual(data["version"], "0.4.0")
            self.assertEqual(data["packages"][""]["version"], "0.4.0")
            self.assertEqual(data["packages"]["node_modules/é"]["version"], "1.0.0")
            lock["version"] = "0.4.0"
            lock["packages"][""]["version"] = "0.4.0"
            self.assertEqual(
                lockfile.read_text(encoding="utf-8"),
                json.dumps(lock, indent=2, ensure_ascii=False) + "\n",
            )


if __name__ == "__main__":
    unittest.main()
