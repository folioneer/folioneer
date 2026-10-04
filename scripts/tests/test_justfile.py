"""Tests for the scoped recipes of the justfile (FLOW-014) — run with `just test-scripts`.

`just --dry-run` prints the commands a recipe would run without running them. The CI job
that runs the script tests has no `just`, so these are skipped there: `just harness` is
what runs them.
"""

import shutil
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def dry_run(*recipe: str) -> str:
    result = subprocess.run(
        ["just", "--dry-run", *recipe],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    return result.stdout + result.stderr


@unittest.skipUnless(shutil.which("just"), "just is not installed")
class ScopedRecipes(unittest.TestCase):
    # FLOW-014 — a part of the backend suite runs through the recipe, on two build jobs
    # unless the machine's own setting says otherwise.
    def test_the_backend_recipe_passes_its_arguments_and_caps_the_build(self):
        command = dry_run("test-rust", "--lib", "div_040")
        self.assertIn('cargo test "$@"', command)
        self.assertIn(
            'CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-${KIT_CHECK_JOBS:-2}}"', command
        )

    # FLOW-014 — a part of the frontend suite runs through the recipe.
    def test_the_frontend_recipe_passes_its_arguments(self):
        self.assertIn('npm test -- "$@"', dry_run("test", "src/features/settings"))

    # The lint recipe takes the layer to check.
    def test_the_lint_recipe_takes_a_layer(self):
        self.assertIn('scripts/check.py --fast "$@"', dry_run("check", "--frontend"))

    # Without a scope the unit recipe still runs both suites.
    def test_without_a_scope_both_suites_run(self):
        both = dry_run("test-unit")
        self.assertIn("npm test --", both)
        self.assertIn("cargo test", both)

    # An argument reaches the command whole: a space does not split it and the shell reads
    # nothing in it. `check` is the one recipe cheap enough to run for real here.
    def test_an_argument_with_a_space_or_a_shell_character_stays_one_argument(self):
        result = subprocess.run(
            ["just", "check", "--no such flag; echo SPLIT"],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "unrecognized arguments: --no such flag; echo SPLIT", result.stderr
        )
        self.assertNotIn("\nSPLIT", result.stdout)


if __name__ == "__main__":
    unittest.main()
