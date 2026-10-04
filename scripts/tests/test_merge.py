"""Tests for scripts/merge.py — one entry lands as one commit — run with `just test-scripts`."""

import importlib.util
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "merge", Path(__file__).resolve().parents[1] / "merge.py"
)
merge = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(merge)

# A git hook exports GIT_DIR (and GIT_INDEX_FILE, GIT_WORK_TREE…). Inherited, they would
# make every `git` below — the helpers here and merge.py's own calls — act on the real
# repository instead of the throwaway one. They are removed while this module runs.
_SAVED_GIT_ENV: dict[str, str] = {}


def clear_git_environment(environ, saved):
    """Moves every GIT_* variable out of `environ` into `saved`."""
    for key in [k for k in environ if k.startswith("GIT_")]:
        saved[key] = environ.pop(key)


def setUpModule():
    clear_git_environment(os.environ, _SAVED_GIT_ENV)


def tearDownModule():
    os.environ.update(_SAVED_GIT_ENV)


class NoRealRepository(unittest.TestCase):
    def test_no_git_variable_reaches_the_tests(self):
        self.assertEqual([k for k in os.environ if k.startswith("GIT_")], [])

    def test_git_variables_are_set_aside_and_kept_for_restoring(self):
        environ = {"GIT_DIR": "/real/.git", "GIT_INDEX_FILE": "index", "PATH": "/bin"}
        saved = {}

        clear_git_environment(environ, saved)

        self.assertEqual(environ, {"PATH": "/bin"})
        self.assertEqual(saved, {"GIT_DIR": "/real/.git", "GIT_INDEX_FILE": "index"})


class FoldingFixups(unittest.TestCase):
    """A repository with `main` and a checked-out `work` branch, in a temporary folder."""

    def setUp(self):
        self.folder = tempfile.TemporaryDirectory()
        self.addCleanup(self.folder.cleanup)
        self.previous = os.getcwd()
        self.addCleanup(os.chdir, self.previous)
        os.chdir(self.folder.name)
        self.run_git("init", "--quiet", "--initial-branch", "main")
        self.run_git("config", "user.name", "test")
        self.run_git("config", "user.email", "test@example.invalid")
        self.run_git("config", "core.hooksPath", os.devnull)
        self.commit("chore: start", {"README.md": "start\n"})
        self.run_git("checkout", "--quiet", "-b", "work")

    def run_git(self, *args):
        return subprocess.run(
            ["git", *args], capture_output=True, text=True, check=True
        ).stdout.strip()

    def commit(self, title, files):
        for name, content in files.items():
            path = Path(name)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")
        self.run_git("add", "--all")
        self.run_git("commit", "--quiet", "-m", title)
        return self.run_git("rev-parse", "HEAD")

    def titles(self):
        return self.run_git("log", "--format=%s", "main..work").splitlines()

    def test_a_fixup_is_folded_into_the_commit_it_names_and_the_tree_is_unchanged(self):
        self.commit("feat: show the total", {"total.txt": "total\n"})
        tested = self.commit(
            "fixup! feat: show the total", {"total.txt": "total, fixed\n"}
        )

        result = merge.rebase_folding_fixups("main")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.titles(), ["feat: show the total"])
        self.assertEqual(
            Path("total.txt").read_text(encoding="utf-8"), "total, fixed\n"
        )
        self.assertEqual(merge._fold_commits("main", "work"), [])
        self.assertTrue(
            merge._rebase_left_the_checks_standing(
                tested, self.run_git("rev-parse", "HEAD")
            )
        )

    def test_a_fixup_is_folded_when_the_target_moved_too(self):
        self.commit("feat: show the total", {"total.txt": "total\n"})
        self.commit("fixup! feat: show the total", {"total.txt": "total, fixed\n"})
        self.run_git("checkout", "--quiet", "main")
        self.commit("docs: queue an entry", {"docs/todo.md": "entry\n"})
        self.run_git("checkout", "--quiet", "work")

        result = merge.rebase_folding_fixups("main")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.titles(), ["feat: show the total"])
        self.assertTrue(Path("docs/todo.md").exists())

    def test_a_fixup_that_names_no_commit_is_refused_and_the_branch_goes_back(self):
        self.commit("feat: show the total", {"total.txt": "total\n"})
        before = self.commit("fixup! feat: something else", {"other.txt": "other\n"})
        self.run_git("checkout", "--quiet", "main")
        self.commit("docs: queue an entry", {"docs/todo.md": "entry\n"})
        self.run_git("checkout", "--quiet", "work")
        self.assertEqual(merge.rebase_folding_fixups("main").returncode, 0)
        self.assertNotEqual(self.run_git("rev-parse", "HEAD"), before)

        with self.assertRaises(SystemExit):
            merge.refuse_unfolded_fixups("main", "work", before)

        self.assertEqual(self.run_git("rev-parse", "HEAD"), before)

    def test_a_branch_whose_fixups_were_folded_is_not_refused(self):
        self.commit("feat: show the total", {"total.txt": "total\n"})
        before = self.commit(
            "fixup! feat: show the total", {"total.txt": "total, fixed\n"}
        )
        merge.rebase_folding_fixups("main")

        merge.refuse_unfolded_fixups("main", "work", before)

        self.assertEqual(self.titles(), ["feat: show the total"])

    def test_squash_and_amend_commits_are_refused_before_the_rebase(self):
        self.commit("feat: show the total", {"total.txt": "total\n"})
        self.commit("squash! feat: show the total", {"a.txt": "a\n"})
        head = self.commit("amend! feat: show the total", {"b.txt": "b\n"})

        with self.assertRaises(SystemExit):
            merge.refuse_unreadable_folds("main", "work")

        self.assertEqual(self.run_git("rev-parse", "HEAD"), head)

    def test_fixup_commits_alone_pass_the_check_before_the_rebase(self):
        self.commit("feat: show the total", {"total.txt": "total\n"})
        self.commit("fixup! feat: show the total", {"total.txt": "total, fixed\n"})

        merge.refuse_unreadable_folds("main", "work")

    def test_the_checks_stand_for_record_files_and_fall_for_anything_else(self):
        tested = self.commit("feat: show the total", {"total.txt": "total\n"})
        records = self.commit(
            "docs: close the entry",
            {
                "docs/todo.md": "closed\n",
                "docs/flow.md": "closed\n",
                "docs/plan/a-plan.md": "x\n",
            },
        )
        code = self.commit("fix: the total", {"total.txt": "other\n"})

        self.assertTrue(merge._rebase_left_the_checks_standing(tested, tested))
        self.assertTrue(merge._rebase_left_the_checks_standing(tested, records))
        self.assertFalse(merge._rebase_left_the_checks_standing(tested, code))


class Worktrees(unittest.TestCase):
    LISTING = (
        "worktree /home/me/project\nHEAD 1111\nbranch refs/heads/main\n\n"
        "worktree /home/me/project-docs\nHEAD 2222\nbranch refs/heads/docs/notes\n\n"
        "worktree /home/me/project-detached\nHEAD 3333\ndetached\n"
    )

    # FLOW-013 — from a worktree, the target is held by the main folder: named, so the
    # refusal can say where to merge from.
    def test_the_folder_holding_the_target_is_found_from_another_worktree(self):
        self.assertEqual(
            merge.worktree_holding("main", self.LISTING, "/home/me/project-docs"),
            "/home/me/project",
        )

    # From the folder that holds the target itself, nothing is in the way.
    def test_the_folder_that_holds_the_target_is_not_in_its_own_way(self):
        self.assertIsNone(merge.worktree_holding("main", self.LISTING, "/home/me/project"))
        self.assertIsNone(merge.worktree_holding("release", self.LISTING, "/home/me/project"))

    # A branch whose name ends like the target is not the target.
    def test_a_branch_named_alike_is_not_the_target(self):
        listing = "worktree /w\nHEAD 1\nbranch refs/heads/not-main\n"
        self.assertIsNone(merge.worktree_holding("main", listing, "/elsewhere"))


class ReviewerNotes(unittest.TestCase):
    # FLOW-002 — a reviewer that did not run says so in its sticky comment's heading; the
    # refusal repeats it, so a red check is not read as a finding.
    def test_a_reviewer_that_did_not_run_is_told_apart_from_a_finding(self):
        comments = [
            "<!-- e2e-screenshots -->\n## E2E screenshots — success\n",
            "<!-- review:arch -->\n## reviewer-arch — did not run (usage limit)\n\nRe-run…",
            "<!-- review:infra -->\n## reviewer-infra — no report\n\nThe reviewer produced…",
            "<!-- review:backend -->\n## reviewer-backend — 3 files reviewed\n\n🔴 1 critical",
        ]
        self.assertEqual(
            merge.reviewer_heading(comments, "reviewer-arch"),
            "did not run (usage limit)",
        )
        self.assertEqual(
            merge.reviewer_heading(comments, "reviewer-infra"), "no report"
        )
        # A real report's heading is not a note about the run: nothing is added.
        self.assertIsNone(merge.reviewer_heading(comments, "reviewer-backend"))
        # A lane with no comment yet.
        self.assertIsNone(merge.reviewer_heading(comments, "reviewer-sql"))


if __name__ == "__main__":
    unittest.main()
