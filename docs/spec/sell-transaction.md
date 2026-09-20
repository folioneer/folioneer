# Business Rules — Sell Transaction (SEL)

## Context

A sell transaction represents a financial event in which the user disposes of some or all units of an asset held in an account. It reduces the `Holding.quantity` for the relevant `(account_id, asset_id)` pair and crystallizes a realized profit or loss at the moment of the sale.

This spec extends the `Transaction` entity defined in the TRX spec (`docs/spec/financial-asset-transaction.md`) by activating the `Sell` transaction type and specifying the computation and display of realized P&L. The orchestration logic resides in the existing `use_cases/record_transaction/` use case, which already handles cross-context atomicity across `transaction/`, `account/`, and `asset/` bounded contexts.

All financial values are stored as `i64` micro-units per [ADR-001](../adr/001-use-i64-for-monetary-amounts.md).

---

## Entity Definition

### Transaction (extended)

The `Transaction` entity is defined in the TRX spec. This spec adds one field activated only for `Sell` transactions:

| Field          | Business meaning                                                                                                                                    |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `realized_pnl` | Net profit or loss crystallized at sell time, in account currency (micros). Positive = gain, negative = loss. Present only for `Sell` transactions. |

All other fields (`id`, `account_id`, `asset_id`, `transaction_type`, `date`, `quantity`, `unit_price`, `exchange_rate`, `fees`, `total_amount`, `note`) retain the definitions from the TRX spec.

> For `Sell` transactions, `total_amount` represents the net proceeds received (gross proceeds minus fees), denominated in account currency — the inverse sign convention from `Purchase` where `total_amount` represents total cost including fees.

---

## Business Rules

### Eligibility and Initiation (010–019)

**SEL-010 — Sell entry point (frontend)**: A sell can be initiated from the "Account Details" view via a "Sell" action button on each holding row. The button is visible only when `Holding.quantity > 0` (zero-quantity holdings are excluded from display by ACD-020). The button is disabled and visually greyed out when the asset is archived, to prevent sells on potentially stale positions until the archive eligibility guard (OQ-6) is enforced.

**SEL-011 — Contextual pre-filling (frontend)**: When initiated from a holding row, both the account and the asset are pre-filled in the sell form and cannot be changed by the user.

**SEL-012 — Closed position guard (backend)**: The backend rejects a sell if the current `Holding.quantity` is zero for the `(account_id, asset_id)` pair at the time of submission, returning a specific error.

### Creation (020–029)

**SEL-020 — Sell field validation (backend)**: A sell transaction is valid if: `account_id` and `asset_id` exist; `date` satisfies the same bounds as TRX-020 (not in the future, not before `1900-01-01`); `quantity` is strictly positive; `unit_price` is positive or zero; `exchange_rate` is strictly positive; `fees` is zero or positive; and backend-computed `total_amount` is positive.

**SEL-021 — Oversell guard (backend)**: The backend rejects the sell if the submitted `quantity` exceeds the current `Holding.quantity` for the `(account_id, asset_id)` pair at the moment of processing, returning a specific error message.

**SEL-022 — Maximum quantity hint (frontend)**: The sell form displays the current `Holding.quantity` as the maximum sellable quantity. When the entered quantity exceeds this value, the form shows an inline validation error and the Save button is disabled until corrected.

**SEL-023 — Sell total amount formula (backend)**: `total_amount` for a sell is computed by the backend as `floor(floor(quantity × unit_price / MICRO) × exchange_rate / MICRO) − fees`. Fees reduce the net proceeds. All values are `i64` micro-units (TRX-024); arithmetic uses `i128` intermediates to prevent overflow. `total_amount` is never received from the frontend. In total-entry mode the user-typed total is stored instead and this formula does not apply — see SEL-050.

**SEL-024 — Realized P&L computation (backend)**: When a sell transaction is persisted, the backend computes `realized_pnl = total_sell_amount − floor(Holding.average_price × sold_quantity / MICRO)`, where `Holding.average_price` is the VWAP state immediately before this sell in the full chronological recalculation sequence for the `(account_id, asset_id)` pair. Chronological order is defined as `date ASC, created_at ASC` — when two transactions share the same date, the one with the earlier `created_at` timestamp is processed first. The `transactions` table must include a `created_at TEXT NOT NULL` column storing ISO 8601 timestamps, defaulting to `datetime('now')` on insert. Both terms use `i128` intermediates and integer floor division before scaling back to `i64`. The result is recorded with the transaction. `realized_pnl` is never received from the frontend.

**SEL-025 — Holding quantity decrease (backend)**: A sell transaction decreases `Holding.quantity` by the sold quantity.

**SEL-026 — Zero quantity retention (backend)**: When `Holding.quantity` reaches zero after a sell, the `Holding` record is retained in the database with `quantity = 0` and `average_price` preserved at its last value (per TRX-040). Exclusion of zero-quantity holdings from the active holdings display is already enforced by ACD-020 and requires no action from this use case.

**SEL-027 — VWAP unchanged by sells (backend)**: Sell transactions do not modify `Holding.average_price`. VWAP recalculation (TRX-030) includes only `Purchase` and `OpeningBalance` transactions (TRX-048); `Sell` transactions are excluded from the cost basis computation.

**SEL-028 — Atomicity (backend)**: The transaction insert, `Holding.quantity` update, and `realized_pnl` computation are performed within a single database transaction. A failure in any step rolls back the entire operation.

**SEL-029 — Sell form default values (frontend)**: The date defaults to the current day. `exchange_rate` defaults to `1.0`. `fees` defaults to `0`. The `transaction_type` is implicitly `Sell` and is not displayed as an editable field.

**SEL-036 — Exchange rate field visibility (frontend)**: The Exchange Rate field in the sell form is visible only when the asset's currency differs from the account's currency. When both currencies are the same, the field is hidden and `exchange_rate` is implicitly `1.0`, consistent with the purchase form (TRX-023).

**SEL-037 — Archived asset sell guard (backend + frontend)**: The backend rejects a sell submission if the asset is archived at the time of processing. The frontend disables the Sell button (SEL-010) when the asset is archived as a defensive guard, since the archive eligibility guard (OQ-6 in asset spec) is not yet enforced and an archived asset could theoretically still carry a position. Once OQ-6 is implemented, this guard becomes redundant but remains harmless.

**SEL-038 — Realized P&L aggregation service method (backend)**: `TransactionService` exposes a method that returns the sum of `realized_pnl` across all sell transactions grouped by `asset_id` for a given `account_id`. When no sell transactions exist for an `(account_id, asset_id)` pair, the method returns `0` for that asset. If the query fails, the error is propagated to the use case, which returns an error response; the frontend transitions to the error state (ACD-038). This method is called by `use_cases/account_details/` to populate the `realized_pnl` field in `AccountDetailsResponse` per holding (SEL-042, ADR-005).

### Update and Deletion (030–039)

**SEL-030 — Edit sell validation (backend)**: When modifying a sell transaction, the same field constraints as SEL-020 apply. The oversell guard (SEL-021) is re-evaluated against the holding quantity as it stands immediately before this sell in the full chronological recalculation sequence for the `(account_id, asset_id)` pair.

**SEL-031 — Full recalculation on edit (backend)**: Modifying any field of a sell transaction triggers a full chronological recalculation of `Holding.quantity` and `realized_pnl` for all transactions (purchases and sells) in the `(account_id, asset_id)` pair, in the same manner as TRX-031 for purchases.

**SEL-032 — Cascading oversell detection on purchase edit (backend)**: If editing a purchase transaction reduces the quantity available at a later sell in the chronological sequence such that the sell would now exceed the holding at that point, the edit is rejected. The error message identifies the sell transaction that would become invalid.

**SEL-033 — Delete sell transaction (backend)**: Deleting a sell transaction triggers a full recalculation of the `Holding` and `realized_pnl` for the `(account_id, asset_id)` pair. If no transactions remain for that pair, the `Holding` record is removed (per TRX-034).

**SEL-034 — Delete confirmation (frontend)**: Deleting a sell transaction requires explicit user confirmation in a dialog, consistent with TRX-035.

**SEL-035 — Transaction type immutability (backend)**: The `transaction_type` field is immutable once a transaction is saved. The backend rejects any edit attempt that changes `transaction_type` on an existing transaction. To change type, the user must delete and re-create the transaction.

### Display (040–049)

**SEL-040 — Sell type indicator in Transaction List (frontend)**: Sell transactions are displayed with a distinct "Sell" type label in the Transaction List view, visually differentiated from "Purchase" rows.

**SEL-041 — Realized P&L per sell row in Transaction List (frontend)**: Each sell transaction row in the Transaction List displays its `realized_pnl` value formatted as a decimal amount in account currency.

**SEL-042 — Cumulative realized P&L in Account Details (frontend + backend)**: The Account Details holdings table includes a "Realized P&L" column showing the sum of `realized_pnl` across all sell transactions for the current account and each asset. The `use_cases/account_details/` use case fetches this aggregation via `TransactionService` and includes it in `AccountDetailsResponse` per holding (per ADR-005). When no sell exists for a holding, the value is zero and the cell displays a neutral placeholder (e.g., "—").

**SEL-043 — P&L visual differentiation (frontend)**: A positive `realized_pnl` is displayed using the success/gain color token; a negative `realized_pnl` using the error/loss color token. A zero `realized_pnl` — whether from no sells or from gains and losses that cancel exactly — displays as a neutral placeholder (`—`), identical to the no-sells case (SEL-042).

**SEL-044 — Loading and error states (frontend)**: The sell form and all display components that show realized P&L follow the same loading skeleton and error-with-retry patterns as the rest of the application.

**SEL-045 — Success feedback (frontend)**: On successful sell creation, edit, or deletion, the form modal closes and a success snackbar is displayed. The Account Details holdings table (including the Realized P&L column) refreshes to reflect the updated state via the `TransactionUpdated` event (TRX-037, TRX-038).

### Total-Entry Mode (050–059)

**SEL-050 — Total-entry sell (backend)**: `sell_holding` accepts an optional user-entered `total_amount` — the all-in net proceeds credited by the broker, in account-currency micro-units, after fees. When provided, the typed total is ground truth: it is stored verbatim as `total_amount` (the SEL-023 formula does not apply) and the unit price is derived as `unit_price = round(((total_amount + fees) × MICRO × MICRO) / (quantity × exchange_rate))`, rounding half away from zero, with `i128` intermediates. Validation: `total_amount` must be strictly positive (`TotalAmountNotPositive`). When absent, SEL-023 applies unchanged. Realized P&L (SEL-024) consumes the stored `total_amount` unchanged. The entry mode is not persisted: a correction (TRX-031, SEL-030) operates on the stored decomposition (`quantity`, `unit_price`, `exchange_rate`, `fees`) and recomputes `total_amount` per SEL-023, unless the correction itself supplies a typed total (SEL-051). For purchases, see TRX-060.

**SEL-051 — Total-entry sell correction (backend)**: `correct_transaction` accepts an optional user-entered `total_amount` with the exact SEL-050 semantics. When provided on a `Sell` correction, the typed net proceeds are ground truth: they are stored verbatim as `total_amount` and the unit price is derived per the SEL-050 formula, with the same validation (`TotalAmountNotPositive`, `UnitPriceOutOfRange`); the caller-supplied `unit_price` is ignored, and realized P&L (SEL-024) is recomputed from the stored total. When absent, the existing recompute per SEL-023 applies unchanged. For purchase corrections, see TRX-061.

---

## Workflow

```
[User clicks "Sell" on a holding row in Account Details]
  → Form opens with Account + Asset pre-filled (SEL-011)
  → Max quantity shown as hint (SEL-022)
  │
  ├─ [Enter Quantity] (≤ current holding, SEL-022)
  ├─ [Enter Unit Price]
  ├─ [Enter Exchange Rate] (default: 1.0)
  ├─ [Enter Fees] (default: 0)
  ├─ [Enter Date] (default: today)
  ├─ [Enter Note] (optional)
  │
  └─ [Save]
       → Backend validates fields (SEL-020)
       → Backend checks oversell guard (SEL-021)
       → Backend computes total_amount (SEL-023)
       → Backend computes realized_pnl (SEL-024)
       → Persists Transaction atomically (SEL-028)
         → Decreases Holding.quantity (SEL-025)
         → Retains Holding at qty=0 if applicable (SEL-026)
       → Publishes TransactionUpdated (TRX-037)
       → Frontend refreshes holdings and P&L display (TRX-038)
```

---

## UX Draft

### Entry Point

"Sell" action button on each active holding row in the Account Details view. Visible only when `Holding.quantity > 0`.

### Main Component

**FormModal** — reuses the transaction form layout with the following fields:

- Account (pre-filled, read-only)
- Asset (pre-filled, read-only)
- Date (date picker, default: today)
- Quantity (number field, upper bound = current holding quantity shown as hint)
- Unit Price (amount field with asset currency suffix)
- Exchange Rate (visible only if asset currency ≠ account currency, default: 1.0)
- Fees (amount field with account currency suffix, default: 0)
- Total Proceeds (read-only, auto-calculated from SEL-023)
- Note (textarea, optional)

### States

- **Empty**: Fields at defaults; account and asset pre-filled.
- **Loading**: Form submission in progress (spinner, fields disabled).
- **Error**: Inline validation errors (quantity exceeds holding, invalid date, etc.) or backend rejection (oversell guard, closed position).
- **Success**: Modal closes; success snackbar; Account Details holdings refreshed.

### User Flow

1. User opens Account Details for an account.
2. User clicks "Sell" on a holding row with `quantity > 0`.
3. Sell form opens — account and asset pre-filled; maximum quantity displayed.
4. User enters quantity, unit price, fees, and date.
5. Total proceeds auto-calculated and shown read-only.
6. User clicks "Save".
7. On success: modal closes, holdings table updates, realized P&L appears in the holding row summary.
