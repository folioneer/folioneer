#!/usr/bin/env python3
"""Deterministic data collection for the /whats-next skill.

Emits one JSON document on stdout: the owner's queue (`docs/todo.md` § Next) with the
state of each reference, the entries ready to queue, the entries blocked with what
each waits on, the tech debt grouped by theme, the flow entries, the open pull requests with their CI
state, the local git state, the roadmap and the open GitHub issues.

Usage:
    python3 scripts/whats-next.py            # JSON to stdout
    python3 scripts/whats-next.py --pretty   # indented JSON for inspection

The skill turns this into three lists and a proposed queue order for the owner to
accept or edit. This script has no judgment — only collection and the readiness rule
of `docs/workflow.md`.
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


ENTRY_HEADING = re.compile(r"^##\s+#(?P<number>\d+)\s+—\s+(?P<title>.+?)\s*$")
DEBT_HEADING = re.compile(
    r"^##\s+(?P<date>\d{4}-\d{2}-\d{2})\s+—\s+(?P<ref>TD-\d+)\s+—\s+(?P<title>.+?)\s*$"
)
FLOW_HEADING = re.compile(r"^##\s+(?P<ref>FLOW-\d+)\s+—\s+(?P<title>.+?)\s*$")
QUEUE_LINE = re.compile(r"^\s*\d+\.\s+(?P<ref>#\d+|TD-\d+|FLOW-\d+)\b")


def _sections(text: str) -> list[tuple[str, list[str]]]:
    """Split a Markdown file into (`## ` heading line, body lines)."""
    sections: list[tuple[str, list[str]]] = []
    for line in text.splitlines():
        if line.startswith("## "):
            sections.append((line, []))
        elif sections:
            sections[-1][1].append(line)
    return sections


def parse_queue(todo_text: str) -> list[str]:
    """The references of `## Next`, in the owner's order."""
    for heading, body in _sections(todo_text):
        if heading.strip() == "## Next":
            return [m.group("ref") for m in map(QUEUE_LINE.match, body) if m]
    return []


def _field(body: list[str], name: str) -> str | None:
    """The text after `**name:**` on its line, or None when the line is absent."""
    prefix = f"**{name}:**"
    for line in body:
        if line.startswith(prefix):
            return line[len(prefix) :].strip()
    return None


def _question_items(body: list[str]) -> list[tuple[bool, str]]:
    """The list items under `**Open questions:**`, each with whether it is ticked.
    An item with no checkbox counts as unticked."""
    items: list[tuple[bool, str]] = []
    listing = False
    for line in body:
        if line.startswith("**Open questions:**"):
            listing = True
            continue
        if not listing:
            continue
        if line.startswith("**") or line.startswith("## "):
            break
        item = re.match(
            r"^\s*(?:[-*]|\d+\.)\s+(?:\[(?P<box>[ xX])\]\s+)?(?P<text>.+)$", line
        )
        if item:
            items.append(
                ((item.group("box") or " ") != " ", item.group("text").strip())
            )
    return items


def readiness(body: list[str]) -> list[str]:
    """What an entry waits on before the agent may run it; empty when it is ready.

    Ready (docs/workflow.md): a Done when; open questions that are `none` or a list
    with every box ticked; a design that is `none` or `validated`. Anything else —
    a missing line, other words — waits: the rule fails closed.
    """
    waits: list[str] = []
    if not _field(body, "Done when"):
        waits.append("no Done when")

    questions = _field(body, "Open questions")
    items = _question_items(body)
    if questions is None:
        waits.append("no Open questions line")
    elif questions.rstrip(".").lower() == "none":
        pass
    elif questions:
        waits.append(f"question: {questions}")
    elif not items:
        waits.append("no Open questions line")
    waits.extend(f"question: {text}" for ticked, text in items if not ticked)

    design = _field(body, "Design")
    if design is None:
        waits.append("no Design line")
    elif design.rstrip(".").lower() not in ("none", "validated"):
        waits.append(f"design to validate: {design}")
    return waits


def parse_entries(todo_text: str) -> list[dict]:
    """Every `## #NNN — …` entry of the todo file, with what it waits on."""
    entries: list[dict] = []
    for heading, body in _sections(todo_text):
        match = ENTRY_HEADING.match(heading)
        if not match:
            continue
        entries.append(
            {
                "ref": f"#{match.group('number')}",
                "title": match.group("title"),
                "user_value": _field(body, "User value"),
                "waits_on": readiness(body),
            }
        )
    return entries


def debt_theme(title: str) -> str:
    """A title with its code spans and figures blanked: entries that differ only by
    a name or a count share a theme."""
    return re.sub(r"\b\d+\b", "N", re.sub(r"`[^`]*`", "`…`", title))


def _debt_field(body: list[str], name: str) -> str | None:
    prefix = f"- {name}:"
    for line in body:
        if line.strip().startswith(prefix):
            return line.strip()[len(prefix) :].strip()
    return None


def parse_debt(debt_text: str) -> list[dict]:
    """Every `## date — TD-NNN — …` entry of the tech-debt file."""
    entries: list[dict] = []
    for heading, body in _sections(debt_text):
        match = DEBT_HEADING.match(heading)
        if not match:
            continue
        entries.append(
            {
                "ref": match.group("ref"),
                "date": match.group("date"),
                "title": match.group("title"),
                "theme": debt_theme(match.group("title")),
                "severity": _debt_field(body, "Severity"),
                "user_value": _debt_field(body, "User value"),
                "waits_on": [] if _debt_field(body, "Done when") else ["no Done when"],
            }
        )
    return entries


def parse_flow(flow_text: str) -> list[dict]:
    """Every `## FLOW-NNN — …` entry of the flow file that proposes a change. An entry
    with a verdict and no proposal is a record, not work: it is left out, and a queued
    reference to it reads as closed. An entry carrying both is work. One whose
    `Needs the owner:` line starts with yes waits on that decision."""
    entries: list[dict] = []
    for heading, body in _sections(flow_text):
        match = FLOW_HEADING.match(heading)
        if not match or _debt_field(body, "Proposal") is None:
            continue
        needs_owner = (
            (_debt_field(body, "Needs the owner") or "")
            .strip("* ")
            .lower()
            .startswith("yes")
        )
        entries.append(
            {
                "ref": match.group("ref"),
                "title": match.group("title"),
                "kind": _debt_field(body, "Kind"),
                "waits_on": ["the owner's decision"] if needs_owner else [],
            }
        )
    return entries


def group_debt(entries: list[dict]) -> list[dict]:
    """Tech-debt entries grouped by theme, in the order themes first appear."""
    themes: dict[str, list[dict]] = {}
    for entry in entries:
        themes.setdefault(entry["theme"], []).append(entry)
    return [
        {
            "theme": theme,
            "refs": [entry["ref"] for entry in members],
            "entries": members,
        }
        for theme, members in themes.items()
    ]


def classify(queue: list[str], entries: list[dict]) -> dict:
    """The three lists: queued (in the owner's order), ready but not queued, blocked.

    A queued reference with no entry left is reported as `closed`: its work merged
    and the owner has not yet removed it from the queue.
    """
    by_ref = {entry["ref"]: entry for entry in entries}
    queued = [
        by_ref[ref] | {"state": "blocked" if by_ref[ref]["waits_on"] else "ready"}
        if ref in by_ref
        else {"ref": ref, "state": "closed"}
        for ref in queue
    ]
    in_queue = set(queue)
    unqueued = [entry for entry in entries if entry["ref"] not in in_queue]
    return {
        "queued": queued,
        "ready": [entry for entry in unqueued if not entry["waits_on"]],
        "blocked": [entry for entry in unqueued if entry["waits_on"]],
    }


FAILING = (
    "FAILURE",
    "TIMED_OUT",
    "CANCELLED",
    "ERROR",
    "ACTION_REQUIRED",
    "STARTUP_FAILURE",
    "STALE",
)


def ci_state(checks: list[dict]) -> str:
    """One word for a pull request's checks: failing, running, green or none."""
    if not checks:
        return "none"
    results = [
        (check.get("conclusion") or check.get("state") or "").upper()
        for check in checks
    ]
    if any(result in FAILING for result in results):
        return "failing"
    if any(result in ("", "PENDING", "EXPECTED") for result in results):
        return "running"
    return "green"


def _gh_json(*args: str) -> list[dict]:
    """A `gh … --json` listing, or [] when gh is absent, offline or refused."""
    if not shutil.which("gh"):
        return []
    try:
        result = subprocess.run(
            ["gh", *args], check=True, capture_output=True, text=True, timeout=15
        )
        data = json.loads(result.stdout)
        return data if isinstance(data, list) else []
    except (
        subprocess.CalledProcessError,
        subprocess.TimeoutExpired,
        json.JSONDecodeError,
    ):
        return []


def collect_pull_requests() -> list[dict]:
    """Open pull requests with the state of their checks."""
    return [
        {
            "number": pull["number"],
            "title": pull["title"],
            "branch": pull["headRefName"],
            "ci": ci_state(pull.get("statusCheckRollup") or []),
        }
        for pull in _gh_json(
            "pr",
            "list",
            "--state",
            "open",
            "--json",
            "number,title,headRefName,statusCheckRollup",
        )
    ]


def collect_work() -> dict:
    """The queue, the todo entries, the tech debt and the flow entries, classified.

    `docs/flow.md` holds its own `FLOW-NNN` entries and the todo and tech-debt entries
    that are about the flow; the latter keep their reference and are read as what they
    are."""
    todo_text = _read(ROOT / "docs" / "todo.md") or ""
    flow_text = _read(ROOT / "docs" / "flow.md") or ""
    debt = parse_debt(_read(ROOT / "docs" / "techdebt.md") or "")
    flow = parse_flow(flow_text) + parse_entries(flow_text) + parse_debt(flow_text)
    lists = classify(parse_queue(todo_text), parse_entries(todo_text) + debt + flow)
    queued = {entry["ref"] for entry in lists["queued"]}
    in_todo = {entry["ref"] for entry in parse_entries(todo_text)}
    for name in ("ready", "blocked"):
        lists[name] = [entry for entry in lists[name] if entry["ref"] in in_todo]
    lists["techdebt_not_queued"] = group_debt(
        [entry for entry in debt if entry["ref"] not in queued]
    )
    lists["flow_not_queued"] = [entry for entry in flow if entry["ref"] not in queued]
    return lists


def collect_in_flight() -> dict:
    """Uncommitted changes, unmerged branches, recent commits."""
    porcelain = _git("status", "--porcelain")
    uncommitted_files = []
    for line in porcelain.splitlines():
        if len(line) >= 3:
            uncommitted_files.append({"status": line[:2].strip(), "file": line[3:]})

    branch_raw = _git("branch", "--no-merged", "main")
    unmerged = [
        b.strip().lstrip("*+ ").strip()
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


ENTRY_REF = re.compile(r"#\d{3}\b|\bTD-\d{3}\b|\bFLOW-\d{3}\b")


def entries_naming(number: int, texts: list[str]) -> list[str]:
    """The references of the entries that name GitHub issue `number` — as `gh#N`, by its
    URL, or as `issue N` — in the order the documents are given. An issue no entry names
    has nothing tracking it in the repository: it is to file or to close."""
    mention = re.compile(rf"\bgh#{number}\b|/issues/{number}\b|\bissue {number}\b")
    refs = []
    for text in texts:
        for heading, body in _sections(text):
            ref = ENTRY_REF.search(heading)
            if ref and mention.search("\n".join(body)) and ref.group(0) not in refs:
                refs.append(ref.group(0))
    return refs


def collect_gh_issues() -> list[dict]:
    """Open GitHub issues, each with the entries that name it (`entries`)."""
    texts = [
        _read(ROOT / "docs" / name) or "" for name in ("todo.md", "techdebt.md", "flow.md")
    ]
    return [
        {**issue, "entries": entries_naming(issue["number"], texts)}
        for issue in _open_gh_issues()
    ]


def _open_gh_issues() -> list[dict]:
    return _gh_json(
        "issue",
        "list",
        "--state",
        "open",
        "--json",
        "number,title,url,updatedAt",
        "--limit",
        "20",
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--pretty", action="store_true", help="indent JSON for human inspection"
    )
    args = parser.parse_args()

    out = {
        "version": 4,
        **collect_work(),
        "pull_requests": collect_pull_requests(),
        "in_flight": collect_in_flight(),
        "roadmap": collect_roadmap(),
        "gh_issues": collect_gh_issues(),
    }
    indent = 2 if args.pretty else None
    json.dump(out, sys.stdout, indent=indent, ensure_ascii=False)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
