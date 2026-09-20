# Business Rules — Interest Credit (INT)

## Context

Fund-like assets — the driving case is the euro fund inside a French assurance
vie — credit a periodic interest whose rate varies per year. The interest is
**capitalized**: the fund amount itself grows; no cash is paid out. Interest can
also be credited on the account's cash line (interest-bearing cash).

The `Interest` transaction type is the crediting mirror of the ManagementFee
deduction (FEE): a quantity increase at zero cost, reusing the FreeShares
mechanics (FSD-022/023) — with two differences: the cash line is a valid target,
and the entry form accepts either a rate or a direct amount.

---

## Rules

**INT-010 — Entry point (frontend)**: the account-details header's Record action
row gains a "Record interest" square icon button (`id="add-menu-interest"`),
opening the interest modal. Hidden in the read-only as-of view. Not gated by the
management-fees account parameter (INT-050).

**INT-011 — Preconditions (backend)**: the account must exist
(`AccountNotFound`); the target asset must exist (`AssetNotFound`) and be either
currently held (`quantity > 0`) or the account's own Cash Asset (INT-023). A
non-held, non-cash asset is rejected with `AssetNotHeld`.

**INT-012 — Interest-bearing eligibility (backend)**: a non-cash target must
carry the asset's `interest_bearing` opt-in flag (AST-024); a non-flagged
non-cash asset is rejected with `InterestNotEligible`, checked before the
INT-011 held check. The account's Cash Asset is always eligible regardless of
the flag (INT-023) — the eligibility check is skipped for a Cash-class asset.

**INT-020 — Form (frontend)**: asset selector listing the active
`interest_bearing`-flagged non-cash holdings (AST-024 / INT-012) **plus the
always-eligible cash line**, a date, an interest **percentage** field and a
direct **quantity** field (exactly one of the two must be filled, INT-021), and
an optional note.

**INT-021 — Amount validation (backend)**: exactly one of `percent_micros` /
`quantity_micros` must be provided; both or neither → `InterestAmountInvalid`.
Percent mode: strictly positive (`PercentageNotPositive`) and at most 100%
micro-percent (`PercentageAboveHundred`). Quantity mode: strictly positive
(`QuantityNotPositive`). Date follows the TRX-020 bounds.

**INT-022 — Percent computation (backend)**: credited quantity =
`floor(holding_qty_as_of(date) × percent_micros / 100_000_000)` — the holding
quantity as of the interest date, mirroring FEE-022a in the crediting direction.
A computed credit of 0 (empty holding or rate too small) → `QuantityNotPositive`.

**INT-023 — Cash-line interest (backend)**: an Interest whose target is the
account's Cash Asset credits the cash balance by `quantity` (account-currency
micros) with no Deposit recorded — the journal distinguishes it by its type. The
cash replay treats `Interest` on the cash asset as a credit of `quantity`;
Interest on a non-cash asset never touches cash. The cash line carries no cost
basis and no unrealized P&L, so the zero-cost packing cannot distort either.

**INT-024 — Zero-cost mechanics (backend)**: for a non-cash asset the credited
quantity is added at zero cost — the VWAP numerator is unchanged, so the average
price dilutes to `cost_basis / new_quantity` (the FSD-023 mechanics; the
interest gain surfaces as unrealized P&L). Wire packing mirrors FreeShares:
`unit_price = 0`, `exchange_rate = 1_000_000`, `fees = 0`, `total_amount = 0`,
`realized_pnl = None`.

**INT-025 — Reactivity (backend + frontend)**: a persisted Interest publishes
`TransactionUpdated`, so the account views re-fetch (ACD-039).

**INT-030 — Journal rendering (frontend)**: the transaction list shows the
localized "Interest" type label; the unit-price and total-amount cells render
the neutral placeholder (extends the FSD-050 / FEE-055 quantity-only
convention). Quantity still shows the credited units.

**INT-040 — Correction (frontend + backend)**: editing an Interest opens a
dedicated edit-interest modal via the URL-driven shell mount
(`?modal=edit-interest`), mirroring the FreeShares edit flow (FSD-040): asset
locked, date / quantity / note editable, submitted through `correct_transaction`
with the Interest zero-cost packing preserved.

**INT-041 — Deletion (backend)**: standard transaction deletion applies; the
replay guards reject a deletion that would drive later sells into
`CascadingOversell`.

**INT-050 — Independence from the fee parameter**: the Interest surfaces are NOT
gated by the account's `management_fees_enabled` parameter (FEE-075) — interest
crediting is available on every account.

---

## Out of scope (v1)

- Recurring interest schedules (the rate varies per year; a one-off entry per
  crediting is the workflow). Revisit if a fixed-rate use case appears.
- A dedicated "interest received" reporting figure (column/total). The journal
  type carries the information; aggregation can follow later.
- E2E scenario — covered at the unit/integration tiers; the flow reuses
  UI primitives (SelectField/DateField/CalcField) already exercised by the FSD
  and FEE E2E suites.
