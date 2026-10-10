"""Tests for scripts/contract-check.py (FLOW-024) — run with `just test-scripts`."""

import importlib.util
import json
import sys
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "contract-check.py"
spec = importlib.util.spec_from_file_location("contract_check", SCRIPT)
contract_check = importlib.util.module_from_spec(spec)
sys.modules["contract_check"] = contract_check
spec.loader.exec_module(contract_check)

BINDINGS = """
export const commands = {
/**
 * Lists the things.
 */
async getThings() : Promise<Result<Thing[], ThingError>> {
    try {
    return { status: "ok", data: await TAURI_INVOKE("get_things") };
} catch (e) {
    if(e instanceof Error) throw e;
    else return { status: "error", error: e  as any };
}
},
async renameThing(thingId: string, newName: string) : Promise<Result<Thing | null, RenameError>> {
    try {
    return { status: "ok", data: await TAURI_INVOKE("rename_thing", { thingId, newName }) };
} catch (e) {
    if(e instanceof Error) throw e;
    else return { status: "error", error: e  as any };
}
},
async addThing(dto: AddThingDTO) : Promise<Result<null, ThingError>> {
    try {
    return { status: "ok", data: await TAURI_INVOKE("add_thing", { dto }) };
} catch (e) {
    if(e instanceof Error) throw e;
    else return { status: "error", error: e  as any };
}
},
async countThings() : Promise<number> {
    return await TAURI_INVOKE("count_things");
},
}

export type AddThingDTO = { name: string; colour: Colour }
export type Colour = "red" | "dark_blue"
export type RenameError = 
/**
 * A rejection of the thing itself.
 */
ThingError | { code: "NameTaken"; holder: string }
export type Thing = { 
/**
 * Its identifier.
 */
id: string; name: string; weight: number | null }
export type ThingError = { code: "NotFound" } | { code: "DatabaseError" }

/** tauri-specta globals **/
import { invoke as TAURI_INVOKE } from "@tauri-apps/api/core";
"""

CONTRACT = """# Contract — Thing

## Commands

| Command        | Args                                  | Return          | Errors |
| -------------- | ------------------------------------- | --------------- | ------ |
| `get_things`   | —                                     | `Vec<Thing>`    | `DatabaseError` _(never `NotFound`: an empty list)_ |
| `rename_thing` | `thing_id: String, new_name: String`  | `Option<Thing>` | `NotFound` (THG-010), `NameTaken { holder }` (THG-011 — not `Gone`), `DatabaseError` |
| `add_thing`    | `AddThingDTO`                         | `()`            | `DatabaseError` |
| `count_things` | —                                     | `u32`           | _(infallible)_ |

## Shared Types

```rust
struct Thing {
    id: String,       // its identifier
    name: String,
    weight: Option<i64>,
}

struct AddThingDTO { name: String, colour: Colour }

enum Colour { Red, DarkBlue }  // written "red" | "dark_blue"
```

## Events

| Event          | Payload |
| -------------- | ------- |
| `ThingRenamed` | —       |
"""


def gaps(contract: str = CONTRACT, bindings: str = BINDINGS, name: str = "thing") -> list[str]:
    return contract_check.gaps(contract_check.read_bindings(bindings), [contract_check.read_contract(name, contract)])


class ContractCheck(unittest.TestCase):
    # FLOW-024 — the bindings are read as commands with their arguments, what they return
    # and their error type; a command that cannot fail has none.
    def test_the_bindings_are_read_as_commands(self):
        bindings = contract_check.read_bindings(BINDINGS)
        self.assertEqual(sorted(bindings.commands), ["add_thing", "count_things", "get_things", "rename_thing"])
        rename = bindings.commands["rename_thing"]
        self.assertEqual(rename.args, (("thing_id", "string"), ("new_name", "string")))
        self.assertEqual((rename.returns, rename.error), ("Thing | null", "RenameError"))
        self.assertEqual(bindings.commands["count_things"].error, "")
        self.assertEqual(bindings.codes("RenameError"), {"NotFound", "DatabaseError", "NameTaken"})
        self.assertEqual(bindings.fields("Thing"), ["id", "name", "weight"])

    # FLOW-024 — a contract that says what the bindings say has no gap; a code named in a
    # note, and the event table, are not read as promises.
    def test_a_contract_that_matches_has_no_gap(self):
        self.assertEqual(gaps(), [])

    # FLOW-024 — a command without a row, and a row without a command.
    def test_a_missing_row_and_a_row_for_nothing_are_gaps(self):
        without = CONTRACT.replace("| `count_things` | —                                     | `u32`           | _(infallible)_ |\n", "")
        self.assertEqual(gaps(without), ["no contract has a row for `count_things`"])
        extra = CONTRACT.replace("| `count_things` |", "| `count_stars`  |")
        self.assertIn("thing: `count_stars` has a row and is no command", gaps(extra))

    # FLOW-024 — a command described by two contracts.
    def test_a_command_in_two_contracts_is_a_gap(self):
        bindings = contract_check.read_bindings(BINDINGS)
        contracts = [contract_check.read_contract(name, CONTRACT) for name in ("thing", "other")]
        self.assertIn("`get_things` has a row in other and thing", contract_check.gaps(bindings, contracts))

    # FLOW-024 — a row promising a code its command's error type cannot carry.
    def test_a_code_the_error_type_cannot_carry_is_a_gap(self):
        promised = CONTRACT.replace("| `add_thing`    | `AddThingDTO`                         | `()`            | `DatabaseError` |", "| `add_thing`    | `AddThingDTO`                         | `()`            | `NameTaken`, `DatabaseError` |")
        self.assertEqual(gaps(promised), ["thing: `add_thing` cannot return `NameTaken` (`ThingError` has no such code)"])
        fallible = CONTRACT.replace("_(infallible)_", "`DatabaseError`")
        self.assertEqual(gaps(fallible), ["thing: `count_things` cannot return `DatabaseError` (the command cannot fail)"])

    # FLOW-024 — a member type of the error type stands for its codes.
    def test_a_member_error_type_stands_for_its_codes(self):
        named = CONTRACT.replace("`NotFound` (THG-010), `NameTaken { holder }` (THG-011 — not `Gone`), `DatabaseError`", "`ThingError` codes, `NameTaken`")
        self.assertEqual(gaps(named), [])

    # FLOW-024 — arguments and the return type are those of the command.
    def test_other_arguments_or_another_return_type_are_gaps(self):
        renamed = CONTRACT.replace("`thing_id: String, new_name: String`", "`id: String, new_name: String`        ")
        self.assertEqual(gaps(renamed), ["thing: `rename_thing` takes (thing_id, new_name); the row gives (id, new_name)"])
        returned = CONTRACT.replace("| `Option<Thing>` |", "| `Thing`         |")
        self.assertEqual(gaps(returned), ["thing: `rename_thing` returns `Option<Thing>`; the row gives `Thing`"])

    # FLOW-024 — a struct or an enum of the contract has the generated fields and variants,
    # whatever the case the wire writes a variant in.
    def test_fields_and_variants_follow_the_generated_types(self):
        fewer = CONTRACT.replace("    weight: Option<i64>,\n", "    colour: String,\n")
        self.assertEqual(gaps(fewer), ["thing: `Thing` has `colour`, which the generated type has not", "thing: `Thing` lacks `weight`"])
        variant = CONTRACT.replace("enum Colour { Red, DarkBlue }", "enum Colour { Red }")
        self.assertEqual(gaps(variant), ["thing: `Colour` lacks `dark_blue`"])

    # FLOW-024 — a type a row names is defined by a contract.
    def test_a_named_type_no_contract_defines_is_a_gap(self):
        undefined = CONTRACT.replace("struct AddThingDTO { name: String, colour: Colour }\n", "")
        self.assertEqual(gaps(undefined), ["thing: `add_thing` names `AddThingDTO`, which no contract defines"])

    # FLOW-024 — the list only shrinks: a gap outside it fails, and so does a line of it
    # that is no longer a gap.
    def test_a_new_gap_and_a_fixed_line_both_fail(self):
        self.assertEqual(contract_check.verdict(["a", "b"], ["a"]), (["b"], []))
        self.assertEqual(contract_check.verdict(["a"], ["a", "c"]), ([], ["c"]))
        self.assertEqual(contract_check.verdict(["a"], ["a"]), ([], []))

    # FLOW-024 — the repository's own contracts differ from its bindings only where
    # contract-gaps.json says so.
    def test_the_contracts_of_the_repository_hold(self):
        found = contract_check.gaps(*contract_check.load())
        known = json.loads(contract_check.KNOWN.read_text(encoding="utf-8"))["gaps"]
        self.assertEqual(contract_check.verdict(found, known), ([], []))
        self.assertGreater(len(contract_check.load()[0].commands), 80)


if __name__ == "__main__":
    unittest.main()
