#!/usr/bin/env python3
"""Every rule has one home, every piece of the agentic setup one line.

Three checks against drift:

1. Rule homes (todo TODO-050). A rule or an entry is defined where its ID opens a line
   followed by an em dash: `**B24** — …` and `## E11 — …` in the rules docs,
   `**MKT-143 — …**` in a spec, `# TODO-003 — …`, `# 2026-09-27 — DEBT-008 — …` and
   `# FLOW-001 — …` in an entry. Anywhere else an ID is a mention. The check lists every definition in
   the documents agents read (`docs/`, `CLAUDE.md`, `ARCHITECTURE.md`, `.claude/`) and
   fails when one ID is defined in two places — the copy is removed and the other
   document links to the home instead.
2. Entry files. A todo, debt or flow entry is one file under `docs/work/`, named by
   the one ID it defines (`docs/work/debt/DEBT-008.md`), in the folder of its kind.
3. The agentic setup. `docs/agentic-setup.md` gives one list item to each piece,
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
    r"|\*\*([A-Z]{3}-\d{3}) —"
    r"|# ((?:TODO|FLOW)-\d{3}) —"
    r"|# \d{4}-\d{2}-\d{2} — (DEBT-\d{3}) —)"
)
ENTRY_FILE = re.compile(r"docs/work/(todo|debt|flow)/(.+)\.md")
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


def misnamed(found: dict[str, list[str]]) -> list[str]:
    """Entry files whose name, folder and heading do not say the same ID."""
    problems = []
    for path, ids in sorted(found.items()):
        entry = ENTRY_FILE.fullmatch(path)
        if not entry:
            continue
        folder, name = entry.groups()
        if not re.fullmatch(rf"{folder.upper()}-\d{{3}}", name):
            problems.append(
                f"{path}: an entry of {folder}/ is named {folder.upper()}-NNN.md"
            )
        elif ids != [name]:
            problems.append(
                f"{path}: defines {', '.join(ids) or 'no ID'}, not the one ID {name} it is named after"
            )
    return problems


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


QUEUE = "docs/work/queue.md"
# The queue's lines and an entry's size as `scripts/work-queue.py` reads them (`QUEUE_LINE`,
# `_field`, `_debt_field`): what `just queue` takes as queued and sized is what is checked.
QUEUED = re.compile(r"^\s*(?:-|\d+\.)\s+((?:TODO|DEBT|FLOW)-\d+)\b", re.MULTILINE)
SIZE = re.compile(r"^(?:\*\*Size:\*\*|\s*- Size:)\s*(XL|S|M|L)\b", re.MULTILINE)


def unsized_queued(queue_text: str, entry_text) -> list[str]:
    """A queued entry carries a size, and a programme is sliced before it is queued
    (`docs/workflow.md` § 2). `entry_text(ref)` is the entry's file, or None once it has
    shipped: nothing is asked of that one."""
    problems = []
    for ref in QUEUED.findall(re.sub(r"<!--.*?-->", "", queue_text, flags=re.DOTALL)):
        text = entry_text(ref)
        if text is None:
            continue
        size = SIZE.search(text)
        if size is None:
            problems.append(
                f"{QUEUE}: {ref} is queued without a size (S, M or L) — docs/workflow.md § 2"
            )
        elif size.group(1) == "XL":
            problems.append(
                f"{QUEUE}: {ref} is queued as XL — a programme is sliced before it is queued"
            )
    return problems


def queue_problems() -> list[str]:
    """`unsized_queued` on this repository's queue and entry files."""
    queue = ROOT / QUEUE
    if not queue.exists():
        return []

    def entry_text(ref: str) -> str | None:
        path = ROOT / "docs" / "work" / ref.split("-")[0].lower() / f"{ref}.md"
        return path.read_text(encoding="utf-8") if path.exists() else None

    return unsized_queued(queue.read_text(encoding="utf-8"), entry_text)


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
    problems += misnamed(found)

    listed = listed_pieces((ROOT / SETUP).read_text(encoding="utf-8"))
    pieces = setup_pieces(tracked("scripts", ".claude", ".githooks"))
    problems += setup_problems(
        pieces, listed, exists=lambda path: (ROOT / path).exists()
    )
    problems += queue_problems()

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
