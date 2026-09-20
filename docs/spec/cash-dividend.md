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
| `exchange_rate`    | Conversion rate from the asset's native currency to the account currency. Exactly `1` when the asset and account currencies match (DIV-022).                 |
| `total_amount`     | The cash credited to the account's Cash Holding, in **account currency** (i64 micros) — the net dividend converted at `exchange_rate`.                       |
| `note`             | Optional free-text note.                                                                                                                                     |

> A dividend carries **no share quantity and no per-unit price** — it does not change how many units of the asset are held. The net amount the user receives is entered in the asset's native currency (DIV-020) and converted to `total_amount` in account currency (DIV-022). Fields that exist on `Transaction` but carry no business meaning for a dividend follow a fixed convention rather than user input; their exact packing is a contract/plan concern, not a business rule.

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

**DIV-010 — Entry point (frontend)**: The dividend recording flow is initiated from the Account Details header's consolidated "Record" menu (DIV-012), via a "Dividend" item that opens the dividend modal (DIV-020). The paying asset is chosen inside the modal (DIV-020), not derived from a holding row. v1 does **not** add a per-holding-row dividend action (deferred — see Open Questions / Deferred).

**DIV-011 — Eligibility (backend)**: A dividend may be recorded only for an `(account, asset)` pair where the account currently holds the asset with `quantity > 0` and the asset is not a Cash Asset. The action is rejected with a specific error when the account is unknown, the asset is unknown, the asset is not currently held in that account (no active holding), or the asset is a Cash Asset.

**DIV-012 — Header "Record" menu consolidation (frontend)**: The Account Details header groups its "record an entry" actions under a single "Record" dropdown menu rather than separate buttons. The menu's items are: New position (the Opening-balance action, TRX-055), Dividend (DIV-010), and Record free shares (FSD-010). Each item opens its existing dedicated modal. This **supersedes the standalone-button header placement** of TRX-055 (Add a position) — see the reciprocal back-reference in that rule. The menu carries **no** cash actions: Deposit and Withdrawal are reached exclusively from the Cash _row's_ inline actions (CSH-091 / CSH-019), since the cash row is always present (CSH-095). The Performance and Refresh-prices header actions, and the primary "Add transaction" (buy/sell) action, are unchanged and remain outside this menu.

### Recording a Dividend (020–029)

**DIV-020 — Form fields (frontend)**: The Record-dividend form accepts an **asset selector** listing the account's active (`quantity > 0`), non-cash holdings (the paying asset; the Cash Holding is excluded), `date` (default: today), the **total net amount received** (positive decimal, in the selected asset's native currency), an `exchange_rate` (shown only when the asset currency differs from the account currency — DIV-022), and an optional `note`. There is no quantity, unit-price, or fees input. The currency label next to the amount reflects the selected asset's currency.

**DIV-021 — Input validation (frontend + backend)**: The amount must parse as a strictly positive decimal (`> 0`); the `date` must be a well-formed ISO 8601 calendar date that is not in the future and not older than the TRX lower bound; when an exchange rate is required (DIV-022, currencies differ) it must be strictly positive (mirroring TRX-020). The frontend validates inline and disables submit until valid; the backend re-validates and rejects with explicit error variants.

**DIV-022 — Currency conversion (frontend + backend)**: The dividend is entered in the asset's native currency and credited to the account in account currency, converted via `exchange_rate`, reusing the same mechanism as Buy/Sell (TRX-021). When the asset currency equals the account currency, `exchange_rate` is `1` and no rate input is shown; otherwise the user supplies the rate and `total_amount = net_amount × exchange_rate`.

**DIV-023 — Recording effect (backend)**: Recording a Dividend, within a single Unit of Work ([ADR-006](../adr/006-unit-of-work.md)): (a) credits the account's always-present Cash Holding by `total_amount` (account currency) — identical to how Sell credits cash (CSH-050, CSH-012); (b) leaves the paying asset's holding `quantity`, average cost, and cost basis **unchanged**; (c) persists the Transaction with `transaction_type = Dividend` attributed to the paying asset. All steps commit together or all roll back.

**DIV-024 — No effect on cost basis or realized P&L (backend)**: A Dividend never alters the paying asset's `quantity`, average cost, or cost basis (the holding is left untouched per DIV-023b), and never contributes to realized P&L. Dividend income is kept distinct from capital gains so the two can be reported separately.

**DIV-027 — No effect on recorded market price (backend)**: Recording a dividend **does not create or modify any `AssetPrice` record** for the asset. The ex-dividend drop in market price is reflected by the price-fetch feed (or manual entry), never synthesised by the dividend — modelling it as a price cut would double-count the market's own ex-div adjustment and corrupt the price history. The dividend's value is conserved as the cash credit (DIV-023).

**DIV-028 — Account-currency entry mode (frontend)**: When the selected asset's currency differs from the account currency, the form offers a two-way entry-mode switch: **asset currency + rate** (DIV-020/022, the default) or **account currency**. In account-currency mode the amount field's currency label shows the account currency and the exchange-rate input is hidden — the user types exactly what was credited to the account (e.g. the euros received for a USD asset's dividend), with no rate to look up.

**DIV-029 — Account-currency recording (frontend)**: In account-currency mode the typed amount is credited verbatim: the entry is recorded with the amount equal to the typed value and an exchange rate of 1, so `total_amount = typed amount` (the same typed-verbatim philosophy as TRX-060). The asset-currency gross is deliberately not captured in this mode — the stored amount is the account-currency credit.

**DIV-025 — In-flight, success, and error feedback (frontend)**: While the request is in progress the submit button is disabled and shows a spinner. On success the form closes and a snackbar confirms "Dividend recorded". On validation failure or backend rejection the form stays open with an inline error adjacent to the offending field; the user can correct and resubmit.

**DIV-026 — Reactivity (backend + frontend)**: Recording a Dividend publishes the existing `TransactionUpdated` event (per the AccountService convention, as Deposit/Withdrawal do in CSH-100). The Account Details view re-fetches on that event (ACD-039), so the Cash row balance and the Global Value total (CSH-094) reflect the new dividend without a manual refresh.

### Edit and Delete (040–049)

**DIV-040 — Edit (backend + frontend)**: A recorded Dividend can be edited through the existing transaction-correction flow (TXL). Editing re-applies the chronological replay across the account's cash-affecting transactions, recomputing the Cash Holding from scratch (mirroring CSH-023). Editable fields: `date`, net amount, `exchange_rate`, and `note`. The paying asset (`asset_id`) is **immutable** on a dividend edit — changing it requires deleting and re-recording the dividend. The DIV-011 eligibility guards are re-evaluated on edit. A successful edit publishes `TransactionUpdated` (DIV-026).

**DIV-041 — Delete (backend + frontend)**: A recorded Dividend can be deleted through the existing cancel flow. Deletion triggers a chronological replay with the dividend's cash credit removed. If removing that credit would drive the running cash balance strictly negative for any later cash-debit transaction (Purchase or Withdrawal), the deletion is rejected with the shared `InsufficientCash { current_balance_micros, currency }` error, mirroring the Sell-delete behaviour (CSH-051, CSH-080). A successful delete publishes `TransactionUpdated` (DIV-026).

### Display (050–059)

**DIV-050 — Transaction list inclusion (frontend)**: Dividend transactions appear in the transaction list (TXL) when the user filters by the **paying asset** for that account. The Type column displays "Dividend" (cross-amends TXL-023). The Realized P&L column renders the neutral placeholder `—`. The amount/total columns render the stored values. When filtering by a different asset, the dividend does not appear.

**DIV-051 — Reflected in Global Value (backend)**: Because a Dividend credits the Cash Holding, its amount is already included in `total_global_value` (CSH-094) through the cash term — independently of the per-asset dividend aggregation in DIV-070–073 (which sums the dividend transactions themselves, not the cash holding).

### Total Return (070–079)

**DIV-070 — Dividends received per holding (backend)**: `HoldingDetail.dividends_received` is the sum of `total_amount` across all Dividend transactions for that `(account, asset)`, in account currency. It is `0` when the asset has paid no dividends, and is always computable (dividends are stored in account currency, so no currency-mismatch gap applies).

**DIV-071 — Dividend-inclusive total return (backend)**: `HoldingDetail.total_return_pct` is computed as `(unrealized_pnl + dividends_received) × 100 / cost_basis` in i64 micros, using i128 intermediates (consistent with MKT-035). It is `None` under exactly the same conditions that make `performance_pct` `None` — no recorded price, asset/account currency mismatch (MKT-034), or `cost_basis = 0`. This is the figure that prevents a high-yield holding from appearing to underperform when judged on price movement alone.

**DIV-072 — Total-return display (frontend)**: The Account Details holding row surfaces `dividends_received` (formatted in account currency) and `total_return_pct` alongside the existing price-only Performance % (MKT-035). `dividends_received` is **always** shown (it is always computable — DIV-070), even when `total_return_pct` is `None`: a currency-mismatched or unpriced holding therefore shows its real dividends-received amount next to a `—` total return. When `total_return_pct` is `None` it renders the neutral placeholder `—`, consistent with MKT-034. The price-only Performance % column is retained so the user can distinguish price return from total return. (Exact column-vs-sub-line layout is a UX detail.)

**DIV-073 — Account total dividends (backend + frontend)**: `AccountDetailsResponse.total_dividends_received` is the sum of dividend cash credited across all of the account's dividend transactions, in account currency (`0` when none). The Account Details header displays it alongside the existing Global Value / Total Cost Basis / Total Realized P&L totals.

---

## Workflow

```
Account Details header → "Record" menu → "Dividend" (DIV-010/012)
  → modal: asset selector (active non-cash holdings), date (today),
           net amount (selected-asset currency), exchange rate (if currencies differ), note (DIV-020)
  → submit
      backend validate: amount > 0, date ≤ today, asset held (qty > 0), not cash (DIV-011/021)
      backend (single Unit of Work, ADR-006):
        ├─ credit the always-present Cash Holding by total_amount        (DIV-023, CSH-050/012)
        ├─ leave paying asset's holding qty / cost basis unchanged        (DIV-024)
        └─ persist Transaction(type=Dividend, asset_id=paying asset)      (DIV-023)
      publish TransactionUpdated                                          (DIV-026)
  → modal closes + snackbar "Dividend recorded"                          (DIV-025)
  → Account Details re-fetches → Cash row + Global Value updated         (DIV-026)

Transaction list (filtered by the paying asset)
  → dividend row shows Type = "Dividend", Realized P&L = "—"             (DIV-050)
  → edit (date / amount / rate / note) → chronological replay            (DIV-040)
  → delete → replay without the credit; rejected if it underflows cash   (DIV-041)
```

---

## UX Draft

### Entry Point

A "Dividend" item in the Account Details header's consolidated "Record" dropdown menu (DIV-012), which also hosts New position and Record free shares (cash Deposit/Withdraw live on the cash row, not this menu — DIV-012). Selecting it opens the dividend modal. (A per-holding-row dividend action is deferred — DIV-010.)

### Main Component

A small `FormModal` — `DividendTransactionModal`:

- Asset selector — the account's active, non-cash holdings (the paying asset).
- Date (default today, `DateField`).
- Net amount received (positive decimal, `AmountField` with the selected asset's currency suffix).
- Exchange rate (shown only when asset currency ≠ account currency, mirroring the Buy/Sell exchange-rate input).
- Note (optional `TextField`).
- Submit / Cancel.

### States

- **Idle**: form with today's date, empty amount, submit disabled until valid.
- **In-flight**: submit disabled + spinner (DIV-025).
- **Validation / backend error**: inline error adjacent to the field; modal stays open (DIV-025).
- **Success**: modal closes; snackbar "Dividend recorded"; Account Details re-fetches — Cash row + Global Value rise by the dividend (DIV-026).

### User Flow

1. User opens Account Details for an account holding 50 shares of an asset.
2. User opens the header "Record" menu and chooses "Dividend".
3. Modal opens; user picks the paying asset from their active holdings, enters the net amount received (asset currency), adjusts the date if needed, supplies an exchange rate if the asset trades in another currency.
4. User submits.
5. Backend validates, credits the cash holding, leaves the position unchanged, persists the dividend, publishes `TransactionUpdated`.
6. Modal closes, snackbar confirms; the Cash row balance and Global Value update; the holding row's Dividends-received and Total-return % update (DIV-072); the account header's Total dividends received updates (DIV-073); and the dividend now shows in the transaction list under that asset with Type "Dividend".

---

## Open Questions / Deferred

**Deferred to future specs** (explicitly out of v1 scope, agreed with the user):

- **Stock dividends** — dividends paid as additional shares (increase quantity, with their own cost-basis treatment) rather than cash.
- **DRIP / auto-reinvestment** — automatically buying more of the asset with the dividend cash (a dividend plus an implied purchase).
- **Return-of-capital distributions** — distributions that reduce the position's cost basis rather than count as income.
- **Withholding-tax breakdown** — capturing gross dividend, withholding tax, and net separately (v1 records the net cash received only).
- **Richer dividend reporting** — dividend **yield**, per-period income statements, and a dividend **timeline**. (v1 _does_ include a per-`(account, asset)` dividends-received total, a dividend-inclusive total-return % per holding, and a per-account dividends-received total — DIV-070–073.)
- **Dividends-received on closed positions in the holdings view** — v1 sums _all_ the account's dividends into `total_dividends_received` (DIV-073), but the per-holding `dividends_received` row figure surfaces on active holdings only; showing it on closed-position rows is deferred.
- **Dividends on closed positions** — recording a _new_ dividend declared while held but paid after the position was fully sold (v1 allows recording only on actively-held positions, `quantity > 0`).
- **Per-holding-row dividend action** — a contextual "Record dividend" action directly on a holding row (asset pre-selected). v1 initiates the flow from the header "Record" menu with an in-modal asset picker (DIV-010/012); a row-level shortcut is to be designed later.

None — all questions have been resolved.
