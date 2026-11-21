"""Tests for .githooks/pre-commit and .githooks/pre-push — run with `just test-scripts`."""

import unittest
from pathlib import Path

HOOKS = Path(__file__).resolve().parents[2] / ".githooks"


class FailClosed(unittest.TestCase):
    # #704 — a hook whose check script is missing fails: it never skips the checks and
    # lets the commit or the push through (daneel's reviewer-infra: failing closed is intended).
    def test_no_hook_lets_a_change_through_when_its_check_script_is_missing(self):
        for name in ("pre-commit", "pre-push"):
            text = (HOOKS / name).read_text(encoding="utf-8")
            self.assertIn('python3 "$CHECK_SCRIPT"', text, name)
            self.assertNotIn('[ ! -f "$CHECK_SCRIPT" ]', text, name)
            self.assertNotIn("Skipping validation", text, name)


if __name__ == "__main__":
    unittest.main()
