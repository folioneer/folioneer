#!/usr/bin/env python3
"""Enumerate FE test targets for a feature domain.

Scans `src/features/{domain}/` (and sub-features per F3) for `.tsx` files and
emits JSON describing each candidate: file path, component name, whether it
imports from `gateway`, whether it imports from `./presenter` or `./shared/presenter`.

Consumed by the `test-writer-frontend` agent at Step 1 to drive Step 4's
component-test decisions. Mechanical scan only — the agent applies the
4-row state-decision table itself.

Usage:
    python3 scripts/list-fe-test-targets.py <domain>
    python3 scripts/list-fe-test-targets.py --feature-dir src/features/<domain>
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from pathlib import Path

if os.environ.get("NO_COLOR"):
    RED = NC = ""
else:
    RED = "\033[0;31m"
    NC = "\033[0m"


def _project_root() -> Path:
    """Resolve via `git rev-parse --show-toplevel`, fall back to cwd."""
    import subprocess

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


_IMPORT_RE = re.compile(
    r'import\s+(?:[^"\']+\s+from\s+)?["\']([^"\']+)["\']',
    re.MULTILINE,
)

_COMPONENT_RE = re.compile(
    r"export\s+(?:default\s+)?(?:const|function)\s+([A-Z]\w*)",
)


def _imports_module(imports: list[str], name: str) -> bool:
    """True iff `imports` includes a relative reference to a module named `name`
    inside the same feature (e.g. `./gateway`, `../shared/presenter`). Cross-
    feature imports are intentionally excluded — they are an F26 violation, not
    a same-feature dependency."""
    pattern = re.compile(rf"\.{{1,2}}/(?:[^/]+/)*{re.escape(name)}(?:\.ts)?$")
    return any(pattern.match(i) for i in imports)


def _classify(path: Path) -> dict | None:
    """Return target metadata, or None for files that aren't test candidates
    (e.g. index files, type-only modules, test files themselves)."""
    if path.name.endswith(".test.tsx") or path.name.endswith(".test.ts"):
        return None
    if path.name in ("index.tsx", "index.ts"):
        return None
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError as exc:
        print(f"{RED}warn: could not read {path}: {exc}{NC}", file=sys.stderr)
        return None

    imports = _IMPORT_RE.findall(text)
    imports_gateway = _imports_module(imports, "gateway")
    imports_presenter = _imports_module(imports, "presenter")

    components = _COMPONENT_RE.findall(text)
    component = components[0] if components else path.stem

    return {
        "file": str(path),
        "component": component,
        "imports_gateway": imports_gateway,
        "imports_presenter": imports_presenter,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("domain", nargs="?", help="Feature domain name (e.g. 'users')")
    parser.add_argument(
        "--feature-dir",
        type=Path,
        help="Explicit feature directory (overrides --domain resolution)",
    )
    args = parser.parse_args()

    root = _project_root()
    if args.feature_dir:
        feature_dir = (
            args.feature_dir
            if args.feature_dir.is_absolute()
            else root / args.feature_dir
        )
    elif args.domain:
        feature_dir = root / "src" / "features" / args.domain
    else:
        print(f"{RED}error: provide a domain or --feature-dir.{NC}", file=sys.stderr)
        return 2

    if not feature_dir.is_dir():
        # No feature folder yet (greenfield) — emit empty list, not an error.
        json.dump([], sys.stdout)
        sys.stdout.write("\n")
        return 0

    targets: list[dict] = []
    for path in sorted(feature_dir.rglob("*.tsx")):
        entry = _classify(path)
        if entry is None:
            continue
        # Make path relative to project root for readability
        try:
            entry["file"] = str(Path(entry["file"]).relative_to(root))
        except ValueError:
            pass
        targets.append(entry)

    json.dump(targets, sys.stdout, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
