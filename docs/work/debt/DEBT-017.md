# 2026-09-12 — DEBT-017 — The coverage floors sit eight points under what is measured

- Found by: manual (`python3 scripts/coverage-gate.py --backend`; figures refreshed 2026-09-19); refreshed from CI on 2026-10-18 (#320, #322)
- Where: `coverage-gates.json` (backend floor 85.5 %, target 90 %; frontend floor 80 %), and `/run-entry`'s closing step, which never raises a floor
- Severity: 🟡
- Size: S — one pull request: the two floors and one line of `/run-entry`.
- Observation: CI measured 93.95 % of 16 838 backend logic lines (#320) and 88.03 % of 4 730 frontend feature lines (#322); neither floor has moved since the first commits, although the rule says they only rise. A pull request can take either figure down eight points and pass. The floor in `coverage-gates.json` is a ratchet — raise it in the same change that lifts coverage, never lower it.
- User value: None — a harness that catches logic regressions in these paths.
- Mutation survivors: the 2026-09-14 sweep (issue #137) found 301 logic changes no test notices — `context/account/domain/account.rs` 52, `use_cases/shared/valuation.rs` 33, `context/sync/domain/resolution.rs` 24, `use_cases/global_performance/orchestrator.rs` 21; each names an assertion that is missing or too weak.
- Done when: the floors in `coverage-gates.json` read 93.5 (backend, target removed once passed) and 87.5 (frontend), the gate passes on `main`, and `/run-entry`'s closing step raises a floor to half a point under the figure its pull request measured.
