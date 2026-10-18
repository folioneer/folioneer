#!/usr/bin/env python3
"""Every rule has one home, every piece of the agentic setup one line.

Two checks against drift:

1. Rule homes. A rule is defined where its ID opens a line followed by an em dash:
   `**B24** — …` and `## E11 — …` in the rules docs, `**MKT-143 — …**` in a spec.
   Anywhere else an ID is a mention. The check lists every definition in the
   documents agents read (`docs/`, `CLAUDE.md`, `ARCHITECTURE.md`, `.claude/`) and
   fails when one ID is defined in two places — the copy is removed and the other
   document links to the home instead.
2. The agentic setup. `docs/agentic-setup.md` gives one list item to each piece,
   opened by its backticked path. Every script, skill, agent prompt and git hook
   that exists is listed once, and everything listed exists.

Use: python3 scripts/rule-homes.py
"""

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFINITION = re.compile(
    r"^(?:\*\*([A-Z]{1,3}\d+)\*\* —"
    r"|#{2,3} ([A-Z]{1,3}\d+) —"
    r"|\*\*([A-Z]{3}-\d{3}) —)"
)
SETUP = "docs/agentic-setup.md"
PIECE = re.compile(r"^- `([^`\s]+)`")
SKILL = re.compile(r"(\.claude/skills/[^/]+/).+")


def defined_ids(text: str) -> list[str]:
    """The rule IDs a document defines, in order."""
    ids = []
    for line in text.splitlines():
        match = DEFINITION.match(line)
        if match:
            ids.append(next(group for group in match.groups() if group))
    return ids


def duplicates(found: dict[str, list[str]]) -> dict[str, list[str]]:
    """IDs defined in more than one place, with every place, sorted."""
    homes: dict[str, list[str]] = {}
    for path, ids in found.items():
        for rule_id in ids:
            homes.setdefault(rule_id, []).append(path)
    return {rule_id: sorted(paths) for rule_id, paths in homes.items() if len(paths) > 1}


def listed_pieces(setup_text: str) -> list[str]:
    """The paths the setup document gives a line to, in order: each opens a list item."""
    return [
        match.group(1)
        for line in setup_text.splitlines()
        if (match := PIECE.match(line))
    ]


def setup_pieces(files: list[str]) -> list[str]:
    """The pieces that owe a line: scripts, skills (as their folder), agent prompts, git hooks."""
    pieces = set()
    for f in files:
        skill = SKILL.fullmatch(f)
        if skill:
            pieces.add(skill.group(1))
        elif re.fullmatch(r"scripts/[^/]+|\.claude/agents/[^/]+\.md", f):
            pieces.add(f)
        elif re.fullmatch(r"\.githooks/[^/.]+", f):
            pieces.add(f)
    return sorted(pieces)


def setup_problems(pieces: list[str], listed: list[str], exists) -> list[str]:
    """Pieces without a line, lines without their piece, and pieces listed twice."""
    problems = [
        f"{piece}: exists but has no line in {SETUP}"
        for piece in pieces
        if piece not in listed
    ]
    problems += [
        f"{path}: listed in {SETUP} but missing"
        for path in dict.fromkeys(listed)
        if not exists(path)
    ]
    problems += [
        f"{path}: listed twice in {SETUP}"
        for path in dict.fromkeys(listed)
        if listed.count(path) > 1
    ]
    return problems


def tracked(*patterns: str) -> list[str]:
    out = subprocess.run(
        ["git", "ls-files", *patterns],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    return [line for line in out.stdout.splitlines() if (ROOT / line).is_file()]


def documents() -> list[Path]:
    paths = [ROOT / "CLAUDE.md", ROOT / "ARCHITECTURE.md"]
    paths += sorted((ROOT / "docs").rglob("*.md"))
    paths += sorted((ROOT / ".claude").rglob("*.md"))
    return [path for path in paths if path.is_file()]


def main() -> int:
    found = {
        str(path.relative_to(ROOT)): defined_ids(path.read_text(encoding="utf-8"))
        for path in documents()
    }
    twice = duplicates(found)
    problems = [
        f"{rule_id}: defined in {', '.join(paths)}"
        for rule_id, paths in sorted(twice.items())
    ]

    listed = listed_pieces((ROOT / SETUP).read_text(encoding="utf-8"))
    pieces = setup_pieces(tracked("scripts", ".claude", ".githooks"))
    problems += setup_problems(
        pieces, listed, exists=lambda path: (ROOT / path).exists()
    )

    if problems:
        print(f"❌ rule homes: {len(problems)} problem(s)")
        for problem in problems:
            print(f"   {problem}")
        return 1
    total = sum(len(ids) for ids in found.values())
    print(
        f"✅ rule homes: {total} rules, each defined once; {len(pieces)} setup pieces listed"
    )
    return 0


if __name__ == "__main__":
    import usage_log

    usage_log.start()
    sys.exit(main())
