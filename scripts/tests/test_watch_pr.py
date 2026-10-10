"""Tests for scripts/watch-pr.py (FLOW-027) — run with `just test-scripts`."""

import importlib.util
import sys
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location("watch_pr", Path(__file__).resolve().parents[1] / "watch-pr.py")
watch_pr = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = watch_pr  # dataclasses resolve their module by name
SPEC.loader.exec_module(watch_pr)

Run = watch_pr.Run
SHA = "a" * 40


def run(name, conclusion="success", status="completed", run_id=1, completed_at="2026-10-04T12:00:00Z"):
    done = status == "completed"
    return Run(run_id, name, status, conclusion if done else "", f"https://x/{run_id}", completed_at if done else "")


class Clock:
    """A clock the sleeps move, so a watch runs to its end without waiting."""

    def __init__(self):
        self.now = 0.0

    def __call__(self):
        return self.now

    def sleep(self, seconds):
        self.now += seconds


class Latest(unittest.TestCase):
    def test_a_re_run_replaces_the_run_of_the_same_name(self):
        runs = [run("Backend", "failure", run_id=1), run("Backend", "success", run_id=7), run("E2E", run_id=2)]
        self.assertEqual({r.name: r.conclusion for r in watch_pr.latest(runs)}, {"Backend": "success", "E2E": "success"})


class Verdict(unittest.TestCase):
    def test_a_missing_required_check_keeps_the_watch_open(self):
        self.assertEqual(watch_pr.verdict([run("Backend")], ["Backend", "E2E"]), ("running", ["E2E"]))

    def test_every_check_passed_or_skipped_is_green(self):
        runs = [run("Backend"), run("reviewer-sql", "skipped"), run("codecov", "neutral")]
        self.assertEqual(watch_pr.verdict(runs, ["Backend"]), ("green", []))

    def test_a_finished_check_that_did_not_pass_fails_once_all_are_done(self):
        runs = [run("Backend", "failure"), run("E2E", status="in_progress")]
        self.assertEqual(watch_pr.verdict(runs, []), ("running", ["E2E"]))
        runs = [run("Backend", "failure"), run("E2E", "cancelled"), run("Frontend")]
        self.assertEqual(watch_pr.verdict(runs, []), ("failing", ["Backend", "E2E"]))

    def test_a_check_in_progress_with_a_completion_time_is_stuck(self):
        stuck = Run(1, "E2E", "in_progress", "", "https://x/1", "2026-10-04T12:00:00Z")
        self.assertTrue(stuck.stuck)
        self.assertFalse(run("E2E", status="in_progress").stuck)


class Watch(unittest.TestCase):
    def watch(self, heads, answers, required=("Backend",), **options):
        """Run a watch over scripted answers: each is a list of runs or an error to raise."""
        self.out = []
        clock = Clock()
        answers = iter(answers)
        heads = iter(heads)
        last_head = [SHA]

        def read_head():
            last_head[0] = next(heads, last_head[0])
            return last_head[0]

        def read_runs(sha):
            answer = next(answers)
            if isinstance(answer, Exception):
                raise answer
            return answer

        return watch_pr.watch(read_head, read_runs, list(required), self.out.append, clock.sleep, clock, **options)

    def test_flow_027_green_ends_the_watch_with_zero(self):
        code = self.watch([SHA], [[run("Backend", status="queued")], [run("Backend")]])
        self.assertEqual(code, 0)
        self.assertEqual(self.out, ["ALL CHECKS GREEN on aaaaaaa"])

    def test_flow_027_a_failure_is_printed_when_it_lands_and_again_at_the_end(self):
        failing = run("reviewer-infra", "failure", run_id=9)
        code = self.watch([SHA], [[failing, run("Backend", status="in_progress")], [failing, run("Backend")]])
        self.assertEqual(code, 1)
        self.assertEqual(
            self.out,
            [
                "reviewer-infra: failure — https://x/9",
                "FINISHED WITH FAILURES on aaaaaaa:",
                "  reviewer-infra: failure — https://x/9",
            ],
        )

    def test_flow_027_a_network_error_is_retried(self):
        error = watch_pr.GitHubError("connection reset")
        code = self.watch([SHA], [error, error, [run("Backend")]])
        self.assertEqual(code, 0)
        self.assertEqual(self.out[:2], ["poll error (1/5): connection reset", "poll error (2/5): connection reset"])

    def test_flow_027_github_unreachable_five_times_in_a_row_ends_with_two(self):
        code = self.watch([SHA], [watch_pr.GitHubError("down")] * 5)
        self.assertEqual(code, 2)
        self.assertEqual(self.out[-1], "GITHUB UNREACHABLE after 5 tries")

    def test_an_error_between_two_answers_does_not_count_towards_the_limit(self):
        error = watch_pr.GitHubError("down")
        pending = [run("Backend", status="queued")]
        code = self.watch([SHA], [error] * 4 + [pending] + [error] * 4 + [[run("Backend")]])
        self.assertEqual(code, 0)

    def test_a_stuck_check_is_flagged_once(self):
        stuck = Run(3, "E2E", "in_progress", "", "https://x/3", "2026-10-04T12:00:00Z")
        code = self.watch([SHA], [[stuck], [stuck], [run("E2E"), run("Backend")]])
        self.assertEqual(code, 0)
        self.assertEqual(self.out, ["E2E: stuck in_progress with a completion time — https://x/3", "ALL CHECKS GREEN on aaaaaaa"])

    def test_a_push_moves_the_watch_to_the_new_head(self):
        new = "b" * 40
        code = self.watch([SHA, new], [[run("Backend", "failure")], [run("Backend")]], required=["Backend", "E2E"], timeout=60)
        self.assertEqual(self.out[:2], ["Backend: failure — https://x/1", "head moved to bbbbbbb"])
        self.assertEqual(code, 3)

    def test_a_failure_that_keeps_required_checks_from_ever_reporting_ends_the_watch(self):
        # The job that starts the reviewer lanes failed, so the lanes never reported.
        failed = [run("Lanes touched by the diff", "failure", run_id=5), run("Backend")]
        code = self.watch([SHA], [failed] * 10, required=["Backend", "reviewer-infra", "reviewer-arch"])
        self.assertEqual(code, 1)
        self.assertEqual(
            self.out,
            [
                "Lanes touched by the diff: failure — https://x/5",
                "FINISHED WITH FAILURES on aaaaaaa (never reported: reviewer-infra, reviewer-arch):",
                "  Lanes touched by the diff: failure — https://x/5",
            ],
        )

    def test_a_failure_does_not_end_the_watch_while_another_check_still_runs(self):
        running = [run("Frontend", "failure", run_id=5), run("Backend", status="in_progress")]
        done = [run("Frontend", "failure", run_id=5), run("Backend")]
        code = self.watch([SHA], [running] * 6 + [done] * 3, required=["Backend", "E2E"], patience=3)
        self.assertEqual(code, 1)
        self.assertEqual(self.out[1], "FINISHED WITH FAILURES on aaaaaaa (never reported: E2E):")

    def test_the_watch_gives_up_at_its_timeout_and_names_what_runs(self):
        code = self.watch([SHA], [[run("Backend", status="in_progress")]] * 10, timeout=60, interval=30)
        self.assertEqual(code, 3)
        self.assertEqual(self.out, ["TIMED OUT after 60 s on aaaaaaa, still waiting on: Backend"])


class Parse(unittest.TestCase):
    def test_check_runs_are_read_from_the_api_lines(self):
        line = '{"id": 4, "name": "E2E", "status": "completed", "conclusion": "success", "html_url": "https://x/4", "completed_at": "t"}'
        queued = '{"id": 5, "name": "Backend", "status": "queued", "conclusion": null, "html_url": "https://x/5", "completed_at": null}'
        self.assertEqual(
            watch_pr.parse_runs(line + "\n" + queued + "\n"),
            [Run(4, "E2E", "completed", "success", "https://x/4", "t"), Run(5, "Backend", "queued", "", "https://x/5", "")],
        )



class CannotWatch(unittest.TestCase):
    # FLOW-027 — a branch without a pull request is said at once, not retried as an outage.
    def test_flow_027_a_branch_without_a_pull_request_is_not_retried(self):
        def no_pull_request(*args):
            raise watch_pr.GitHubError('no pull requests found for branch "x"')

        original, watch_pr.gh = watch_pr.gh, no_pull_request
        try:
            with self.assertRaises(watch_pr.CannotWatch):
                watch_pr.head_of(None)
        finally:
            watch_pr.gh = original

    # FLOW-027 — an answer that is no pull request is a failed poll, retried like any other.
    def test_flow_027_an_answer_without_a_state_is_a_failed_poll(self):
        original, watch_pr.gh = watch_pr.gh, lambda *args: "[]"
        try:
            with self.assertRaises(watch_pr.GitHubError):
                watch_pr.head_of("12")
        finally:
            watch_pr.gh = original

if __name__ == "__main__":
    unittest.main()
