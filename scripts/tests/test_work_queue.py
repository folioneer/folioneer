"""Tests for scripts/work-queue.py (todo TODO-052) — run with `just test-scripts`."""

import importlib.util
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "work_queue", Path(__file__).resolve().parents[1] / "work-queue.py"
)
work_queue = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(work_queue)

QUEUE = """# Queue

Owned by the human.

<!-- The human's queue: one reference per line. -->
<!-- A reference in a comment queues nothing:
- TODO-005
-->

- DEBT-007
- TODO-003
- TODO-001
- TODO-099
"""

TODO = """# TODO-001 — (frontend) — A ready entry

Text.

**User value:** Something.
**Done when:** a test proves it.
**Design:** none
**Open questions:** none

# TODO-002 — (fullstack) — Waits on two answers

**User value:** Something.
**Done when:** a test proves it.
**Design:** none
**Open questions:**

- [x] Answered already? Yes.
- [ ] First open one?
- [ ] Second open one?

# TODO-003 — (frontend) — Waits on its design

**User value:** Something.
**Done when:** a test proves it.
**Design:** proposed (screenshots/design/003-*.png)
**Open questions:** none

# TODO-004 — (frontend) — Answered and validated

**User value:** Something.
**Done when:** a test proves it.
**Design:** validated
**Open questions:**

- [x] Answered? Yes.

# TODO-005 — (tooling) — No acceptance yet

**User value:** Something.
**Design:** none
**Open questions:** none
"""

DEBT = """# 2026-09-28 — DEBT-007 — Feature `accounts` still makes 3 business decision(s) in the interface

- Severity: 🟡
- User value: None directly.
- Done when: the count is zero.

# 2026-09-28 — DEBT-008 — Feature `assets` still makes 12 business decision(s) in the interface

- Severity: 🟡
- Done when: the count is zero.

# 2026-10-01 — DEBT-009 — The start-up waits thirty seconds

- Severity: 🔵
- Observation: no acceptance written.
"""


def refs(entries):
    return [entry["ref"] for entry in entries]


class Readiness(unittest.TestCase):
    def setUp(self):
        self.entries = {entry["ref"]: entry for entry in work_queue.parse_entries(TODO)}

    # TODO-052 — ready: a Done when, no unticked question, a design none or validated.
    def test_an_entry_with_nothing_pending_is_ready(self):
        self.assertEqual(self.entries["TODO-001"]["waits_on"], [])
        self.assertEqual(self.entries["TODO-004"]["waits_on"], [])

    # TODO-052 — a blocked entry names each unticked question, never a ticked one.
    def test_an_entry_waits_on_each_unticked_question(self):
        self.assertEqual(
            self.entries["TODO-002"]["waits_on"],
            ["question: First open one?", "question: Second open one?"],
        )

    # TODO-052 — a proposed design waits on the owner's approval.
    def test_a_proposed_design_waits_on_its_validation(self):
        self.assertEqual(
            self.entries["TODO-003"]["waits_on"],
            ["design to validate: proposed (screenshots/design/003-*.png)"],
        )

    def test_an_entry_without_a_done_when_is_not_ready(self):
        self.assertEqual(self.entries["TODO-005"]["waits_on"], ["no Done when"])

    # TODO-052 — the rule fails closed: a missing line, or words other than the expected
    # ones, wait on the owner rather than read as ready.
    def test_anything_but_the_expected_words_waits(self):
        base = ["**Done when:** x", "**Design:** none", "**Open questions:** none"]

        def waits(replaced: str, by: list[str]) -> list[str]:
            return work_queue.readiness(
                [line for line in base if not line.startswith(replaced)] + by
            )

        self.assertEqual(work_queue.readiness(base), [])
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

    def test_every_entry_file_is_read(self):
        self.assertEqual(sorted(self.entries), ["TODO-001", "TODO-002", "TODO-003", "TODO-004", "TODO-005"])


class Queue(unittest.TestCase):
    # TODO-052 — the queue is read in the owner's order, comments and prose ignored.
    def test_the_queue_keeps_the_owners_order(self):
        self.assertEqual(
            work_queue.parse_queue(QUEUE), ["DEBT-007", "TODO-003", "TODO-001", "TODO-099"]
        )

    def test_a_missing_or_empty_queue_file_queues_nothing(self):
        self.assertEqual(work_queue.parse_queue(""), [])
        self.assertEqual(work_queue.parse_queue("# Queue\n\nOwned by the human.\n"), [])

    # A numbered item is still read, so a hand-edited queue is never silently empty.
    def test_a_numbered_item_is_read_too(self):
        self.assertEqual(work_queue.parse_queue("1. FLOW-001\n2. DEBT-015\n"), ["FLOW-001", "DEBT-015"])

    # What is left to work: a queued reference whose entry file is gone has shipped.
    def test_what_remains_leaves_out_what_shipped(self):
        entries = work_queue.parse_entries(TODO) + work_queue.parse_debt(DEBT)
        queued = work_queue.classify(work_queue.parse_queue(QUEUE), entries)["queued"]
        self.assertEqual(work_queue.remaining(queued), ["DEBT-007", "TODO-003", "TODO-001"])


class ThreeLists(unittest.TestCase):
    def setUp(self):
        entries = work_queue.parse_entries(TODO) + work_queue.parse_debt(DEBT)
        self.lists = work_queue.classify(work_queue.parse_queue(QUEUE), entries)

    # TODO-052 — queued, in order, each with its state; a reference whose entry file is
    # gone has shipped: it is reported closed, and the queue is left as written.
    def test_queued_entries_carry_their_state_in_order(self):
        self.assertEqual(
            [(entry["ref"], entry["state"]) for entry in self.lists["queued"]],
            [
                ("DEBT-007", "ready"),
                ("TODO-003", "blocked"),
                ("TODO-001", "ready"),
                ("TODO-099", "closed"),
            ],
        )

    # A queued reference that is not an id is unknown, never shipped: it stays in what
    # remains, so a headless run does not report an empty queue.
    def test_a_reference_that_is_not_an_id_is_unknown_and_remains(self):
        queued = work_queue.classify(work_queue.parse_queue("- TODO-0599\n- TODO-099\n"), [])["queued"]
        self.assertEqual(
            queued, [{"ref": "TODO-0599", "state": "unknown"}, {"ref": "TODO-099", "state": "closed"}]
        )
        self.assertEqual(work_queue.remaining(queued), ["TODO-0599"])

    # TODO-052 — an entry appears in exactly one list.
    def test_ready_and_blocked_leave_out_what_is_queued(self):
        self.assertEqual(refs(self.lists["ready"]), ["TODO-004", "DEBT-008"])
        self.assertEqual(refs(self.lists["blocked"]), ["TODO-002", "TODO-005", "DEBT-009"])


class DebtThemes(unittest.TestCase):
    # TODO-052 — entries that differ only by a name or a count are one theme.
    def test_entries_differing_by_a_name_or_a_count_share_a_theme(self):
        groups = work_queue.group_debt(work_queue.parse_debt(DEBT))
        self.assertEqual(
            [group["refs"] for group in groups], [["DEBT-007", "DEBT-008"], ["DEBT-009"]]
        )

    # TODO-052 — a figure inside a word (E2E) is part of the title, not a count.
    def test_a_figure_inside_a_word_is_kept(self):
        self.assertEqual(
            work_queue.debt_theme("Every E2E file of `x` waits 30 seconds"),
            "Every E2E file of `…` waits N seconds",
        )

    def test_a_debt_entry_waits_on_its_unticked_questions(self):
        debt = work_queue.parse_debt(
            """# Tech Debt

# 2026-10-04 — DEBT-078 — A promise nothing keeps

- Severity: 🟡
- Done when: the promise is kept or withdrawn.

**Open questions:**

- [ ] Build it now, or withdraw the promise?
- [x] Is it both paths? → Yes.

# 2026-10-05 — DEBT-081 — Another entry

- Done when: fixed.
"""
        )
        self.assertEqual(
            debt[0]["waits_on"], ["question: Build it now, or withdraw the promise?"]
        )
        self.assertEqual(debt[1]["waits_on"], [])

    def test_a_debt_entry_without_a_done_when_is_not_ready(self):
        debt = {entry["ref"]: entry for entry in work_queue.parse_debt(DEBT)}
        self.assertEqual(debt["DEBT-007"]["waits_on"], [])
        self.assertEqual(debt["DEBT-009"]["waits_on"], ["no Done when"])


FLOW = """# FLOW-001 — A proposal the agent can run

- Kind: quality
- Proposal: gate on the exit code.

# FLOW-002 — A verdict

- Kind: speed
- Verdict: **keep.**

# FLOW-003 — A proposal that is the owner's to decide

- Kind: human review
- Proposal: two blocks.
- Needs the owner: yes — it changes who is asked.

# 2026-09-12 — DEBT-015 — A debt entry that is about the flow

- Severity: 🔵
- Done when: the specs select by id.

# TODO-049 — (tooling) — A todo entry that is about the flow

**User value:** None.
**Done when:** references are renamed.
**Design:** none
**Open questions:** none
"""


class FlowEntries(unittest.TestCase):
    # The flow file's own entries: a proposal is work, a verdict is a record, and one
    # that needs the owner waits on that decision.
    def test_a_proposal_is_work_and_a_verdict_is_not(self):
        flow = {entry["ref"]: entry for entry in work_queue.parse_flow(FLOW)}
        self.assertEqual(sorted(flow), ["FLOW-001", "FLOW-003"])
        self.assertEqual(flow["FLOW-001"]["waits_on"], [])
        self.assertEqual(flow["FLOW-003"]["waits_on"], ["the owner's decision"])

    # A proposal beside a verdict is still work; the owner's yes may be written in bold.
    def test_a_proposal_beside_a_verdict_is_work(self):
        text = (
            "# FLOW-009 — Both\n\n- Verdict: keep, with one change.\n"
            "- Proposal: the change.\n- Needs the owner: **Yes** — it is theirs.\n\n"
            "# FLOW-010 — No decision needed\n\n- Proposal: x.\n- Needs the owner: no\n"
        )
        flow = {
            entry["ref"]: entry["waits_on"] for entry in work_queue.parse_flow(text)
        }
        self.assertEqual(flow, {"FLOW-009": ["the owner's decision"], "FLOW-010": []})

    # A queued reference to a verdict — nothing to run — reads as closed.
    def test_a_queued_verdict_reads_as_closed(self):
        lists = work_queue.classify(["FLOW-002"], work_queue.parse_flow(FLOW))
        self.assertEqual(lists["queued"], [{"ref": "FLOW-002", "state": "closed"}])

    # Each kind is read by its own heading: a file of another kind is not an entry of it.
    def test_an_entry_is_read_by_the_heading_of_its_kind(self):
        self.assertEqual(refs(work_queue.parse_debt(FLOW)), ["DEBT-015"])
        self.assertEqual(refs(work_queue.parse_entries(FLOW)), ["TODO-049"])

    # A flow reference can be queued like any other.
    def test_a_flow_reference_can_be_queued(self):
        queue = work_queue.parse_queue("- FLOW-001\n- DEBT-015\n")
        self.assertEqual(queue, ["FLOW-001", "DEBT-015"])
        entries = work_queue.parse_flow(FLOW) + work_queue.parse_debt(FLOW)
        states = [
            entry["state"] for entry in work_queue.classify(queue, entries)["queued"]
        ]
        self.assertEqual(states, ["ready", "ready"])


class Closure(unittest.TestCase):
    """An entry that ships is deleted; the queue is not edited."""

    def test_closing_names_the_file_of_each_entry(self):
        self.assertEqual(
            work_queue.close(["TODO-003", "DEBT-007", "FLOW-001"], exists=lambda path: True),
            [
                "docs/work/todo/TODO-003.md",
                "docs/work/debt/DEBT-007.md",
                "docs/work/flow/FLOW-001.md",
            ],
        )

    def test_nothing_is_closed_unless_every_reference_names_an_entry(self):
        gone = lambda path: path != "docs/work/todo/TODO-099.md"
        for refs_given in (["TODO-003", "TODO-099"], ["TODO-003", "gh12"], ["TODO-003", "TODO-003"]):
            with self.assertRaises(work_queue.NoSuchEntry, msg=refs_given):
                work_queue.close(refs_given, exists=gone)


class NextId(unittest.TestCase):
    """Ids are permanent and never reused, counting the entries already deleted."""

    def test_the_next_id_follows_every_name_the_kind_has_had(self):
        floor = work_queue.ID_FLOORS["DEBT"]
        names = [f"DEBT-{floor + 2:03d}.md", f"DEBT-{floor + 5:03d}.md", "DEBT-001.md", "notes.md"]
        self.assertEqual(work_queue.next_id("DEBT", names), f"DEBT-{floor + 6:03d}")

    def test_a_kind_with_no_file_yet_starts_after_its_floor(self):
        self.assertEqual(
            work_queue.next_id("FLOW", []), f"FLOW-{work_queue.ID_FLOORS['FLOW'] + 1:03d}"
        )

    def test_another_kinds_files_do_not_count(self):
        floor = work_queue.ID_FLOORS["TODO"]
        self.assertEqual(work_queue.next_id("TODO", ["DEBT-999.md"]), f"TODO-{floor + 1:03d}")


class CiState(unittest.TestCase):
    # TODO-052 — a pull request's checks read as one word; one failure outweighs the rest.
    def test_checks_reduce_to_one_word(self):
        success = {"conclusion": "SUCCESS", "status": "COMPLETED"}
        running = {"conclusion": "", "status": "IN_PROGRESS"}
        failure = {"conclusion": "FAILURE", "status": "COMPLETED"}
        skipped = {"conclusion": "SKIPPED", "status": "COMPLETED"}
        self.assertEqual(work_queue.ci_state([]), "none")
        self.assertEqual(work_queue.ci_state([success, skipped]), "green")
        self.assertEqual(work_queue.ci_state([success, running]), "running")
        self.assertEqual(work_queue.ci_state([running, failure]), "failing")
        self.assertEqual(
            work_queue.ci_state([{"state": "SUCCESS"}, {"conclusion": "NEUTRAL"}]),
            "green",
        )
        self.assertEqual(work_queue.ci_state([{"state": "PENDING"}]), "running")
        for conclusion in ("ACTION_REQUIRED", "STARTUP_FAILURE", "STALE", "TIMED_OUT"):
            self.assertEqual(
                work_queue.ci_state([success, {"conclusion": conclusion}]), "failing"
            )


ISSUE_DOCS = [
    """# TODO-003 — (ci) — Named commands for the reviewers

Asked in gh79 and again in https://github.com/o/r/issues/12.
""",
    """# 2026-10-04 — DEBT-077 — Twenty-eight unnoticed changes

- Done when: each is sorted; issue 65 is closed.

# 2026-10-04 — DEBT-079 — Unrelated

- Observation: figure 790, gh7 and issues/650 are other numbers.
""",
    """# FLOW-020 — Nobody counts why a pull request goes round again

- Observed: see gh79.
""",
]


class IssuesAndTheirEntries(unittest.TestCase):
    def test_an_issue_is_found_under_each_way_of_naming_it(self):
        self.assertEqual(work_queue.entries_naming(79, ISSUE_DOCS), ["TODO-003", "FLOW-020"])
        self.assertEqual(work_queue.entries_naming(12, ISSUE_DOCS), ["TODO-003"])
        self.assertEqual(work_queue.entries_naming(65, ISSUE_DOCS), ["DEBT-077"])

    def test_another_number_that_starts_or_ends_the_same_is_not_the_issue(self):
        self.assertEqual(work_queue.entries_naming(7, ISSUE_DOCS), ["DEBT-079"])
        self.assertEqual(work_queue.entries_naming(6, ISSUE_DOCS), [])
        self.assertEqual(work_queue.entries_naming(790, ISSUE_DOCS), [])

    def test_an_issue_no_entry_names_has_none(self):
        self.assertEqual(work_queue.entries_naming(42, ISSUE_DOCS), [])
        self.assertEqual(work_queue.entries_naming(79, []), [])


class SkillProposal(unittest.TestCase):
    """The skill's own example is what the agent copies: it must show what it is told to
    propose."""

    SKILL = (
        Path(__file__).resolve().parents[2] / ".claude" / "skills" / "plan-queue" / "SKILL.md"
    ).read_text()

    def proposal_example(self):
        block = self.SKILL.split("### Proposed queue", 1)[1].split("```", 1)[0]
        return [line for line in block.splitlines() if line.strip()]

    def test_the_example_proposes_a_flow_entry_and_an_untracked_issue(self):
        # The example is the skill file's of this commit: an issue reads `gh{n}` there.
        example = self.proposal_example()
        self.assertTrue(any("FLOW-" in line for line in example), example)
        self.assertTrue(
            any("gh{n}" in line and "to file or close" in line for line in example), example
        )

    def test_the_example_stops_at_ten_lines_as_the_ordering_step_says(self):
        numbers = [
            int(line.split(".", 1)[0]) for line in self.proposal_example() if line[0].isdigit()
        ]
        self.assertEqual(max(numbers), 10)
        ordering = self.SKILL.split("### Step 2", 1)[1].split("### Step 3", 1)[0]
        self.assertIn("ten lines at most", ordering)
        self.assertIn("flow entr", ordering)
        self.assertIn("to file or close", ordering)

    def test_the_issues_section_says_which_entries_name_each_issue(self):
        issues = self.SKILL.split("### GitHub issues", 1)[1].split("###", 1)[0]
        self.assertIn("no entry", issues)


if __name__ == "__main__":
    unittest.main()
