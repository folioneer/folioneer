#!/usr/bin/env python3
"""Drop inline test modules from an lcov report, in place.

Rust keeps unit tests in the same file as the code, under `#[cfg(test)]`:
test modules, but also test-only impl blocks and helper functions. llvm-cov
instruments them like any code, so a file's coverage would count its own tests
as covered logic. For every file in the report, each such item is located by
its braces and the records inside it are removed; the file's line, function
and branch totals are recomputed. Code after a test item stays. A source file the report names but that cannot be read is an
error: silently keeping its tests would drift the number upward.

Use:
    python3 scripts/coverage-strip-tests.py coverage/backend/lcov.info
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

TEST_ATTRIBUTE = re.compile(r"^\s*#\[cfg\(test\)\]\s*$")
ATTRIBUTE_LINE = re.compile(r"^\s*#\[")
# Any item the attribute can gate: a module, an impl block, a function, a type.
ITEM_LINE = re.compile(
    r"^\s*(pub(\([^)]*\))?\s+)?((async|unsafe|const)\s+)*(mod|impl|fn|struct|enum|trait|use|type|const|static)\b"
)


def _brace_end(lines: list[str], start: int) -> int:
    """0-based index of the line closing the block opened on `lines[start]`.

    Braces inside string literals and comments are skipped so a `{` in a test
    fixture cannot shift the range.
    """
    depth = 0
    in_block_comment = False
    for index in range(start, len(lines)):
        line = lines[index]
        position = 0
        while position < len(line):
            rest = line[position:]
            if in_block_comment:
                close = rest.find("*/")
                if close == -1:
                    break
                position += close + 2
                in_block_comment = False
                continue
            if rest.startswith("//"):
                break
            if rest.startswith("/*"):
                in_block_comment = True
                position += 2
                continue
            if rest.startswith('"') or re.match(r'r#*"', rest):
                position += _string_length(rest)
                continue
            if rest[0] == "'" and len(rest) > 2 and rest[2] == "'":
                position += 3  # a char literal such as '{'
                continue
            if rest[0] == "{":
                depth += 1
            elif rest[0] == "}":
                depth -= 1
                if depth == 0:
                    return index
            position += 1
    return len(lines) - 1


def _string_length(rest: str) -> int:
    """Length of the string literal that starts `rest`, on this line."""
    raw = re.match(r'r(#*)"', rest)
    if raw:
        close = rest.find('"' + raw.group(1), raw.end())
        return len(rest) if close == -1 else close + 1 + len(raw.group(1))
    position = 1
    while position < len(rest):
        if rest[position] == "\\":
            position += 2
            continue
        if rest[position] == '"':
            return position + 1
        position += 1
    return len(rest)


def _item_end(lines: list[str], start: int) -> int:
    """0-based index of the last line of the item starting on `lines[start]`.

    A brace-delimited item ends at its closing brace; a one-line item
    (`use`, a unit struct, a type alias) ends at its `;`.
    """
    for index in range(start, len(lines)):
        line = lines[index].split("//")[0]
        brace, semicolon = line.find("{"), line.find(";")
        if brace != -1 and (semicolon == -1 or brace < semicolon):
            return _brace_end(lines, index)
        if semicolon != -1:
            return index
    return len(lines) - 1


def test_module_ranges(source: Path) -> list[tuple[int, int]]:
    """1-based inclusive (first, last) line ranges of the file's `#[cfg(test)]` items.

    Other attributes, blank lines and comments may sit between the attribute
    and its item. An attribute that gates nothing recognised is an error.
    """
    lines = source.read_text(encoding="utf-8").splitlines()
    ranges: list[tuple[int, int]] = []
    index = 0
    while index < len(lines):
        if TEST_ATTRIBUTE.match(lines[index]):
            item = index + 1
            while item < len(lines) and (
                ATTRIBUTE_LINE.match(lines[item]) or not lines[item].strip() or lines[item].lstrip().startswith("//")
            ):
                item += 1
            if item >= len(lines) or not ITEM_LINE.match(lines[item]):
                raise ValueError(
                    f"{source}:{index + 1}: #[cfg(test)] does not gate an item this script knows; "
                    "leaving it in the report would count tests as logic"
                )
            end = _item_end(lines, item)
            ranges.append((index + 1, end + 1))
            index = end + 1
            continue
        index += 1
    return ranges


def _inside(number: int, ranges: list[tuple[int, int]]) -> bool:
    return any(first <= number <= last for first, last in ranges)


def _strip_record(record: list[str], ranges: list[tuple[int, int]]) -> tuple[list[str], int]:
    """Rewrite one SF record; return (lines, DA records dropped)."""
    kept: list[str] = []
    dropped_functions: set[str] = set()
    lines_found = lines_hit = 0
    functions_found = functions_hit = 0
    branches_found = branches_hit = 0
    dropped = 0
    for line in record:
        key, _, value = line.partition(":")
        if key == "FN":
            number, name = value.split(",", 1)
            if _inside(int(number), ranges):
                dropped_functions.add(name)
                continue
            functions_found += 1
        elif key == "FNDA":
            count, name = value.split(",", 1)
            if name in dropped_functions:
                continue
            functions_hit += int(count) > 0
        elif key == "DA":
            number, count = value.split(",")[:2]
            if _inside(int(number), ranges):
                dropped += 1
                continue
            lines_found += 1
            lines_hit += int(count) > 0
        elif key == "BRDA":
            number, _, _, taken = value.split(",")[:4]
            if _inside(int(number), ranges):
                continue
            branches_found += 1
            branches_hit += taken not in ("-", "0")
        elif key in ("LF", "LH", "FNF", "FNH", "BRF", "BRH"):
            continue
        elif key == "end_of_record":
            kept.extend(
                [f"FNF:{functions_found}", f"FNH:{functions_hit}", f"LF:{lines_found}", f"LH:{lines_hit}"]
            )
            if branches_found:
                kept.extend([f"BRF:{branches_found}", f"BRH:{branches_hit}"])
        kept.append(line)
    return kept, dropped


def strip(report: Path) -> tuple[int, int]:
    """Rewrite `report`; return (files trimmed, DA records dropped)."""
    out: list[str] = []
    record: list[str] = []
    ranges: list[tuple[int, int]] = []
    trimmed = dropped = 0
    for line in report.read_text(encoding="utf-8").splitlines():
        if line.startswith("SF:"):
            ranges = test_module_ranges(Path(line[3:]))
        record.append(line)
        if line == "end_of_record":
            if ranges:
                kept, count = _strip_record(record, ranges)
                out.extend(kept)
                trimmed += 1
                dropped += count
            else:
                out.extend(record)
            record, ranges = [], []
    out.extend(record)
    report.write_text("\n".join(out) + "\n", encoding="utf-8")
    return trimmed, dropped


def main() -> int:
    if len(sys.argv) != 2:
        print("use: coverage-strip-tests.py <lcov report>", file=sys.stderr)
        return 2
    report = Path(sys.argv[1])
    if not report.is_file():
        print(f"no report at {report}", file=sys.stderr)
        return 1
    try:
        trimmed, dropped = strip(report)
    except OSError as error:
        print(f"cannot read a source file the report names: {error}", file=sys.stderr)
        return 1
    except ValueError as error:
        print(str(error), file=sys.stderr)
        return 1
    print(f"stripped inline test modules from {trimmed} files ({dropped} lines)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
