"""Tests for scripts/rule-homes.py — run with `just test-scripts`."""

import importlib.util
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "rule_homes", Path(__file__).resolve().parents[1] / "rule-homes.py"
)
rule_homes = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(rule_homes)


class Definitions(unittest.TestCase):
    # The three shapes a rule is defined with are recognised; a mention is not.
    def test_definitions_and_mentions(self):
        text = "\n".join(
            [
                "**B24** — Use cases MAY depend on repository traits.",
                "## E11 — E2E tests stop at the ComboboxField boundary",
                "**MKT-143 — Current Value column (frontend + backend)**: text",
                "The canonical tree is **F0**; see B24 and MKT-143.",
            ]
        )
        self.assertEqual(
            rule_homes.defined_ids(text),
            ["B24", "E11", "MKT-143"],
        )


class OneHome(unittest.TestCase):
    # A rule defined in two places is reported with both homes.
    def test_a_rule_defined_twice_is_reported(self):
        found = {
            "docs/backend-rules.md": ["B24", "B25"],
            "CLAUDE.md": ["B24"],
        }
        self.assertEqual(
            rule_homes.duplicates(found),
            {"B24": ["CLAUDE.md", "docs/backend-rules.md"]},
        )

    # Every rule in one place: nothing reported.
    def test_one_home_each_reports_nothing(self):
        self.assertEqual(rule_homes.duplicates({"a.md": ["B1"], "b.md": ["E1"]}), {})


class AgenticSetup(unittest.TestCase):
    """Every script, skill, agent prompt and hook has its line in the setup document."""

    SETUP = "\n".join(
        [
            "## Generic",
            "- `scripts/merge.py` — merges a green branch. Never: merges a red one.",
            "- `.claude/skills/run-entry/` — runs one entry.",
            "  It names `scripts/harness.sh` in its prose: not a listed piece.",
            "- `docs/workflow.md` — how work moves.",
            "## This project",
            "- `.githooks/pre-push` — the fast checks.",
        ]
    )

    def test_a_piece_is_listed_where_its_path_opens_a_list_item(self):
        self.assertEqual(
            rule_homes.listed_pieces(self.SETUP),
            [
                "scripts/merge.py",
                ".claude/skills/run-entry/",
                "docs/workflow.md",
                ".githooks/pre-push",
            ],
        )

    def test_the_pieces_are_the_scripts_skills_agent_prompts_and_hooks(self):
        files = [
            "scripts/merge.py",
            "scripts/tests/test_merge.py",
            "scripts/verdict.sh",
            ".claude/skills/run-entry/SKILL.md",
            ".claude/skills/run-entry/template.md",
            ".claude/agents/reviewer-infra.md",
            ".claude/rules/backend.md",
            ".claude/settings.json",
            ".githooks/pre-push",
            ".githooks/README.md",
            "docs/workflow.md",
            "src/main.ts",
        ]
        self.assertEqual(
            rule_homes.setup_pieces(files),
            [
                ".claude/agents/reviewer-infra.md",
                ".claude/skills/run-entry/",
                ".githooks/pre-push",
                "scripts/merge.py",
                "scripts/verdict.sh",
            ],
        )

    def test_a_piece_that_exists_without_a_line_is_reported(self):
        pieces = ["scripts/merge.py", "scripts/new-tool.py"]
        self.assertEqual(
            rule_homes.setup_problems(
                pieces, ["scripts/merge.py"], exists=lambda path: True
            ),
            ["scripts/new-tool.py: exists but has no line in docs/agentic-setup.md"],
        )

    def test_a_line_whose_piece_is_gone_is_reported(self):
        listed = ["scripts/merge.py", "scripts/gone.py", "docs/workflow.md"]
        self.assertEqual(
            rule_homes.setup_problems(
                ["scripts/merge.py"],
                listed,
                exists=lambda path: path != "scripts/gone.py",
            ),
            ["scripts/gone.py: listed in docs/agentic-setup.md but missing"],
        )

    def test_a_piece_listed_twice_is_reported(self):
        listed = ["scripts/merge.py", "scripts/merge.py"]
        self.assertEqual(
            rule_homes.setup_problems(
                ["scripts/merge.py"], listed, exists=lambda path: True
            ),
            ["scripts/merge.py: listed twice in docs/agentic-setup.md"],
        )

    def test_this_repository_lists_every_piece_it_holds(self):
        text = (rule_homes.ROOT / rule_homes.SETUP).read_text(encoding="utf-8")
        pieces = rule_homes.setup_pieces(
            rule_homes.tracked("scripts", ".claude", ".githooks")
        )
        self.assertEqual(
            rule_homes.setup_problems(
                pieces,
                rule_homes.listed_pieces(text),
                exists=lambda path: (rule_homes.ROOT / path).exists(),
            ),
            [],
        )


if __name__ == "__main__":
    unittest.main()
