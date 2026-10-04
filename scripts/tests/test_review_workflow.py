"""Tests on the review workflow's own shell (FLOW-002) — run with `just test-scripts`.

The step that posts a lane's report is shell inside YAML. Its usage-limit test is pulled
out of the file and run on the messages a stopped session really leaves.
"""

import re
import subprocess
import unittest
from pathlib import Path

WORKFLOW = Path(__file__).resolve().parents[2] / ".github" / "workflows" / "review.yml"


def usage_limit_pattern() -> str:
    match = re.search(r'grep -qiE "([^"]+)" <<<"\$last"', WORKFLOW.read_text())
    assert match, "the usage-limit test is no longer in review.yml"
    return match.group(1)


def stopped_by_the_limit(message: str) -> bool:
    return (
        subprocess.run(
            ["grep", "-qiE", usage_limit_pattern()], input=message, text=True
        ).returncode
        == 0
    )


class UsageLimit(unittest.TestCase):
    # FLOW-002 — the message seen on pull request #91, and its siblings.
    def test_a_session_stopped_by_the_usage_limit_is_recognised(self):
        self.assertTrue(
            stopped_by_the_limit("You've hit your session limit · resets 9:10pm (UTC)")
        )
        self.assertTrue(
            stopped_by_the_limit("You've hit your weekly limit · resets Monday")
        )
        self.assertTrue(stopped_by_the_limit("Usage limit reached"))

    # A reviewer that ran and said something else is not excused.
    def test_another_last_message_is_not_a_usage_limit(self):
        self.assertFalse(
            stopped_by_the_limit("I could not find any changed file in my lane.")
        )
        self.assertFalse(
            stopped_by_the_limit("The diff adds a rate limit to the fetch.")
        )
        self.assertFalse(stopped_by_the_limit(""))


if __name__ == "__main__":
    unittest.main()
