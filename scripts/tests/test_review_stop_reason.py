"""Tests for scripts/review-stop-reason.py (FLOW-026) — run with `just test-scripts`."""

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "review_stop_reason", Path(__file__).resolve().parents[1] / "review-stop-reason.py"
)
reason = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(reason)

SECRET = "sk-ant-not-a-real-token"


class StopReason(unittest.TestCase):
    def test_flow_026_a_session_that_ran_out_of_turns_says_so(self):
        events = [
            {"type": "assistant", "message": {"content": SECRET}},
            {"type": "result", "subtype": "error_max_turns", "is_error": True, "num_turns": 60, "result": SECRET},
        ]
        self.assertEqual(reason.stop_reason(events), "the session ended with `error_max_turns` after 60 turns (error: yes)")

    def test_flow_026_a_session_that_ended_normally_without_a_report_says_so(self):
        events = {"type": "result", "subtype": "success", "is_error": False, "num_turns": 12, "result": "done"}
        self.assertEqual(reason.stop_reason(events), "the session ended with `success` after 12 turns (error: no)")

    def test_flow_026_a_log_without_a_result_names_its_last_event(self):
        events = [{"type": "system"}, {"type": "assistant", "message": {"content": SECRET}}]
        self.assertEqual(
            reason.stop_reason(events), "the session log holds no result: the session did not end by itself (last event: `assistant`)"
        )
        self.assertEqual(reason.stop_reason([]), "the session log is empty")

    def test_flow_026_only_named_fields_are_printed_never_free_text(self):
        events = [{"type": "result", "subtype": SECRET + " with spaces", "is_error": SECRET, "num_turns": SECRET, "result": SECRET}]
        self.assertNotIn(SECRET, reason.stop_reason(events))

    def test_flow_026_a_missing_or_unreadable_log_is_said_not_raised(self):
        with tempfile.TemporaryDirectory() as folder:
            missing = Path(folder) / "none.json"
            self.assertEqual(reason.read(missing), f"no session log at {missing}")
            broken = Path(folder) / "broken.json"
            broken.write_text("{not json", encoding="utf-8")
            self.assertEqual(reason.read(broken), "the session log is not JSON")
            lines = Path(folder) / "lines.jsonl"
            lines.write_text(
                json.dumps({"type": "system"}) + "\n" + json.dumps({"type": "result", "subtype": "success", "num_turns": 3}) + "\n",
                encoding="utf-8",
            )
            self.assertEqual(reason.read(lines), "the session ended with `success` after 3 turns (error: no)")


if __name__ == "__main__":
    unittest.main()
