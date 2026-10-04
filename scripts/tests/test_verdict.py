"""Tests for scripts/verdict.sh (FLOW-001) — run with `just test-scripts`."""

import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "verdict.sh"


def verdict(*command: str) -> tuple[int, str, str]:
    """Run verdict.sh in a scratch folder; return its exit code, what it printed and
    the log it kept."""
    with tempfile.TemporaryDirectory() as folder:
        result = subprocess.run(
            ["bash", str(SCRIPT), "suite", *command],
            cwd=folder,
            capture_output=True,
            text=True,
        )
        log = Path(folder, "tmp", "suite.log")
        return result.returncode, result.stdout, log.read_text() if log.exists() else ""


class Verdict(unittest.TestCase):
    # FLOW-001 — a passing command gives one line and keeps its output out of the way.
    def test_a_passing_command_prints_one_line_and_keeps_its_output(self):
        code, printed, log = verdict("sh", "-c", "echo building; echo done")
        self.assertEqual(code, 0)
        self.assertEqual(len(printed.strip().splitlines()), 1)
        self.assertRegex(printed, r"^✅ suite: passed in \d+s — log: tmp/suite\.log")
        self.assertEqual(log, "building\ndone\n")

    # FLOW-001 — the case that was missed three times: every test reported as passing,
    # and an exit code that says otherwise. The exit code is the verdict.
    def test_output_that_reads_as_a_success_does_not_hide_a_failing_exit_code(self):
        code, printed, _ = verdict(
            "sh",
            "-c",
            "echo ' Test Files  183 passed (183)'; echo 'Unhandled Errors' >&2; exit 1",
        )
        self.assertEqual(code, 1)
        self.assertRegex(printed.splitlines()[0], r"^❌ suite: FAILED \(exit 1\)")
        self.assertIn("Unhandled Errors", printed)
        self.assertIn("Test Files  183 passed", printed)

    # The end of the log follows a failure, as many lines as asked.
    def test_a_failure_shows_the_end_of_the_log(self):
        with tempfile.TemporaryDirectory() as folder:
            result = subprocess.run(
                ["bash", str(SCRIPT), "suite", "sh", "-c", "seq 1 50; exit 1"],
                cwd=folder,
                capture_output=True,
                text=True,
                env={"PATH": "/usr/bin:/bin", "VERDICT_TAIL": "3"},
            )
        self.assertEqual(result.stdout.splitlines()[2:], ["48", "49", "50"])

    # A second run replaces the log; nothing of the first is left in it.
    def test_a_second_run_replaces_the_log(self):
        with tempfile.TemporaryDirectory() as folder:
            for word in ("first", "second"):
                subprocess.run(
                    ["bash", str(SCRIPT), "suite", "echo", word],
                    cwd=folder,
                    capture_output=True,
                )
            self.assertEqual(Path(folder, "tmp", "suite.log").read_text(), "second\n")
            self.assertEqual(
                [p.name for p in Path(folder, "tmp").iterdir()], ["suite.log"]
            )

    # A name that would leave tmp/ is refused before anything runs.
    def test_a_name_that_would_leave_the_log_folder_is_refused(self):
        for name in ("../x", "a/b", "..", ""):
            with tempfile.TemporaryDirectory() as folder:
                result = subprocess.run(
                    ["bash", str(SCRIPT), name, "touch", "ran"],
                    cwd=folder,
                    capture_output=True,
                    text=True,
                )
                self.assertEqual(result.returncode, 2, name)
                self.assertFalse(Path(folder, "ran").exists(), name)

    # A command that asks a question reads nothing and fails at once, instead of waiting
    # unseen behind the hidden output.
    def test_a_command_that_reads_gets_nothing(self):
        code, _, log = verdict(
            "sh", "-c", "read answer && echo got || { echo no-input; exit 4; }"
        )
        self.assertEqual(code, 4)
        self.assertEqual(log, "no-input\n")

    # The command's own exit code comes back, so a caller can chain on it.
    def test_the_exit_code_is_the_commands(self):
        self.assertEqual(verdict("sh", "-c", "exit 7")[0], 7)

    # A command that cannot be started is a failure too, not a silent pass.
    def test_a_command_that_does_not_exist_fails(self):
        code, printed, _ = verdict("no-such-command-here")
        self.assertEqual(code, 127)
        self.assertIn("FAILED", printed)

    def test_without_a_command_it_says_how_to_call_it(self):
        result = subprocess.run(
            ["bash", str(SCRIPT), "suite"], capture_output=True, text=True
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("usage", result.stderr)


if __name__ == "__main__":
    unittest.main()
