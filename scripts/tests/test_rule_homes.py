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
                "# TODO-003 — (frontend) — An entry",
                "# 2026-07-29 — DEBT-008 — An observation",
                "# FLOW-004 — A flow entry",
                "See TODO-003 — and `# FLOW-004 —` in the queue.",
            ]
        )
        self.assertEqual(
            rule_homes.defined_ids(text),
            ["B24", "E11", "MKT-143", "TODO-003", "DEBT-008", "FLOW-004"],
        )


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


class EntryFiles(unittest.TestCase):
    """A todo, debt or flow entry is one file, named by the one ID it defines."""

    def test_an_entry_named_after_its_id_in_its_folder_passes(self):
        found = {
            "docs/work/debt/DEBT-008.md": ["DEBT-008"],
            "docs/work/audits.md": [],
            "docs/work/queue.md": [],
            "docs/backend-rules.md": ["B1", "B2"],
        }
        self.assertEqual(rule_homes.misnamed(found), [])

    def test_a_name_that_disagrees_with_the_heading_is_reported(self):
        found = {
            "docs/work/debt/DEBT-008.md": ["DEBT-009"],
            "docs/work/flow/FLOW-001.md": [],
            "docs/work/todo/TODO-002.md": ["TODO-002", "TODO-003"],
        }
        self.assertEqual(
            rule_homes.misnamed(found),
            [
                "docs/work/debt/DEBT-008.md: defines DEBT-009, not the one ID DEBT-008 it is named after",
                "docs/work/flow/FLOW-001.md: defines no ID, not the one ID FLOW-001 it is named after",
                "docs/work/todo/TODO-002.md: defines TODO-002, TODO-003, not the one ID TODO-002 it is named after",
            ],
        )

    def test_a_file_of_another_kind_or_another_name_is_reported(self):
        found = {
            "docs/work/todo/DEBT-008.md": ["DEBT-008"],
            "docs/work/flow/notes.md": [],
            "docs/work/debt/DEBT-8.md": [],
        }
        self.assertEqual(
            rule_homes.misnamed(found),
            [
                "docs/work/debt/DEBT-8.md: an entry of debt/ is named DEBT-NNN.md",
                "docs/work/flow/notes.md: an entry of flow/ is named FLOW-NNN.md",
                "docs/work/todo/DEBT-008.md: an entry of todo/ is named TODO-NNN.md",
            ],
        )


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
