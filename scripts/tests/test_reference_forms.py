"""Tests for scripts/reference-forms.py (TODO-049) — run with `just test-scripts`."""

import importlib.util
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "reference-forms.py"
spec = importlib.util.spec_from_file_location("reference_forms", SCRIPT)
reference_forms = importlib.util.module_from_spec(spec)
spec.loader.exec_module(reference_forms)


class ReferenceForms(unittest.TestCase):
    # TODO-049 — the three forms that were replaced are found, with what to write instead.
    def test_the_old_forms_are_found_with_their_line(self):
        text = "Closes #043.\nSee TD-064 and TD-NNN.\nAsked in gh#79.\n"
        self.assertEqual(
            [(line, written) for line, written, _ in reference_forms.old_forms(text)],
            [(1, "#043"), (2, "TD-064"), (2, "TD-NNN"), (3, "gh#79")],
        )
        advice = reference_forms.old_forms("#043")[0][2]
        self.assertIn("TODO-NNN", advice)

    # TODO-049 — the new forms, a pull request number, a colour and an anchor are not
    # references of the old form.
    def test_the_new_forms_and_what_only_looks_like_a_reference_pass(self):
        for text in (
            "TODO-043, DEBT-064, FLOW-021 and gh79",
            "PR #143 and pull request #96",
            "color: #003d82; background: #000000;",
            "docs/workflow.md#043 and &#039;",
            "STD-064 is no debt entry",
        ):
            self.assertEqual(reference_forms.old_forms(text), [], text)

    # TODO-049 — in a style sheet a hash and three digits is a colour: it is not read as a
    # todo entry there, while the other forms still are.
    def test_a_short_colour_in_a_style_sheet_is_no_todo_entry(self):
        sheet = "a { color: #012; } /* TD-064 */"
        self.assertEqual(
            [written for _, written, _ in reference_forms.old_forms(sheet, style_sheet=True)],
            ["TD-064"],
        )
        self.assertEqual(len(reference_forms.old_forms(sheet)), 2)

    # TODO-049 — the changelog keeps the old forms; binary and generated files are not read.
    def test_the_changelog_and_what_is_not_text_are_left_alone(self):
        for path in (
            "CHANGELOG.md",
            "screenshots/AssetTable-light-idle.png",
            "src-tauri/.sqlx/query-1.json",
            "package-lock.json",
            "scripts/reference-forms.py",
        ):
            self.assertFalse(reference_forms.checked(path), path)
        for path in (
            "docs/todo.md",
            "src-tauri/src/lib.rs",
            "scripts/whats-next.py",
            "licence-allowlist.json",
        ):
            self.assertTrue(reference_forms.checked(path), path)

    # TODO-049 — no file of the repository still carries an old form.
    def test_the_repository_carries_no_old_form(self):
        self.assertEqual(reference_forms.main([]), 0)


if __name__ == "__main__":
    unittest.main()
