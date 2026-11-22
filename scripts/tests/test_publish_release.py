"""`scripts/publish-release.sh` publishes only the checked newest draft, on a yes — run with `just test-scripts`.

The script reads its confirmation from a terminal: these tests give it one (`pty`), and fake
`gh` and `git` that answer what it asks and record any `gh release edit`.
"""

import os
import pty
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "publish-release.sh"
TAG_SHA = "a" * 40

FAKE_GIT = """#!/bin/sh
case "$1" in
  fetch) exit 0 ;;
  merge-base) exit 0 ;;
  ls-remote)
    case "$*" in
      *"^{}"*) printf '%s\\trefs/tags/%s^{}\\n' "$TAG_SHA" "$DRAFT_TAG" ;;
      *--sort*) printf '%s\\trefs/tags/%s\\n' "$TAG_SHA" "$NEWEST_TAG" ;;
    esac ;;
esac
"""

FAKE_GH = """#!/bin/sh
case "$1 $2" in
  "release list") echo "$DRAFT_TAG" ;;
  "run list") printf '%s\\t%s\\t%s\\n' "$RUN_CONCLUSION" "$RUN_SHA" "https://example.test/run" ;;
  "release view") printf 'Folioneer %s\\n\\nnotes\\n' "$DRAFT_TAG" ;;
  "release edit") echo "$*" >> "$CALLS" ;;
esac
"""


class PublishRelease(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        bin_dir = Path(self.tmp.name)
        for name, body in (("git", FAKE_GIT), ("gh", FAKE_GH)):
            fake = bin_dir / name
            fake.write_text(body)
            fake.chmod(0o755)
        self.calls = bin_dir / "calls"
        self.env = {
            **os.environ,
            "PATH": f"{bin_dir}:{os.environ['PATH']}",
            "USAGE_LOG": "off",
            "CALLS": str(self.calls),
            "TAG_SHA": TAG_SHA,
            "DRAFT_TAG": "v1.2.0",
            "NEWEST_TAG": "v1.2.0",
            "RUN_CONCLUSION": "success",
            "RUN_SHA": TAG_SHA,
        }

    def run_in_terminal(self, answer: str = "", **env) -> subprocess.CompletedProcess:
        main, child = pty.openpty()
        try:
            proc = subprocess.Popen(
                ["bash", str(SCRIPT)],
                stdin=child,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                env={**self.env, **env},
            )
            os.write(main, f"{answer}\n".encode())
            out, err = proc.communicate(timeout=30)
        finally:
            os.close(child)
            os.close(main)
        return subprocess.CompletedProcess(proc.args, proc.returncode, out, err)

    def published(self) -> str:
        return self.calls.read_text() if self.calls.exists() else ""

    def test_without_a_terminal_it_refuses_and_calls_nothing(self):
        result = subprocess.run(
            ["bash", str(SCRIPT)], stdin=subprocess.DEVNULL, capture_output=True, text=True, env=self.env
        )

        self.assertEqual(result.returncode, 1)
        self.assertIn("needs a terminal", result.stderr)
        self.assertEqual(self.published(), "")

    def test_a_checked_draft_is_published_on_a_yes(self):
        result = self.run_in_terminal("y")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.published().split(), ["release", "edit", "v1.2.0", "--draft=false", "--latest"])

    def test_anything_but_a_yes_publishes_nothing(self):
        result = self.run_in_terminal("")

        self.assertEqual(result.returncode, 1)
        self.assertIn("not published", result.stderr)
        self.assertEqual(self.published(), "")

    def test_a_draft_that_is_not_the_newest_tag_is_refused(self):
        result = self.run_in_terminal("y", NEWEST_TAG="v1.3.0")

        self.assertEqual(result.returncode, 1)
        self.assertIn("the newest tag is v1.3.0", result.stderr)
        self.assertEqual(self.published(), "")

    def test_a_release_run_that_did_not_end_green_is_refused(self):
        for conclusion in ("failure", "unfinished"):
            with self.subTest(conclusion=conclusion):
                result = self.run_in_terminal("y", RUN_CONCLUSION=conclusion)

                self.assertEqual(result.returncode, 1)
                self.assertIn(f"ended {conclusion}", result.stderr)
                self.assertEqual(self.published(), "")

    def test_a_release_run_on_another_commit_is_refused(self):
        result = self.run_in_terminal("y", RUN_SHA="b" * 40)

        self.assertEqual(result.returncode, 1)
        self.assertIn("no release workflow run on v1.2.0's commit", result.stderr)
        self.assertEqual(self.published(), "")


if __name__ == "__main__":
    unittest.main()
