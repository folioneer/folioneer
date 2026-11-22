#!/usr/bin/env python3
"""The usage log: one line per run of a script or a recipe (daneel flow; `/flow:flow-audit` reads it).

`logs/usage.log`, local and never committed: time, tool, duration, result, separated
by tabs. The flow audit reads it after a batch ends, then empties it. Nothing is written
in CI, nor when `USAGE_LOG=off` (the script tests); `USAGE_LOG=<path>` moves the file.

A tool reaches the log in one of three ways:
  - a Python script calls `usage_log.start()` first thing in its `__main__` block;
  - a shell script sources `scripts/usage-log.sh`;
  - a recipe that runs no script depends on `(_used "<recipe>")` in the justfile.

Writing the log never fails the tool: an error here is swallowed.

A Python script leaves through `sys.exit`: a raised `SystemExit` passes neither of the two
ways out noted here and would be logged as `ok`. Two runs that cap the file at the same
moment can lose a line. A script that only CI runs, or that is copied elsewhere before it
runs, does not log.
"""

from __future__ import annotations

import atexit
import os
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MAX_LINES = 5000


def log_path() -> Path | None:
    """Where the log lives, or None when this run is not logged."""
    chosen = os.environ.get("USAGE_LOG", "")
    if os.environ.get("CI") or chosen == "off":
        return None
    return Path(chosen) if chosen else ROOT / "logs" / "usage.log"


def result(code: object) -> str:
    """An exit code as the log says it: `ok` or `exit N`."""
    if code in (0, None):
        return "ok"
    return f"exit {code}" if isinstance(code, int) else "exit 1"


def record(tool: str, seconds: float | None, outcome: str, now: datetime | None = None) -> None:
    path = log_path()
    if path is None:
        return
    stamp = (now or datetime.now(timezone.utc)).strftime("%Y-%m-%dT%H:%M:%SZ")
    took = "-" if seconds is None else f"{seconds:.1f}s"
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("a", encoding="utf-8") as log:
            log.write(f"{stamp}\t{tool}\t{took}\t{outcome}\n")
        lines = path.read_text(encoding="utf-8").splitlines(keepends=True)
        if len(lines) > MAX_LINES:
            path.write_text("".join(lines[-MAX_LINES:]), encoding="utf-8")
    except (OSError, ValueError):  # a log that cannot be written, or read back to cap it
        pass


def start(tool: str | None = None) -> None:
    """Log this process when it ends, with its duration and exit code."""
    name = tool or Path(sys.argv[0]).name
    started = time.monotonic()
    code: list[object] = [0]
    leave, report = sys.exit, sys.excepthook

    def exit_noting_the_code(status: object = 0) -> None:
        code[0] = status
        leave(status)

    def report_noting_the_failure(*raised) -> None:
        code[0] = 1
        report(*raised)

    # atexit sees no exit code: it is noted on the two ways out a script has.
    sys.exit, sys.excepthook = exit_noting_the_code, report_noting_the_failure
    atexit.register(lambda: record(name, time.monotonic() - started, result(code[0])))


def main(argv: list[str]) -> int:
    if len(argv) == 2 and argv[0] == "used":
        record(argv[1], None, "started")
        return 0
    if len(argv) == 4 and argv[0] == "record":
        # From scripts/usage-log.sh: the tool, when it started (epoch seconds), its exit code.
        try:
            record(argv[1], max(0.0, time.time() - float(argv[2])), result(int(argv[3])))
        except ValueError:
            return 2
        return 0
    print("usage: usage_log.py used <recipe> | record <tool> <start-epoch> <exit-code>", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
