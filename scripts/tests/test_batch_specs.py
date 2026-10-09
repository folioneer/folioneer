"""Tests for scripts/batch-specs.sh — the specs a batch touched since the last release —
run with `just test-scripts`."""

import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "batch-specs.sh"


class BatchSpecs(unittest.TestCase):
    def setUp(self):
        self.repo = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.repo, ignore_errors=True)
        # A git hook exports GIT_DIR and friends; inherited, they would point every
        # `git` below at the real repository.
        self.env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
        self.env.update(
            GIT_AUTHOR_NAME="t",
            GIT_AUTHOR_EMAIL="t@example.invalid",
            GIT_COMMITTER_NAME="t",
            GIT_COMMITTER_EMAIL="t@example.invalid",
        )
        self.git("init", "-q", "-b", "main")
        self.write("docs/spec/asset.md", "one")
        self.write("docs/spec/sync.md", "one")
        self.write("docs/spec/gone.md", "one")
        self.write("docs/todo.md", "one")
        self.commit("first")

    def git(self, *args):
        return subprocess.run(
            ["git", *args], cwd=self.repo, env=self.env, capture_output=True, text=True, check=True
        ).stdout

    def write(self, path, text):
        target = self.repo / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text)

    def commit(self, message):
        self.git("add", "-A")
        self.git("commit", "-q", "-m", message)

    def run_script(self, *args):
        return subprocess.run(
            ["bash", str(SCRIPT), *args],
            cwd=self.repo,
            env=self.env,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_lists_the_specs_changed_or_added_since_the_last_release_and_nothing_else(self):
        self.git("tag", "v0.1.0")
        self.write("docs/spec/asset.md", "two")
        self.write("docs/spec/new.md", "a rule of its own")
        self.write("docs/todo.md", "two")
        (self.repo / "docs/spec/gone.md").unlink()
        self.commit("the batch")

        listed = self.run_script()

        self.assertEqual(listed.returncode, 0, listed.stderr)
        self.assertEqual(listed.stdout.split(), ["docs/spec/asset.md", "docs/spec/new.md"])

    def test_starts_from_the_latest_release_tag_not_an_earlier_one(self):
        self.git("tag", "v0.1.0")
        self.write("docs/spec/asset.md", "two")
        self.commit("released in 0.2.0")
        self.git("tag", "v0.2.0")
        self.write("docs/spec/sync.md", "two")
        self.commit("the batch")

        self.assertEqual(self.run_script().stdout.split(), ["docs/spec/sync.md"])
        self.assertEqual(
            self.run_script("v0.1.0").stdout.split(),
            ["docs/spec/asset.md", "docs/spec/sync.md"],
        )

    def test_a_batch_that_touched_no_spec_lists_nothing(self):
        self.git("tag", "v0.1.0")
        self.write("docs/todo.md", "two")
        self.commit("docs only")

        listed = self.run_script()
        self.assertEqual((listed.returncode, listed.stdout), (0, ""))

    def test_without_a_release_tag_it_asks_for_a_starting_point(self):
        refused = self.run_script()
        self.assertEqual(refused.returncode, 1)
        self.assertIn("no release tag", refused.stderr)
        self.assertEqual(self.run_script("a", "b").returncode, 2)


if __name__ == "__main__":
    unittest.main()
