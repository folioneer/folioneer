# 2026-10-10 — DEBT-103 — An E2E lookup of an element on its way out fails the scenario

- Found by: the E2E run of pull request 181, a change to the Rust code of management fees ([run 38056367344](https://github.com/folioneer/folioneer/actions/runs/38056367344/job/114225612173)); not re-run
- Where: `e2e/currency/currency_rates.test.ts:336` (FXR-053, skipped by this entry); the same shape in every spec that looks an element up to wait for it to go — `await $(selector)` followed by `waitForExist({ reverse: true })`
- Severity: 🟡
- Observation: `WebDriverError: Stale element found when trying to create the node handle when running "element" with method "POST"`, at the lookup of `#rate-row-USD-EUR-…` just after the delete dialog closed: the driver found the row while React was removing it. It is the failure DEBT-019 had in `dismissLeftoverModal`, in a scenario's own code; that fix re-locates inside a `browser.waitUntil` and treats a lookup the page changed under as "ask again". The same run logs the warning "Request encountered a stale element" in three other specs that passed, so the race is common and usually lands where the test survives it. Not counted: how many scenarios have the shape
- User value: None — a pull request is not refused for a race between the driver and a re-render.
- Done when: a helper waits for an element to be gone by asking again until no lookup finds it, a lookup the page changed under counting as "not yet"; the scenarios that wait for a removal use it; FXR-053 runs again.
