"""Tests on how the review workflow reads a report's verdict — run with `just test-scripts`.

The lines live in `.github/workflows/review.yml`, between `verdict:begin` and
`verdict:end`. They are pulled out of the file and run on headlines reviewers really wrote.
"""

import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path

WORKFLOW = Path(__file__).resolve().parents[2] / ".github" / "workflows" / "review.yml"


def verdict_lines() -> str:
    text = WORKFLOW.read_text()
    start = text.index("# verdict:begin")
    return textwrap.dedent(
        text[text.rindex("\n", 0, start) + 1 : text.index("# verdict:end")]
    )


def criticals(report: str) -> str:
    with tempfile.NamedTemporaryFile("w", suffix=".md") as file:
        file.write(report)
        file.flush()
        script = f'report="{file.name}"\n{verdict_lines()}\necho "$criticals"\n'
        return subprocess.run(
            ["bash", "-c", script], capture_output=True, text=True, check=True
        ).stdout.strip()


class Verdict(unittest.TestCase):
    def test_a_count_of_criticals_is_read(self):
        self.assertEqual(
            criticals("🔴 2 critical, 🟡 1 warning(s), 🔵 0 suggestion(s)"), "2"
        )
        self.assertEqual(
            criticals("Review complete: 🔴 4 critical, 🟡 2 warning(s)."), "4"
        )

    # Pull request #100, first round: a clean report headed with another sign.
    def test_a_zero_count_under_another_sign_is_clean(self):
        self.assertEqual(
            criticals("🔵 0 critical, 0 warning(s), 3 suggestion(s) across 2 file(s)."),
            "0",
        )

    # Pull request #100, second round: no criticals, and the count left out.
    def test_a_report_of_suggestions_only_is_clean(self):
        report = "## reviewer-infra — 2 files reviewed\n\n🔵 1 suggestion across 1 file(s).\n"
        self.assertEqual(criticals(report), "0")
        self.assertEqual(criticals("✅ No issues found."), "0")

    # A critical without a count line still fails the check.
    def test_a_critical_without_a_count_fails(self):
        self.assertEqual(
            criticals("### A — Coverage\n🔴 Rule PAY-020 has no command.\n"), "1"
        )

    # A report written with none of a report's signs cannot be read as clean.
    def test_a_report_that_cannot_be_read_fails(self):
        self.assertEqual(criticals("I could not review this pull request."), "1")
        self.assertEqual(criticals(""), "1")


if __name__ == "__main__":
    unittest.main()
