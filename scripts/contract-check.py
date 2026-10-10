#!/usr/bin/env python3
"""A contract says what the generated bindings say (FLOW-024).

`src/bindings.ts` is generated from the core: it is what the window can call. Each
`docs/contracts/*-contract.md` describes those commands for whoever plans or reviews.
This check reads both and reports where they differ:

- a command of the bindings that no contract has a row for, a row for a command that
  does not exist, a command with a row in two contracts;
- a row whose arguments or return type are not those of the command;
- a row promising an error code that the command's error type cannot carry;
- a type a row names that no contract defines;
- a struct or an enum of a contract whose fields or variants are not the generated ones.

What it cannot see: the bindings give the error *type* of a command, not which of its
codes that command returns. A code the code returns and a row omits is found by reading
the command (`contract-reviewer`, `/contract`), not here.

Gaps known when the check arrived are listed in `contract-gaps.json`. A gap outside the
list fails; so does a line of the list that is no longer a gap, so the list only shrinks.

Use: python3 scripts/contract-check.py            the gate
     python3 scripts/contract-check.py --all      every gap, listed ones included
     python3 scripts/contract-check.py --shrink   drop from the list what is fixed
"""

from __future__ import annotations

import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BINDINGS = ROOT / "src" / "bindings.ts"
CONTRACTS = ROOT / "docs" / "contracts"
KNOWN = ROOT / "contract-gaps.json"

# Commands of the bindings that are no part of a domain's interface.
NO_CONTRACT = {"log_frontend"}

COMMENT = re.compile(r"/\*\*.*?\*/", re.S)
SIGNATURE = re.compile(r"^async (\w+)\((.*)\) : Promise<(.*)> \{$", re.M)
INVOKE = re.compile(r"TAURI_INVOKE\(\"(\w+)\"")
TYPE = re.compile(r"^export type (\w+)(?:<[^>]*>)? = ", re.M)
CODE = re.compile(r"code: \"(\w+)\"")
NUMBERS = {"i64", "i32", "u32", "u64", "u8", "u16", "f64", "usize", "number"}


@dataclass(frozen=True)
class Command:
    name: str
    args: tuple[tuple[str, str], ...]  # (snake_case name, generated type)
    returns: str
    error: str  # the error type's name, "" when the command cannot fail


@dataclass
class Bindings:
    commands: dict[str, Command] = field(default_factory=dict)
    types: dict[str, str] = field(default_factory=dict)  # name -> body, comments removed

    def codes(self, name: str, seen: frozenset[str] = frozenset()) -> set[str]:
        """Every error code a type can carry, through the types it is made of."""
        body = self.types.get(name)
        if body is None or name in seen:
            return set()
        codes = set(CODE.findall(body))
        for part in split_top(body, "|"):
            part = part.strip()
            if re.fullmatch(r"\w+", part):
                codes |= self.codes(part, seen | {name})
        return codes

    def fields(self, name: str) -> list[str] | None:
        """The field names of an object type; None for anything else."""
        body = self.types.get(name, "").strip()
        if not (body.startswith("{") and body.endswith("}")) or len(split_top(body, "|")) > 1:
            return None
        return [part.split(":", 1)[0].strip().rstrip("?") for part in split_top(body[1:-1], ";") if ":" in part]

    def variants(self, name: str) -> list[str] | None:
        """The variants of a union of string literals or of coded objects; None otherwise."""
        body = self.types.get(name, "").strip()
        parts = [part.strip() for part in split_top(body, "|") if part.strip()]
        if parts and all(re.fullmatch(r"\"\w+\"", part) for part in parts):
            return [part.strip('"') for part in parts]
        return None


def split_top(text: str, separator: str) -> list[str]:
    """Split on a separator that is not inside brackets, braces, angles or quotes."""
    parts, depth, start, quoted = [], 0, 0, False
    for index, char in enumerate(text):
        if char == '"':
            quoted = not quoted
        elif quoted:
            continue
        elif char in "{[(<":
            depth += 1
        elif char in "}])>":
            depth -= 1
        elif char == separator and depth == 0:
            parts.append(text[start:index])
            start = index + 1
    parts.append(text[start:])
    return parts


def bare(name: str) -> str:
    """A variant whatever its case: `NeedsAttention` is written "needs_attention"."""
    return name.replace("_", "").lower()


def snake(name: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def read_bindings(text: str) -> Bindings:
    bindings = Bindings()
    text = COMMENT.sub("", text)
    for match in SIGNATURE.finditer(text):
        raw_args, promised = match.group(2, 3)
        invoked = INVOKE.search(text, match.end())
        if invoked is None:
            continue
        name = invoked.group(1)
        args = tuple((snake(arg.split(":", 1)[0].strip()), arg.split(":", 1)[1].strip()) for arg in split_top(raw_args, ",") if ":" in arg)
        result = re.fullmatch(r"Result<(.*)>", promised.strip())
        if result:
            returns, error = (part.strip() for part in split_top(result.group(1), ","))
        else:
            returns, error = promised.strip(), ""
        bindings.commands[name] = Command(name, args, returns, error)
    starts = list(TYPE.finditer(text))
    for index, match in enumerate(starts):
        end = starts[index + 1].start() if index + 1 < len(starts) else len(text)
        body = text[match.end() : end]
        # The last type is followed by the generated runtime, which is no part of it.
        body = re.split(r"\n/\*\* tauri-specta globals \*\*/|\nimport |\nexport const |\nexport function ", body)[0]
        bindings.types[match.group(1)] = body.strip()
    return bindings


def rust_form(generated: str) -> str:
    """A generated type as a contract writes it: `Asset[]` is `Vec<Asset>`, `T | null` is
    `Option<T>`, `null` is `()`. Every number reads as `number`: the bindings keep no width."""
    text = generated.strip()
    parts = [part.strip() for part in split_top(text, "|")]
    if len(parts) == 2 and "null" in parts:
        return f"Option<{rust_form(next(part for part in parts if part != 'null'))}>"
    if text.endswith("[]"):
        return f"Vec<{rust_form(text[:-2])}>"
    if text.startswith("(") and text.endswith(")"):
        return rust_form(text[1:-1])
    return {"null": "()", "string": "String", "boolean": "bool"}.get(text, text)


def same_type(written: str, generated: str) -> bool:
    def flat(text: str) -> str:
        text = re.sub(r"\s+", "", text)
        return re.sub(r"\b(" + "|".join(sorted(NUMBERS)) + r")\b", "number", text)

    return flat(written) == flat(rust_form(generated))


@dataclass(frozen=True)
class Row:
    contract: str
    command: str
    args: str
    returns: str
    errors: str


@dataclass
class Contract:
    name: str
    rows: list[Row] = field(default_factory=list)
    structs: dict[str, list[str]] = field(default_factory=dict)
    enums: dict[str, list[str]] = field(default_factory=dict)


def cells(line: str) -> list[str]:
    """The cells of a table row; a pipe inside backticks is no separator."""
    parts, start, coded = [], 0, False
    for index, char in enumerate(line):
        if char == "`":
            coded = not coded
        elif char == "|" and not coded:
            parts.append(line[start:index].strip())
            start = index + 1
    return parts[1:]


def outside_notes(text: str) -> str:
    """A cell without what it says in brackets or in italics: a note may name a code the
    command does not return."""
    out, depth, coded = [], 0, False
    for char in re.sub(r"(?<![\w`])_[^_]*_(?![\w`])", " ", text):
        if char == "`":
            coded = not coded
        if not coded and char == "(":
            depth += 1
        elif not coded and char == ")":
            depth = max(depth - 1, 0)
        elif depth == 0:
            out.append(char)
    return "".join(out)


def promised_codes(errors: str) -> list[str]:
    return [code.split("{")[0].strip() for code in re.findall(r"`([A-Z]\w*(?: \{[^`]*\})?)`", outside_notes(errors))]


def read_contract(name: str, text: str) -> Contract:
    contract = Contract(name)
    in_commands = False
    for line in text.splitlines():
        if not line.startswith("|"):
            in_commands = False
            continue
        row = cells(line)
        if row and row[0] == "Command":
            in_commands = len(row) >= 4
        elif in_commands and len(row) >= 4 and re.fullmatch(r"`\w+`", row[0]):
            contract.rows.append(Row(name, row[0].strip("`"), row[1], row[2], row[3]))
    for block in re.findall(r"```rust\n(.*?)```", text, re.S):
        block = re.sub(r"//[^\n]*", "", block)
        for match in re.finditer(r"\b(struct|enum) (\w+)\s*\{", block):
            body = braced(block, match.end() - 1)
            if match.group(1) == "struct":
                contract.structs[match.group(2)] = [part.split(":", 1)[0].strip() for part in split_top(body, ",") if ":" in part]
            else:
                contract.enums[match.group(2)] = [name.group(0) for name in (re.match(r"\w+", part.strip()) for part in split_top(body, ",")) if name]
    for row in contract.rows:
        for inline in re.findall(r"\b([A-Z]\w+) \{", row.args):
            contract.structs.setdefault(inline, [])
    return contract


def braced(text: str, opening: int) -> str:
    """What stands between the brace at `opening` and the one that closes it."""
    depth = 0
    for index in range(opening, len(text)):
        depth += {"{": 1, "}": -1}.get(text[index], 0)
        if depth == 0:
            return text[opening + 1 : index]
    return text[opening + 1 :]


def named_types(text: str) -> set[str]:
    """The type names a cell uses, the standard ones aside."""
    standard = {"String", "Vec", "Option", "Result"}
    return {name for name in re.findall(r"\b[A-Z]\w+\b", " ".join(re.findall(r"`([^`]*)`", text))) if name not in standard}


def args_gap(row: Row, command: Command) -> str | None:
    written = " ".join(re.findall(r"`([^`]*)`", row.args))
    if not command.args:
        return None if not written else f"`{row.command}` takes no argument; the row gives `{written}`"
    names = [name for name, _ in command.args]
    types = {generated for _, generated in command.args if re.fullmatch(r"[A-Z]\w+", generated)}
    if len(command.args) == 1 and types and next(iter(types)) in written:
        return None
    given = re.findall(r"\b([a-z_][a-z0-9_]*)\s*:", re.sub(r"\{[^}]*\}", "", written))
    if given != names:
        return f"`{row.command}` takes ({', '.join(names)}); the row gives ({', '.join(given) or written or '—'})"
    return None


def gaps(bindings: Bindings, contracts: list[Contract]) -> list[str]:
    found: list[str] = []
    rows: dict[str, list[Row]] = {}
    for contract in contracts:
        for row in contract.rows:
            rows.setdefault(row.command, []).append(row)
    defined = {name for contract in contracts for name in (*contract.structs, *contract.enums)}

    for name in sorted(bindings.commands):
        if name not in rows and name not in NO_CONTRACT:
            found.append(f"no contract has a row for `{name}`")
    for name, its_rows in sorted(rows.items()):
        if len({row.contract for row in its_rows}) > 1:
            found.append(f"`{name}` has a row in {' and '.join(sorted({row.contract for row in its_rows}))}")
        row = its_rows[0]
        command = bindings.commands.get(name)
        if command is None:
            found.append(f"{row.contract}: `{name}` has a row and is no command")
            continue
        gap = args_gap(row, command)
        if gap:
            found.append(f"{row.contract}: {gap}")
        written = " ".join(re.findall(r"`([^`]*)`", row.returns))
        if not same_type(written, command.returns):
            found.append(f"{row.contract}: `{name}` returns `{rust_form(command.returns)}`; the row gives `{written or '—'}`")
        carried = bindings.codes(command.error) if command.error else set()
        for code in promised_codes(row.errors):
            if code not in carried and not (bindings.codes(code) and bindings.codes(code) <= carried):
                found.append(f"{row.contract}: `{name}` cannot return `{code}`" + (f" (`{command.error}` has no such code)" if command.error else " (the command cannot fail)"))
        for type_name in sorted(named_types(row.args) | named_types(row.returns)):
            if type_name in bindings.types and type_name not in defined:
                found.append(f"{row.contract}: `{name}` names `{type_name}`, which no contract defines")

    for contract in contracts:
        for type_name, written in sorted(contract.structs.items()):
            generated = bindings.fields(type_name)
            if generated is None or not written:
                continue
            for missing in [name for name in generated if name not in written]:
                found.append(f"{contract.name}: `{type_name}` lacks `{missing}`")
            for extra in [name for name in written if name not in generated]:
                found.append(f"{contract.name}: `{type_name}` has `{extra}`, which the generated type has not")
        for type_name, written in sorted(contract.enums.items()):
            generated = bindings.variants(type_name)
            if generated is None and type_name in bindings.types and CODE.search(bindings.types[type_name]):
                generated = sorted(bindings.codes(type_name))
            if generated is None:
                continue
            for missing in [name for name in generated if bare(name) not in map(bare, written)]:
                found.append(f"{contract.name}: `{type_name}` lacks `{missing}`")
            for extra in [name for name in written if bare(name) not in map(bare, generated)]:
                found.append(f"{contract.name}: `{type_name}` has `{extra}`, which the generated type has not")
    return sorted(set(found))


def verdict(found: list[str], known: list[str]) -> tuple[list[str], list[str]]:
    """(the gaps outside the list, the lines of the list that are no longer gaps)."""
    return [gap for gap in found if gap not in known], [line for line in known if line not in found]


def load() -> tuple[Bindings, list[Contract]]:
    bindings = read_bindings(BINDINGS.read_text(encoding="utf-8"))
    contracts = [read_contract(path.name.removesuffix("-contract.md"), path.read_text(encoding="utf-8")) for path in sorted(CONTRACTS.glob("*-contract.md"))]
    return bindings, contracts


def main(argv: list[str]) -> int:
    bindings, contracts = load()
    found = gaps(bindings, contracts)
    try:
        listed = json.loads(KNOWN.read_text(encoding="utf-8"))
        about, known = listed["about"], listed["gaps"]
    except (OSError, json.JSONDecodeError, KeyError) as error:
        print(f"❌ contract-check: {KNOWN.name} cannot be read ({error.__class__.__name__}); it holds `about` and `gaps`", file=sys.stderr)
        return 2
    if argv == ["--all"]:
        print("\n".join(found))
        return 0
    new, fixed = verdict(found, known)
    if argv == ["--shrink"]:
        if new:
            print("❌ contract-check: the list only shrinks; these gaps are new:\n  " + "\n  ".join(new), file=sys.stderr)
            return 1
        KNOWN.write_text(json.dumps({"about": about, "gaps": sorted({line for line in known if line in found})}, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        print(f"✅ contract-check: {len(fixed)} line(s) dropped, {len(known) - len(fixed)} left")
        return 0
    if argv:
        print(__doc__, file=sys.stderr)
        return 2
    if new or fixed:
        if new:
            print("❌ contract-check: a contract and the bindings differ:\n  " + "\n  ".join(new), file=sys.stderr)
        if fixed:
            print("❌ contract-check: listed in contract-gaps.json and no longer a gap (run with --shrink):\n  " + "\n  ".join(fixed), file=sys.stderr)
        return 1
    print(f"✅ contract-check: {len(bindings.commands)} commands, no gap outside the {len(known)} listed")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
