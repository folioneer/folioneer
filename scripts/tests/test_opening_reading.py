"""The opening reading of a batch (FLOW-037): the workflow names its two steps and the
skills that run it say the same — run with `just test-scripts`."""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / "docs" / "workflow.md"
RUN_QUEUE = ROOT / ".claude" / "skills" / "run-queue" / "SKILL.md"
RUN_ENTRY = ROOT / ".claude" / "skills" / "run-entry" / "SKILL.md"

STEPS = ("**The sentences that list**", "**The ADRs of the records**")


def one_line(path: Path) -> str:
    return " ".join(path.read_text(encoding="utf-8").split())


class OpeningReading(unittest.TestCase):
    def test_the_workflow_and_run_queue_name_the_same_two_steps(self):
        for path in (WORKFLOW, RUN_QUEUE):
            text = one_line(path)
            for step in STEPS:
                with self.subTest(file=path.name, step=step):
                    self.assertIn(step, text)

    def test_a_pull_request_says_whether_each_test_was_seen_red(self):
        self.assertIn("seen red", one_line(WORKFLOW))
        steps = RUN_ENTRY.read_text(encoding="utf-8").split("\n## Step ")
        for number in ("3", "8"):
            step = next(text for text in steps if text.startswith(f"{number} "))
            with self.subTest(step=number):
                self.assertIn("seen red", " ".join(step.split()))

if __name__ == "__main__":
    unittest.main()
