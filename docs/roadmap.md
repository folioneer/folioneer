# Roadmap — Financial Asset Lifecycle

This document maps every operation needed to cover the full lifecycle of a financial asset in Folioneer, from catalog entry to position closure. It is a planning reference, not a spec — each phase links to the relevant spec(s) and todo items when they exist.

---

## Lifecycle Overview

```
[1. Catalog]  →  [2. Acquisition]  →  [3. Monitoring]  →  [4. Corporate Events]
                                              ↓
                                      [5. Disposition]  →  [6. Closure]  →  [7. Archive]
```

---

## Phase 1 — Asset Catalog

Manage the list of tradable instruments the user can reference in transactions.

| Operation                 | Status  | Notes                                                                                                                                                                                |
| ------------------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Create asset              | ✅ Done | Name, ticker, class, category, currency, risk level                                                                                                                                  |
| Edit asset                | ✅ Done |                                                                                                                                                                                      |
| Archive asset             | ✅ Done | Soft delete; preserves history                                                                                                                                                       |
| Unarchive asset           | ✅ Done |                                                                                                                                                                                      |
| Delete asset              | ✅ Done | Hard delete; only safe when no holdings exist                                                                                                                                        |
| Archive eligibility guard | ✅ Done | Block archiving if `Holding.quantity > 0` (OQ-6) — `ArchiveAssetUseCase`                                                                                                             |
| Delete eligibility guard  | ✅ Done | Block hard delete if any transaction exists for the asset                                                                                                                            |
| Inline asset creation     | ✅ Done | From `/transactions/new`, `ComboboxField` `onCreateNew` navigates to `/assets?createNew=<query>&returnPath=...`; on success returns to `/transactions/new` with new asset pre-filled |

Spec: `docs/spec/asset.md`

---

## Phase 2 — Acquisition (Buy)

Record a purchase that opens or increases a position in an account.

| Operation                     | Status  | Notes                                                                                                                            |
| ----------------------------- | ------- | -------------------------------------------------------------------------------------------------------------------------------- |
| Add purchase transaction      | ✅ Done | Quantity, unit price, exchange rate, fees, note                                                                                  |
| Total amount auto-computation | ✅ Done | Backend formula: `floor(qty×price/M)×rate/M + fees` (TRX-026)                                                                    |
| Multi-currency support        | ✅ Done | Exchange rate stored per transaction (TRX-021)                                                                                   |
| VWAP cost basis update        | ✅ Done | `avg_price = Σ total_amount / Σ quantity` (TRX-030)                                                                              |
| Atomic holding update         | ✅ Done | Transaction + holding in one DB transaction (TRX-027)                                                                            |
| Archived asset auto-unarchive | ✅ Done | With frontend confirmation dialog (TRX-028, TRX-029)                                                                             |
| Edit purchase transaction     | ✅ Done | Full recalculation of holding on save (TRX-031)                                                                                  |
| Delete transaction            | ✅ Done | Full flow: backend + confirmation dialog + snackbar (TRX-035)                                                                    |
| Delete confirmation dialog    | ✅ Done | ConfirmationDialog in TransactionListPage (TRX-035)                                                                              |
| Transaction list view         | ✅ Done | Per-account/asset filter, sort, edit/delete actions (TXL spec)                                                                   |
| Account currency field        | ✅ Done | Exchange rate visibility now compares asset vs account currency (TRX-021, SEL-036)                                               |
| Buy from holding row          | ✅ Done | `BuyTransactionModal` opened from holding row in Account Details, mirrors sell pattern (TRX-041)                                 |
| Opening balance entry         | ✅ Done | Seed a position with quantity + total cost from Account Details header; no fee breakdown; participates in VWAP (TRX-042–TRX-058) |

Spec: `docs/spec/financial-asset-transaction.md`

---

## Phase 3 — Position Monitoring

Display the current state of positions and their performance.

| Operation                     | Status     | Notes                                                                                                                                                                                |
| ----------------------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Holdings computed from buys   | ✅ Done    | Quantity + VWAP average price per (account, asset) pair                                                                                                                              |
| Account Details view          | ✅ Done    | Holdings list + total cost basis per account (ACD spec)                                                                                                                              |
| Holdings reactivity           | ✅ Done    | Re-fetch on `TransactionUpdated` event (ACD-039)                                                                                                                                     |
| Current market price (manual) | ✅ Done    | Manual entry in Account Details holding row; stored as `AssetPrice` (MKT spec)                                                                                                       |
| Current market price (feed)   | ✅ Done    | Yahoo Finance keyless auto-fetch on launch + manual refresh (MKT-100..MKT-142, ADR-017)                                                                                              |
| Unrealized P&L                | ✅ Done    | `(current_price − average_price) × quantity`; displayed per holding (MKT spec)                                                                                                       |
| Performance %                 | ✅ Done    | `unrealized_pnl / cost_basis × 100`; displayed per holding (MKT spec)                                                                                                                |
| Foreign-holding valuation     | ✅ Done    | `CurrencyRate` values holdings whose currency differs from the account; lifts the same-currency guards so foreign holdings get unrealized P&L / global value (FXR spec, PRs #63–#66) |
| Portfolio summary / dashboard | 🔲 Planned | Aggregate view across all accounts                                                                                                                                                   |

Specs: `docs/spec/account-details.md`, `docs/spec/market-price.md`, `docs/spec/fx-rate.md`

---

## Phase 4 — Corporate Events

Non-trade events that alter quantity or value without a cash exchange at the position level.

| Operation               | Status     | Notes                                                                                        |
| ----------------------- | ---------- | -------------------------------------------------------------------------------------------- |
| Cash dividend           | ✅ Done    | `TransactionType::Dividend`; credits cash, holding untouched (DIV spec)                      |
| Free share distribution | ✅ Done    | `TransactionType::FreeShares`; adds units at zero cost, no cash leg, dilutes VWAP (FSD spec) |
| Stock split             | 🔲 Planned | Multiplies quantity, divides price; requires dedicated operation                             |
| Reverse split           | 🔲 Planned | Divides quantity, multiplies price                                                           |
| Merger / acquisition    | 🔲 Planned | Asset substitution; complex; out of scope until sell is implemented                          |

> Cash dividends (DIV) and free-share distributions (FSD) shipped. The remaining events (splits, reverse splits, merger, plus stock dividends / DRIP / return-of-capital — see `docs/spec/cash-dividend.md` § Deferred) are not specced yet.

---

## Phase 5 — Disposition (Sell)

Record a sale that reduces or closes a position.

| Operation                    | Status  | Notes                                                                              |
| ---------------------------- | ------- | ---------------------------------------------------------------------------------- |
| Sell transaction type        | ✅ Done | `TransactionType::Sell` variant; backend + frontend (SEL-010)                      |
| Sell form                    | ✅ Done | `SellTransactionModal` from Account Details holding row (SEL-010)                  |
| Holding quantity decrease    | ✅ Done | `recalculate_holding` decreases qty on sell (SEL-025)                              |
| Sell validation              | ✅ Done | Oversell guard frontend + backend (SEL-021, SEL-022)                               |
| Realized P&L computation     | ✅ Done | `(sell_proceeds − vwap × qty)` crystallized at sell time (SEL-024)                 |
| Realized P&L display         | ✅ Done | Per sell row in transaction list + cumulated per holding in ACD (SEL-038, SEL-042) |
| Partial sell                 | ✅ Done | Holding remains open; VWAP unchanged by sells (SEL-027)                            |
| Edit/delete sell transaction | ✅ Done | Full chronological recalculation on edit/delete (SEL-030, SEL-031, SEL-033)        |

> Prerequisite: sell transaction type must be introduced before corporate events (phase 4) can be fully modelled.

---

## Phase 6 — Position Closure

A position is closed when `Holding.quantity` reaches zero.

| Operation                       | Status     | Notes                                                                            |
| ------------------------------- | ---------- | -------------------------------------------------------------------------------- |
| Zero-quantity holding retention | ✅ Specced | Holding kept in DB at `quantity = 0`; `average_price` preserved (TRX-040)        |
| Closed position exclusion       | ✅ Done    | `quantity > 0` filter in Account Details (ACD-020)                               |
| "All positions closed" state    | ✅ Done    | Distinct empty-state message in Account Details (ACD-034)                        |
| Closed position history         | ✅ Done    | View past closed positions with realized P&L per position (ACD-044–ACD-050)      |
| Cumulative realized P&L         | ✅ Done    | `total_realized_pnl` summed across all holdings; shown in account summary header |

---

## Phase 7 — Archive

An asset can be archived once it has no active positions, preserving its historical data.

| Operation                  | Status  | Notes                                                                    |
| -------------------------- | ------- | ------------------------------------------------------------------------ |
| Archive action             | ✅ Done | Available from asset table                                               |
| Auto-unarchive on new buy  | ✅ Done | Transparent to user, with confirmation dialog (TRX-028, TRX-029)         |
| Archive eligibility guard  | ✅ Done | Block archiving if any `Holding.quantity > 0` across all accounts (OQ-6) |
| Archived asset in holdings | ✅ Done | Included in Account Details as long as `quantity > 0` (ACD-021)          |

---

## Implementation Order (Recommended)

Based on value and dependency chain:

1. ~~**Delete confirmation UI**~~ — ✅ done
2. ~~**Transaction list view**~~ — ✅ done
3. ~~**Inline asset creation from Account Details**~~ — ✅ done
4. ~~**Sell transaction**~~ — ✅ done (SEL spec)
5. ~~**Realized P&L**~~ — ✅ done (per transaction + per holding, SEL-024/SEL-038/SEL-042)
6. ~~**Archive eligibility guard**~~ — ✅ done (`ArchiveAssetUseCase`, OQ-6)
7. ~~**Current market price**~~ — ✅ done (manual entry, MKT spec)
8. ~~**Unrealized P&L + performance %**~~ — ✅ done
9. **Corporate events (dividends, splits)** — cash dividends ✅ done (DIV spec); stock dividends / DRIP / splits / return-of-capital / withholding-tax breakdown remain deferred (see `docs/spec/cash-dividend.md` § Deferred)
10. **Portfolio dashboard** — aggregation of phases 3–6; last to implement
