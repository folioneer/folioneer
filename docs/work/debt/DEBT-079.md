# 2026-10-04 — DEBT-079 — The rate edit scenario fails at random and is skipped

- Found by: CI on pull request 124 (run 37225909601), which does not touch currencies
- Where: `e2e/currency/currency_rates.test.ts` — "FXR-052: editing a rate via the UI updates the rate row", at the lookup of the old rate row just after the edit dialog closes
- Severity: 🟡
- Observation: `stale element reference: Stale element found when trying to create the node handle` on `$("#rate-row-…")` once the dialog is gone. The rate list is re-rendered when the rate changes; the scenario asks for a row of the list while it is being replaced. Whether the list re-renders once or twice after an edit, and whether the scenario should wait for the new row before looking for the old one, is not established. The test is skipped (`it.skip`): editing a rate through the interface has no end-to-end coverage until this is fixed.
- History: the same failure was filed on 2026-09-27 as DEBT-039, whose fix (views keep their rows on a re-fetch, F29) did not hold; this entry replaces it.
- User value: None directly — the scenario protects the rate edit again.
- Done when: the cause is found (in the scenario or in how the rate list refreshes), fixed, and the scenario is re-enabled and passes twenty runs in a row.
