"""Tests for scripts/check.py (todo TODO-046) — run with `just test-scripts`."""

import importlib.util
import os
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

SPEC = importlib.util.spec_from_file_location(
    "check", Path(__file__).resolve().parents[1] / "check.py"
)
check = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(check)

runs_prepare_check = check.QualityChecker.runs_sqlx_prepare_check


class SqlxPrepareCheck(unittest.TestCase):
    # TODO-046 — the full check makes cargo rebuild the crate, so the local loop leaves it to CI.
    def test_the_local_loop_leaves_the_full_check_to_ci(self):
        with mock.patch.dict(os.environ, {}, clear=True):
            self.assertFalse(runs_prepare_check(SimpleNamespace(strict_mode=False)))

    # TODO-046 — CI keeps the full check on every pull request.
    def test_ci_runs_the_full_check(self):
        with mock.patch.dict(os.environ, {"CI": "true"}, clear=True):
            self.assertTrue(runs_prepare_check(SimpleNamespace(strict_mode=False)))

    # TODO-046 — a release (--strict) runs it too.
    def test_a_release_runs_the_full_check(self):
        with mock.patch.dict(os.environ, {}, clear=True):
            self.assertTrue(runs_prepare_check(SimpleNamespace(strict_mode=True)))

    # A release run from a terminal sets no database address, and sqlx-cli 0.9 refuses to
    # start without one: the check is given the check database, at the address CI uses.
    def test_a_release_gives_the_check_a_database_when_the_shell_sets_none(self):
        checker = SimpleNamespace(repo_root=Path("/repo"))
        with mock.patch.dict(os.environ, {}, clear=True):
            self.assertEqual(
                check.QualityChecker.sqlx_check_database(checker),
                {"DATABASE_URL": "sqlite:/repo/src-tauri/.local/dev_check.sqlite"},
            )

    # The address the shell or the workflow sets is left as it is.
    def test_an_address_already_set_is_kept(self):
        checker = SimpleNamespace(repo_root=Path("/repo"))
        with mock.patch.dict(os.environ, {"DATABASE_URL": "sqlite:/ci/db.sqlite"}, clear=True):
            self.assertEqual(check.QualityChecker.sqlx_check_database(checker), {})


class TestsWithoutTheApp(unittest.TestCase):
    # FLOW-042 — a change of the agent connection, the module the `app` feature splits, builds
    # the library's tests without it, as CI's Windows job does.
    def test_a_change_of_the_agent_connection_builds_them(self):
        with mock.patch.dict(os.environ, {}, clear=True):
            self.assertTrue(
                check.builds_tests_without_the_app(
                    ["docs/workflow.md", "src-tauri/src/agent_connection/bridge.rs"]
                )
            )

    # FLOW-042 — anything else leaves the build out: it costs about a minute.
    def test_another_change_leaves_them_out(self):
        with mock.patch.dict(os.environ, {}, clear=True):
            self.assertFalse(
                check.builds_tests_without_the_app(["src-tauri/src/context/sync/mod.rs"])
            )

    # FLOW-042 — in CI the Windows job builds them; the Linux job does not build them twice.
    def test_ci_leaves_them_to_its_windows_job(self):
        with mock.patch.dict(os.environ, {"CI": "true"}, clear=True):
            self.assertFalse(
                check.builds_tests_without_the_app(["src-tauri/src/agent_connection/bridge.rs"])
            )


    # FLOW-042 — when the changed paths cannot be read, the tests are built rather than skipped.
    def test_unreadable_changes_build_them(self):
        checker = SimpleNamespace(repo_root=Path("/repo"))
        failed = SimpleNamespace(returncode=128, stdout="")
        with mock.patch.dict(os.environ, {}, clear=True), mock.patch.object(
            check.subprocess, "run", return_value=failed
        ):
            self.assertTrue(check.QualityChecker.builds_tests_without_the_app(checker))


if __name__ == "__main__":
    unittest.main()
