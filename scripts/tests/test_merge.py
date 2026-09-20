"""Tests for scripts/merge.py — one entry lands as one commit — run with `just test-scripts`."""

import importlib.util
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location("merge", Path(__file__).resolve().parents[1] / "merge.py")
merge = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(merge)


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
        return subprocess.run(["git", *args], capture_output=True, text=True, check=True).stdout.strip()

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
        tested = self.commit("fixup! feat: show the total", {"total.txt": "total, fixed\n"})

        result = merge.rebase_folding_fixups("main")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.titles(), ["feat: show the total"])
        self.assertEqual(Path("total.txt").read_text(encoding="utf-8"), "total, fixed\n")
        self.assertEqual(merge._fold_commits("main", "work"), [])
        self.assertTrue(merge._rebase_left_the_checks_standing(tested, self.run_git("rev-parse", "HEAD")))

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
        before = self.commit("fixup! feat: show the total", {"total.txt": "total, fixed\n"})
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
        records = self.commit("docs: close the entry", {"docs/todo.md": "closed\n", "docs/plan/a-plan.md": "x\n"})
        code = self.commit("fix: the total", {"total.txt": "other\n"})

        self.assertTrue(merge._rebase_left_the_checks_standing(tested, tested))
        self.assertTrue(merge._rebase_left_the_checks_standing(tested, records))
        self.assertFalse(merge._rebase_left_the_checks_standing(tested, code))


if __name__ == "__main__":
    unittest.main()
