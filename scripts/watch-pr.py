#!/usr/bin/env python3
"""Wait for the checks of a pull request and say how they ended.

Use:  just watch-pr            the pull request of the current branch
      just watch-pr 178        that pull request
Exit: 0 every check green · 1 a check failed · 2 GitHub unreachable, or nothing to ask it · 3 timed out
      4 the pull request is not open

It reads the check runs of the head commit, the newest run per name, as
`scripts/merge.py` does, and it is done only once every check named in
`required-checks.json` has reported. A push moves the watch to the new head. A
failed poll is retried: five in a row end the watch. When a check has failed and
the required ones still missing do not appear over four polls, the watch ends as
failed: a job that starts others (the reviewer lanes) failed before starting them.

Every line printed is something to act on (a failure when it lands, a stuck check,
the verdict), so the script can run under a monitor without noise.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from collections.abc import Callable, Iterable
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PASSING = ("success", "skipped", "neutral")


class GitHubError(Exception):
    """GitHub could not be asked, or did not answer: worth another try."""


@dataclass(frozen=True)
class Run:
    id: int
    name: str
    status: str
    conclusion: str
    url: str
    completed_at: str

    @property
    def done(self) -> bool:
        return self.status == "completed"

    @property
    def stuck(self) -> bool:
        """GitHub recorded an end but still reports the run in progress."""
        return not self.done and bool(self.completed_at)


def parse_runs(lines: str) -> list[Run]:
    runs = []
    for line in lines.splitlines():
        if line.strip():
            raw = json.loads(line)
            runs.append(
                Run(int(raw["id"]), raw["name"], raw["status"], raw.get("conclusion") or "", raw.get("html_url") or "", raw.get("completed_at") or "")
            )
    return runs


def latest(runs: Iterable[Run]) -> list[Run]:
    """One run per name: a re-run has a higher id and replaces the earlier one."""
    newest: dict[str, Run] = {}
    for run in runs:
        if run.name not in newest or run.id > newest[run.name].id:
            newest[run.name] = run
    return list(newest.values())


def verdict(runs: list[Run], required: list[str]) -> tuple[str, list[str]]:
    """("running", what is awaited), ("failing", the failed checks) or ("green", [])."""
    names = {run.name for run in runs}
    waiting = [name for name in required if name not in names] + [run.name for run in runs if not run.done]
    if waiting:
        return "running", waiting
    failing = sorted(run.name for run in runs if run.conclusion not in PASSING)
    return ("failing", failing) if failing else ("green", [])


def _failure(run: Run) -> str:
    return f"{run.name}: {run.conclusion} — {run.url}"


def watch(
    read_head: Callable[[], str],
    read_runs: Callable[[str], list[Run]],
    required: list[str],
    out: Callable[[str], None],
    sleep: Callable[[float], None],
    clock: Callable[[], float],
    interval: float = 30,
    timeout: float = 1800,
    max_errors: int = 5,
    patience: int = 4,
) -> int:
    deadline = clock() + timeout
    head, errors, stalled, waiting = "", 0, 0, ["the first answer"]
    told: set[tuple[str, str]] = set()
    while True:
        try:
            sha = read_head()
            runs = latest(read_runs(sha))
        except GitHubError as error:
            errors += 1
            out(f"poll error ({errors}/{max_errors}): {error}")
            if errors >= max_errors:
                out(f"GITHUB UNREACHABLE after {max_errors} tries")
                return 2
        else:
            errors = 0
            if sha != head:
                if head:
                    out(f"head moved to {sha[:7]}")
                head, told, stalled = sha, set(), 0
            for run in runs:
                kind = "stuck" if run.stuck else "failed" if run.done and run.conclusion not in PASSING else ""
                if kind and (run.name, kind) not in told:
                    told.add((run.name, kind))
                    out(_failure(run) if kind == "failed" else f"{run.name}: stuck in_progress with a completion time — {run.url}")
            state, names = verdict(runs, required)
            if state == "green":
                out(f"ALL CHECKS GREEN on {sha[:7]}")
                return 0
            failed = [run for run in sorted(runs, key=lambda r: r.name) if run.done and run.conclusion not in PASSING]
            # Everything that reported is done and something failed: what is still missing may never come.
            stalled = stalled + 1 if failed and all(run.done for run in runs) else 0
            if state == "failing" or stalled >= patience:
                never = f" (never reported: {', '.join(names)})" if state == "running" else ""
                out(f"FINISHED WITH FAILURES on {sha[:7]}{never}:")
                for run in failed:
                    out(f"  {_failure(run)}")
                return 1
            waiting = names
        sleep(interval)
        if clock() >= deadline:
            out(f"TIMED OUT after {timeout:g} s on {head[:7] or 'no head'}, still waiting on: {', '.join(waiting)}")
            return 3


class NotOpen(Exception):
    """The pull request is merged or closed: nothing left to watch."""


class CannotWatch(Exception):
    """Trying again would change nothing: no `gh`, no pull request for the branch."""


def gh(*args: str) -> str:
    try:
        result = subprocess.run(["gh", *args], capture_output=True, text=True, timeout=60, cwd=ROOT)
    except FileNotFoundError as error:
        raise CannotWatch("gh is not installed") from error
    except (OSError, subprocess.TimeoutExpired) as error:
        raise GitHubError(str(error)) from error
    if result.returncode != 0:
        raise GitHubError((result.stderr or result.stdout).strip().splitlines()[-1] if (result.stderr or result.stdout).strip() else "gh failed")
    return result.stdout


def head_of(pull: str | None) -> str:
    """The head commit of the pull request (of the current branch when none is named)."""
    try:
        answer = gh("pr", "view", *([pull] if pull else []), "--json", "headRefOid,state")
    except GitHubError as error:
        if "no pull requests found" in str(error) or "Could not resolve to a PullRequest" in str(error):
            raise CannotWatch(str(error)) from error
        raise
    try:
        data = json.loads(answer)
        state, head = data["state"], data["headRefOid"]
    except (json.JSONDecodeError, KeyError, TypeError) as error:
        raise GitHubError("gh pr view did not answer a pull request") from error
    if state != "OPEN":
        raise NotOpen(state)
    return head


def runs_of(sha: str) -> list[Run]:
    answer = gh(
        "api", f"repos/{{owner}}/{{repo}}/commits/{sha}/check-runs?filter=latest", "--paginate",
        "--jq", ".check_runs[] | {id, name, status, conclusion, html_url, completed_at}",
    )
    try:
        return parse_runs(answer)
    except (json.JSONDecodeError, KeyError) as error:
        raise GitHubError("the check runs could not be read") from error


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("pull", nargs="?", help="pull request number (default: the current branch's)")
    parser.add_argument("--interval", type=float, default=30, help="seconds between two polls (default 30)")
    parser.add_argument("--timeout", type=float, default=1800, help="seconds before giving up (default 1800)")
    args = parser.parse_args(argv)

    def say(line: str) -> None:
        print(line, flush=True)

    try:
        required = list(json.loads((ROOT / "required-checks.json").read_text(encoding="utf-8"))["checks"])
    except (OSError, json.JSONDecodeError, KeyError, TypeError):
        say("CANNOT WATCH: required-checks.json cannot be read")
        return 2

    try:
        return watch(lambda: head_of(args.pull), runs_of, required, say, time.sleep, time.monotonic, args.interval, args.timeout)
    except NotOpen as state:
        say(f"PULL REQUEST {state}: nothing to watch")
        return 4
    except CannotWatch as reason:
        say(f"CANNOT WATCH: {reason}")
        return 2


if __name__ == "__main__":
    sys.exit(main())
