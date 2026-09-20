#!/usr/bin/env python3
"""Deterministic data collection for the /feature-planner skill.

Consumed by /feature-planner Steps 1–3: the JSON output drives ADR analysis
(Step 1), per-scope convention selection (Step 2), and path verification
(Step 3) — replaces live regex extraction + filesystem walks. Same pattern
as scripts/whats-next.py.

Emits a single JSON document on stdout describing:

- spec: TRIGRAM-NNN rules (id, scope, description), trigram, registration status
- layout: backend/frontend module roots from ARCHITECTURE.md or fallback search
- conventions: presence map of each kit convention doc (per-scope subset)
- adrs: ADRs in docs/adr/ with title and status

Usage:
    python3 scripts/plan-context.py docs/spec/{feature}.md
    python3 scripts/plan-context.py --pretty docs/spec/{feature}.md
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path


def _project_root() -> Path:
    """Resolve repo root via `git rev-parse --show-toplevel`; fall back to cwd."""
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

# Rule shape per docs/spec-writer convention:
#   **TRIGRAM-NNN** (frontend|backend|frontend + backend): description
# Bold markers, leading bullets, and surrounding whitespace are tolerated.
RULE_PAT = re.compile(
    r"\b(?P<id>[A-Z]{3,4}-\d{3})\b"
    r".*?"
    r"\((?P<scope>frontend\s*\+\s*backend|frontend|backend)\)"
    r"\s*[:\-—]?\s*"
    r"(?P<desc>.+)"
)


def _read(path: Path, *, strict: bool = False) -> str | None:
    """Read a file's content; return None on absence/decode-failure.

    With strict=True, permission errors (and other OSError) propagate. Use
    for the spec file so a non-FNF read failure surfaces as a script crash
    rather than being silently encoded as `found: false` in the JSON.
    """
    try:
        return path.read_text(encoding="utf-8")
    except FileNotFoundError:
        return None
    except UnicodeDecodeError:
        return None
    except OSError:
        if strict:
            raise
        return None


def _rel(path: Path) -> str:
    """Path string relative to ROOT, or absolute if outside."""
    try:
        return str(path.resolve().relative_to(ROOT))
    except ValueError:
        return str(path)


def collect_spec(spec_path: Path) -> dict:
    """Parse the spec: extract rules, infer trigram, check spec-index registration."""
    text = _read(spec_path, strict=True)
    if text is None:
        return {
            "path": _rel(spec_path),
            "found": False,
            "trigram": None,
            "registered": False,
            "rules": [],
        }

    rules: list[dict] = []
    trigrams: set[str] = set()
    for line in text.splitlines():
        clean = line.strip().lstrip("-* ").replace("**", "").replace("__", "")
        m = RULE_PAT.search(clean)
        if m:
            rule_id = m.group("id")
            trigrams.add(rule_id.split("-")[0])
            rules.append(
                {
                    "id": rule_id,
                    "scope": m.group("scope").strip(),
                    "description": m.group("desc").strip(),
                }
            )

    # A well-formed spec has exactly one trigram; ambiguous specs return None
    # so the consuming skill can ask the user to clarify.
    primary_trigram = next(iter(trigrams)) if len(trigrams) == 1 else None

    registered = False
    if primary_trigram:
        index_text = _read(ROOT / "docs" / "spec-index.md")
        if index_text and primary_trigram in index_text:
            registered = True

    return {
        "path": _rel(spec_path),
        "found": True,
        "trigram": primary_trigram,
        "registered": registered,
        "rules": rules,
    }


def collect_layout() -> dict:
    """Discover backend + frontend module roots from ARCHITECTURE.md or fallback.

    Auto-discovery is intentionally narrow — only the layouts the kit ships
    for (Tauri 2 / standard Rust+Node). Projects with novel layouts (e.g.
    `apps/web/src`, `packages/backend/src`) are expected to declare them in
    ARCHITECTURE.md; the planner reads it there. No regex tuning per project.
    """
    arch_text = _read(ROOT / "ARCHITECTURE.md")

    backend_root: str | None = None
    frontend_root: str | None = None
    fallbacks_used: list[str] = []

    if arch_text:
        # Backtick-wrapped paths in ARCHITECTURE.md are the canonical roots.
        be_pat = re.compile(r"`(src-tauri/src(?:/\w+)?|server/src|src/server)`")
        fe_pat = re.compile(r"`(src(?:/features?)?|client/src)`")
        for line in arch_text.splitlines():
            if not backend_root:
                m = be_pat.search(line)
                if m:
                    backend_root = m.group(1)
            if not frontend_root:
                m = fe_pat.search(line)
                if m and "src-tauri" not in m.group(1):
                    frontend_root = m.group(1)

    if not backend_root:
        for candidate in ("src-tauri/src", "server/src", "src/server"):
            if (ROOT / candidate).is_dir():
                backend_root = candidate
                fallbacks_used.append(f"backend: {candidate}")
                break

    if not frontend_root:
        for candidate in ("src", "client/src"):
            if (ROOT / candidate).is_dir():
                frontend_root = candidate
                fallbacks_used.append(f"frontend: {candidate}")
                break

    return {
        "architecture_present": arch_text is not None,
        "backend_root": backend_root,
        "frontend_root": frontend_root,
        "fallbacks_used": fallbacks_used,
    }


CONVENTION_FILES = {
    "architecture_md": "ARCHITECTURE.md",
    "backend_rules": "docs/backend-rules.md",
    "frontend_rules": "docs/frontend-rules.md",
    "ddd_reference": "docs/ddd-reference.md",
    "error_model": "docs/error-model.md",
    "i18n_rules": "docs/i18n-rules.md",
    "test_convention": "docs/test_convention.md",
    "frontend_visual_proof": "docs/frontend-visual-proof.md",
    "e2e_rules": "docs/e2e-rules.md",
}


def collect_conventions() -> dict:
    """Presence map of each kit convention doc."""
    return {key: (ROOT / path).is_file() for key, path in CONVENTION_FILES.items()}


def collect_adrs() -> list:
    """List ADRs in docs/adr/ with extracted title and status (if present)."""
    adr_dir = ROOT / "docs" / "adr"
    if not adr_dir.is_dir():
        return []

    out: list[dict] = []
    title_pat = re.compile(r"^#\s+(.+?)\s*$")
    status_pat = re.compile(r"^[*-]?\s*\*?\*?Status\*?\*?[:\s]+(\w+)", re.IGNORECASE)

    for path in sorted(adr_dir.glob("*.md")):
        if path.name.lower() in ("readme.md", "index.md"):
            continue
        text = _read(path)
        title: str | None = None
        status: str | None = None
        if text:
            for line in text.splitlines()[:30]:
                if title is None:
                    m = title_pat.match(line)
                    if m:
                        title = m.group(1)
                if status is None:
                    m = status_pat.match(line)
                    if m:
                        status = m.group(1).lower()
                if title and status:
                    break
        out.append(
            {
                "path": _rel(path),
                "title": title or path.stem,
                "status": status,
            }
        )
    return out


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Collect plan context for the /feature-planner skill"
    )
    parser.add_argument("spec_path", help="Path to docs/spec/{feature}.md")
    parser.add_argument("--pretty", action="store_true", help="Indent the JSON output")
    args = parser.parse_args()

    spec_path = Path(args.spec_path)
    if not spec_path.is_absolute():
        spec_path = (Path.cwd() / spec_path).resolve()

    data = {
        "spec": collect_spec(spec_path),
        "layout": collect_layout(),
        "conventions": collect_conventions(),
        "adrs": collect_adrs(),
    }

    indent = 2 if args.pretty else None
    print(json.dumps(data, indent=indent))
    return 0


if __name__ == "__main__":
    sys.exit(main())
