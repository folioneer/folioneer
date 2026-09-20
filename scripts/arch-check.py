#!/usr/bin/env python3
"""Enforce the architecture rules that can be checked mechanically.

    python3 scripts/arch-check.py                    # check, exit 1 on any violation
    python3 scripts/arch-check.py --write-allowlist  # shrink arch-allowlist.json to today's state

Rules (frontend under `src/`, backend under `src-tauri/src/`):

  A1  only a gateway calls the Tauri bindings — `commands.` appears only in
      files named `*gateway.ts`
  A2  a feature never imports a sibling feature; `features/shell/` is the
      composition root and may host any feature's modal (F26); the known
      crossings are frozen in the allowlist and may only disappear
  A3  a bounded context never reaches into another (`crate::context::<other>`)
      outside its test modules
  A4  a Tauri command never returns `Result<_, String>` — errors are typed
  A5  feature code renders no raw interactive element; buttons, inputs and
      links come from `ui/` so the stable-id and a11y rules hold in one place
  A6  every interactive `ui/` component a feature renders carries an `id`
      (E1–E4); today's id-less tags are frozen per file in the allowlist
  A7  feature code derives nothing: no `.reduce(`; `Math.` usage is frozen per
      file in the allowlist (display rounding and documented previews)
  A8  feature code carries no literal user-facing attribute text
      (`aria-label`, `placeholder`, `title`, `label`) — strings come from i18n

The allowlist is a ratchet: a count above its recorded value fails, a count
below it fails too until `--write-allowlist` lowers the record. Nothing is
ever added by the script.
"""

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ALLOWLIST = ROOT / "arch-allowlist.json"
FRONTEND = ROOT / "src"
FEATURES = FRONTEND / "features"
BACKEND = ROOT / "src-tauri" / "src"
CONTEXTS = BACKEND / "context"

COMPOSITION_ROOT = "shell"
DEV_PAGES = ("design-system",)
INTERACTIVE_COMPONENTS = ("Button", "IconButton", "TextField", "DateField", "CalcField", "FAB")
RAW_INTERACTIVE = re.compile(r"<(button|input|select|textarea|a)[\s>/]")
LITERAL_ATTRIBUTE = re.compile(r'(?<![\w-])(aria-label|placeholder|title|label)="[^"]*[A-Za-z]{3,}')
IMPORT = re.compile(r'(?:from\s+|import\()\s*"([^"]+)"')


def without_comments(line: str) -> str:
    """Drop a trailing `//` comment and whole-line block-comment continuations."""
    stripped = line.lstrip()
    if stripped.startswith(("//", "/*", "*")):
        return ""
    return re.sub(r"//.*$", "", line)


def without_strings(line: str) -> str:
    """Blank the contents of string literals so their characters are not counted."""
    return re.sub(r'"(?:[^"\\]|\\.)*"|\'(?:[^\'\\]|\\.)*\'|`(?:[^`\\]|\\.)*`', '""', line)


def rel(path: Path) -> str:
    return path.relative_to(ROOT).as_posix()


def is_test(path: Path) -> bool:
    return ".test." in path.name or ".spec." in path.name or "__preview__" in path.parts


def frontend_sources(base: Path):
    for path in sorted(base.rglob("*.ts*")):
        if path.suffix in (".ts", ".tsx") and not is_test(path) and path.name != "bindings.ts":
            yield path


def feature_of(path: Path) -> str | None:
    try:
        return path.relative_to(FEATURES).parts[0]
    except ValueError:
        return None


def production_lines(rust: str):
    """Yield (line_number, line) for everything outside `#[cfg(test)]` items.

    A `#[cfg(test)]` before `mod … {` hides the whole block (braces counted);
    before any other item it hides that item alone.
    """
    lines = rust.splitlines()
    index = 0
    while index < len(lines):
        line = lines[index]
        if line.strip() == "#[cfg(test)]":
            index += 1
            if index < len(lines) and lines[index].lstrip().startswith("mod "):
                depth = 0
                while index < len(lines):
                    code = without_strings(without_comments(lines[index]))
                    depth += code.count("{") - code.count("}")
                    index += 1
                    if depth <= 0:
                        break
            else:
                index += 1
            continue
        yield index + 1, line
        index += 1


# --- rules -----------------------------------------------------------------


def a1_gateway_only() -> list[str]:
    hits = []
    for path in frontend_sources(FRONTEND):
        if path.name.lower().endswith("gateway.ts"):
            continue
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if "commands." in line:
                hits.append(f"A1 {rel(path)}:{number}: `commands.` outside a gateway")
    return hits


def a2_cross_feature() -> list[dict]:
    """Return every sibling-feature import; the allowlist decides which are frozen."""
    crossings = []
    for path in frontend_sources(FEATURES):
        feature = feature_of(path)
        if feature == COMPOSITION_ROOT:
            continue
        for target in IMPORT.findall(path.read_text(encoding="utf-8")):
            other = None
            if target.startswith("@/features/"):
                other = target.split("/")[2]
            elif target.startswith("."):
                resolved = (path.parent / target).resolve()
                try:
                    other = resolved.relative_to(FEATURES).parts[0]
                except ValueError:
                    other = None
            if other and other != feature:
                crossings.append({"from": rel(path), "to": target})
    return crossings


def a3_cross_context() -> list[str]:
    hits = []
    for path in sorted(CONTEXTS.rglob("*.rs")):
        context = path.relative_to(CONTEXTS).parts[0]
        for number, line in production_lines(path.read_text(encoding="utf-8")):
            for other in re.findall(r"crate::context::(\w+)", line):
                if other != context:
                    hits.append(f"A3 {rel(path)}:{number}: reaches into context `{other}`")
    return hits


def returns_string_error(line: str) -> bool:
    """True when a `Result<…>` on the line has `String` as its outermost error type."""
    for match in re.finditer(r"\bResult<", line):
        depth = 1
        index = match.end()
        last_comma = None
        while index < len(line) and depth > 0:
            char = line[index]
            if char == "<":
                depth += 1
            elif char == ">":
                depth -= 1
            elif char == "," and depth == 1:
                last_comma = index
            index += 1
        if depth == 0 and last_comma is not None:
            if line[last_comma + 1 : index - 1].strip() == "String":
                return True
    return False


def a4_typed_wire_errors() -> list[str]:
    hits = []
    for path in sorted(BACKEND.rglob("api.rs")):
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if returns_string_error(without_comments(line)):
                hits.append(f"A4 {rel(path)}:{number}: command returns `Result<_, String>`")
    return hits


def a5_no_raw_interactive() -> list[str]:
    hits = []
    for path in frontend_sources(FEATURES):
        if path.suffix != ".tsx":
            continue
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            match = RAW_INTERACTIVE.search(without_comments(line))
            if match:
                hits.append(f"A5 {rel(path)}:{number}: raw `<{match.group(1)}>` — use the ui/ component")
    return hits


def opening_tags(source: str, names: tuple[str, ...]):
    """Yield (line_number, component, tag_text) for each JSX opening tag of `names`."""
    pattern = re.compile(r"<(%s)\b" % "|".join(names))
    for match in pattern.finditer(source):
        index = match.end()
        depth = 0
        while index < len(source):
            char = source[index]
            if char == "{":
                depth += 1
            elif char == "}":
                depth -= 1
            elif char == ">" and depth == 0:
                break
            index += 1
        yield source.count("\n", 0, match.start()) + 1, match.group(1), source[match.start() : index + 1]


def a6_missing_ids() -> dict[str, int]:
    """Count interactive `ui/` tags without `id=` per file.

    A tag carrying a spread (`{...props}`) is not counted: the id may arrive
    through the spread, and the check cannot see it — a known blind spot.
    """
    counts: dict[str, int] = {}
    for path in frontend_sources(FEATURES):
        if path.suffix != ".tsx":
            continue
        source = path.read_text(encoding="utf-8")
        for _, _, tag in opening_tags(source, INTERACTIVE_COMPONENTS):
            if not re.search(r"\bid=", tag) and "{..." not in tag:
                counts[rel(path)] = counts.get(rel(path), 0) + 1
    return counts


def a7_derivation() -> tuple[list[str], dict[str, int]]:
    hits = []
    math_counts: dict[str, int] = {}
    for path in frontend_sources(FEATURES):
        for number, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            line = without_strings(without_comments(raw))
            if ".reduce(" in line:
                hits.append(f"A7 {rel(path)}:{number}: `.reduce(` — aggregation belongs to the backend")
            if "Math." in line:
                math_counts[rel(path)] = math_counts.get(rel(path), 0) + 1
    return hits, math_counts


def a8_literal_attributes() -> list[str]:
    hits = []
    for path in frontend_sources(FEATURES):
        if path.suffix != ".tsx" or feature_of(path) in DEV_PAGES:
            continue
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            match = LITERAL_ATTRIBUTE.search(without_comments(line))
            if match:
                hits.append(f"A8 {rel(path)}:{number}: literal `{match.group(1)}` text — use i18n")
    return hits


# --- allowlist ratchet -----------------------------------------------------


def ratchet_counts(rule: str, label: str, actual: dict[str, int], recorded: dict[str, int]) -> list[str]:
    hits = []
    for file, count in sorted(actual.items()):
        allowed = recorded.get(file, 0)
        if count > allowed:
            hits.append(f"{rule} {file}: {count} {label} (allowlist records {allowed})")
        elif count < allowed:
            hits.append(f"{rule} {file}: {count} {label}, allowlist records {allowed} — run --write-allowlist")
    for file in sorted(set(recorded) - set(actual)):
        hits.append(f"{rule} {file}: no {label} left, allowlist still records {recorded[file]} — run --write-allowlist")
    return hits


def ratchet_pairs(actual: list[dict], recorded: list[dict]) -> list[str]:
    key = lambda item: (item["from"], item["to"])
    actual_keys = {key(item) for item in actual}
    recorded_keys = {key(item) for item in recorded}
    hits = [f"A2 {f}: imports sibling feature `{t}`" for f, t in sorted(actual_keys - recorded_keys)]
    hits += [
        f"A2 {f}: no longer imports `{t}`, allowlist still records it — run --write-allowlist"
        for f, t in sorted(recorded_keys - actual_keys)
    ]
    return hits


def current_state() -> dict:
    crossings = a2_cross_feature()
    _, math_counts = a7_derivation()
    return {
        "cross_feature_imports": sorted(crossings, key=lambda item: (item["from"], item["to"])),
        "missing_ids": dict(sorted(a6_missing_ids().items())),
        "math_usage": dict(sorted(math_counts.items())),
    }


def write_allowlist(state: dict, recorded: dict) -> int:
    if not ALLOWLIST.exists():
        recorded = state  # first run freezes today's state; the ratchet starts here
    grew = []
    pair = lambda item: (item["from"], item["to"])
    known = {pair(item) for item in recorded.get("cross_feature_imports", [])}
    grew += [f"{f} → {t}" for f, t in sorted({pair(i) for i in state["cross_feature_imports"]} - known)]
    for section in ("missing_ids", "math_usage"):
        for file, count in state[section].items():
            if count > recorded.get(section, {}).get(file, 0):
                grew.append(f"{file} ({section}: {count})")
    if grew:
        print("❌ refusing to write, these would grow — fix the code instead:")
        for item in grew:
            print(f"   {item}")
        return 1
    ALLOWLIST.write_text(json.dumps(state, indent=2) + "\n", encoding="utf-8")
    print(f"✅ {ALLOWLIST.name} rewritten to today's state")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--write-allowlist",
        action="store_true",
        help="lower arch-allowlist.json to the current state (never raises it)",
    )
    args = parser.parse_args()

    recorded = json.loads(ALLOWLIST.read_text(encoding="utf-8")) if ALLOWLIST.exists() else {}
    state = current_state()
    if args.write_allowlist:
        return write_allowlist(state, recorded)

    hits = []
    hits += a1_gateway_only()
    hits += ratchet_pairs(state["cross_feature_imports"], recorded.get("cross_feature_imports", []))
    hits += a3_cross_context()
    hits += a4_typed_wire_errors()
    hits += a5_no_raw_interactive()
    hits += ratchet_counts("A6", "id-less interactive tags", state["missing_ids"], recorded.get("missing_ids", {}))
    reduce_hits, _ = a7_derivation()
    hits += reduce_hits
    hits += ratchet_counts("A7", "`Math.` uses", state["math_usage"], recorded.get("math_usage", {}))
    hits += a8_literal_attributes()

    if hits:
        print(f"❌ architecture check: {len(hits)} violation(s)")
        for hit in hits:
            print(f"   {hit}")
        return 1
    frozen = (
        len(recorded.get("cross_feature_imports", [])),
        sum(recorded.get("missing_ids", {}).values()),
        sum(recorded.get("math_usage", {}).values()),
    )
    print(
        "✅ architecture check: A1–A8 hold "
        f"(frozen debt: {frozen[0]} cross-feature imports, {frozen[1]} id-less tags, {frozen[2]} Math. uses)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
