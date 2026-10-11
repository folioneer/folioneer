"""Tests for scripts/run-entry.sh (TODO-049) — run with `just test-scripts`."""

import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1]
SCRIPT = SCRIPTS / "run-entry.sh"

QUEUE = """# Queue

<!-- The human's queue: one reference per line. -->

- TODO-051
- DEBT-064
- FLOW-021
- TODO-009
"""

ENTRIES = {
    "todo/TODO-051.md": "# TODO-051 — (tooling) — Queued\n\nMentions DEBT-001 outside the queue.\n",
    "debt/DEBT-064.md": "# 2026-10-01 — DEBT-064 — Queued\n\n- Done when: it is.\n",
    "flow/FLOW-021.md": "# FLOW-021 — Queued\n\n- Proposal: one.\n",
    "todo/TODO-010.md": "# TODO-010 — (fullstack) — Not queued\n",
}


def queue_line() -> str:
    """The line of the script that reads the queue."""
    lines = [
        line for line in SCRIPT.read_text(encoding="utf-8").splitlines() if line.startswith("queued=$(")
    ]
    assert len(lines) == 1, lines
    return lines[0]


def queued(queue: str) -> list[str]:
    """What the script reads as left to work, in a folder holding `queue` and ENTRIES."""
    with tempfile.TemporaryDirectory() as folder:
        root = Path(folder)
        (root / "scripts").mkdir()
        for script in ("work-queue.py", "usage_log.py"):
            shutil.copy(SCRIPTS / script, root / "scripts" / script)
        for name, text in ENTRIES.items():
            (root / "docs" / "work" / name).parent.mkdir(parents=True, exist_ok=True)
            (root / "docs" / "work" / name).write_text(text, encoding="utf-8")
        (root / "docs" / "work" / "queue.md").write_text(queue, encoding="utf-8")
        printed = subprocess.run(
            ["bash", "-c", f'log() {{ :; }}\n{queue_line()}\nprintf "%s" "$queued"'],
            cwd=folder,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    return printed.split()


class Queue(unittest.TestCase):
    # TODO-049 — the queue is read in the three forms a reference takes, in the owner's
    # order, and only from the queue file: the comment and the entries are not the queue.
    # A queued reference whose entry file is gone has shipped: it is not run again.
    def test_what_is_left_of_the_queue_is_read_in_the_owners_order(self):
        self.assertEqual(queued(QUEUE), ["TODO-051", "DEBT-064", "FLOW-021"])

    # TODO-049 — a reference in a form that was replaced queues nothing.
    def test_a_replaced_form_queues_nothing(self):
        old = QUEUE.replace("- TODO-051", "- #051").replace("- DEBT-064", "- TD-064")
        self.assertEqual(queued(old), ["FLOW-021"])


if __name__ == "__main__":
    unittest.main()
