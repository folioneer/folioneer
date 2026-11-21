# Business Rules — Cash Dividend (DIV)

## Context

A Cash Dividend records income paid out by an asset the user holds (e.g. a quarterly equity dividend) as cash arriving in the account. It is the first of the roadmap's Phase 4 "Corporate Events" and a sibling of the Cash Tracking spec (CSH): like a Sell, a dividend **credits the account's cash holding**, but unlike a Sell it leaves the paying asset's position untouched and is **not** a capital gain.

The feature adds a new `Dividend` variant to `TransactionType` (joining `Purchase | Sell | OpeningBalance | Deposit | Withdrawal`). A dividend transaction is **attributed to the paying asset** (its `asset_id` is that asset, so it surfaces under that asset in the transaction list), while its monetary effect is a credit to the account's Cash Holding — mirroring how CSH-050 re-links Sell proceeds to cash. It touches the `account` bounded context (Transaction, Holding, the Account aggregate root) and reuses the cash machinery defined in CSH.

All monetary values are `i64` micro-units ([ADR-001](../adr/001-use-i64-for-monetary-amounts.md)); the dividend's credit to cash and its replay on edit/delete commit atomically within the existing Unit of Work ([ADR-006](../adr/006-unit-of-work.md)).

This v1 covers **cash dividends only**, and it **folds dividend income into per-asset performance** so a high-yield position is not shown as underperforming on price alone: each holding exposes a dividends-received total and a dividend-inclusive total-return % (DIV-070–073). Crucially, a dividend is **not** modelled as a price change — the ex-dividend price drop comes from the market data feed, never synthesised here (DIV-024); the dividend's value is preserved as the cash credit. Stock dividends (paid as additional shares), DRIP / auto-reinvestment, return-of-capital (cost-basis-reducing distributions), withholding-tax gross/net breakdown, and richer dividend reporting (yield, per-period income statements, a dividend timeline) are each deferred to their own future specs (see Open Questions / Deferred).

---

## Entity Definition

### Dividend Transaction (new `TransactionType` variant)

A cash income event paid by a held asset, recorded against that asset and credited to the account's cash holding.

| Field              | Business meaning                                                                                                                                             |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `transaction_type` | `TransactionType::Dividend`.                                                                                                                                 |
| `account_id`       | The account receiving the dividend.                                                                                                                          |
| `asset_id`         | The **paying asset** — the holding the dividend is attributed to. **Not** the Cash Asset. This is what makes the dividend appear under that asset (DIV-050). |
| `date`             | Business date the dividend was received (must not be in the future, must not be older than the existing TRX lower bound).                                    |
| `exchange_rate`    | Conversion rate from the asset's native currency to the account currency. Exactly `1` when the asset and account currencies match (DIV-022A).                |
| `dividend_amount`  | The net amount, in the currency it was entered in (DIV-022, DIV-029); absent on every other type (DIV-023).                                                  |
| `total_amount`     | The cash credited to the account's Cash Holding, in **account currency** (i64 micros) — the net dividend converted at `exchange_rate`.                       |
| `note`             | Optional free-text note.                                                                                                                                     |

> A dividend carries **no share quantity and no per-unit price** — it does not change how many units of the asset are held. The net amount the user receives is entered in the asset's native currency and converted to `total_amount` in account currency (DIV-022), or entered in the account's currency (DIV-029). Fields that exist on `Transaction` but carry no business meaning for a dividend follow a fixed convention rather than user input: its `quantity` is 0, and the surfaces that give a quantity give its net amount there (TRX-024A).

### HoldingDetail (extended)

The `HoldingDetail` DTO (owned by ACD, already extended by MKT) gains two fields for dividend-inclusive performance.

| Field                | Business meaning                                                                                                                                                                                                                                  |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `dividends_received` | Sum of all dividend cash credited for this `(account, asset)`, in account currency (i64 micros). `0` when the asset has paid no dividends. Always computable (dividends are stored in account currency).                                          |
| `total_return_pct`   | Dividend-inclusive return on the current position: `(unrealized_pnl + dividends_received) × 100 / cost_basis` (i64 micros). `None` under the same conditions as `performance_pct` (no price / currency mismatch / zero cost basis — MKT-034/035). |

### AccountDetailsResponse (extended)

The `AccountDetailsResponse` DTO (owned by ACD) gains one field.

| Field                      | Business meaning                                                                                                                      |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| `total_dividends_received` | Sum of dividend cash credited across **all** of the account's dividend transactions, in account currency (i64 micros). `0` when none. |

---

## Business Rules

### Eligibility and Initiation (010–019)

**DIV-010 — Entry point (frontend)**: The dividend recording flow is initiated from the Account Details header's consolidated "Record" menu (DIV-012), via a "Dividend" item that opens the dividend modal (the validated mock on #734). The paying asset is chosen inside the modal, not derived from a holding row. v1 does **not** add a per-holding-row dividend action (deferred — see Open Questions / Deferred).

**DIV-011 — Eligibility (backend)**: A dividend is recorded only for an `(account, asset)` pair where the account held the asset on the dividend's date or before it — a purchase, an opening balance or a transfer in of the asset is dated on or before it — and the asset is not a Cash Asset (owner, 2026-11-14, #774). A dividend is recorded on the day it was paid: everywhere the right to it is fixed at the ex-date and the cash comes later — two trading days in France, Italy and Spain, weeks in the United States, up to about three months in Poland — so one paid after the whole position was sold is recorded. The action is rejected with a specific error when the account is unknown, the asset is unknown, the asset was never held in that account on or before that date, or the asset is a Cash Asset. The rule is the same whatever records the dividend (TRX-012).

**DIV-012 — Header "Record" menu consolidation (frontend)**: The Account Details header groups its "record an entry" actions under a single "Record" dropdown menu rather than separate buttons. The menu's items are: New position (the Opening-balance action, TRX-055), Import (IMP-060), Dividend (DIV-010), and Record free shares (FSD-010). Each item opens its existing dedicated modal. This **supersedes the standalone-button header placement** of TRX-055 (Add a position) — see the reciprocal back-reference in that rule. The menu carries **no** cash actions: Deposit and Withdrawal are reached exclusively from the Cash _row's_ inline actions (CSH-091 / CSH-019), since the cash row is always present (CSH-095). The Performance and Refresh-prices header actions, and the primary "Add transaction" (buy/sell) action, are unchanged and remain outside this menu.

### Recording a Dividend (020–029)

**DIV-021 — Input validation (backend)**: The net amount is strictly positive and passes no limit TRX-020 puts on money; the date is a well-formed ISO 8601 calendar date, not in the future and not older than the TRX lower bound; between two currencies the rate is strictly positive (TRX-020). Each is refused with a specific error.

**DIV-022 — Currency conversion (backend)**: The net amount is credited to the account in account currency, converted via `exchange_rate`, reusing the same mechanism as Buy/Sell (TRX-021): `total_amount = net_amount × exchange_rate`. The net amount is in the asset's currency, or in the account's when DIV-029 applies; the rate is 1 when the asset and the account share a currency (DIV-022A). An amount that converts to nothing in the account's currency — the product floors to zero — is refused with a specific error.

**DIV-022A — Within one currency the rate is 1 (backend)**: When the asset's currency equals the account's, a dividend given any other rate is refused with a specific error, on recording and on a correction alike; it is never replaced by 1 (owner, 2026-11-16, #734).

**DIV-023 — Recording effect (backend)**: Recording a Dividend, within a single Unit of Work ([ADR-006](../adr/006-unit-of-work.md)): (a) credits the account's always-present Cash Holding by `total_amount` (account currency) — identical to how Sell credits cash (CSH-050, CSH-012); (b) leaves the paying asset's holding `quantity`, average cost, and cost basis **unchanged**; (c) persists the Transaction with `transaction_type = Dividend` attributed to the paying asset, its net amount, in the currency it was entered in (DIV-022, DIV-029), in a field of its own, `dividend_amount`, and its `quantity` 0. All steps commit together or all roll back.

**DIV-024 — No effect on cost basis or realized P&L (backend)**: A Dividend never alters the paying asset's `quantity`, average cost, or cost basis (the holding is left untouched per DIV-023b), and never contributes to realized P&L. Dividend income is kept distinct from capital gains so the two can be reported separately.

**DIV-027 — No effect on recorded market price (backend)**: Recording a dividend **does not create or modify any `AssetPrice` record** for the asset. The ex-dividend drop in market price is reflected by the price-fetch feed (or manual entry), never synthesised by the dividend — modelling it as a price cut would double-count the market's own ex-div adjustment and corrupt the price history. The dividend's value is conserved as the cash credit (DIV-023).

**DIV-029 — Account-currency mode (backend)**: Between two currencies, a dividend at a rate of exactly 1 has its net amount in the account's currency and is credited verbatim, `total_amount = net amount` (the typed-verbatim philosophy of TRX-060): this is the account-currency mode, in which the gross in the asset's currency is not captured.

**DIV-026 — Reactivity (backend + frontend)**: Recording a Dividend publishes the existing `TransactionUpdated` event (per the AccountService convention, as Deposit/Withdrawal do in CSH-100). The Account Details view re-fetches on that event (ACD-039), so the Cash row balance and the Global Value total (CSH-094) reflect the new dividend without a manual refresh.

### Edit and Delete (040–049)

**DIV-040 — Edit (backend)**: A recorded Dividend can be edited through the existing transaction-correction flow (TXL). Editing re-applies the chronological replay across the account's cash-affecting transactions, recomputing the Cash Holding from scratch (mirroring CSH-023). Editable fields: `date`, net amount, `exchange_rate`, and `note`. The paying asset (`asset_id`) is **immutable** on a dividend edit — changing it requires deleting and re-recording the dividend. The DIV-011 eligibility guards are re-evaluated on edit (DIV-040A). A successful edit publishes `TransactionUpdated` (DIV-026).

**DIV-040A — A correction to a date before the first holding (backend)**: Correcting a dividend to a date before its asset was first held is rejected with a specific error, whatever date the dividend had before. The error is the account's own, the one DIV-045 gives; a new recording refused for the same reason is stopped earlier, by the recorder, which says the asset was not held (DIV-011).

**DIV-041 — Delete (backend + frontend)**: A recorded Dividend can be deleted through the existing cancel flow. Deletion triggers a chronological replay with the dividend's cash credit removed. If removing that credit would drive the running cash balance strictly negative for any later cash-debit transaction (Purchase or Withdrawal), the deletion is rejected with the shared `InsufficientCash { current_balance_micros, currency }` error, mirroring the Sell-delete behaviour (CSH-051, CSH-080). A successful delete publishes `TransactionUpdated` (DIV-026).

**DIV-042 — Correction modal (frontend)**: A dividend is corrected in its own modal (`EditDividendModal`), never in the generic correction modal of a trade: the edit action of a Dividend row (TXL-030) opens it. It states the paying asset and the account without offering to change them (DIV-040), and shows the date, the net amount, the exchange rate when one applies, the total credited to the account as the draft check returns it (TRX-062), and the note. It shows no unit price, no fees and no market price to record.

**DIV-043 — The correction keeps the recorded entry mode (frontend)**: The amount is shown in the currency it was recorded in. For a dividend recorded in asset currency (DIV-022), that is the asset's currency, and the exchange rate is shown when the asset and the account differ in currency. For one recorded in account-currency mode (DIV-029), it is the account's currency and no rate is shown. The mode is read from the stored rate: between two different currencies, a rate of exactly 1 is account-currency mode. A dividend typed in asset currency at a rate of exactly 1 is stored identically and is shown the same way; the figures it records are the same in either view.

**DIV-044 — A correction saved untouched changes nothing (frontend)**: The amount and the rate are filled to their last recorded decimal, so saving the modal without typing records the same figures; for a dividend whose asset and account share a currency, the rate filled is 1, the only one DIV-022A accepts. The first problem the draft check reports is shown as on the other transaction forms (TRX-067).

**DIV-045 — Another transaction's change never leaves a dividend before any holding (backend)**: Correcting or cancelling a transaction of the paying asset — a transfer's parts included —, or replacing four transactions with a transfer (AGT-090, AGT-091, AGT-093), is rejected with a specific error, the one DIV-040A gives, when it would leave a dividend dated before the asset was first held: the purchase, the opening balance or the transfer in that first brought it in moved after the dividend or cancelled, or the purchase replaced by a transfer in dated after it. It is refused as SEL-032 refuses a change that leaves a later sale short (owner, 2026-11-14, #774); the dividend is moved or removed first.

**DIV-045A — A sale moved before a dividend (backend)**: DIV-045 does not refuse a sale moved before a dividend of its asset: the dividend then falls after the position was sold, which DIV-011 accepts. The sale's own rules still apply (SEL-012, SEL-021, SEL-032, CSH-080).

**DIV-045B — A dividend already before any holding refuses nothing (backend)**: A dividend that the ledger already holds dated earlier than its asset's first holding — a change from another computer is applied without these checks (CFR-017) — does not refuse a change to another transaction.

### Display (050–059)

**DIV-050 — Transaction list inclusion (frontend)**: Dividend transactions appear in the transaction list (TXL) when the user filters by the **paying asset** for that account. The Type column displays "Dividend" (cross-amends TXL-023). The Quantity column shows its net amount (TRX-024A). The Realized P&L column renders the neutral placeholder `—`. The amount/total columns render the stored values. When filtering by a different asset, the dividend does not appear.

**DIV-051 — Reflected in Global Value (backend)**: Because a Dividend credits the Cash Holding, its amount is already included in `total_global_value` (CSH-094) through the cash term — independently of the per-asset dividend aggregation in DIV-070–073 (which sums the dividend transactions themselves, not the cash holding).

### Total Return (070–079)

**DIV-070 — Dividends received per holding (backend)**: `HoldingDetail.dividends_received` is the sum of `total_amount` across all Dividend transactions for that `(account, asset)`, in account currency. It is `0` when the asset has paid no dividends, and is always computable (dividends are stored in account currency, so no currency-mismatch gap applies).

**DIV-071 — Dividend-inclusive total return (backend)**: `HoldingDetail.total_return_pct` is computed as `(unrealized_pnl + dividends_received) × 100 / cost_basis` in i64 micros, using i128 intermediates (consistent with MKT-035). It is `None` under exactly the same conditions that make `performance_pct` `None` — no recorded price, asset/account currency mismatch (MKT-034), or `cost_basis = 0`. This is the figure that prevents a high-yield holding from appearing to underperform when judged on price movement alone.

**DIV-072 — Total-return display (frontend)**: The Account Details holding row surfaces `dividends_received` (formatted in account currency) and `total_return_pct` alongside the existing price-only Performance % (MKT-035). `dividends_received` is **always** shown (it is always computable — DIV-070), even when `total_return_pct` is `None`: a currency-mismatched or unpriced holding therefore shows its real dividends-received amount next to a `—` total return. When `total_return_pct` is `None` it renders the neutral placeholder `—`, consistent with MKT-034. The price-only Performance % column is retained so the user can distinguish price return from total return. (Exact column-vs-sub-line layout is a UX detail.)

**DIV-073 — Account total dividends (backend)**: The account's total dividends received is the sum of the dividend cash credited across all of the account's dividend transactions, in account currency (`0` when none).

---

## Workflow

The Record-dividend form is the validated mock on #734 (folioneer/folioneer-design, `734/`).

```
record a dividend
  backend validate: amount > 0, date ≤ today, rate > 0 between two currencies, 1 within one
                    (DIV-021, DIV-022A), asset held on the date or before, not cash (DIV-011)
  backend (single Unit of Work, ADR-006):
    ├─ credit the always-present Cash Holding by total_amount        (DIV-023, CSH-050/012)
    ├─ leave paying asset's holding qty / cost basis unchanged        (DIV-024)
    └─ persist Transaction(type=Dividend, asset_id=paying asset)      (DIV-023)
  publish TransactionUpdated                                          (DIV-026)

correct a dividend → the same checks, chronological replay            (DIV-040, DIV-022A)
delete a dividend  → replay without the credit; refused if it underflows cash (DIV-041)
```

---

## Open Questions / Deferred

**Deferred to future specs** (explicitly out of v1 scope, agreed with the user):

- **Stock dividends** — dividends paid as additional shares (increase quantity, with their own cost-basis treatment) rather than cash.
- **DRIP / auto-reinvestment** — automatically buying more of the asset with the dividend cash (a dividend plus an implied purchase).
- **Return-of-capital distributions** — distributions that reduce the position's cost basis rather than count as income.
- **Withholding-tax breakdown** — capturing gross dividend, withholding tax, and net separately (v1 records the net cash received only).
- **Richer dividend reporting** — dividend **yield**, per-period income statements, and a dividend **timeline**. (v1 _does_ include a per-`(account, asset)` dividends-received total, a dividend-inclusive total-return % per holding, and a per-account dividends-received total — DIV-070–073.)
- **Dividends-received on closed positions in the holdings view** — v1 sums _all_ the account's dividends into `total_dividends_received` (DIV-073), but the per-holding `dividends_received` row figure surfaces on active holdings only; showing it on closed-position rows is deferred.
- **Per-holding-row dividend action** — a contextual "Record dividend" action directly on a holding row (asset pre-selected). v1 initiates the flow from the header "Record" menu with an in-modal asset picker (DIV-010/012); a row-level shortcut is to be designed later.

None — all questions have been resolved.
