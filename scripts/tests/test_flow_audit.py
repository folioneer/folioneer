"""Tests for scripts/flow-audit.py (FLOW-018) — run with `just test-scripts`."""

import importlib.util
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "flow_audit", Path(__file__).resolve().parents[1] / "flow-audit.py"
)
flow_audit = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(flow_audit)

SINCE = flow_audit.parse_moment("2026-10-03T22:00:00Z")
UNTIL = flow_audit.parse_moment("2026-10-05T00:00:00Z")


def pull_request(
    number, title, created, merged=None, closed=None, additions=10, branch=None
):
    return {
        "number": number,
        "title": title,
        "createdAt": created,
        "mergedAt": merged,
        "closedAt": closed or merged,
        "additions": additions,
        "headRefName": branch or f"branch-{number}",
    }


def run(workflow, branch, sha, conclusion, started, updated, event="pull_request"):
    return {
        "workflowName": workflow,
        "headBranch": branch,
        "headSha": sha,
        "conclusion": conclusion,
        "event": event,
        "createdAt": started,
        "startedAt": started,
        "updatedAt": updated,
    }


PULL_REQUESTS = [
    # Merged before the window: not counted.
    pull_request(
        90, "fix: before", "2026-10-03T20:00:00Z", merged="2026-10-03T21:00:00Z"
    ),
    pull_request(
        101, "feat: one", "2026-10-04T08:00:00Z", merged="2026-10-04T08:10:00Z"
    ),
    pull_request(
        102,
        "fix: two",
        "2026-10-04T09:00:00Z",
        merged="2026-10-04T09:30:00Z",
        additions=40,
    ),
    pull_request(
        103,
        "Docs without a type",
        "2026-10-04T10:00:00Z",
        merged="2026-10-04T11:20:00Z",
    ),
    # Closed in the window without merging.
    pull_request(
        104, "docs: redone", "2026-10-04T12:00:00Z", closed="2026-10-04T12:30:00Z"
    ),
    # Still open.
    pull_request(105, "feat: open", "2026-10-04T13:00:00Z", closed=None),
]

RUNS = [
    # Branch 101: one round, green.
    run(
        "Quality",
        "branch-101",
        "a1",
        "success",
        "2026-10-04T08:01:00Z",
        "2026-10-04T08:04:00Z",
    ),
    run(
        "E2E",
        "branch-101",
        "a1",
        "success",
        "2026-10-04T08:01:00Z",
        "2026-10-04T08:06:00Z",
    ),
    # Branch 102: two rounds, the first red on review.
    run(
        "Quality",
        "branch-102",
        "b1",
        "success",
        "2026-10-04T09:01:00Z",
        "2026-10-04T09:06:00Z",
    ),
    run(
        "Review",
        "branch-102",
        "b1",
        "failure",
        "2026-10-04T09:01:00Z",
        "2026-10-04T09:03:00Z",
    ),
    run(
        "Quality",
        "branch-102",
        "b2",
        "success",
        "2026-10-04T09:15:00Z",
        "2026-10-04T09:19:00Z",
    ),
    run(
        "Review",
        "branch-102",
        "b2",
        "success",
        "2026-10-04T09:15:00Z",
        "2026-10-04T09:17:00Z",
    ),
    # Not a pull request's run, and a branch outside the window: neither is counted.
    run(
        "Quality",
        "main",
        "m1",
        "failure",
        "2026-10-04T09:40:00Z",
        "2026-10-04T09:44:00Z",
        "push",
    ),
    run(
        "Quality",
        "branch-90",
        "z1",
        "failure",
        "2026-10-04T09:40:00Z",
        "2026-10-04T09:44:00Z",
    ),
]


class MeasurePullRequestsTest(unittest.TestCase):
    def setUp(self):
        self.figures = flow_audit.measure_pull_requests(PULL_REQUESTS, SINCE, UNTIL)

    def test_counts_what_merged_in_the_window(self):
        self.assertEqual(self.figures["merged"], 3)
        self.assertEqual((self.figures["first"], self.figures["last"]), (101, 103))
        self.assertEqual(self.figures["lines_added"], 60)

    def test_lists_what_was_closed_without_merging(self):
        self.assertEqual(self.figures["closed_without_merging"], [104])

    def test_times_each_pull_request_from_opening_to_merging(self):
        # 10, 30 and 80 minutes.
        self.assertEqual(self.figures["mean_minutes_to_merge"], 40.0)
        self.assertEqual(self.figures["median_minutes_to_merge"], 30.0)

    def test_groups_by_the_type_the_title_starts_with(self):
        self.assertEqual(self.figures["by_type"], {"feat": 1, "fix": 1, "other": 1})

    def test_an_empty_window_has_no_time_to_state(self):
        empty = flow_audit.measure_pull_requests([], SINCE, UNTIL)
        self.assertEqual(empty["merged"], 0)
        self.assertIsNone(empty["mean_minutes_to_merge"])
        self.assertIsNone(empty["first"])


class MeasureCiTest(unittest.TestCase):
    def setUp(self):
        self.figures = flow_audit.measure_ci(RUNS, PULL_REQUESTS, SINCE, UNTIL)

    def test_a_round_is_one_pushed_commit_of_a_branch(self):
        self.assertEqual(self.figures["rounds"], 3)
        self.assertEqual(self.figures["branches"], 2)
        self.assertEqual(self.figures["rounds_beyond_the_first"], 1)

    def test_failures_are_counted_by_workflow_on_the_window_s_branches_only(self):
        self.assertEqual(self.figures["failures_by_workflow"], {"Review": 1})

    def test_a_workflow_s_duration_is_the_median_of_its_green_runs(self):
        # Quality: 3, 5 and 4 minutes; Review's red run is left out.
        self.assertEqual(
            self.figures["median_minutes_by_workflow"],
            {"E2E": 5.0, "Quality": 4.0, "Review": 2.0},
        )


class CommitTypeTest(unittest.TestCase):
    def test_reads_the_conventional_type(self):
        self.assertEqual(flow_audit.commit_type("refactor: a thing"), "refactor")
        self.assertEqual(flow_audit.commit_type("A title: with a colon"), "other")
        self.assertEqual(flow_audit.commit_type("note: not a type"), "other")
        self.assertEqual(flow_audit.commit_type("no type"), "other")


class MeasureTest(unittest.TestCase):
    def test_a_window_carries_its_bounds_and_both_sets_of_figures(self):
        report = flow_audit.measure(PULL_REQUESTS, RUNS, SINCE, UNTIL)
        self.assertEqual(report["since"], "2026-10-03T22:00:00+00:00")
        self.assertEqual(report["pull_requests"]["merged"], 3)
        self.assertEqual(report["ci"]["rounds"], 3)

    def test_the_window_before_counts_what_this_one_leaves_out(self):
        before = flow_audit.measure(
            PULL_REQUESTS, RUNS, flow_audit.parse_moment("2026-10-01T00:00:00Z"), SINCE
        )
        self.assertEqual(before["pull_requests"]["merged"], 1)
        self.assertEqual(before["pull_requests"]["first"], 90)


class WindowBoundsTest(unittest.TestCase):
    def test_the_window_opens_after_its_start_and_closes_on_its_end(self):
        at_start = pull_request(
            1, "fix: a", "2026-10-03T21:00:00Z", merged="2026-10-03T22:00:00Z"
        )
        at_end = pull_request(
            2, "fix: b", "2026-10-04T23:00:00Z", merged="2026-10-05T00:00:00Z"
        )
        figures = flow_audit.measure_pull_requests([at_start, at_end], SINCE, UNTIL)
        self.assertEqual((figures["merged"], figures["first"]), (1, 2))

    def test_a_run_outside_the_window_is_left_out_even_on_a_counted_branch(self):
        early = run(
            "Quality",
            "branch-101",
            "a0",
            "failure",
            "2026-10-03T21:00:00Z",
            "2026-10-03T21:05:00Z",
        )
        figures = flow_audit.measure_ci([*RUNS, early], PULL_REQUESTS, SINCE, UNTIL)
        self.assertEqual(figures["rounds"], 3)
        self.assertEqual(figures["failures_by_workflow"], {"Review": 1})


class TruncatedTest(unittest.TestCase):
    def test_a_full_list_that_starts_too_late_is_reported(self):
        late = [{"createdAt": "2026-10-04T00:00:00Z"}] * flow_audit.RUN_LIMIT
        self.assertEqual(flow_audit.truncated([], late, SINCE), ["workflow runs"])

    def test_a_list_that_reaches_back_far_enough_is_not(self):
        full = [{"createdAt": "2026-10-01T00:00:00Z"}] * flow_audit.RUN_LIMIT
        self.assertEqual(flow_audit.truncated([], full, SINCE), [])
        short = [{"createdAt": "2026-10-04T00:00:00Z"}]
        self.assertEqual(flow_audit.truncated([], short, SINCE), [])


if __name__ == "__main__":
    unittest.main()
