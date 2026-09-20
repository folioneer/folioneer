#!/usr/bin/env python3
"""Fail when line coverage of the code that carries logic drops under its floor.

Reads the lcov reports the test runners already produce and checks one number
per layer: the share of covered lines across the paths listed in
`coverage-gates.json`. Anything outside those paths (wire adapters, generated
bindings, tests) does not count either way.

    python3 scripts/coverage-gate.py              # both layers
    python3 scripts/coverage-gate.py --frontend   # one layer
    python3 scripts/coverage-gate.py --backend

Exit 1 when a layer is under its floor, or when its report is missing. The
floors are a ratchet: raise them in the same change that lifts coverage,
never lower them.
"""

import argparse
import json
import sys
from fnmatch import fnmatchcase
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parent.parent
CONFIG = ROOT / "coverage-gates.json"
ROOT_SEGMENTS = ("src-tauri", "src")


def relative_path(raw: str) -> str:
    """Map an lcov SF path to a repo-relative one.

    Vitest writes `src/...`; llvm-cov writes absolute paths. The path is cut
    at its first `src-tauri` or `src` segment (whole segments, so `src-tauri`
    is never mistaken for `src`), which makes the globs in the config match on
    any machine.
    """
    parts = PurePosixPath(raw.replace("\\", "/")).parts
    for index, part in enumerate(parts):
        if part in ROOT_SEGMENTS:
            return "/".join(parts[index:])
    return raw


def parse_lcov(path: Path) -> dict[str, tuple[int, int]]:
    """Return {file: (lines_found, lines_hit)} from an lcov report.

    A file may appear in several records (one per test binary); their counts
    are summed.
    """
    files: dict[str, tuple[int, int]] = {}
    current = None
    found = hit = 0
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("SF:"):
            current = relative_path(line[3:])
            found = hit = 0
        elif line.startswith("LF:"):
            found = int(line[3:])
        elif line.startswith("LH:"):
            hit = int(line[3:])
        elif line == "end_of_record" and current is not None:
            seen_found, seen_hit = files.get(current, (0, 0))
            files[current] = (seen_found + found, seen_hit + hit)
            current = None
    return files


def selected(path: str, include: list[str], exclude: list[str]) -> bool:
    # fnmatch's `*` crosses `/` (unlike a shell glob), so `src/features/*`
    # reaches every nested file. The config relies on that.
    return any(fnmatchcase(path, g) for g in include) and not any(
        fnmatchcase(path, g) for g in exclude
    )


def check_layer(name: str, spec: dict) -> bool:
    report = ROOT / spec["lcov"]
    if not report.exists():
        print(f"❌ {name}: no report at {spec['lcov']} — run the coverage recipe first")
        return False

    files = {
        f: counts
        for f, counts in parse_lcov(report).items()
        if selected(f, spec["include"], spec["exclude"])
    }
    found = sum(c[0] for c in files.values())
    hit = sum(c[1] for c in files.values())
    if found == 0:
        print(f"❌ {name}: no line matched the include globs — check coverage-gates.json")
        return False

    percent = 100.0 * hit / found
    floor = float(spec["lines"])
    target = spec.get("target")
    ok = percent >= floor
    status = "✅" if ok else "❌"
    goal = f", target {target:.0f}%" if target is not None and target > floor else ""
    print(
        f"{status} {name}: {percent:.2f}% of {found} logic lines covered "
        f"({len(files)} files, floor {floor:.2f}%{goal})"
    )

    if not ok:
        print("   Lowest-covered files (lines found ≥ 20):")
        worst = sorted(
            ((c[1] / c[0], f, c) for f, c in files.items() if c[0] >= 20),
            key=lambda item: item[0],
        )[:10]
        for ratio, f, (lf, lh) in worst:
            print(f"   {100 * ratio:6.1f}%  {lf - lh:4d} uncovered of {lf:4d}  {f}")
    return ok


def load_config() -> dict:
    try:
        return json.loads(CONFIG.read_text(encoding="utf-8"))
    except FileNotFoundError:
        sys.exit(f"❌ {CONFIG.name} not found at the repo root")
    except json.JSONDecodeError as error:
        sys.exit(f"❌ {CONFIG.name} is not valid JSON: {error}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--frontend", action="store_true", help="check only the frontend layer")
    parser.add_argument("--backend", action="store_true", help="check only the backend layer")
    args = parser.parse_args()

    config = load_config()
    layers = [k for k in ("frontend", "backend") if getattr(args, k)] or ["frontend", "backend"]
    missing = [layer for layer in layers if layer not in config]
    if missing:
        sys.exit(f"❌ {CONFIG.name} has no section for: {', '.join(missing)}")

    results = [check_layer(layer, config[layer]) for layer in layers]
    return 0 if all(results) else 1


if __name__ == "__main__":
    sys.exit(main())
