"""Tests for scripts/commit-check.sh — the commit rules CI holds a commit to — run with `just test-scripts`."""

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "commit-check.sh"
OWNER = ("phil-linux", "phileggel@users.noreply.github.com")


class CommitCheck(unittest.TestCase):
    """A repository with one commit on `main`, in a temporary folder."""

    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        self.addCleanup(self.folder.cleanup)
        # A git hook exports GIT_DIR and the like; inherited, they would point the
        # commands below at the real repository.
        self.env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
        self.git("init", "--quiet", "--initial-branch", "main")
        self.git("config", "core.hooksPath", os.devnull)
        self.base = self.commit("chore: start")

    def git(self, *args, author=OWNER):
        name, email = author
        env = {
            **self.env,
            "GIT_AUTHOR_NAME": name, "GIT_AUTHOR_EMAIL": email,
            "GIT_COMMITTER_NAME": name, "GIT_COMMITTER_EMAIL": email,
        }
        return subprocess.run(
            ["git", *args], cwd=self.folder.name, env=env,
            capture_output=True, text=True, check=True,
        ).stdout.strip()

    def commit(self, *message, author=OWNER):
        args = [arg for part in message for arg in ("-m", part)]
        self.git("commit", "--quiet", "--allow-empty", *args, author=author)
        return self.git("rev-parse", "HEAD")

    def check(self, revisions):
        return subprocess.run(
            ["bash", str(SCRIPT), revisions], cwd=self.folder.name, env=self.env,
            capture_output=True, text=True, check=False,
        )

    def test_a_commit_by_the_owner_that_follows_the_rules_passes(self):
        self.commit("docs: record the answers", "TODO-001.")

        result = self.check(f"{self.base}..HEAD")

        self.assertEqual(result.returncode, 0, result.stdout)

    # Pull request #313: a commit authored by the agent, not the owner, landed on main.
    def test_a_commit_by_another_author_fails(self):
        self.commit("docs: record the answers", author=("Claude", "noreply@anthropic.com"))

        result = self.check(f"{self.base}..HEAD")

        self.assertEqual(result.returncode, 1)
        self.assertIn("authored by Claude <noreply@anthropic.com>", result.stdout)

    def test_a_bot_is_held_to_the_format_only(self):
        bot = ("dependabot[bot]", "49699333+dependabot[bot]@users.noreply.github.com")
        self.commit("chore(deps): bump a library across 3 directories with 12 updates", author=bot)
        self.assertEqual(self.check(f"{self.base}..HEAD").returncode, 0)

        self.commit("bump a library", author=bot)
        self.assertEqual(self.check(f"{self.base}..HEAD").returncode, 1)

    def test_the_message_rules_hold_as_the_hook_holds_them(self):
        last = self.base
        self.commit("docs: a title that ends with a period.")
        self.assertIn("ends with period", self.check(f"{last}..HEAD").stdout)

        last = self.git("rev-parse", "HEAD")
        self.commit("docs: record", "Co-Authored-By: someone <a@b.c>")
        self.assertIn("Co-Authored-By", self.check(f"{last}..HEAD").stdout)

        last = self.git("rev-parse", "HEAD")
        self.commit("fixup! docs: record the answers")
        self.assertEqual(self.check(f"{last}..HEAD").returncode, 0)

    # A good commit pushed after a bad one leaves the range red: main is checked since the release.
    def test_a_bad_commit_stays_in_the_range_after_a_good_one(self):
        self.commit("docs: record the answers", author=("Claude", "noreply@anthropic.com"))
        self.commit("docs: record the answers")

        self.assertEqual(self.check(f"{self.base}..HEAD").returncode, 1)

    # The way out of a red main that is not a rewrite: the owner lists the commit as accepted.
    def test_a_commit_the_owner_accepted_is_passed(self):
        bad = self.commit("docs: record the answers", author=("Claude", "noreply@anthropic.com"))
        Path(self.folder.name, "commit-check-accepted.txt").write_text(
            f"{bad}  # merged before the guard; content kept\n", encoding="utf-8"
        )

        result = self.check(f"{self.base}..HEAD")

        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn("accepted as it is", result.stdout)

    # #672 — the description after `: ` holds 100 characters; the type and the scope are not counted.
    def test_the_description_holds_100_characters_whatever_the_scope(self):
        self.commit("feat(command-line): " + "a" * 100)
        self.assertEqual(self.check(f"{self.base}..HEAD").returncode, 0)

        last = self.git("rev-parse", "HEAD")
        self.commit("feat: " + "a" * 101)
        result = self.check(f"{last}..HEAD")
        self.assertEqual(result.returncode, 1)
        self.assertIn("description > 100 chars (101)", result.stdout)

    # #672 — a scope is a word of the list in docs/commit-rules.md.
    def test_a_scope_off_the_list_fails(self):
        self.commit("feat(transfer): move a holding to another account, keeping its cost")
        self.assertEqual(self.check(f"{self.base}..HEAD").returncode, 0)

        last = self.git("rev-parse", "HEAD")
        self.commit("feat(backend): move a holding to another account")
        result = self.check(f"{last}..HEAD")
        self.assertEqual(result.returncode, 1)
        self.assertIn("scope 'backend' is not in docs/commit-rules.md", result.stdout)

    def test_the_address_github_writes_with_the_account_id_is_the_owner(self):
        self.commit("docs: record", author=("phileggel", "1234+phileggel@users.noreply.github.com"))
        self.assertEqual(self.check(f"{self.base}..HEAD").returncode, 0)


if __name__ == "__main__":
    unittest.main()
