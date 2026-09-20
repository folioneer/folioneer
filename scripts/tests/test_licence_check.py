"""Tests for scripts/licence-check.py (todo #031) — run with `just test-scripts`."""

import importlib.util
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "licence_check", Path(__file__).resolve().parents[1] / "licence-check.py"
)
licence_check = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(licence_check)

ALLOWED = {"MIT", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "ISC"}


def cargo_metadata(packages, edges, root="app 1.0.0"):
    """A `cargo metadata` document: packages as (id, name, licence), edges as (from, to, kinds)."""
    nodes = {}
    for source, target, kinds in edges:
        nodes.setdefault(source, []).append(
            {"pkg": target, "dep_kinds": [{"kind": kind} for kind in kinds]}
        )
    return {
        "packages": [{"id": pid, "name": name, "license": licence} for pid, name, licence in packages],
        "workspace_members": [root],
        "resolve": {
            "root": root,
            "nodes": [{"id": pid, "deps": nodes.get(pid, [])} for pid, _, _ in packages],
        },
    }


class Expressions(unittest.TestCase):
    def test_a_single_allowed_licence_passes(self):
        self.assertTrue(licence_check.is_allowed("MIT", ALLOWED))

    def test_a_licence_outside_the_list_fails(self):
        self.assertFalse(licence_check.is_allowed("GPL-3.0-or-later", ALLOWED))

    def test_one_allowed_choice_among_several_is_enough(self):
        self.assertTrue(licence_check.is_allowed("MIT OR Apache-2.0 OR LGPL-2.1-or-later", ALLOWED))
        self.assertTrue(licence_check.is_allowed("GPL-2.0 OR MIT", ALLOWED))

    def test_every_licence_of_a_conjunction_must_be_allowed(self):
        self.assertTrue(licence_check.is_allowed("MIT AND ISC", ALLOWED))
        self.assertFalse(licence_check.is_allowed("MIT AND GPL-3.0-only", ALLOWED))

    def test_and_binds_tighter_than_or_and_parentheses_group(self):
        self.assertTrue(licence_check.is_allowed("GPL-3.0-only AND MIT OR ISC", ALLOWED))
        self.assertFalse(licence_check.is_allowed("(GPL-3.0-only OR MIT) AND LGPL-3.0-only", ALLOWED))
        self.assertTrue(licence_check.is_allowed("(GPL-3.0-only OR MIT) AND (ISC OR MPL-2.0)", ALLOWED))

    def test_an_exception_clause_is_part_of_the_licence_it_qualifies(self):
        self.assertTrue(licence_check.is_allowed("Apache-2.0 WITH LLVM-exception", ALLOWED))
        self.assertFalse(licence_check.is_allowed("GPL-2.0-only WITH Classpath-exception-2.0", ALLOWED))

    def test_the_old_slash_separator_reads_as_or(self):
        self.assertTrue(licence_check.is_allowed("MIT/Apache-2.0", ALLOWED))
        self.assertFalse(licence_check.is_allowed("GPL-2.0/LGPL-2.1", ALLOWED))

    def test_a_missing_or_unreadable_licence_fails(self):
        for expression in (None, "", "UNLICENSED", "SEE LICENSE IN licence.txt", "MIT OR", "(MIT"):
            self.assertFalse(licence_check.is_allowed(expression, ALLOWED), expression)


class CargoScope(unittest.TestCase):
    def test_what_ships_excludes_the_application_itself_and_development_only_crates(self):
        metadata = cargo_metadata(
            [
                ("app 1.0.0", "app", "AGPL-3.0-or-later"),
                ("serde 1.0.0", "serde", "MIT OR Apache-2.0"),
                ("itoa 1.0.0", "itoa", "MIT"),
                ("mockall 0.13.0", "mockall", "GPL-3.0-only"),
                ("cc 1.0.0", "cc", "MIT"),
            ],
            [
                ("app 1.0.0", "serde 1.0.0", [None]),
                ("serde 1.0.0", "itoa 1.0.0", [None]),
                ("app 1.0.0", "mockall 0.13.0", ["dev"]),
                ("app 1.0.0", "cc 1.0.0", ["build"]),
            ],
        )
        shipped = licence_check.cargo_shipped(metadata)
        self.assertEqual(sorted(name for name, _ in shipped), ["cc", "itoa", "serde"])

    def test_a_crate_reached_both_ways_ships(self):
        metadata = cargo_metadata(
            [("app 1.0.0", "app", None), ("tokio 1.0.0", "tokio", "MIT")],
            [("app 1.0.0", "tokio 1.0.0", ["dev", None])],
        )
        self.assertEqual(licence_check.cargo_shipped(metadata), [("tokio", "MIT")])


class NpmScope(unittest.TestCase):
    def test_what_ships_excludes_the_application_itself_and_development_only_packages(self):
        lock = {
            "packages": {
                "": {"name": "app", "license": "AGPL-3.0-or-later"},
                "node_modules/react": {"license": "MIT"},
                "node_modules/@scope/pkg": {"license": "ISC"},
                "node_modules/react/node_modules/nested": {"license": "Apache-2.0"},
                "node_modules/vitest": {"license": "GPL-3.0", "dev": True},
                "node_modules/fsevents": {"license": "MIT", "devOptional": True},
            }
        }
        self.assertEqual(
            licence_check.npm_shipped(lock),
            [("@scope/pkg", "ISC"), ("fsevents", "MIT"), ("nested", "Apache-2.0"), ("react", "MIT")],
        )


class Verdict(unittest.TestCase):
    def test_a_licence_outside_the_list_is_reported_with_its_package(self):
        problems = licence_check.check("cargo", [("readline", "GPL-3.0-or-later")], ALLOWED, {})
        self.assertEqual(len(problems), 1)
        self.assertIn("readline", problems[0])
        self.assertIn("GPL-3.0-or-later", problems[0])

    def test_a_named_exception_passes_only_under_the_licence_it_was_accepted_with(self):
        exceptions = {"cssparser": "MPL-2.0"}
        self.assertEqual(licence_check.check("cargo", [("cssparser", "MPL-2.0")], ALLOWED, exceptions), [])
        changed = licence_check.check("cargo", [("cssparser", "GPL-3.0-only")], ALLOWED, exceptions)
        self.assertEqual(len(changed), 1)
        self.assertIn("accepted by name under MPL-2.0", changed[0])

    def test_another_package_under_an_excepted_licence_still_fails(self):
        shipped = [("cssparser", "MPL-2.0"), ("selectors", "MPL-2.0")]
        problems = licence_check.check("cargo", shipped, ALLOWED, {"cssparser": "MPL-2.0"})
        self.assertEqual(len(problems), 1)
        self.assertIn("selectors", problems[0])

    def test_an_exception_nothing_uses_any_more_is_reported(self):
        problems = licence_check.check("cargo", [("serde", "MIT")], ALLOWED, {"cssparser": "MPL-2.0"})
        self.assertEqual(len(problems), 1)
        self.assertIn("cssparser", problems[0])


if __name__ == "__main__":
    unittest.main()
