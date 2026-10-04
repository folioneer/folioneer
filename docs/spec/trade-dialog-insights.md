# Business Rules — Trade Dialog Insights (TDI)

## Context

When recording a buy or sell, the user benefits from seeing two derived figures
computed from the holding's history, so they can judge the trade before
committing it:

1. The **average cost price** of the holding as it stands on the trade date —
   the VWAP cost basis the new trade builds on (buy) or sells against (sell).
2. On a sell, the **potential realized P&L** the typed sell would produce.

Both are read-only, recomputed-on-demand insights (ADR-013); they never persist
anything and never affect the transaction being recorded.

## Entity — Holding snapshot (as of a date)

A point-in-time reconstruction of a single (account, asset) holding, implied by
replaying only the transactions dated on or before a cut-off date.

| Field           | Type | Notes                                                                                                                                        |
| --------------- | ---- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `quantity`      | i64  | Units held as of the date (micro-units), 0 when nothing is held.                                                                             |
| `average_price` | i64  | VWAP cost basis per unit in **account** currency (micro-units; fees + FX included, identical to `Holding.average_price`), 0 when never held. |

## Business Rules

### Query (010–019)

**TDI-010 — As-of-date snapshot**: `get_holding_snapshot_as_of(account_id,
asset_id, date)` returns the holding's `quantity` and `average_price` implied by
replaying every transaction for that (account, asset) pair dated on or before
`date`, using the same chronological VWAP algorithm as `recalculate_holding`
(TRX-040 / SEL-026). The transaction currently being entered is not part of the
history (it is unsaved), so the snapshot reflects the state the trade acts on.

**TDI-011 — Inclusive cut-off**: a transaction dated exactly on `date` is
included in the snapshot.

**TDI-012 — Valid date required**: `date` must parse as ISO `YYYY-MM-DD`;
otherwise the query rejects with `InvalidDate`. A future date is accepted (it
simply includes all history, since no transaction may be dated in the future
per TRX-021).

**TDI-013 — No oversell guard**: the snapshot is a read-only valuation of
already-validated history, so it does not run the oversell / insufficient-cash
guards that the mutation path (`recalculate_holding`) enforces.

### Average-cost display (020–029)

**TDI-020 — Average cost on the buy/sell dialog**: the buy and sell dialogs show
the holding's `average_price` as of the entered trade date (or today when no
date is entered) as an info line under the unit-price field. It is the same
account-currency cost basis the holdings table shows (fees + FX included), shown
as a plain number without an explicit currency symbol, consistent with the
read-only total field on the same dialog.

**TDI-021 — Hidden when not held**: when the snapshot `quantity` is 0 (nothing
held as of the date), no average-cost line is shown — there is no cost basis to
display.

### Potential P&L display (030–039)

**TDI-030 — Potential P&L on the sell dialog (backend + frontend)**: the sell dialog
shows the potential realized P&L of the typed sell as an info line under the computed
total proceeds. The draft check of a new sale (TRX-062) returns it:
`total_proceeds − floor(average_price × quantity / 1_000_000)`, where `average_price` is
the holding's VWAP as of the sell date (TDI-010) — the rule recording a sell applies
(SEL-024), computed by the same code. The dialog shows the figure it is given and
computes none.

**TDI-031 — Shown only when computable (backend + frontend)**: the draft check returns a
potential P&L only for a new sale whose position, as of the sell date, holds at least the
quantity sold. It returns none — and the check stands — when nothing is held at that
date, when less is held than is sold, or when that position cannot be read; and none for
a purchase, a sale being corrected, or a draft that has a problem. The dialog shows the
line when a figure is returned and hides it otherwise.

**TDI-032 — Sign-coloured**: a gain renders in the success colour, a loss in the
error colour, consistent with the realized-P&L column elsewhere (SEL-043).
