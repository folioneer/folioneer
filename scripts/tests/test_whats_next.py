"""Tests for scripts/whats-next.py (todo #052) — run with `just test-scripts`."""

import importlib.util
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "whats_next", Path(__file__).resolve().parents[1] / "whats-next.py"
)
whats_next = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(whats_next)

TODO = """# TODO

## Next

<!-- The human's queue. -->

1. TD-007
2. #003
3. #001
4. #099

## #001 — (frontend) — A ready entry

Text.

**User value:** Something.
**Done when:** a test proves it.
**Design:** none
**Open questions:** none

## #002 — (fullstack) — Waits on two answers

**User value:** Something.
**Done when:** a test proves it.
**Design:** none
**Open questions:**

- [x] Answered already? Yes.
- [ ] First open one?
- [ ] Second open one?

## #003 — (frontend) — Waits on its design

**User value:** Something.
**Done when:** a test proves it.
**Design:** proposed (screenshots/design/003-*.png)
**Open questions:** none

## #004 — (frontend) — Answered and validated

**User value:** Something.
**Done when:** a test proves it.
**Design:** validated
**Open questions:**

- [x] Answered? Yes.

## #005 — (tooling) — No acceptance yet

**User value:** Something.
**Design:** none
**Open questions:** none
"""

DEBT = """# Tech debt

## 2026-09-28 — TD-007 — Feature `accounts` still makes 3 business decision(s) in the interface

- Severity: 🟡
- User value: None directly.
- Done when: the count is zero.

## 2026-09-28 — TD-008 — Feature `assets` still makes 12 business decision(s) in the interface

- Severity: 🟡
- Done when: the count is zero.

## 2026-10-01 — TD-009 — The start-up waits thirty seconds

- Severity: 🔵
- Observation: no acceptance written.
"""


def refs(entries):
    return [entry["ref"] for entry in entries]


class Readiness(unittest.TestCase):
    def setUp(self):
        self.entries = {entry["ref"]: entry for entry in whats_next.parse_entries(TODO)}

    # #052 — ready: a Done when, no unticked question, a design none or validated.
    def test_an_entry_with_nothing_pending_is_ready(self):
        self.assertEqual(self.entries["#001"]["waits_on"], [])
        self.assertEqual(self.entries["#004"]["waits_on"], [])

    # #052 — a blocked entry names each unticked question, never a ticked one.
    def test_an_entry_waits_on_each_unticked_question(self):
        self.assertEqual(
            self.entries["#002"]["waits_on"],
            ["question: First open one?", "question: Second open one?"],
        )

    # #052 — a proposed design waits on the owner's approval.
    def test_a_proposed_design_waits_on_its_validation(self):
        self.assertEqual(
            self.entries["#003"]["waits_on"],
            ["design to validate: proposed (screenshots/design/003-*.png)"],
        )

    def test_an_entry_without_a_done_when_is_not_ready(self):
        self.assertEqual(self.entries["#005"]["waits_on"], ["no Done when"])

    # #052 — the rule fails closed: a missing line, or words other than the expected
    # ones, wait on the owner rather than read as ready.
    def test_anything_but_the_expected_words_waits(self):
        base = ["**Done when:** x", "**Design:** none", "**Open questions:** none"]

        def waits(replaced: str, by: list[str]) -> list[str]:
            return whats_next.readiness(
                [line for line in base if not line.startswith(replaced)] + by
            )

        self.assertEqual(whats_next.readiness(base), [])
        self.assertEqual(waits("**Open", []), ["no Open questions line"])
        self.assertEqual(
            waits("**Open", ["**Open questions:**"]), ["no Open questions line"]
        )
        self.assertEqual(
            waits("**Open", ["**Open questions:** which currency?"]),
            ["question: which currency?"],
        )
        self.assertEqual(
            waits("**Open", ["**Open questions:**", "", "* [ ] Starred?", "1. Bare?"]),
            ["question: Starred?", "question: Bare?"],
        )
        self.assertEqual(waits("**Design", []), ["no Design line"])
        self.assertEqual(
            waits("**Design", ["**Design:** none, but mocks wanted"]),
            ["design to validate: none, but mocks wanted"],
        )
        self.assertEqual(waits("**Design", ["**Design:** Validated."]), [])

    def test_the_queue_section_is_not_an_entry(self):
        self.assertEqual(sorted(self.entries), ["#001", "#002", "#003", "#004", "#005"])


class Queue(unittest.TestCase):
    # #052 — the queue is read in the owner's order, comments and prose ignored.
    def test_the_queue_keeps_the_owners_order(self):
        self.assertEqual(
            whats_next.parse_queue(TODO), ["TD-007", "#003", "#001", "#099"]
        )

    def test_a_file_without_a_queue_has_an_empty_one(self):
        self.assertEqual(whats_next.parse_queue("# TODO\n\n## #001 — (x) — y\n"), [])


class ThreeLists(unittest.TestCase):
    def setUp(self):
        entries = whats_next.parse_entries(TODO) + whats_next.parse_debt(DEBT)
        self.lists = whats_next.classify(whats_next.parse_queue(TODO), entries)

    # #052 — queued, in order, each with its state; a reference whose entry is gone
    # is reported closed rather than dropped.
    def test_queued_entries_carry_their_state_in_order(self):
        self.assertEqual(
            [(entry["ref"], entry["state"]) for entry in self.lists["queued"]],
            [
                ("TD-007", "ready"),
                ("#003", "blocked"),
                ("#001", "ready"),
                ("#099", "closed"),
            ],
        )

    # #052 — an entry appears in exactly one list.
    def test_ready_and_blocked_leave_out_what_is_queued(self):
        self.assertEqual(refs(self.lists["ready"]), ["#004", "TD-008"])
        self.assertEqual(refs(self.lists["blocked"]), ["#002", "#005", "TD-009"])


class DebtThemes(unittest.TestCase):
    # #052 — entries that differ only by a name or a count are one theme.
    def test_entries_differing_by_a_name_or_a_count_share_a_theme(self):
        groups = whats_next.group_debt(whats_next.parse_debt(DEBT))
        self.assertEqual(
            [group["refs"] for group in groups], [["TD-007", "TD-008"], ["TD-009"]]
        )

    # #052 — a figure inside a word (E2E) is part of the title, not a count.
    def test_a_figure_inside_a_word_is_kept(self):
        self.assertEqual(
            whats_next.debt_theme("Every E2E file of `x` waits 30 seconds"),
            "Every E2E file of `…` waits N seconds",
        )

    def test_a_debt_entry_without_a_done_when_is_not_ready(self):
        debt = {entry["ref"]: entry for entry in whats_next.parse_debt(DEBT)}
        self.assertEqual(debt["TD-007"]["waits_on"], [])
        self.assertEqual(debt["TD-009"]["waits_on"], ["no Done when"])


FLOW = """# Flow

## FLOW-001 — A proposal the agent can run

- Kind: quality
- Proposal: gate on the exit code.

## FLOW-002 — A verdict

- Kind: speed
- Verdict: **keep.**

## FLOW-003 — A proposal that is the owner's to decide

- Kind: human review
- Proposal: two blocks.
- Needs the owner: yes — it changes who is asked.

## 2026-09-12 — TD-015 — A debt entry that is about the flow

- Severity: 🔵
- Done when: the specs select by id.

## #049 — (tooling) — A todo entry that is about the flow

**User value:** None.
**Done when:** references are renamed.
**Design:** none
**Open questions:** none
"""


class FlowEntries(unittest.TestCase):
    # The flow file's own entries: a proposal is work, a verdict is a record, and one
    # that needs the owner waits on that decision.
    def test_a_proposal_is_work_and_a_verdict_is_not(self):
        flow = {entry["ref"]: entry for entry in whats_next.parse_flow(FLOW)}
        self.assertEqual(sorted(flow), ["FLOW-001", "FLOW-003"])
        self.assertEqual(flow["FLOW-001"]["waits_on"], [])
        self.assertEqual(flow["FLOW-003"]["waits_on"], ["the owner's decision"])

    # A proposal beside a verdict is still work; the owner's yes may be written in bold.
    def test_a_proposal_beside_a_verdict_is_work(self):
        text = (
            "## FLOW-009 — Both\n\n- Verdict: keep, with one change.\n"
            "- Proposal: the change.\n- Needs the owner: **Yes** — it is theirs.\n\n"
            "## FLOW-010 — No decision needed\n\n- Proposal: x.\n- Needs the owner: no\n"
        )
        flow = {
            entry["ref"]: entry["waits_on"] for entry in whats_next.parse_flow(text)
        }
        self.assertEqual(flow, {"FLOW-009": ["the owner's decision"], "FLOW-010": []})

    # A queued reference to a verdict — nothing to run — reads as closed, to remove.
    def test_a_queued_verdict_reads_as_closed(self):
        lists = whats_next.classify(["FLOW-002"], whats_next.parse_flow(FLOW))
        self.assertEqual(lists["queued"], [{"ref": "FLOW-002", "state": "closed"}])

    # An entry moved to the flow file keeps its reference and its readiness rule.
    def test_entries_moved_to_the_flow_file_keep_their_reference(self):
        self.assertEqual(refs(whats_next.parse_debt(FLOW)), ["TD-015"])
        self.assertEqual(refs(whats_next.parse_entries(FLOW)), ["#049"])

    # A flow reference can be queued like any other.
    def test_a_flow_reference_can_be_queued(self):
        queue = whats_next.parse_queue("## Next\n\n1. FLOW-001\n2. TD-015\n")
        self.assertEqual(queue, ["FLOW-001", "TD-015"])
        entries = whats_next.parse_flow(FLOW) + whats_next.parse_debt(FLOW)
        states = [
            entry["state"] for entry in whats_next.classify(queue, entries)["queued"]
        ]
        self.assertEqual(states, ["ready", "ready"])


class CiState(unittest.TestCase):
    # #052 — a pull request's checks read as one word; one failure outweighs the rest.
    def test_checks_reduce_to_one_word(self):
        success = {"conclusion": "SUCCESS", "status": "COMPLETED"}
        running = {"conclusion": "", "status": "IN_PROGRESS"}
        failure = {"conclusion": "FAILURE", "status": "COMPLETED"}
        skipped = {"conclusion": "SKIPPED", "status": "COMPLETED"}
        self.assertEqual(whats_next.ci_state([]), "none")
        self.assertEqual(whats_next.ci_state([success, skipped]), "green")
        self.assertEqual(whats_next.ci_state([success, running]), "running")
        self.assertEqual(whats_next.ci_state([running, failure]), "failing")
        self.assertEqual(
            whats_next.ci_state([{"state": "SUCCESS"}, {"conclusion": "NEUTRAL"}]),
            "green",
        )
        self.assertEqual(whats_next.ci_state([{"state": "PENDING"}]), "running")
        for conclusion in ("ACTION_REQUIRED", "STARTUP_FAILURE", "STALE", "TIMED_OUT"):
            self.assertEqual(
                whats_next.ci_state([success, {"conclusion": conclusion}]), "failing"
            )


if __name__ == "__main__":
    unittest.main()
