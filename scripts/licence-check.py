#!/usr/bin/env python3
"""licence-check.py — what the application ships stays under licences that allow it to be sold.

The application is under the AGPL-3.0-or-later and its owner keeps the right to offer it
under other terms (README § License). That right survives only as long as nothing the
application ships forbids it: one GPL, LGPL, SSPL or unlicensed dependency would close the
door quietly. This check refuses any shipped dependency whose licence is outside
`licence-allowlist.json`.

What ships:
  - Rust: every crate the application reaches through normal or build dependencies, as
    `cargo metadata` resolves them; crates reached only through dev-dependencies never
    ship and are left out.
  - npm: every package of `package-lock.json` not flagged `dev`; the lockfile records each
    package's licence, so nothing has to be installed.

A licence expression passes when the allowed licences satisfy it: one allowed choice of an
`OR`, every member of an `AND`; `X WITH exception` is a licence of its own. A package may
also pass by name, under the exact licence it was accepted with (`exceptions`), so that a
weak-copyleft crate is a recorded decision and a second one is a new decision. An exception
nothing uses any more fails too: the list states what is true today.

Use: python3 scripts/licence-check.py        (or: just licence-check)
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ALLOWLIST = ROOT / "licence-allowlist.json"
CARGO_MANIFEST = ROOT / "src-tauri" / "Cargo.toml"
PACKAGE_LOCK = ROOT / "package-lock.json"

TOKEN = re.compile(r"\(|\)|[^\s()]+")


def is_allowed(expression: str | None, allowed: set[str]) -> bool:
    """True when the allowed licences satisfy an SPDX licence expression."""
    if not expression:
        return False
    tokens = TOKEN.findall(expression.replace("/", " OR "))
    position = 0

    def peek() -> str | None:
        return tokens[position] if position < len(tokens) else None

    def take() -> str:
        nonlocal position
        position += 1
        return tokens[position - 1]

    def licence() -> bool:
        if peek() == "(":
            take()
            value = either()
            if peek() != ")":
                raise ValueError("unbalanced parenthesis")
            take()
            return value
        name = take()
        if name in ("AND", "OR", "WITH", ")"):
            raise ValueError(f"unexpected {name}")
        if peek() == "WITH":
            take()
            name = f"{name} WITH {take()}"
        return name in allowed

    def both() -> bool:
        value = licence()
        while peek() == "AND":
            take()
            value = licence() and value
        return value

    def either() -> bool:
        value = both()
        while peek() == "OR":
            take()
            value = both() or value
        return value

    try:
        value = either()
    except (ValueError, IndexError):
        return False
    return value and position == len(tokens)


def cargo_shipped(metadata: dict) -> list[tuple[str, str | None]]:
    """(name, licence) of every crate the application reaches without a dev-dependency."""
    packages = {package["id"]: package for package in metadata["packages"]}
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    own = set(metadata["workspace_members"])
    reached: set[str] = set()
    pending = [metadata["resolve"]["root"]]
    while pending:
        current = pending.pop()
        for dependency in nodes[current]["deps"]:
            ships = any(kind.get("kind") != "dev" for kind in dependency["dep_kinds"])
            if ships and dependency["pkg"] not in reached:
                reached.add(dependency["pkg"])
                pending.append(dependency["pkg"])
    return sorted(
        {(packages[pid]["name"], packages[pid].get("license")) for pid in reached - own},
        key=lambda entry: (entry[0], entry[1] or ""),
    )


def npm_shipped(lock: dict) -> list[tuple[str, str | None]]:
    """(name, licence) of every package of the lockfile that is not development-only."""
    shipped = set()
    for path, entry in lock["packages"].items():
        # `devOptional` stays in scope: npm sets it on a package that is a dev dependency
        # and also an optional dependency of something that ships, so it can be installed.
        if not path or entry.get("dev") or entry.get("link"):
            continue
        name = entry.get("name") or path.rsplit("node_modules/", 1)[-1]
        shipped.add((name, entry.get("license")))
    return sorted(shipped, key=lambda entry: (entry[0], entry[1] or ""))


def check(
    ecosystem: str,
    shipped: list[tuple[str, str | None]],
    allowed: set[str],
    exceptions: dict[str, str],
) -> list[str]:
    """One line per shipped package outside the allow-list, and per exception no longer used."""
    problems = []
    used = set()
    for name, licence in shipped:
        accepted = exceptions.get(name)
        if accepted is not None:
            used.add(name)
        if accepted is not None and accepted == licence:
            continue
        if not is_allowed(licence, allowed):
            was = f" (accepted by name under {accepted})" if accepted is not None else ""
            problems.append(f"{ecosystem}: {name} — {licence or 'no licence stated'}{was}")
    for name in sorted(set(exceptions) - used):
        problems.append(
            f"{ecosystem}: exception `{name}` ({exceptions[name]}) matches nothing shipped — remove it"
        )
    return problems


def fail(message: str) -> int:
    print(f"❌ licence check: {message}", file=sys.stderr)
    return 1


def main() -> int:
    try:
        config = json.loads(ALLOWLIST.read_text(encoding="utf-8"))
        allowed = set(config["allowed"])
        cargo_exceptions = config["exceptions"]["cargo"]
        npm_exceptions = config["exceptions"]["npm"]
    except (OSError, ValueError, KeyError, TypeError) as error:
        return fail(f"{ALLOWLIST.name} is missing or malformed ({error!r})")
    try:
        metadata = subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--locked", "--manifest-path", str(CARGO_MANIFEST)],
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        return fail(f"cargo could not be run ({error!r})")
    if metadata.returncode != 0:
        return fail(f"cargo metadata failed\n{metadata.stderr.strip()}")
    try:
        lock = json.loads(PACKAGE_LOCK.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        return fail(f"{PACKAGE_LOCK.name} is missing or malformed ({error!r})")
    crates = cargo_shipped(json.loads(metadata.stdout))
    packages = npm_shipped(lock)
    problems = check("cargo", crates, allowed, cargo_exceptions) + check(
        "npm", packages, allowed, npm_exceptions
    )
    if problems:
        print("❌ licence check: what the application ships must stay sellable", file=sys.stderr)
        for problem in problems:
            print(f"   {problem}", file=sys.stderr)
        print(
            "   Replace the dependency, or record a deliberate decision in licence-allowlist.json.",
            file=sys.stderr,
        )
        return 1
    print(
        f"✅ licence check: {len(crates)} crates and {len(packages)} npm packages ship, "
        f"all under allowed licences ({len(cargo_exceptions) + len(npm_exceptions)} by name)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
