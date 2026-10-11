#!/usr/bin/env python3
"""The state of the work queue, for the /plan-queue skill and `just queue`.

Emits one JSON document on stdout: the owner's queue (`docs/work/queue.md`) with the
state of each reference, the entries ready to queue, the entries blocked with what
each waits on, the tech debt grouped by theme, the flow entries, the open pull requests with their CI
state, the local git state, the roadmap and the open GitHub issues.

An entry is one file under `docs/work/`, named by its id. The queue is written once
per batch and no closure edits it: a queued reference whose entry file is gone has
shipped, and reads as `closed`.

Usage:
    python3 scripts/work-queue.py                # JSON to stdout
    python3 scripts/work-queue.py --pretty       # indented JSON for inspection
    python3 scripts/work-queue.py remaining      # the queued references not shipped yet
    python3 scripts/work-queue.py close <id>...  # the closure of a task: deletes each entry's file
    python3 scripts/work-queue.py next-id <TODO|DEBT|FLOW>   # the next free id of a kind

The skill turns the document into three lists and a proposed queue order for the owner
to accept or edit. This script has no judgment — only collection and the readiness rule
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
            cwd=ROOT,
        ).stdout.rstrip("\n")
    except FileNotFoundError:
        return ""


QUEUE_HOME = "docs/work/queue.md"
# An entry is one file named by its id, so two closures never touch the same file.
ENTRY_DIRS = {"TODO": "docs/work/todo", "DEBT": "docs/work/debt", "FLOW": "docs/work/flow"}
ENTRY_ID = re.compile(r"(TODO|DEBT|FLOW)-(\d{3})")
# The highest id each kind had given when the entries moved to one file each
# (2026-10-11): the ids of entries closed before the move are in no folder's history.
ID_FLOORS = {"TODO": 67, "DEBT": 111, "FLOW": 33}

ENTRY_HEADING = re.compile(r"^#\s+TODO-(?P<number>\d+)\s+—\s+(?P<title>.+?)\s*$")
DEBT_HEADING = re.compile(
    r"^#\s+(?P<date>\d{4}-\d{2}-\d{2})\s+—\s+(?P<ref>DEBT-\d+)\s+—\s+(?P<title>.+?)\s*$"
)
FLOW_HEADING = re.compile(r"^#\s+(?P<ref>FLOW-\d+)\s+—\s+(?P<title>.+?)\s*$")
# One reference per line, as a plain list item. A numbered item is still read, so a
# hand-edited queue is never silently empty.
QUEUE_LINE = re.compile(
    r"^\s*(?:-|\d+\.)\s+(?P<ref>TODO-\d+|DEBT-\d+|FLOW-\d+)\b", re.MULTILINE
)


def _sections(text: str) -> list[tuple[str, list[str]]]:
    """Split entry files read one after the other into (`# ` heading line, body lines)."""
    sections: list[tuple[str, list[str]]] = []
    for line in text.splitlines():
        if line.startswith("# "):
            sections.append((line, []))
        elif sections:
            sections[-1][1].append(line)
    return sections


def parse_queue(queue_text: str) -> list[str]:
    """The references of the queue file, in the owner's order; a comment queues nothing."""
    return QUEUE_LINE.findall(re.sub(r"<!--.*?-->", "", queue_text, flags=re.DOTALL))


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
        if line.startswith("**") or line.startswith("# "):
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


# The pull requests a size stands for (`docs/workflow.md` § 2); XL is sliced, never counted.
SIZE_PULL_REQUESTS = {"S": (1, 1), "M": (1, 1), "L": (2, 3)}


def size_of(line: str | None) -> str | None:
    """The size an entry's `Size` line opens with — S, M, L or XL — or None."""
    match = re.match(r"(XL|S|M|L)\b", (line or "").strip())
    return match.group(1) if match else None


def queue_size(queued: list[dict]) -> dict:
    """What the queued entries still to work add up to, in pull requests, with the
    entries the sum leaves out: those without a size, and those to slice (XL)."""
    left = [entry for entry in queued if entry["state"] not in ("closed", "unknown")]
    counted = [SIZE_PULL_REQUESTS[entry["size"]] for entry in left if entry.get("size") in SIZE_PULL_REQUESTS]
    return {
        "pull_requests": {
            "at_least": sum(low for low, _ in counted),
            "at_most": sum(high for _, high in counted),
        },
        "unsized": [entry["ref"] for entry in left if entry.get("size") is None],
        "to_slice": [entry["ref"] for entry in left if entry.get("size") == "XL"],
    }


def parse_entries(todo_text: str) -> list[dict]:
    """Every `# TODO-NNN — …` entry of the todo files, with what it waits on."""
    entries: list[dict] = []
    for heading, body in _sections(todo_text):
        match = ENTRY_HEADING.match(heading)
        if not match:
            continue
        entries.append(
            {
                "ref": f"TODO-{match.group('number')}",
                "title": match.group("title"),
                "size": size_of(_field(body, "Size")),
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
    """Every `# date — DEBT-NNN — …` entry of the tech-debt files. An entry waits on its
    Done when and, like a todo entry, on each unticked item under `**Open questions:**`."""
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
                "size": size_of(_debt_field(body, "Size")),
                "user_value": _debt_field(body, "User value"),
                "waits_on": ([] if _debt_field(body, "Done when") else ["no Done when"])
                + [
                    f"question: {text}"
                    for ticked, text in _question_items(body)
                    if not ticked
                ],
            }
        )
    return entries


def parse_flow(flow_text: str) -> list[dict]:
    """Every `# FLOW-NNN — …` entry of the flow files that proposes a change. An entry
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
                "size": size_of(_debt_field(body, "Size")),
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

    A queued reference with no entry left is reported as `closed`: its work has
    shipped, and the queue is left as the owner wrote it. One that is not an id at all
    (`TODO-0599`) is `unknown`: a slip of the hand is never read as shipped.
    """
    by_ref = {entry["ref"]: entry for entry in entries}
    queued = [
        by_ref[ref] | {"state": "blocked" if by_ref[ref]["waits_on"] else "ready"}
        if ref in by_ref
        else {"ref": ref, "state": "closed" if ENTRY_ID.fullmatch(ref) else "unknown"}
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


def kind_text(kind: str) -> str:
    """The entries of `kind`, one file after the other; debt newest first."""
    folder = ROOT / ENTRY_DIRS[kind]
    files = sorted(folder.glob(f"{kind}-*.md"), reverse=kind == "DEBT") if folder.is_dir() else []
    return "\n".join(path.read_text(encoding="utf-8") for path in files)


def collect_work() -> dict:
    """The queue, the todo entries, the tech debt and the flow entries, classified."""
    todo = parse_entries(kind_text("TODO"))
    debt = parse_debt(kind_text("DEBT"))
    flow = parse_flow(kind_text("FLOW"))
    lists = classify(parse_queue(_read(ROOT / QUEUE_HOME) or ""), todo + debt + flow)
    queued = {entry["ref"] for entry in lists["queued"]}
    in_todo = {entry["ref"] for entry in todo}
    for name in ("ready", "blocked"):
        lists[name] = [entry for entry in lists[name] if entry["ref"] in in_todo]
    lists["techdebt_not_queued"] = group_debt(
        [entry for entry in debt if entry["ref"] not in queued]
    )
    lists["flow_not_queued"] = [entry for entry in flow if entry["ref"] not in queued]
    return lists


def remaining(queued: list[dict]) -> list[str]:
    """The queued references still to work, in order: those whose entry has not shipped."""
    return [entry["ref"] for entry in queued if entry["state"] != "closed"]


class NoSuchEntry(Exception):
    """A reference given to `close` that names no entry file."""


def entry_path(ref: str) -> str | None:
    """The file of the entry `ref`; None when `ref` is not an entry id."""
    match = ENTRY_ID.fullmatch(ref)
    return f"{ENTRY_DIRS[match.group(1)]}/{ref}.md" if match else None


def close(refs: list[str], exists) -> list[str]:
    """The files closing `refs` deletes (docs/workflow.md § 3, closure); the queue is left as written.

    Raises before anything is decided unless every reference names an entry.
    """
    gone: list[str] = []
    for ref in refs:
        path = entry_path(ref)
        if path is None:
            raise NoSuchEntry(f"{ref}: not an entry id (TODO-NNN, DEBT-NNN or FLOW-NNN)")
        if path in gone or not exists(path):
            raise NoSuchEntry(f"{ref}: no such entry ({path})")
        gone.append(path)
    return gone


def next_id(kind: str, names: list[str]) -> str:
    """The next free id of `kind`, given every file name the kind has had: ids are never reused."""
    given = [int(m.group(2)) for m in map(ENTRY_ID.search, names) if m and m.group(1) == kind]
    return f"{kind}-{max([ID_FLOORS[kind], *given]) + 1:03d}"


class HistoryUnread(Exception):
    """The git history that says which ids were given could not be read whole."""


def names_ever(kind: str) -> list[str]:
    """Every file the folder of `kind` holds or has held, from the working tree and the git
    history of every branch, so an id taken on a branch not merged yet is never given twice.

    Raises when the history is not all there: a shallow clone has forgotten the entries
    already deleted, and their ids would be given again.
    """
    folder = ROOT / ENTRY_DIRS[kind]
    names = [path.name for path in folder.glob("*.md")] if folder.is_dir() else []
    if _git("rev-parse", "--is-shallow-repository") != "false":
        raise HistoryUnread("the git history is shallow or cannot be read")
    log = subprocess.run(
        ["git", "log", "--all", "--diff-filter=A", "--name-only", "--format=", "--", ENTRY_DIRS[kind]],
        capture_output=True,
        text=True,
        check=False,
        cwd=ROOT,
    )
    if log.returncode != 0:
        raise HistoryUnread(log.stderr.strip() or "git log failed")
    return names + log.stdout.split()


MEASURED = re.compile(r"^## Measured — the (\d+(?:\.\d+)+) batch", re.MULTILINE)


def _version(text: str) -> tuple[int, ...] | None:
    """`v0.9.0` or `0.9.0` as numbers to compare, or None for anything else."""
    match = re.fullmatch(r"v?(\d+(?:\.\d+)+)", text.strip())
    return tuple(int(part) for part in match.group(1).split(".")) if match else None


def audits_owed(latest_release: str | None, audits_text: str | None) -> str | None:
    """The last published release when it is later than every batch `docs/work/audits.md`
    measured: `/prune` and `/flow-audit` have not run since it (`docs/workflow.md` § 3).
    None when they have, or when no release is known."""
    released = _version(latest_release or "")
    if released is None:
        return None
    measured = [_version(batch) for batch in MEASURED.findall(audits_text or "")]
    if any(batch is not None and batch >= released for batch in measured):
        return None
    return latest_release


def latest_published_release() -> str | None:
    """The tag of the last release that was published — a draft is not one — or None when
    gh cannot say."""
    releases = _gh_json(
        "release", "list", "--exclude-drafts", "--limit", "1", "--json", "tagName"
    )
    return releases[0].get("tagName") if releases else None


def collect_in_flight() -> dict:
    """Uncommitted changes, unmerged branches, recent commits, and the release whose audits
    are owed."""
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
        "audits_owed": audits_owed(
            latest_published_release(),
            _read(ROOT / "docs" / "work" / "audits.md"),
        ),
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


ENTRY_REF = re.compile(r"\bTODO-\d{3}\b|\bDEBT-\d{3}\b|\bFLOW-\d{3}\b")


def entries_naming(number: int, texts: list[str]) -> list[str]:
    """The references of the entries that name GitHub issue `number` — as `ghN`, by its
    URL, or as `issue N` — in the order the documents are given. An issue no entry names
    has nothing tracking it in the repository: it is to file or to close."""
    mention = re.compile(rf"\bgh{number}\b|/issues/{number}\b|\bissue {number}\b")
    refs = []
    for text in texts:
        for heading, body in _sections(text):
            ref = ENTRY_REF.search(heading)
            if ref and mention.search("\n".join(body)) and ref.group(0) not in refs:
                refs.append(ref.group(0))
    return refs


def collect_gh_issues() -> list[dict]:
    """Open GitHub issues, each with the entries that name it (`entries`)."""
    texts = [kind_text(kind) for kind in ENTRY_DIRS]
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


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    if argv[:1] == ["close"]:
        if len(argv) < 2:
            print("usage: work-queue.py close <TODO-NNN | DEBT-NNN | FLOW-NNN>...", file=sys.stderr)
            return 2
        try:
            gone = close(argv[1:], lambda path: (ROOT / path).is_file())
        except NoSuchEntry as error:
            print(f"❌ {error} — nothing was changed", file=sys.stderr)
            return 1
        for path in gone:
            (ROOT / path).unlink()
            print(f"{path} deleted")
        return 0
    if argv[:1] == ["next-id"]:
        if len(argv) != 2 or argv[1] not in ENTRY_DIRS:
            print("usage: work-queue.py next-id <TODO | DEBT | FLOW>", file=sys.stderr)
            return 2
        try:
            print(next_id(argv[1], names_ever(argv[1])))
        except HistoryUnread as error:
            print(f"❌ no id given: {error}", file=sys.stderr)
            return 1
        return 0
    if argv == ["remaining"]:
        print("\n".join(remaining(collect_work()["queued"])))
        return 0

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--pretty", action="store_true", help="indent JSON for human inspection"
    )
    args = parser.parse_args(argv)

    work = collect_work()
    out = {
        "version": 6,
        **work,
        "queue_size": queue_size(work["queued"]),
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
    import usage_log

    usage_log.start()
    sys.exit(main())
