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
        manager.head_runs_are_green = lambda: False
        manager.run_tests = lambda: called.append("tests") or True
        self.assertIs(manager._resolve_version(), release._Resolution.EARLY_FAIL)
        self.assertEqual(called, [])


if __name__ == "__main__":
    unittest.main()
