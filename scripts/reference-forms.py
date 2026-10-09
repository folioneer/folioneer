#!/usr/bin/env python3
"""A reference says what it points at (TODO-049).

A todo entry is `TODO-NNN`, a tech-debt entry `DEBT-NNN`, a flow entry `FLOW-NNN` and
a GitHub issue `ghNN`. The forms they replaced — a hash and three digits for a todo
entry, which reads like a pull request number; the two-letter prefix for tech debt; a
hash after `gh` for an issue — are refused in every file git tracks.

Left as they are: `CHANGELOG.md`, which keeps what was released under the old forms,
this check, its tests and the queue script's tests, which have to name what they refuse,
and the account contract until it is brought up to date (DEBT-088).

Use: python3 scripts/reference-forms.py [file ...]   (every tracked file when none)
"""

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# A hash and three digits starting with 0, not inside a word, a colour or an anchor. A
# hash before a number of 100 or more is a pull request, which keeps that form: no todo
# entry was ever written that way. A three-digit colour made of digits only and starting
# with 0 (`#012`) would read as one: style sheets are not read for this form.
OLD_TODO = re.compile(r"(?<![\w&#/])#0\d\d\b")
OLD_DEBT = re.compile(r"\bTD-(?:\d{3}|NNN)\b")
OLD_ISSUE = re.compile(r"\bgh#(?:\d+|N+)")
TODO_ADVICE = "a todo entry is TODO-NNN"
STYLE_SHEETS = (".css",)
OLD_FORMS = (
    (OLD_TODO, TODO_ADVICE),
    (OLD_DEBT, "a tech-debt entry is DEBT-NNN"),
    (OLD_ISSUE, "a GitHub issue is ghNN"),
)

KEPT = {
    "CHANGELOG.md",
    "scripts/reference-forms.py",
    "scripts/tests/test_reference_forms.py",
    "scripts/tests/test_next_todo.py",
    # Out of date against the code (DEBT-088): its references are renamed with that work.
    "docs/contracts/account-contract.md",
}
NOT_TEXT = (".png", ".ico", ".icns", ".woff", ".woff2", ".lock", "package-lock.json")
GENERATED = ("src-tauri/.sqlx/", "screenshots/")


def checked(path: str) -> bool:
    """Whether the check reads this file."""
    return not (path in KEPT or path.endswith(NOT_TEXT) or path.startswith(GENERATED))


def old_forms(text: str, style_sheet: bool = False) -> list[tuple[int, str, str]]:
    """Every old-form reference of a text: its line, what was written, what to write. In
    a style sheet a hash and three digits is a colour, never a todo entry."""
    found = []
    for number, line in enumerate(text.splitlines(), start=1):
        for pattern, advice in OLD_FORMS:
            if style_sheet and advice == TODO_ADVICE:
                continue
            for match in pattern.finditer(line):
                found.append((number, match.group(0), advice))
    return found


def tracked() -> list[str]:
    listed = subprocess.run(
        ["git", "ls-files"], cwd=ROOT, check=True, capture_output=True, text=True
    ).stdout
    return [path for path in listed.splitlines() if path]


def main(paths: list[str]) -> int:
    failures = []
    for path in paths or tracked():
        if not checked(path):
            continue
        try:
            text = (ROOT / path).read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        for number, written, advice in old_forms(text, style_sheet=path.endswith(STYLE_SHEETS)):
            failures.append(f"   {path}:{number}: {written} — {advice}")
    if failures:
        print(f"❌ reference forms: {len(failures)} old-form reference(s)")
        print("\n".join(failures))
        return 1
    print("✅ reference forms: every reference says what it points at")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
