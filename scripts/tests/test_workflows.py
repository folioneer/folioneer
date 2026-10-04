"""Tests on the CI workflows' triggers (FLOW-012) — run with `just test-scripts`."""

import re
import unittest
from pathlib import Path

WORKFLOWS = Path(__file__).resolve().parents[2] / ".github" / "workflows"


def pull_request_types(text: str) -> list[str]:
    """Every event type named in a `types:` list of a workflow, inline or as a block."""
    types: list[str] = []
    for inline in re.findall(r"^\s*types:\s*\[([^\]]*)\]", text, re.M):
        types += [name.strip() for name in inline.split(",")]
    for block in re.findall(r"^\s*types:\s*\n((?:\s*-\s*\S+\s*\n)+)", text, re.M):
        types += re.findall(r"-\s*(\S+)", block)
    return types


class PullRequestTriggers(unittest.TestCase):
    # FLOW-012 — editing a pull request's title or body starts no workflow: nothing reads
    # that text, and a run started by an edit cancels the one testing the code.
    def test_no_workflow_runs_when_a_pull_requests_text_is_edited(self):
        workflows = sorted(WORKFLOWS.glob("*.yml"))
        self.assertGreater(len(workflows), 3)
        for workflow in workflows:
            self.assertEqual(
                pull_request_types(workflow.read_text()).count("edited"),
                0,
                workflow.name,
            )

    # Both ways of writing the list are read, so neither hides the trigger.
    def test_both_spellings_of_the_list_are_read(self):
        inline = "on:\n  pull_request:\n    types: [opened, edited, synchronize]\n"
        block = "on:\n  pull_request:\n    types:\n      - opened\n      - edited\n  push:\n"
        self.assertIn("edited", pull_request_types(inline))
        self.assertIn("edited", pull_request_types(block))
        self.assertEqual(pull_request_types("on:\n  push:\n    branches: [main]\n"), [])


if __name__ == "__main__":
    unittest.main()
