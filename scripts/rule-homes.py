#!/usr/bin/env python3
"""Every rule has one home (todo TODO-050).

A rule is defined where its ID opens a line followed by an em dash:
`**B24** — …` and `## E11 — …` in the rules docs, `**MKT-143 — …**` in a spec.
Anywhere else an ID is a mention. The check lists every definition in the
documents agents read (`docs/`, `CLAUDE.md`, `ARCHITECTURE.md`, `.claude/`) and
fails when one ID is defined in two places — the copy is removed and the other
document links to the home instead.

Use: python3 scripts/rule-homes.py
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFINITION = re.compile(
    r"^(?:\*\*([A-Z]{1,3}\d+)\*\* —|#{2,3} ([A-Z]{1,3}\d+) —|\*\*([A-Z]{3}-\d{3}) —)"
)


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
    if twice:
        print(f"❌ rule homes: {len(twice)} rule(s) defined in more than one place")
        for rule_id, paths in sorted(twice.items()):
            print(f"   {rule_id}: {', '.join(paths)}")
        return 1
    total = sum(len(ids) for ids in found.values())
    print(f"✅ rule homes: {total} rules, each defined once")
    return 0


if __name__ == "__main__":
    sys.exit(main())
