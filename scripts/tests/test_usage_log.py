"""Tests for scripts/usage_log.py and scripts/usage-log.sh — run with `just test-scripts`."""

import importlib.util
import os
import subprocess
import sys
import tempfile
import unittest
from datetime import datetime, timezone
from pathlib import Path
from unittest import mock

SCRIPTS = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("usage_log", SCRIPTS / "usage_log.py")
usage = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(usage)

NOON = datetime(2026, 10, 4, 12, 0, 0, tzinfo=timezone.utc)


class Record(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.log = Path(self.dir.name) / "logs" / "usage.log"
        patch = mock.patch.dict(os.environ, {"USAGE_LOG": str(self.log)})
        patch.start()
        self.addCleanup(patch.stop)
        os.environ.pop("CI", None)

    def test_flow_035_a_run_is_one_line_time_tool_duration_result(self):
        usage.record("merge.py", 12.34, "ok", now=NOON)
        usage.record("harness.sh", 0.5, "exit 1", now=NOON)
        self.assertEqual(
            self.log.read_text(encoding="utf-8").splitlines(),
            ["2026-10-04T12:00:00Z\tmerge.py\t12.3s\tok", "2026-10-04T12:00:00Z\tharness.sh\t0.5s\texit 1"],
        )

    def test_flow_035_a_recipe_without_a_script_is_logged_as_started(self):
        usage.record("format", None, "started", now=NOON)
        self.assertEqual(self.log.read_text(encoding="utf-8"), "2026-10-04T12:00:00Z\tformat\t-\tstarted\n")

    def test_flow_035_the_log_is_capped_and_drops_the_oldest_lines(self):
        self.log.parent.mkdir(parents=True)
        self.log.write_text("".join(f"line {n}\n" for n in range(usage.MAX_LINES)), encoding="utf-8")
        usage.record("check.py", 1.0, "ok", now=NOON)
        lines = self.log.read_text(encoding="utf-8").splitlines()
        self.assertEqual(len(lines), usage.MAX_LINES)
        self.assertEqual(lines[0], "line 1")
        self.assertTrue(lines[-1].endswith("\tcheck.py\t1.0s\tok"))

    def test_nothing_is_written_in_ci_or_when_switched_off(self):
        with mock.patch.dict(os.environ, {"CI": "true"}):
            usage.record("check.py", 1.0, "ok")
        with mock.patch.dict(os.environ, {"USAGE_LOG": "off"}):
            usage.record("check.py", 1.0, "ok")
        self.assertFalse(self.log.exists())

    def test_a_log_that_cannot_be_written_never_breaks_the_tool(self):
        blocker = Path(self.dir.name) / "file"
        blocker.write_text("", encoding="utf-8")
        with mock.patch.dict(os.environ, {"USAGE_LOG": str(blocker / "usage.log")}):
            usage.record("check.py", 1.0, "ok")  # the parent is a file: no exception

    def test_a_log_that_cannot_be_read_back_never_breaks_the_tool(self):
        self.log.parent.mkdir(parents=True)
        self.log.write_bytes(b"\xff\xfe not utf-8\n")
        usage.record("check.py", 1.0, "ok")  # no exception

    def test_an_exit_code_reads_as_ok_or_exit_n(self):
        self.assertEqual([usage.result(code) for code in (0, None, 2, "boom")], ["ok", "ok", "exit 2", "exit 1"])


class Entrypoints(unittest.TestCase):
    """The three ways a tool reaches the log, each run as the real process."""

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        self.log = Path(self.dir.name) / "usage.log"
        self.env = {k: v for k, v in os.environ.items() if k != "CI"} | {"USAGE_LOG": str(self.log)}

    def lines(self):
        return [line.split("\t") for line in self.log.read_text(encoding="utf-8").splitlines()]

    def run_python(self, body):
        script = Path(self.dir.name) / "tool.py"
        script.write_text(f"import sys\nsys.path.insert(0, {str(SCRIPTS)!r})\nimport usage_log\nusage_log.start()\n{body}\n", encoding="utf-8")
        return subprocess.run([sys.executable, str(script)], env=self.env, capture_output=True, text=True)

    def test_flow_035_a_python_script_logs_its_exit_code(self):
        self.assertEqual(self.run_python("sys.exit(3)").returncode, 3)
        self.assertEqual(self.run_python("pass").returncode, 0)
        self.assertEqual(self.run_python("raise RuntimeError('x')").returncode, 1)
        self.assertEqual(self.run_python("sys.exit('refused, with its reason')").returncode, 1)
        self.assertEqual(
            [(tool, outcome) for _, tool, _, outcome in self.lines()],
            [("tool.py", "exit 3"), ("tool.py", "ok"), ("tool.py", "exit 1"), ("tool.py", "exit 1")],
        )

    # A raised SystemExit would be logged as ok: no script that logs raises one.
    def test_no_logged_script_raises_system_exit(self):
        raising = [
            path.name
            for path in SCRIPTS.glob("*.py")
            if "usage_log.start()" in path.read_text(encoding="utf-8")
            and "raise SystemExit" in path.read_text(encoding="utf-8")
        ]
        self.assertEqual(raising, [])

    def test_flow_035_a_shell_script_logs_its_exit_code(self):
        script = Path(self.dir.name) / "tool.sh"
        script.write_text(f'#!/usr/bin/env bash\nset -euo pipefail\n. "{SCRIPTS}/usage-log.sh"\nexit 2\n', encoding="utf-8")
        result = subprocess.run(["bash", str(script)], env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 2)
        self.assertEqual([(tool, outcome) for _, tool, _, outcome in self.lines()], [("tool.sh", "exit 2")])

    def test_flow_035_a_recipe_logs_through_the_command_line(self):
        subprocess.run([sys.executable, str(SCRIPTS / "usage_log.py"), "used", "format"], env=self.env, check=True)
        self.assertEqual([(tool, took, outcome) for _, tool, took, outcome in self.lines()], [("format", "-", "started")])


if __name__ == "__main__":
    unittest.main()
