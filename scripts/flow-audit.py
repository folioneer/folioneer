#!/usr/bin/env python3
"""Deterministic figures for the /flow-audit skill (FLOW-018).

Measures how work moved between two batch ends (releases or `batch/<date>` tags) — the pull requests merged, how long each
took from opening to merging, the CI rounds and what failed, how long each workflow
runs — and prints them as one JSON document, beside the same figures for the batch
before when asked.

Usage:
    python3 scripts/flow-audit.py v0.5.0                 # from that batch end to now
    python3 scripts/flow-audit.py v0.5.0 v0.6.0          # between two batch ends
    python3 scripts/flow-audit.py v0.5.0 v0.6.0 --previous v0.4.0
    python3 scripts/flow-audit.py v0.5.0 --pretty

A release is named by its tag; its moment is when it was published on GitHub. A batch
that ended without a release is named by its `batch/<date>` tag, dated by the tag. The skill
posts the figures on the batch's `batch-audit` issue. This script has no judgment — only
counting.
"""

from __future__ import annotations

import argparse
import functools
import json
import re
import shutil
import statistics
import subprocess
import sys
from collections.abc import Callable
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

PULL_REQUEST_FIELDS = (
    "number,title,state,createdAt,mergedAt,closedAt,additions,headRefName"
)
RUN_FIELDS = (
    "workflowName,headBranch,headSha,conclusion,event,createdAt,startedAt,updatedAt"
)
# The workflow whose runs count the CI rounds of a branch: it runs on every push.
ROUND_WORKFLOW = "Quality"
# The conventional commit types of `docs/commit-rules.md`.
COMMIT_TYPES = {
    "feat",
    "fix",
    "refactor",
    "chore",
    "docs",
    "test",
    "ci",
    "perf",
    "build",
}
# The two halves of the usage log itself: they log the others and never themselves.
LOGGERS = ("usage_log.py", "usage-log.sh")
PULL_REQUEST_LIMIT = 500
RUN_LIMIT = 1000


def parse_moment(text: str) -> datetime:
    """A GitHub timestamp (`2026-10-04T05:31:00Z`) as an aware datetime."""
    return datetime.fromisoformat(text.replace("Z", "+00:00"))


def _within(text: str | None, since: datetime, until: datetime) -> bool:
    return text is not None and since < parse_moment(text) <= until


def _minutes(start: str, end: str) -> float:
    return (parse_moment(end) - parse_moment(start)).total_seconds() / 60


def _rounded(value: float) -> float:
    return round(value, 1)


def commit_type(title: str) -> str:
    """The conventional type a pull request's title starts with, or `other`."""
    head = title.split(":", 1)[0].strip()
    return head if ":" in title and head in COMMIT_TYPES else "other"


def measure_pull_requests(
    pull_requests: list[dict], since: datetime, until: datetime
) -> dict:
    """The pull requests merged in the window, and those closed in it without merging."""
    merged = sorted(
        (pr for pr in pull_requests if _within(pr.get("mergedAt"), since, until)),
        key=lambda pr: pr["number"],
    )
    closed = [
        pr
        for pr in pull_requests
        if pr.get("mergedAt") is None and _within(pr.get("closedAt"), since, until)
    ]
    minutes = [_minutes(pr["createdAt"], pr["mergedAt"]) for pr in merged]
    by_type: dict[str, int] = {}
    for pr in merged:
        kind = commit_type(pr["title"])
        by_type[kind] = by_type.get(kind, 0) + 1
    return {
        "merged": len(merged),
        "first": merged[0]["number"] if merged else None,
        "last": merged[-1]["number"] if merged else None,
        "closed_without_merging": sorted(pr["number"] for pr in closed),
        "lines_added": sum(pr.get("additions", 0) for pr in merged),
        "mean_minutes_to_merge": _rounded(statistics.fmean(minutes))
        if minutes
        else None,
        "median_minutes_to_merge": _rounded(statistics.median(minutes))
        if minutes
        else None,
        "by_type": dict(sorted(by_type.items())),
    }


def sort_later_rounds(runs: list[dict], relation: Callable[[str, str], str | None]) -> dict:
    """FLOW-020 — why each round after a branch's first came: after a failed check of the
    round before (a failed run of any workflow on its commit); a rebase, when the round's
    commit does not descend from the round before (the branch was rewritten onto another
    base, the push of `just merge` among them); a fix, a commit pushed on top; or unreadable,
    when `relation(before, sha)` — GitHub's comparison of the two commits, `ahead`,
    `diverged`, … — cannot be read, so that a comparison GitHub refused never passes for a fix.
    """
    first_seen: dict[tuple[str, str], str] = {}
    failed: set[str] = set()
    for run in runs:
        if run.get("conclusion") == "failure":
            failed.add(run["headSha"])
        if run.get("workflowName") == ROUND_WORKFLOW:
            key = (run["headBranch"], run["headSha"])
            first_seen[key] = min(first_seen.get(key, run["createdAt"]), run["createdAt"])
    by_branch: dict[str, list[tuple[str, str]]] = {}
    for (branch, sha), seen in first_seen.items():
        by_branch.setdefault(branch, []).append((seen, sha))
    causes = {"after_a_failed_check": 0, "rebase": 0, "fix": 0, "unreadable": 0}
    for rounds in by_branch.values():
        shas = [sha for _, sha in sorted(rounds)]
        for before, sha in zip(shas, shas[1:]):
            if before in failed:
                causes["after_a_failed_check"] += 1
            else:
                compared = relation(before, sha)
                if compared is None:
                    causes["unreadable"] += 1
                elif compared == "diverged":
                    causes["rebase"] += 1
                else:
                    causes["fix"] += 1
    return causes


def measure_ci(
    runs: list[dict],
    pull_requests: list[dict],
    since: datetime,
    until: datetime,
    relation: Callable[[str, str], str | None] | None = None,
) -> dict:
    """The CI rounds of the window's pull request branches, what failed, how long runs take.

    A round is one pushed commit of a branch that the round workflow ran on; the rounds
    beyond the first of each branch are the ones a rebase or a fix cost. Runs are matched
    to a pull request by its branch, within the window: a pull request opened before the
    window loses the rounds it ran before it. A failure is counted per run, so a commit
    run again after a failure counts once as a round and once per failed run.
    """
    branches = {
        pr["headRefName"]
        for pr in pull_requests
        if _within(pr.get("mergedAt"), since, until)
        or (pr.get("mergedAt") is None and _within(pr.get("closedAt"), since, until))
    }
    in_window = [
        run
        for run in runs
        if run.get("event") == "pull_request"
        and run.get("headBranch") in branches
        and _within(run.get("createdAt"), since, until)
    ]
    rounds = {
        (run["headBranch"], run["headSha"])
        for run in in_window
        if run.get("workflowName") == ROUND_WORKFLOW
    }
    branches_run = {branch for branch, _ in rounds}
    failures: dict[str, int] = {}
    durations: dict[str, list[float]] = {}
    for run in in_window:
        name = run.get("workflowName") or "unknown"
        if run.get("conclusion") == "failure":
            failures[name] = failures.get(name, 0) + 1
        if (
            run.get("conclusion") == "success"
            and run.get("startedAt")
            and run.get("updatedAt")
        ):
            durations.setdefault(name, []).append(
                _minutes(run["startedAt"], run["updatedAt"])
            )
    figures = {
        "rounds": len(rounds),
        "branches": len(branches_run),
        "rounds_beyond_the_first": len(rounds) - len(branches_run),
        "failures_by_workflow": dict(sorted(failures.items())),
        "median_minutes_by_workflow": {
            name: _rounded(statistics.median(values))
            for name, values in sorted(durations.items())
        },
    }
    if relation is not None:
        figures["later_rounds"] = sort_later_rounds(in_window, relation)
    return figures


def measure(
    pull_requests: list[dict],
    runs: list[dict],
    since: datetime,
    until: datetime,
    relation: Callable[[str, str], str | None] | None = None,
) -> dict:
    """Every figure of one window."""
    return {
        "since": since.isoformat(),
        "until": until.isoformat(),
        "pull_requests": measure_pull_requests(pull_requests, since, until),
        "ci": measure_ci(runs, pull_requests, since, until, relation),
    }


def measure_usage(
    log_lines: list[str], since: datetime, until: datetime, tools: list[str]
) -> dict:
    """What the usage log saw in the window: runs and failures by tool, and the tools it never saw.

    The log is local and written outside CI only: a tool that only CI runs is never
    seen, and a log that starts after `since` covers part of the window. A recipe
    without a script is logged when it starts: its failures are not known.
    """
    runs: dict[str, int] = {}
    failed: dict[str, int] = {}
    stamps = []
    for line in log_lines:
        parts = line.rstrip("\n").split("\t")
        if len(parts) != 4 or not _within(parts[0], since, until):
            continue
        stamp, tool, _, outcome = parts
        stamps.append(stamp)
        runs[tool] = runs.get(tool, 0) + 1
        if outcome not in ("ok", "started"):
            failed[tool] = failed.get(tool, 0) + 1
    return {
        "first_line": min(stamps) if stamps else None,
        "runs": dict(sorted(runs.items())),
        "failed": dict(sorted(failed.items())),
        "never_run": sorted(set(tools) - set(runs)),
    }


def known_tools() -> list[str]:
    """Every tool the usage log can name: the scripts that log themselves — one that only
    CI runs does not — and the recipes that run none."""
    scripts = [
        path.name
        for path in (ROOT / "scripts").iterdir()
        if path.suffix in (".py", ".sh")
        and path.name not in LOGGERS
        and re.search(r"usage_log\.start\(\)|usage-log\.sh", path.read_text(encoding="utf-8"))
    ]
    recipes = re.findall(
        r'_used "([^"]+)"', (ROOT / "justfile").read_text(encoding="utf-8")
    )
    return sorted({*scripts, *recipes})


def read_usage_log() -> list[str]:
    log = ROOT / "logs" / "usage.log"
    return log.read_text(encoding="utf-8").splitlines() if log.exists() else []


def _gh_json(*args: str) -> object:
    if not shutil.which("gh"):
        sys.exit("flow-audit: the GitHub command line (gh) is not installed")
    try:
        result = subprocess.run(
            ["gh", *args], check=True, capture_output=True, text=True, timeout=120
        )
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        sys.exit(f"flow-audit: gh {' '.join(args[:2])} failed: {error}")
    return json.loads(result.stdout or "null")


@functools.lru_cache(maxsize=None)
def compared(before: str, sha: str) -> str | None:
    """FLOW-020 — how GitHub relates commit `sha` to `before` (`ahead`, `diverged`, …),
    read once per pair; None when it cannot say (a commit no longer on GitHub)."""
    try:
        result = subprocess.run(
            [
                "gh",
                "api",
                f"repos/{{owner}}/{{repo}}/compare/{before}...{sha}",
                "--jq",
                ".status",
            ],
            check=True,
            capture_output=True,
            text=True,
            timeout=120,
        )
        return result.stdout.strip() or None
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired):
        return None


def _git(*args: str) -> str:
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True, cwd=ROOT).stdout


BATCH_TAG_PREFIX = "batch/"


def batch_end_moment(tag: str) -> datetime:
    """When the batch end `tag` happened: a release's publication on GitHub, or the
    tagger date of a `batch/<date>` tag for a batch that ended without one."""
    if tag.startswith(BATCH_TAG_PREFIX):
        made = _git("for-each-ref", f"refs/tags/{tag}", "--format=%(taggerdate:iso-strict)").strip()
        if not made:
            sys.exit(f"flow-audit: batch tag {tag} has no tagger date")
        return parse_moment(made)
    release = _gh_json("release", "view", tag, "--json", "publishedAt")
    published = release.get("publishedAt") if isinstance(release, dict) else None
    if not published:
        sys.exit(f"flow-audit: release {tag} has no publication date")
    return parse_moment(published)


def collect() -> tuple[list[dict], list[dict]]:
    """The repository's pull requests and workflow runs, as GitHub lists them."""
    pull_requests = _gh_json(
        "pr",
        "list",
        "--state",
        "all",
        "--limit",
        str(PULL_REQUEST_LIMIT),
        "--json",
        PULL_REQUEST_FIELDS,
    )
    runs = _gh_json("run", "list", "--limit", str(RUN_LIMIT), "--json", RUN_FIELDS)
    return list(pull_requests or []), list(runs or [])


def truncated(
    pull_requests: list[dict], runs: list[dict], earliest: datetime
) -> list[str]:
    """What GitHub's lists cut short: a list that came back full and whose oldest item
    is later than the earliest moment asked for does not cover the window."""
    cut = []
    for name, items, limit in (
        ("pull requests", pull_requests, PULL_REQUEST_LIMIT),
        ("workflow runs", runs, RUN_LIMIT),
    ):
        if (
            len(items) >= limit
            and min(parse_moment(i["createdAt"]) for i in items) > earliest
        ):
            cut.append(name)
    return cut


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("since", help="the release (or `batch/<date>` tag) the window starts after")
    parser.add_argument(
        "until", nargs="?", help="the release (or `batch/<date>` tag) the window ends at (default: now)"
    )
    parser.add_argument(
        "--previous", help="the batch end before `since`, to print its window too"
    )
    parser.add_argument("--pretty", action="store_true", help="indented JSON")
    args = parser.parse_args(argv)

    since = batch_end_moment(args.since)
    until = batch_end_moment(args.until) if args.until else datetime.now(timezone.utc)
    pull_requests, runs = collect()
    report = {"current": measure(pull_requests, runs, since, until, compared)}
    report["current"]["usage"] = measure_usage(
        read_usage_log(), since, until, known_tools()
    )
    earliest = since
    if args.previous:
        earliest = batch_end_moment(args.previous)
        report["previous"] = measure(pull_requests, runs, earliest, since)
    report["truncated"] = truncated(pull_requests, runs, earliest)
    if report["truncated"]:
        print(
            f"flow-audit: the list of {' and '.join(report['truncated'])} does not reach"
            " back to the earliest release asked for; the oldest figures are incomplete",
            file=sys.stderr,
        )
    json.dump(report, sys.stdout, indent=2 if args.pretty else None)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    import usage_log

    usage_log.start()
    sys.exit(main())
