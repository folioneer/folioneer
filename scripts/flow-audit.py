#!/usr/bin/env python3
"""Deterministic figures for the /flow-audit skill (FLOW-018).

Measures how work moved between two releases — the pull requests merged, how long each
took from opening to merging, the CI rounds and what failed, how long each workflow
runs — and prints them as one JSON document, beside the same figures for the release
before when asked.

Usage:
    python3 scripts/flow-audit.py v0.5.0                 # from that release to now
    python3 scripts/flow-audit.py v0.5.0 v0.6.0          # between two releases
    python3 scripts/flow-audit.py v0.5.0 v0.6.0 --previous v0.4.0
    python3 scripts/flow-audit.py v0.5.0 --pretty

A release is named by its tag; its moment is when it was published on GitHub. The skill
turns the figures into the "Measured" section of `docs/flow.md`. This script has no
judgment — only counting.
"""

from __future__ import annotations

import argparse
import json
import shutil
import statistics
import subprocess
import sys
from datetime import datetime, timezone

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


def measure_ci(
    runs: list[dict], pull_requests: list[dict], since: datetime, until: datetime
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
    return {
        "rounds": len(rounds),
        "branches": len(branches_run),
        "rounds_beyond_the_first": len(rounds) - len(branches_run),
        "failures_by_workflow": dict(sorted(failures.items())),
        "median_minutes_by_workflow": {
            name: _rounded(statistics.median(values))
            for name, values in sorted(durations.items())
        },
    }


def measure(
    pull_requests: list[dict], runs: list[dict], since: datetime, until: datetime
) -> dict:
    """Every figure of one window."""
    return {
        "since": since.isoformat(),
        "until": until.isoformat(),
        "pull_requests": measure_pull_requests(pull_requests, since, until),
        "ci": measure_ci(runs, pull_requests, since, until),
    }


def _gh_json(*args: str) -> object:
    if not shutil.which("gh"):
        raise SystemExit("flow-audit: the GitHub command line (gh) is not installed")
    try:
        result = subprocess.run(
            ["gh", *args], check=True, capture_output=True, text=True, timeout=120
        )
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        raise SystemExit(
            f"flow-audit: gh {' '.join(args[:2])} failed: {error}"
        ) from error
    return json.loads(result.stdout or "null")


def release_moment(tag: str) -> datetime:
    """When the release `tag` was published."""
    release = _gh_json("release", "view", tag, "--json", "publishedAt")
    published = release.get("publishedAt") if isinstance(release, dict) else None
    if not published:
        raise SystemExit(f"flow-audit: release {tag} has no publication date")
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
    parser.add_argument("since", help="the release the window starts after, by its tag")
    parser.add_argument(
        "until", nargs="?", help="the release the window ends at (default: now)"
    )
    parser.add_argument(
        "--previous", help="the release before `since`, to print its window too"
    )
    parser.add_argument("--pretty", action="store_true", help="indented JSON")
    args = parser.parse_args(argv)

    since = release_moment(args.since)
    until = release_moment(args.until) if args.until else datetime.now(timezone.utc)
    pull_requests, runs = collect()
    report = {"current": measure(pull_requests, runs, since, until)}
    earliest = since
    if args.previous:
        earliest = release_moment(args.previous)
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
    sys.exit(main())
