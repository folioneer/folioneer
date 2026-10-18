# 2026-10-18 — DEBT-151 — The performance page gives a return for a period the holding was not held through

- Found by: `spec-checker` at the 2026-10-18 hand-over; decided by the owner the same day: align on a dash, as account details do (ACD-057)
- Where: `windowed_metric_for_scope` in `src-tauri/src/use_cases/shared/performance.rs:623-663`, `holding_performance_for_span_over_flows` in `use_cases/shared/valuation.rs:842-872`; `docs/spec/account-performance.md` (PRF-083 and the Asset Scope intro)
- Severity: 🟡
- Size: S — one guard, its tests, the spec.
- Observation: a holding bought inside a period gets a period-over-period and a year-to-date figure on the performance page (starting value 0, the purchase counted as money brought in), while account details show no figure for a window the holding was not held at the start of (ACD-057). The same holding, the same period, two answers.
- User value: A holding bought during the year shows no year-to-date return anywhere, instead of a figure on one screen and a dash on the other.
- Done when: a scoped period row of a holding not held at the period's start has no period-over-period and no year-to-date figure, shown as the dash with the account details' tooltip; PRF-083 and the Asset Scope intro say so; a test with a holding bought inside the period goes red on today's code; the golden portfolio moves only where it should, said in the PR.
