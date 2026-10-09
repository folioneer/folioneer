"""Tests for scripts/next-todo.sh (TODO-049) — run with `just test-scripts`."""

import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "next-todo.sh"

TODO = """# TODO

## Next

<!-- The human's queue: references (TODO-NNN or DEBT-NNN) in the order to work them. -->

1. TODO-051
2. DEBT-064
3. FLOW-021

## TODO-009 — (fullstack) — Not queued

Mentions DEBT-001 and TODO-051 outside the queue.
"""


def queue_line() -> str:
    """The line of the script that reads the queue."""
    lines = [
        line for line in SCRIPT.read_text(encoding="utf-8").splitlines() if line.startswith("queued=$(")
    ]
    assert len(lines) == 1, lines
    return lines[0]


def queued(todo: str) -> list[str]:
    """What the script reads as queued from a todo file."""
    with tempfile.TemporaryDirectory() as folder:
        (Path(folder) / "docs").mkdir()
        (Path(folder) / "docs" / "todo.md").write_text(todo, encoding="utf-8")
        printed = subprocess.run(
            ["bash", "-c", f'{queue_line()}\nprintf "%s" "$queued"'],
            cwd=folder,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    return printed.split()


class Queue(unittest.TestCase):
    # TODO-049 — the queue is read in the three forms a reference takes, in the owner's
    # order, and only from § Next: the comment and the entries below are not the queue.
    def test_the_queue_is_read_in_the_new_forms_and_only_under_next(self):
        self.assertEqual(queued(TODO), ["TODO-051", "DEBT-064", "FLOW-021"])

    # TODO-049 — a reference in a form that was replaced queues nothing.
    def test_a_replaced_form_queues_nothing(self):
        old = TODO.replace("1. TODO-051", "1. #051").replace("2. DEBT-064", "2. TD-064")
        self.assertEqual(queued(old), ["FLOW-021"])


if __name__ == "__main__":
    unittest.main()
