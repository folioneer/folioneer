# 2026-10-18 — DEBT-145 — A scheduled run counts a quiet market as a skip and stops rates on a price failure

- Found by: `spec-checker` at the hand-over of the 2026-10-18 batch, on main at 7938e09
- Where: `src-tauri/src/use_cases/scheduled_fetch/orchestrator.rs:254, 268, 418, 1487`
- Severity: 🟡
- Size: M — the run's outcome and its record.
- Observation: SPF-032: an empty answer (no trading day in the window) is counted in `skipped_count`, so a weekend run reads "0 updated, N skipped"; the rule says it is neither a skip nor an error, and the test asserts the current behaviour. SPF-039: rates are refreshed only when the price sweep did not fail (`:268`) and not at all when the price scope is empty (`:254`), while the rule wants each part independent; `ScheduledFetchRun` has no rate outcome.
- User value: A weekend run no longer reports assets as skipped; rates stay current when the price provider is down or every price is typed by hand.
- Done when: an empty answer counts as neither (the test changed); rates are refreshed whatever the price sweep did, with their outcome on the run or the rule's clause dropped; SPF-032 and SPF-039 each have a test that goes red on today's code.
