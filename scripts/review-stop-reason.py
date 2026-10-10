#!/usr/bin/env python3
"""Say in one line why a CI reviewer session stopped, from its execution log.

Use: python3 scripts/review-stop-reason.py <session-log.json>

`review.yml` calls it when a reviewer ended without a report, so the job log and the
pull request comment say why instead of "no report" (FLOW-026). Given a path, it always
prints one line and exits 0: a diagnosis must not fail the job it explains.

The session read a diff nobody has reviewed, and its log is not redacted yet. So
only named fields are printed, and only when they have the expected shape: an
event type or a stop subtype made of lowercase letters and underscores, a number
of turns, a yes or a no. Never a message.

It runs in CI only, beside a token, so it imports nothing from `scripts/`, and the
workflow takes it from the base branch: a pull request cannot change what runs there.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

NAME = re.compile(r"[a-z_]{1,40}")


def _name(value: object) -> str:
    return value if isinstance(value, str) and NAME.fullmatch(value) else "unknown"


def stop_reason(log: object) -> str:
    events = log if isinstance(log, list) else [log]
    events = [event for event in events if isinstance(event, dict)]
    if not events:
        return "the session log is empty"
    results = [event for event in events if event.get("type") == "result"]
    if not results:
        return f"the session log holds no result: the session did not end by itself (last event: `{_name(events[-1].get('type'))}`)"
    last = results[-1]
    turns = last.get("num_turns")
    turns = turns if isinstance(turns, int) and not isinstance(turns, bool) else "?"
    failed = "yes" if last.get("is_error") is True else "no"
    return f"the session ended with `{_name(last.get('subtype'))}` after {turns} turns (error: {failed})"


def read(path: Path) -> str:
    try:
        text = path.read_text(encoding="utf-8")
    except OSError:
        return f"no session log at {path}"
    try:
        return stop_reason(json.loads(text))
    except RecursionError:
        return "the session log is nested too deep to read"
    except json.JSONDecodeError:
        pass
    try:  # one JSON event per line
        return stop_reason([json.loads(line) for line in text.splitlines() if line.strip()])
    except (json.JSONDecodeError, RecursionError):
        return "the session log is not JSON"


def main(argv: list[str]) -> int:
    if len(argv) != 1:
        print("usage: review-stop-reason.py <session-log.json>", file=sys.stderr)
        return 2
    print(read(Path(argv[0])))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
