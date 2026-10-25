"""Tests for .githooks/commit-msg — the rules a commit message meets before it exists — run with `just test-scripts`."""

import subprocess
import tempfile
import unittest
from pathlib import Path

HOOK = Path(__file__).resolve().parents[2] / ".githooks" / "commit-msg"


def check(message):
    with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False, encoding="utf-8") as file:
        file.write(message)
    try:
        return subprocess.run(
            ["bash", str(HOOK), file.name], env={"NO_COLOR": "1", "PATH": "/usr/bin:/bin"},
            capture_output=True, text=True, check=False,
        )
    finally:
        Path(file.name).unlink()


class CommitMsg(unittest.TestCase):
    # #672 — the description after `: ` holds 100 characters; the type and the scope are not counted.
    def test_the_description_holds_100_characters_whatever_the_scope(self):
        self.assertEqual(check("feat(command-line): " + "a" * 100).returncode, 0)

        result = check("feat: " + "a" * 101)
        self.assertEqual(result.returncode, 1)
        self.assertIn("Description longer than 100 characters (101)", result.stdout)

    # #672 — a scope is a word of the list in docs/commit-rules.md.
    def test_a_scope_off_the_list_is_refused(self):
        self.assertEqual(check("fix(agent): a removal no longer deletes the whole transfer").returncode, 0)

        result = check("fix(backend): a removal no longer deletes the whole transfer")
        self.assertEqual(result.returncode, 1)
        self.assertIn("Scope 'backend' is not in docs/commit-rules.md", result.stdout)

    def test_a_title_without_a_scope_passes(self):
        self.assertEqual(check("docs: record the answers").returncode, 0)


if __name__ == "__main__":
    unittest.main()
