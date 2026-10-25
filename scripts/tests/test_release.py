"""Tests for scripts/release.py — run with `just test-scripts`."""

import importlib.util
import json
import sys
import tempfile
import subprocess
import unittest
from pathlib import Path
from unittest import mock

# release.py imports its sibling scripts (check.py) by name.
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

SPEC = importlib.util.spec_from_file_location(
    "release", Path(__file__).resolve().parents[1] / "release.py"
)
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)


class LockfileVersion(unittest.TestCase):
    # DEBT-035 — a release leaves the lockfile stating the released version, and nothing else moves.
    def test_the_lockfile_states_the_released_version(self):
        with tempfile.TemporaryDirectory() as folder:
            lockfile = Path(folder) / "package-lock.json"
            lock = {
                "name": "folioneer",
                "version": "0.0.0",
                "lockfileVersion": 3,
                "requires": True,
                "packages": {
                    "": {"name": "folioneer", "version": "0.0.0", "dependencies": {"é": "1.0.0"}},
                    "node_modules/é": {"version": "1.0.0"},
                },
            }
            lockfile.write_text(json.dumps(lock, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

            release.set_lockfile_version(lockfile, "0.4.0")

            data = json.loads(lockfile.read_text(encoding="utf-8"))
            self.assertEqual(data["version"], "0.4.0")
            self.assertEqual(data["packages"][""]["version"], "0.4.0")
            self.assertEqual(data["packages"]["node_modules/é"]["version"], "1.0.0")
            lock["version"] = "0.4.0"
            lock["packages"][""]["version"] = "0.4.0"
            self.assertEqual(
                lockfile.read_text(encoding="utf-8"),
                json.dumps(lock, indent=2, ensure_ascii=False) + "\n",
            )


def run(name, status="completed", conclusion="success", run_id=1):
    return {"name": name, "status": status, "conclusion": conclusion, "id": run_id,
            "html_url": f"https://example.invalid/runs/{run_id}"}


class HeadRuns(unittest.TestCase):
    # FLOW-031 — a commit whose workflow runs all ended green can be tagged.
    def test_green_runs_are_no_obstacle(self):
        runs = [run("Quality"), run("E2E", conclusion="skipped", run_id=2)]
        self.assertEqual(release.runs_in_the_way(runs), [])

    # FLOW-031 — an unfinished run and a failed one are each named.
    def test_an_unfinished_or_failed_run_is_named(self):
        runs = [run("Quality", status="in_progress", conclusion=None), run("E2E", conclusion="failure", run_id=2)]
        obstacles = release.runs_in_the_way(runs)
        self.assertEqual(len(obstacles), 2)
        self.assertIn("E2E: failure", obstacles[0])
        self.assertIn("runs/2", obstacles[0])
        self.assertIn("Quality: not finished (in_progress)", obstacles[1])

    # FLOW-031 — a run started again replaces the one it repeats.
    def test_the_newest_run_of_a_workflow_counts(self):
        runs = [run("E2E", conclusion="failure", run_id=2), run("E2E", run_id=7)]
        self.assertEqual(release.runs_in_the_way(runs), [])
        runs = [run("E2E", run_id=2), run("E2E", conclusion="cancelled", run_id=7)]
        self.assertEqual(len(release.runs_in_the_way(runs)), 1)

    # FLOW-031 — a commit GitHub lists no run for is not released: nothing was seen green.
    def test_no_run_at_all_is_an_obstacle(self):
        self.assertEqual(len(release.runs_in_the_way([])), 1)

    def manager(self):
        manager = release.ReleaseManager.__new__(release.ReleaseManager)
        manager.mode = release.Mode.REAL
        manager.repo_root = Path(".")
        return manager

    def answers(self, listed):
        def answer(command, **_):
            out = "0123456789abcdef\n" if command[0] == "git" else listed
            return subprocess.CompletedProcess(command, 0, stdout=out, stderr="")
        return answer

    # FLOW-031 — the runs are asked of GitHub for the head commit, and read.
    def test_the_runs_of_the_head_commit_are_read(self):
        listed = json.dumps({"workflow_runs": [run("Quality")]})
        with mock.patch.object(release.subprocess, "run", side_effect=self.answers(listed)) as ran:
            self.assertTrue(self.manager().head_runs_are_green())
        asked = ran.call_args_list[1].args[0]
        self.assertEqual(asked[:2], ["gh", "api"])
        self.assertEqual(asked[2], "repos/{owner}/{repo}/actions/runs?head_sha=0123456789abcdef&per_page=100")
        failed = json.dumps({"workflow_runs": [run("Quality", conclusion="failure")]})
        with mock.patch.object(release.subprocess, "run", side_effect=self.answers(failed)):
            self.assertFalse(self.manager().head_runs_are_green())

    # FLOW-031 — runs that cannot be read are not taken for green.
    def test_runs_that_cannot_be_read_stop_the_release(self):
        unreachable = subprocess.CalledProcessError(1, ["gh"])
        with mock.patch.object(release.subprocess, "run", side_effect=unreachable):
            self.assertFalse(self.manager().head_runs_are_green())
        with mock.patch.object(release.subprocess, "run", side_effect=self.answers("not json")):
            self.assertFalse(self.manager().head_runs_are_green())

    # FLOW-031 — the release asks for the runs before anything else, and stops on a refusal.
    def test_the_release_stops_before_its_tests(self):
        manager = release.ReleaseManager.__new__(release.ReleaseManager)
        manager.mode = release.Mode.REAL
        called = []
        manager.on_fresh_main = lambda: True
        manager.head_runs_are_green = lambda: False
        manager.run_tests = lambda: called.append("tests") or True
        self.assertIs(manager._resolve_version(), release._Resolution.EARLY_FAIL)
        self.assertEqual(called, [])


class Validation(unittest.TestCase):
    # FLOW-011 — a release and the hand-over's `just release-check` run one validation,
    # defined once: everything, with a missing part of the stack a failure.
    def test_a_release_runs_the_validation_the_hand_over_runs(self):
        checker = mock.Mock()
        checker.run_all.return_value = True
        manager = release.ReleaseManager.__new__(release.ReleaseManager)
        manager.mode = release.Mode.REAL
        with mock.patch.object(
            release, "release_validation", return_value=checker
        ) as validation:
            self.assertTrue(manager.run_tests())
        validation.assert_called_once_with()
        checker.run_all.assert_called_once_with()

    def test_the_validation_skips_nothing(self):
        checker = release.release_validation()
        self.assertFalse(checker.fast_mode)
        self.assertTrue(checker.strict_mode)


class ChangelogEntry(unittest.TestCase):
    # #672 — the changelog writes a title's scope before its line, so a reader sees where a change is.
    def test_a_scope_is_written_before_the_line(self):
        manager = release.ReleaseManager.__new__(release.ReleaseManager)
        manager.new_version = "1.2.0"
        manager.breaking_changes = 0
        commits = [
            "feat(transfer): move a holding to another account, keeping its cost",
            "feat: every account has the monthly view",
            "fix(agent): removing one line of a transfer no longer deletes the whole transfer",
        ]
        manager.commits = [manager.parse_commit_message(message) for message in commits]
        manager.features, manager.fixes = 2, 1

        entry = manager._build_changelog_entry()

        self.assertIn("- [transfer] move a holding to another account, keeping its cost\n", entry)
        self.assertIn("- every account has the monthly view\n", entry)
        self.assertIn("- [agent] removing one line of a transfer no longer deletes the whole transfer\n", entry)


class ThroughAPullRequest(unittest.TestCase):
    """The release commit reaches main through a pull request, and the tag goes on what landed."""

    def manager(self):
        manager = release.ReleaseManager.__new__(release.ReleaseManager)
        manager.mode = release.Mode.REAL
        manager.repo_root = Path(".")
        manager.new_version = "0.12.0"
        return manager

    def github(self, checks=0, landed="chore: release v0.12.0", same_tree=0):
        """Answers each command as git and gh would; records them in order."""
        ran = []

        def answer(command, **_):
            command = [str(part) for part in command]
            ran.append(command)
            out, code = "", 0
            if command[:3] == ["git", "rev-parse", "--abbrev-ref"]:
                out = "chore/release-v0.12.0\n"
            elif command[:2] == ["git", "rev-parse"]:
                out = "abc123\n"
            elif command[:3] == ["gh", "pr", "create"]:
                out = "https://github.com/o/r/pull/42\n"
            elif command[1].endswith("watch-pr.py"):
                code = checks
            elif command[:3] == ["git", "log", "-1"]:
                out = landed + "\n"
            elif command[:3] == ["git", "diff", "--quiet"]:
                code = same_tree
            return subprocess.CompletedProcess(command, code, stdout=out, stderr="")

        return ran, answer

    @staticmethod
    def did(ran, *start):
        return any(command[: len(start)] == list(start) for command in ran)

    def test_the_release_lands_through_its_pull_request_then_is_tagged(self):
        ran, answer = self.github()
        with mock.patch.object(release.subprocess, "run", side_effect=answer):
            self.assertTrue(self.manager().push_release())
        merge = next(c for c in ran if c[:3] == ["gh", "pr", "merge"])
        self.assertEqual(merge, ["gh", "pr", "merge", "42", "--rebase", "--match-head-commit", "abc123"])
        steps = [c[:3] for c in ran]
        self.assertLess(steps.index(["gh", "pr", "merge"]), steps.index(["git", "tag", "-a"]))
        self.assertIn(["git", "tag", "-a", "v0.12.0", "-m", "Version 0.12.0", "main"], ran)
        self.assertIn(["git", "push", "--quiet", "--no-verify", "origin", "refs/tags/v0.12.0"], ran)
        self.assertFalse(self.did(ran, "git", "push", "--atomic"))
        self.assertFalse(any(c[:2] == ["git", "push"] and "main" in c for c in ran))

    def test_a_release_whose_checks_fail_is_neither_merged_nor_tagged(self):
        ran, answer = self.github(checks=1)
        with mock.patch.object(release.subprocess, "run", side_effect=answer):
            self.assertFalse(self.manager().push_release())
        self.assertFalse(self.did(ran, "gh", "pr", "merge"))
        self.assertFalse(self.did(ran, "git", "tag"))

    def test_a_head_of_main_that_is_not_the_release_commit_is_not_tagged(self):
        for kwargs in ({"landed": "docs: something else"}, {"same_tree": 1}):
            ran, answer = self.github(**kwargs)
            with mock.patch.object(release.subprocess, "run", side_effect=answer):
                self.assertFalse(self.manager().push_release())
            self.assertFalse(self.did(ran, "git", "tag"), kwargs)

    def test_a_stop_after_the_push_names_both_ways_out(self):
        ran, answer = self.github(checks=3)
        with mock.patch.object(release.subprocess, "run", side_effect=answer), \
                mock.patch("sys.stderr", new_callable=lambda: __import__("io").StringIO()) as err:
            self.assertFalse(self.manager().push_release())
        told = err.getvalue()
        self.assertIn("gh pr view 42 --json state", told)
        self.assertIn("git tag -a v0.12.0 -m 'Version 0.12.0' main", told)
        self.assertIn("gh pr close <n> --delete-branch", told)
        self.assertNotIn("checkout -- .", told)

    # A release branch is cut from main as GitHub holds it, nowhere else.
    def test_a_release_starts_only_from_a_fresh_main(self):
        def git_saying(branch, here, there):
            def answer(command, **_):
                out = branch if "--abbrev-ref" in command else {"HEAD": here, "origin/main": there}.get(command[-1], "")
                return subprocess.CompletedProcess(command, 0, stdout=out + "\n", stderr="")
            return answer

        def edited(command, **_):
            out = "M  CHANGELOG.md" if "status" in command else git_saying("main", "abc", "abc")(command).stdout.strip()
            return subprocess.CompletedProcess(command, 0, stdout=out + "\n", stderr="")

        for answer, expected in (
            (git_saying("main", "abc", "abc"), True),
            (edited, False),
            (git_saying("fix/something", "abc", "abc"), False),
            (git_saying("main", "abc", "def"), False),
        ):
            with mock.patch.object(release.subprocess, "run", side_effect=answer):
                self.assertIs(self.manager().on_fresh_main(), expected)

    def test_a_release_branch_left_from_an_earlier_attempt_stops_before_any_change(self):
        manager = self.manager()
        manager.update_version_files = mock.Mock()
        found = subprocess.CompletedProcess([], 0, stdout="abc\n", stderr="")
        with mock.patch.object(release.subprocess, "run", return_value=found):
            self.assertFalse(manager._apply_changes())
        manager.update_version_files.assert_not_called()

    def test_a_dry_run_runs_no_command(self):
        manager = self.manager()
        manager.mode = release.Mode.DRY_RUN
        with mock.patch.object(release.subprocess, "run", side_effect=AssertionError("a command ran")):
            self.assertTrue(manager.commit_release())
            self.assertTrue(manager._finalize())


if __name__ == "__main__":
    unittest.main()
