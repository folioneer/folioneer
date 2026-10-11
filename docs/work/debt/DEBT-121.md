# 2026-10-12 — DEBT-121 — Seventeen frontend tests fail between local midnight and UTC midnight

- Found by: `just harness --coverage` run at 00:28 CEST on 2026-10-12 (22:28 UTC on the 11th), during TODO-063; the same run under `TZ=UTC` passed
- Where: the hook tests of `src/features/account_details/` — `deposit_transaction`, `withdrawal_transaction`, `dividend_transaction`, `free_shares_transaction`, `interest_transaction` (3), `open_balance`, `sell_transaction`, `split_transaction` (5), `fee_schedule` (2) — and `src/features/unpriced_prices/useUnpricedPrices.test.ts`
- Severity: 🟡
- Size: S — one pull request: frontend tests.
- Observation: each compares the form's default date with `new Date().toISOString()`, the day in UTC, where the form gives the day on this computer: `expected '2026-10-12' to be '2026-10-11'`. On a computer east of UTC they fail from local midnight until UTC midnight — two hours in Paris in summer — and pass in CI, which runs in UTC. Not read: whether a computer west of UTC shows the form a wrong day in its evening, which would be a defect of the application and not of its tests
- User value: None directly; the local harness is green at any hour.
- Done when: these tests hold at any hour in any time zone — a fixed clock, or the same day the form uses — and one of them is seen red with the clock set between the two midnights.
