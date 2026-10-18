"""Tests for scripts/review-lanes.sh — run with `just test-scripts`."""

import json
import os
import re
import subprocess
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "review-lanes.sh"
WORKFLOW = SCRIPT.parents[1] / ".github" / "workflows" / "review.yml"


def lanes(*paths, as_json=False):
    args = ["bash", str(SCRIPT)] + (["--json"] if as_json else [])
    out = subprocess.run(
        args,
        input="\n".join(paths) + "\n",
        capture_output=True,
        text=True,
        check=True,
        env=os.environ | {"USAGE_LOG": "off"},
    ).stdout
    return json.loads(out) if as_json else out.split()


class ReviewLanes(unittest.TestCase):
    def test_rust_fires_backend_and_arch(self):
        self.assertEqual(lanes("src-tauri/src/context/account/domain/wrapper.rs"), ["backend", "arch"])

    def test_typescript_fires_frontend_and_arch(self):
        self.assertEqual(lanes("src/features/accounts/useWrapperLists.ts"), ["frontend", "arch"])

    def test_an_e2e_spec_fires_e2e_only(self):
        self.assertEqual(lanes("e2e/split/split.test.ts"), ["e2e"])

    def test_a_migration_fires_sql(self):
        self.assertEqual(lanes("src-tauri/migrations/202610180001_wrappers.sql"), ["sql"])

    def test_a_command_file_fires_security(self):
        self.assertIn("security", lanes("src-tauri/src/context/account/api.rs"))

    def test_specs_and_contracts_fire_their_reviewers(self):
        self.assertEqual(lanes("docs/spec/account.md", "docs/contracts/account-contract.md"), ["spec", "contract"])

    def test_guides_fire_the_guides_lane(self):
        for path in (
            "CLAUDE.md",
            "docs/workflow.md",
            "docs/agentic-setup.md",
            "docs/frontend-rules.md",
            ".claude/skills/run-entry/SKILL.md",
            ".claude/agents/reviewer-arch.md",
            ".claude/rules/backend.md",
        ):
            self.assertEqual(lanes(path), ["guides"], path)

    def test_a_document_that_is_not_a_guide_fires_nothing(self):
        self.assertEqual(lanes("docs/README.md", "docs/adr/006-unit-of-work.md", "README.md"), [])

    def test_json_names_every_lane(self):
        self.assertEqual(
            set(lanes("justfile", as_json=True)),
            {"backend", "frontend", "arch", "sql", "infra", "security", "e2e", "spec", "contract", "guides"},
        )


class SameMapAsCi(unittest.TestCase):
    """The code lanes are review.yml's: a pattern changed in one and not the other fails here."""

    def test_every_lane_pattern_of_review_yml_is_the_scripts(self):
        workflow = WORKFLOW.read_text(encoding="utf-8")
        script = SCRIPT.read_text(encoding="utf-8")
        ci = dict(re.findall(r"^\s+(\w+)=\$\(has '([^']+)'\)", workflow, re.MULTILINE))
        local = dict(re.findall(r"^(\w+)=\$\(has '([^']+)'\)", script, re.MULTILINE))
        self.assertTrue(ci, "review.yml no longer maps lanes with has '<pattern>'")
        for lane, pattern in ci.items():
            self.assertEqual(local.get(lane), pattern, lane)


if __name__ == "__main__":
    unittest.main()
