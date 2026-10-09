"""Tests for scripts/rule-homes.py (todo TODO-050) — run with `just test-scripts`."""

import importlib.util
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "rule_homes", Path(__file__).resolve().parents[1] / "rule-homes.py"
)
rule_homes = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(rule_homes)


class Definitions(unittest.TestCase):
    # TODO-050 — the three shapes a rule is defined with are recognised; a mention is not.
    def test_definitions_and_mentions(self):
        text = "\n".join(
            [
                "**B24** — Use cases MAY depend on repository traits.",
                "## E11 — E2E tests stop at the ComboboxField boundary",
                "**MKT-143 — Current Value column (frontend + backend)**: text",
                "The canonical tree is **F0**; see B24 and MKT-143.",
            ]
        )
        self.assertEqual(rule_homes.defined_ids(text), ["B24", "E11", "MKT-143"])


class OneHome(unittest.TestCase):
    # TODO-050 — a rule defined in two places is reported with both homes.
    def test_a_rule_defined_twice_is_reported(self):
        found = {
            "docs/backend-rules.md": ["B24", "B25"],
            "CLAUDE.md": ["B24"],
        }
        self.assertEqual(
            rule_homes.duplicates(found),
            {"B24": ["CLAUDE.md", "docs/backend-rules.md"]},
        )

    # TODO-050 — every rule in one place: nothing reported.
    def test_one_home_each_reports_nothing(self):
        self.assertEqual(rule_homes.duplicates({"a.md": ["B1"], "b.md": ["E1"]}), {})


if __name__ == "__main__":
    unittest.main()
