"""Tests for scripts/e2e-scope.py (todo #041) — run with `just test-scripts`."""

import importlib.util
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "e2e_scope", Path(__file__).resolve().parents[1] / "e2e-scope.py"
)
e2e_scope = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(e2e_scope)


class SuiteMustRun(unittest.TestCase):
    # #041 — a pull request of records only (Markdown, docs/, the agent tooling, the
    # committed visual proofs) changes nothing the suite can execute.
    def test_records_only_skip_the_suite(self):
        files = [
            "docs/todo.md",
            "CHANGELOG.md",
            "README.md",
            "docs/adr/021-withdraw-the-keyless-price-source.md",
            ".claude/skills/next-todo/SKILL.md",
            "screenshots/CurrencyRatesView-light-idle.png",
        ]
        self.assertFalse(e2e_scope.suite_must_run(files))

    # #041 — application code, the tests themselves, the workflow and the test tooling
    # all run the suite.
    def test_anything_the_suite_can_execute_runs_it(self):
        for path in [
            "src/App.tsx",
            "src-tauri/src/lib.rs",
            "src-tauri/Cargo.lock",
            "e2e/accounts/accounts.test.ts",
            "e2e/helpers/screenshot.ts",
            ".github/workflows/e2e.yml",
            "wdio.conf.ts",
            "package.json",
            "scripts/visual-proof-capture.mjs",
        ]:
            with self.subTest(path=path):
                self.assertTrue(e2e_scope.suite_must_run(["docs/todo.md", path]))

    # #041 — no file list means nothing proves the change is records only: run.
    def test_an_unknown_change_runs_the_suite(self):
        self.assertTrue(e2e_scope.suite_must_run([]))


if __name__ == "__main__":
    unittest.main()
