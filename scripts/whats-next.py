#!/usr/bin/env python3
"""Deterministic data collection for the /whats-next skill.

Emits a single JSON document on stdout describing every potential work source
in the project (TODOs, planning docs, feature plans, open spec questions,
in-flight git state, roadmap, tech debt). Sections whose source is missing
are emitted as empty — the consumer skips them silently.

Usage:
    python3 scripts/whats-next.py            # JSON to stdout
    python3 scripts/whats-next.py --pretty   # indented JSON for inspection

The skill consumes this output, verifies each candidate against current repo
state, scores it (Value/Effort/Recommend), and picks the suggested next action.
This script has no judgment — only collection.
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path


def _project_root() -> Path:
    """Resolve repo root via `git rev-parse --show-toplevel`, matching the
    convention used by the project's bash helpers. Fall back to `Path.cwd()` if
    git is unavailable or the script is invoked outside a checkout."""
    try:
        out = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
        return Path(out)
    except (subprocess.CalledProcessError, FileNotFoundError):
        return Path.cwd()


ROOT = _project_root()
SOURCE_DIRS = ["src", "src-tauri/src"]
SOURCE_EXTS = (".ts", ".tsx", ".rs")


def _read(path: Path) -> str | None:
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError:
        return None
    except UnicodeDecodeError:
        return None


def _git(*args: str) -> str:
    """Run git and return stdout. Trailing newlines stripped; leading whitespace
    preserved (matters for `git status --porcelain` where col 0 is the staged
    indicator and a leading space is meaningful)."""
    try:
        return subprocess.run(
            ["git", *args],
            check=False,
            capture_output=True,
            text=True,
        ).stdout.rstrip("\n")
    except FileNotFoundError:
        return ""


def collect_todo_file() -> dict | None:
    """Return TODO file content split into sections, or None if absent.

    Two bullet styles are captured per section:
      - Checkbox bullets at any indent (`- [ ] foo`, `- [x] foo`) — surfaced
        with `done: true|false` from the checkbox state.
      - Plain top-level bullets (`- foo`, no checkbox) — surfaced with
        `done: false`. The TODO candidates pool typically uses this style;
        only top-level entries count (nested `  - sub-item` lines are
        treated as continuation of the parent, not separate candidates).
    """
    for name in ("docs/todo.md", "docs/TODO.md"):
        path = ROOT / name
        text = _read(path)
        if text is None:
            continue
        sections: list[dict] = []
        current_heading: str | None = None
        current_items: list[dict] = []
        for line in text.splitlines():
            heading = re.match(r"^##\s+(.+?)\s*$", line)
            if heading:
                if current_heading is not None:
                    sections.append(
                        {"heading": current_heading, "items": current_items}
                    )
                current_heading = heading.group(1)
                current_items = []
                continue
            checkbox = re.match(r"^\s*-\s+\[(?P<state>[ xX])\]\s+(?P<text>.+)$", line)
            if checkbox and current_heading is not None:
                current_items.append(
                    {
                        "done": checkbox.group("state").lower() == "x",
                        "text": checkbox.group("text").strip(),
                    }
                )
                continue
            # Lookahead excludes ONLY checkbox bullets (`- [ ]` / `- [x]`),
            # not link bullets (`- [text](url)`) — link-bullet TODOs are valid.
            plain = re.match(r"^-\s+(?!\[[ xX]\])(?P<text>.+)$", line)
            if plain and current_heading is not None:
                current_items.append(
                    {"done": False, "text": plain.group("text").strip()}
                )
        if current_heading is not None:
            sections.append({"heading": current_heading, "items": current_items})
        return {"path": str(path.relative_to(ROOT)), "sections": sections}
    return None


def collect_inline_todos() -> list[dict]:
    """Scan source dirs for TODO/FIXME comments."""
    hits: list[dict] = []
    pattern = re.compile(r"\b(TODO|FIXME)\b[: ]?\s*(.*)")
    for d in SOURCE_DIRS:
        base = ROOT / d
        if not base.is_dir():
            continue
        for path in base.rglob("*"):
            if not path.is_file() or path.suffix not in SOURCE_EXTS:
                continue
            try:
                lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
            except OSError:
                continue
            for n, line in enumerate(lines, start=1):
                m = pattern.search(line)
                if m:
                    hits.append(
                        {
                            "file": str(path.relative_to(ROOT)),
                            "line": n,
                            "marker": m.group(1),
                            "text": m.group(2).strip(),
                        }
                    )
    return hits


def _extract_open_questions(text: str) -> list[str]:
    """Return unchecked items under any '## Open Questions' heading."""
    items: list[str] = []
    in_section = False
    for line in text.splitlines():
        if re.match(r"^##\s+Open\s+Questions\b", line, re.IGNORECASE):
            in_section = True
            continue
        if in_section and line.startswith("## "):
            in_section = False
            continue
        if in_section:
            m = re.match(r"^\s*-\s+\[\s\]\s+(.+)$", line)
            if m:
                items.append(m.group(1).strip())
    return items


def collect_planning_docs() -> list[dict]:
    """docs/plan-*.md at the docs root."""
    out: list[dict] = []
    docs = ROOT / "docs"
    if not docs.is_dir():
        return out
    for path in sorted(docs.glob("plan-*.md")):
        text = _read(path)
        if text is None:
            continue
        title_match = re.search(r"^#\s+(.+?)\s*$", text, re.MULTILINE)
        out.append(
            {
                "path": str(path.relative_to(ROOT)),
                "title": title_match.group(1) if title_match else path.stem,
                "open_questions": _extract_open_questions(text),
            }
        )
    return out


def collect_feature_plans() -> list[dict]:
    """docs/plan/*-plan.md — extract unchecked items + completion status."""
    out: list[dict] = []
    plan_dir = ROOT / "docs" / "plan"
    if not plan_dir.is_dir():
        return out
    for path in sorted(plan_dir.glob("*-plan.md")):
        text = _read(path)
        if text is None:
            continue
        unchecked: list[str] = []
        total = 0
        done = 0
        for line in text.splitlines():
            m = re.match(r"^\s*-\s+\[(?P<state>[ xX])\]\s+(?P<text>.+)$", line)
            if not m:
                continue
            total += 1
            if m.group("state").lower() == "x":
                done += 1
            else:
                unchecked.append(m.group("text").strip())
        out.append(
            {
                "path": str(path.relative_to(ROOT)),
                "unchecked": unchecked,
                "all_done": total > 0 and done == total,
                "total": total,
                "done": done,
            }
        )
    return out


def collect_spec_open_questions() -> list[dict]:
    """docs/spec/*.md — Open Questions sections."""
    out: list[dict] = []
    spec_dir = ROOT / "docs" / "spec"
    if not spec_dir.is_dir():
        return out
    for path in sorted(spec_dir.glob("*.md")):
        text = _read(path)
        if text is None:
            continue
        questions = _extract_open_questions(text)
        if questions:
            out.append(
                {
                    "path": str(path.relative_to(ROOT)),
                    "questions": questions,
                }
            )
    return out


def collect_in_flight() -> dict:
    """Uncommitted changes, unmerged branches, recent commits."""
    porcelain = _git("status", "--porcelain")
    uncommitted_files = []
    for line in porcelain.splitlines():
        if len(line) >= 3:
            uncommitted_files.append({"status": line[:2].strip(), "file": line[3:]})

    branch_raw = _git("branch", "--no-merged", "main")
    unmerged = [
        b.strip().lstrip("* ").strip()
        for b in branch_raw.splitlines()
        if b.strip() and not b.startswith("*")
    ]

    log_raw = _git("log", "--oneline", "-10")
    recent_commits: list[dict] = []
    for line in log_raw.splitlines():
        parts = line.split(" ", 1)
        if len(parts) == 2:
            recent_commits.append({"sha": parts[0], "subject": parts[1]})

    return {
        "uncommitted_count": len(uncommitted_files),
        "uncommitted_files": uncommitted_files,
        "unmerged_branches": unmerged,
        "recent_commits": recent_commits,
    }


def collect_roadmap() -> dict | None:
    """docs/roadmap.md or roadmap.md — section headings and unchecked bullets."""
    for name in ("docs/roadmap.md", "roadmap.md"):
        path = ROOT / name
        text = _read(path)
        if text is None:
            continue
        headings: list[str] = []
        unchecked: list[str] = []
        for line in text.splitlines():
            m = re.match(r"^##\s+(.+?)\s*$", line)
            if m:
                headings.append(m.group(1).strip())
                continue
            m = re.match(r"^\s*-\s+\[\s\]\s+(.+)$", line)
            if m:
                unchecked.append(m.group(1).strip())
        return {
            "path": str(path.relative_to(ROOT)),
            "headings": headings,
            "unchecked": unchecked,
        }
    return None


def collect_techdebt() -> dict | None:
    """docs/techdebt.md — entries written by the /techdebt skill."""
    path = ROOT / "docs" / "techdebt.md"
    text = _read(path)
    if text is None:
        return None
    entries: list[dict] = []
    blocks = re.split(r"^(?=##\s+\d{4}-\d{2}-\d{2}\b)", text, flags=re.MULTILINE)
    for block in blocks:
        m = re.match(
            r"^##\s+(?P<date>\d{4}-\d{2}-\d{2})\s+—\s+(?P<title>.+?)\s*$",
            block,
            re.MULTILINE,
        )
        if not m:
            continue
        entry = {"date": m.group("date"), "title": m.group("title")}
        for field in ("Found by", "Where", "Context", "Severity", "Observation"):
            fm = re.search(
                rf"^\s*-\s+{re.escape(field)}:\s*(.+?)\s*$", block, re.MULTILINE
            )
            if fm:
                entry[field.lower().replace(" ", "_")] = fm.group(1)
        entry["where_exists"] = _where_path_exists(entry.get("where", ""))
        entries.append(entry)
    return {"path": str(path.relative_to(ROOT)), "entries": entries}


def collect_gh_issues() -> list[dict]:
    """Open GitHub issues via `gh issue list`.

    Returns [] when gh is not on PATH, the repo has no GitHub remote, or the
    call fails for any reason — the script must stay portable to non-GitHub
    downstream projects, so this collector skips silently rather than failing.
    """
    if not shutil.which("gh"):
        return []
    try:
        result = subprocess.run(
            [
                "gh",
                "issue",
                "list",
                "--state",
                "open",
                "--json",
                "number,title,url,updatedAt",
                "--limit",
                "20",
            ],
            check=True,
            capture_output=True,
            text=True,
            timeout=10,
        )
        data = json.loads(result.stdout)
        return data if isinstance(data, list) else []
    except (
        subprocess.CalledProcessError,
        subprocess.TimeoutExpired,
        json.JSONDecodeError,
    ):
        return []


def _where_path_exists(where: str) -> bool | None:
    """If `where` looks like a path, check if it exists. Return None if non-path."""
    if not where:
        return None
    candidate = where.split(":", 1)[0].strip()
    if not candidate or " " in candidate:
        return None
    p = ROOT / candidate
    return p.exists()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--pretty", action="store_true", help="indent JSON for human inspection"
    )
    args = parser.parse_args()

    out = {
        "version": 2,
        "todo_file": collect_todo_file(),
        "inline_todos": collect_inline_todos(),
        "planning_docs": collect_planning_docs(),
        "feature_plans": collect_feature_plans(),
        "spec_open_questions": collect_spec_open_questions(),
        "in_flight": collect_in_flight(),
        "roadmap": collect_roadmap(),
        "techdebt": collect_techdebt(),
        "gh_issues": collect_gh_issues(),
    }
    indent = 2 if args.pretty else None
    json.dump(out, sys.stdout, indent=indent, ensure_ascii=False)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
