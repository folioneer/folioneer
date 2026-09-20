# Business Rules — Free Share Distribution (FSD)

## Context

A Free Share Distribution records additional shares of a held asset received **at no cost** — a bonus issue, stock dividend paid in shares, or an employer/loyalty share attribution (_attribution gratuite d'actions_). It is the second of the roadmap's Phase 4 "Corporate Events", the share-quantity sibling of the Cash Dividend (DIV): where a dividend credits cash and leaves the position untouched, a free distribution **increases the position's quantity and moves no cash at all**.

The feature adds a new free-shares variant to `TransactionType` (joining `Purchase | Sell | OpeningBalance | Deposit | Withdrawal | Dividend`). A free-share distribution is attributed to the distributing asset (its `asset_id` is that asset, so it surfaces under that asset in the transaction list) and its sole effect is `Holding.quantity += quantity` — the total cost basis is unchanged, so the average price (VWAP) dilutes. It touches the `account` bounded context (Transaction, Holding, the Account aggregate root) and reuses the chronological-replay machinery defined in TRX (TRX-031/036).

Quantities are `i64` micro-units ([ADR-001](../adr/001-use-i64-for-monetary-amounts.md), TRX-024); the holding mutation and the transaction insert commit atomically within the existing Unit of Work ([ADR-006](../adr/006-unit-of-work.md)).

This v1 covers **zero-cost distributions only** (the user-confirmed model: no invested capital is added; the position's value is conserved and the market feed reflects any ex-distribution price adjustment). Stock splits and reverse splits (ratio-based quantity changes), a user-declared fiscal attribution value (cost-basis-increasing), distributions on closed positions, and a per-holding-row shortcut are each deferred (see Open Questions / Deferred).

---

## Entity Definition

### Free Share Distribution (new `TransactionType` variant)

A quantity-only corporate event: shares of a held asset received for free, recorded against that asset.

| Field              | Business meaning                                                                                                                  |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------- |
| `transaction_type` | The free-shares variant of `TransactionType`.                                                                                     |
| `account_id`       | The account whose holding receives the shares.                                                                                    |
| `asset_id`         | The **distributing asset** — the holding the free shares are added to. **Not** the Cash Asset.                                    |
| `date`             | Business date the shares were received (must not be in the future, must not be older than the existing TRX lower bound, TRX-020). |
| `quantity`         | Number of free shares received (strictly positive, micro-units per TRX-024 — fractional quantities allowed, as for a Purchase).   |
| `note`             | Optional free-text note.                                                                                                          |

> A free-share distribution carries **no monetary amount, no unit price, no exchange rate, and no fees** — no money changes hands. Fields that exist on `Transaction` but carry no business meaning for a distribution follow a fixed convention rather than user input; their exact packing is a contract/plan concern, not a business rule.

---

## Business Rules

### Eligibility and Initiation (010–019)

**FSD-010 — Entry point (frontend)**: The free-shares recording flow is initiated from the Account Details header's consolidated "Record" menu (DIV-012), via a "Free shares" item that opens the free-shares modal (FSD-020). The distributing asset is chosen inside the modal, not derived from a holding row. v1 does **not** add a per-holding-row action (deferred — see Open Questions / Deferred).

**FSD-011 — Eligibility (backend)**: A free-share distribution may be recorded only for an `(account, asset)` pair where the account currently holds the asset with `quantity > 0` and the asset is not a Cash Asset. The action is rejected with a specific error when the account is unknown, the asset is unknown, the asset is not currently held in that account (no active holding), or the asset is a Cash Asset.

### Recording a Distribution (020–029)

**FSD-020 — Form fields (frontend)**: The Record-free-shares form accepts an **asset selector** listing the account's active (`quantity > 0`), non-cash holdings (the distributing asset; the Cash Holding is excluded), `date` (default: today), the **quantity of free shares received** (strictly positive decimal, same granularity as a Buy quantity — TRX-024), and an optional `note`. There is no amount, unit-price, exchange-rate, or fees input — no money moves.

**FSD-021 — Input validation (frontend + backend)**: The quantity must parse as a strictly positive decimal (`> 0`); the `date` must be a well-formed ISO 8601 calendar date that is not in the future and not older than the TRX lower bound (TRX-020). The frontend validates inline and disables submit until valid; the backend re-validates and rejects with explicit error variants.

**FSD-022 — Recording effect (backend)**: Recording a free-share distribution, within a single Unit of Work ([ADR-006](../adr/006-unit-of-work.md)): (a) increases the holding's `quantity` by the distributed quantity; (b) leaves the holding's total cost basis **unchanged**; (c) persists the Transaction with the free-shares type attributed to the distributing asset; (d) leaves the account's Cash Holding **untouched** — a distribution has no cash leg. All steps commit together or all roll back.

**FSD-023 — Zero acquisition cost and average-price dilution (backend)**: The free shares add **no invested capital**: the position's underlying cost (as recorded in the transaction log) is exactly what it was before the distribution, so the average price becomes `floor(cost_basis / new_quantity)` (diluted, following the established TRX-026 floor convention — the derived cost-basis display may therefore round down by up to one micro-unit per share, as for any non-dividing purchase). Recording a distribution never contributes to realized P&L.

**FSD-024 — No effect on recorded market price (backend)**: Recording a distribution **does not create or modify any `AssetPrice` record** for the asset. Any ex-distribution adjustment in the market price is reflected by the price-fetch feed (or manual entry), never synthesised here — mirroring DIV-027. The position's value conservation (more shares × adjusted price) is the market's business, not this feature's.

**FSD-025 — In-flight, success, and error feedback (frontend)**: While the request is in progress the submit button is disabled and shows a spinner. On success the form closes and a snackbar confirms "Free shares recorded". On validation failure or backend rejection the form stays open with an inline error adjacent to the offending field; the user can correct and resubmit.

**FSD-026 — Reactivity (frontend + backend)**: Recording a distribution publishes the existing `TransactionUpdated` event (per the AccountService convention, as Dividend does in DIV-026). The Account Details view re-fetches on that event (ACD-039), so the holding row's quantity, diluted average price, and recomputed performance figures appear without a manual refresh.

**FSD-027 — Interaction with later sells (backend)**: A distribution participates in the chronological replay (TRX-031/036) at its `date` like any other transaction: sells dated **after** the distribution compute their realized P&L against the **diluted** average price; sells dated **before** it are unaffected. Same-date ordering follows the existing TRX-036 convention.

**FSD-028 — Reversibility (backend)**: A distribution's entire effect is derived from the transaction log via the standard chronological replay (TRX-031/036) — it never irreversibly mutates any derived value. Deleting a distribution restores the holding **exactly** to its pre-distribution state (quantity, average price, cost basis). Independently verifiable by a record → delete → compare test.

### Edit and Delete (040–049)

**FSD-040 — Edit (frontend + backend)**: A recorded distribution can be edited through the existing transaction-correction flow (TXL). Editing re-applies the chronological replay for the `(account, asset)` pair (TRX-031/036). Editable fields: `date`, `quantity`, and `note`. The distributing asset (`asset_id`) is **immutable** on edit — changing it requires deleting and re-recording. The FSD-011 eligibility guards are re-evaluated on edit. A successful edit publishes `TransactionUpdated` (FSD-026).

**FSD-041 — Delete (frontend + backend)**: A recorded distribution can be deleted through the existing cancel flow. Deletion triggers a chronological replay with the free shares removed. If removing them would drive the running holding quantity below what any later Sell consumes (i.e. a later sell would oversell), the deletion is rejected with a specific error — the same replay integrity guard that protects sells today. A successful delete publishes `TransactionUpdated` (FSD-026).

### Display (050–059)

**FSD-050 — Transaction list inclusion (frontend)**: Free-share distributions appear in the transaction list (TXL) when the user filters by the **distributing asset** for that account (cross-amends TXL-023 for the Type column label "Free shares", and TXL-022 for the `—` placeholder columns). The Quantity column shows the distributed quantity; the unit-price, total-amount, and Realized P&L columns render the neutral placeholder `—` (no money moved). When filtering by a different asset, the distribution does not appear.

**FSD-051 — Holding row reflection (frontend)**: After recording, the Account Details holding row shows the increased quantity and the diluted average price; the cost basis figure is unchanged. Unrealized P&L and performance % recompute through the existing MKT math with the new quantity — no special-casing. A distributing asset whose currency differs from the account currency rides the existing currency-mismatch path (MKT-034) unchanged — its P&L columns render `—` exactly as before the distribution.

### Performance (070–079)

**FSD-070 — Performance neutrality (backend)**: A free-share distribution is **not an external cash flow**: account performance (PRF) applies no flow adjustment for it — any value impact surfaces as market movement. It is likewise **not dividend income**: it is excluded from `dividends_received` and `total_return_pct`'s dividend term (DIV-070/071); its contribution to return flows through the position's value (quantity × price) instead. The distribution's added units **do** enter PRF's as-of-date holding reconstruction through the standard transaction replay (PRF-021) — only the cash-flow adjustment is excluded, never the position itself.

---

## Workflow

```
Account Details header → "Record" menu → "Free shares" (FSD-010, DIV-012)
  → modal: asset selector (active non-cash holdings), date (today),
           quantity of free shares, note (FSD-020)
  → submit
      backend validate: quantity > 0, date in bounds, asset held (qty > 0), not cash (FSD-011/021)
      backend (single Unit of Work, ADR-006):
        ├─ holding.quantity += quantity                                  (FSD-022a)
        ├─ cost basis unchanged → average price dilutes                   (FSD-022b/023)
        ├─ Cash Holding untouched (no cash leg)                          (FSD-022d)
        └─ persist Transaction(type=free shares, asset_id=distributor)   (FSD-022c)
      publish TransactionUpdated                                         (FSD-026)
  → modal closes + snackbar "Free shares recorded"                       (FSD-025)
  → Account Details re-fetches → quantity ↑, average price ↓, perf recomputed (FSD-026/051)

Transaction list (filtered by the distributing asset)
  → row shows Type = "Free shares", qty = +N, money columns = "—"        (FSD-050)
  → edit (date / quantity / note) → chronological replay                 (FSD-040)
  → delete → replay without the shares; rejected if a later sell oversells (FSD-041)
```

---

## UX Draft

### Entry Point

A "Free shares" item in the Account Details header's consolidated "Record" dropdown menu (DIV-012), alongside New position and Dividend (cash Deposit/Withdraw live on the cash row, not this menu — DIV-012). Selecting it opens the free-shares modal. (A per-holding-row action is deferred — FSD-010.)

### Main Component

A small `FormModal` — the free-shares modal:

- Asset selector — the account's active, non-cash holdings (the distributing asset).
- Date (default today, `DateField`).
- Quantity of free shares (strictly positive decimal).
- Note (optional `TextField`).
- Submit / Cancel.

### States

- **Idle**: form with today's date, empty quantity, submit disabled until valid.
- **In-flight**: submit disabled + spinner (FSD-025).
- **Validation / backend error**: inline error adjacent to the field; modal stays open (FSD-025).
- **Success**: modal closes; snackbar "Free shares recorded"; Account Details re-fetches — quantity rises, average price dilutes, performance recomputes (FSD-026/051).

### User Flow

1. User opens Account Details for an account holding 50 shares of an asset.
2. User opens the header "Record" menu and chooses "Free shares".
3. Modal opens; user picks the distributing asset from their active holdings, enters the number of free shares received (e.g. 5), adjusts the date if needed.
4. User submits.
5. Backend validates, increases the holding quantity to 55, leaves the cost basis unchanged (average price dilutes), moves no cash, persists the distribution, publishes `TransactionUpdated`.
6. Modal closes, snackbar confirms; the holding row shows 55 shares at the diluted average price; the transaction list shows the distribution under that asset with Type "Free shares".

---

## Open Questions / Deferred

**Deferred to future specs** (explicitly out of v1 scope, agreed with the user):

- **Stock splits / reverse splits** — ratio-based quantity changes (×2, ÷10) affecting the whole position; share most of this feature's mechanics but need ratio semantics and historical-price presentation decisions.
- **User-declared attribution value** — an optional per-share reference value (e.g. the fiscal acquisition value of an _attribution gratuite_) that would increase the cost basis instead of diluting it. v1 is strictly zero-cost (user-confirmed).
- **Distributions on closed positions** — recording a distribution declared while held but settled after the position was fully sold (v1 requires an active holding, `quantity > 0` — FSD-011, mirroring DIV).
- **Per-holding-row action** — a contextual "Record free shares" action directly on a holding row (asset pre-selected). v1 initiates from the header "Record" menu with an in-modal asset picker (FSD-010).
- **Merger / acquisition asset substitution** — roadmap Phase 4 item; unrelated mechanics (asset swap), out of scope.

None — all questions have been resolved.
