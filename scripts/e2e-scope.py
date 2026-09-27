#!/usr/bin/env python3
"""Decides whether a pull request must run the E2E suite (todo #041).

Reads the pull request's file paths on stdin, one per line, and prints `true` when the
suite must run, `false` when the change holds records only. The E2E workflow calls it
first, so a records-only pull request reports its required check green in a minute
instead of building the application.
"""

import sys


# What a records-only change may touch: nothing the suite builds or executes. Anything
# else — application code, the E2E specs and helpers, the workflow, the test tooling,
# dependencies — runs the suite.
RECORD_FOLDERS = ("docs/", ".claude/", "screenshots/")
RECORD_SUFFIXES = (".md",)


def is_record(path):
    return path.startswith(RECORD_FOLDERS) or path.endswith(RECORD_SUFFIXES)


def suite_must_run(files):
    """True unless every file is a record. An empty list proves nothing, so it runs."""
    return not files or not all(is_record(path) for path in files)


if __name__ == "__main__":
    paths = [line.strip() for line in sys.stdin if line.strip()]
    print("true" if suite_must_run(paths) else "false")
