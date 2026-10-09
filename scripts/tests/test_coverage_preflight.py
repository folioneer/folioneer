"""Tests for scripts/coverage-preflight.sh — a coverage run this machine would kill is
refused before it starts — run with `just test-scripts`."""

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "coverage-preflight.sh"
GIB = 1024 * 1024


def preflight(total_kib, foreground=None, meminfo_text=None):
    """Runs the preflight with no terminal attached (its output is captured) on a
    machine whose memory is `total_kib`."""
    with tempfile.NamedTemporaryFile("w", suffix="-meminfo", delete=False) as meminfo:
        meminfo.write(
            meminfo_text
            if meminfo_text is not None
            else f"MemTotal:       {total_kib} kB\nMemFree:         123456 kB\n"
        )
    env = {k: v for k, v in os.environ.items() if k != "HARNESS_FOREGROUND"}
    env["HARNESS_MEMINFO"] = meminfo.name
    if foreground is not None:
        env["HARNESS_FOREGROUND"] = foreground
    try:
        return subprocess.run(
            ["bash", str(SCRIPT)], env=env, capture_output=True, text=True, check=False
        )
    finally:
        os.unlink(meminfo.name)


class CoveragePreflight(unittest.TestCase):
    def test_a_detached_run_on_a_small_machine_is_refused_with_the_way_out(self):
        for total_kib in (7302520, 8 * GIB):
            refused = preflight(total_kib)
            self.assertEqual(refused.returncode, 3, total_kib)
            self.assertIn("refused", refused.stderr)
            self.assertIn("HARNESS_FOREGROUND=1 just harness --coverage", refused.stderr)
            self.assertEqual(refused.stdout, "")

    def test_the_message_states_the_memory_it_read(self):
        self.assertIn("7.0 GiB of memory", preflight(7302520).stderr)

    def test_a_foreground_run_without_a_terminal_says_so_and_goes(self):
        allowed = preflight(7302520, foreground="1")
        self.assertEqual((allowed.returncode, allowed.stderr), (0, ""))

    def test_only_the_stated_value_holds_the_foreground(self):
        for value in ("", "0", "true", "yes"):
            self.assertEqual(preflight(7302520, foreground=value).returncode, 3, value)

    def test_a_machine_with_more_memory_goes_detached(self):
        self.assertEqual(preflight(8 * GIB + 1).returncode, 0)
        self.assertEqual(preflight(16 * GIB).returncode, 0)

    def test_a_machine_whose_memory_cannot_be_read_goes(self):
        self.assertEqual(preflight(0, meminfo_text="").returncode, 0)
        self.assertEqual(preflight(0, meminfo_text="MemTotal: lots kB\n").returncode, 0)
        env = {k: v for k, v in os.environ.items() if k != "HARNESS_FOREGROUND"}
        env["HARNESS_MEMINFO"] = "/nonexistent/meminfo"
        missing = subprocess.run(
            ["bash", str(SCRIPT)], env=env, capture_output=True, text=True, check=False
        )
        self.assertEqual(missing.returncode, 0)


class HarnessAsksFirst(unittest.TestCase):
    def test_the_harness_runs_the_preflight_before_anything_else_under_coverage(self):
        harness = (SCRIPT.parent / "harness.sh").read_text()
        asked = harness.index("bash scripts/coverage-preflight.sh")
        self.assertLess(asked, harness.index("python3 scripts/arch-check.py"))
        guard = harness.rindex('if [ "$coverage" = true ]; then', 0, asked)
        self.assertEqual(harness[guard:asked].count("\n"), 1)


if __name__ == "__main__":
    unittest.main()
