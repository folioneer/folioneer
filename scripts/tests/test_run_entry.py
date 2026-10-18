"""Tests for scripts/run-entry.sh — run with `just test-scripts`."""

import json
import re
import shutil
import subprocess
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "run-entry.sh"

# What `gh project item-list --format json` lists: issues, a draft (no number) and a
# pull request, in the Project's row order.
ITEMS = {
    "items": [
        {"status": "Queued", "content": {"number": 412, "type": "Issue"}},
        {"status": "Ready", "content": {"number": 380, "type": "Issue"}},
        {"status": "Queued", "content": {"title": "a draft", "type": "DraftIssue"}},
        {"status": "Queued", "content": {"number": 377, "type": "PullRequest"}},
        {"status": "Done", "content": {"number": 360, "type": "Issue"}},
    ]
}


def project_filter() -> str:
    """The `--jq` filter the script reads the Project's Queued items with."""
    found = re.findall(r"--jq '([^']+)'", SCRIPT.read_text(encoding="utf-8"))
    assert len(found) == 1, found
    return found[0]


@unittest.skipUnless(shutil.which("jq"), "jq is not installed")
class Batch(unittest.TestCase):
    # The batch is the Queued items in row order; a draft has no number and is not run.
    def test_what_is_left_of_the_batch_is_read_in_the_projects_order(self):
        printed = subprocess.run(
            ["jq", "-r", project_filter()],
            input=json.dumps(ITEMS),
            capture_output=True,
            text=True,
            check=True,
        ).stdout
        self.assertEqual(printed.split(), ["#412", "#377"])


if __name__ == "__main__":
    unittest.main()
