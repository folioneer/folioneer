# Business Rules — Financial Asset Transaction (TRX)

## Context

A `Transaction` represents a financial event affecting an asset's quantity and cost basis within a specific account. This feature allows users to record purchases (and later sales) of financial assets (Stocks, ETF, Digital Assets, etc.). These transactions are the source of truth for calculating current holdings (quantity and average purchase price) and historical performance.

The `Transaction` entity owns its own bounded context (`context/transaction/`). As this feature spans multiple bounded contexts (`transaction/`, `account/`, and `asset/`), the orchestration logic resides in a dedicated Use Case within `src-tauri/src/use_cases/`, ensuring atomic updates across entities. The use case calls `TransactionService` (persists the transaction, publishes `TransactionUpdated`), then calls `AccountService` (updates the Holding); it queries `asset/` only for existence and archive-status checks.

### Holding entity

The `Holding` entity represents the current state of a financial position: an asset held within an account. It is owned by the `account/` bounded context and replaces the former `AssetAccount` entity (see [ADR-002](../adr/002-replace-asset-account-with-holding.md)). All financial fields are stored as `i64` micro-units per [ADR-001](../adr/001-use-i64-for-monetary-amounts.md).

---

## Entity Definition

### Transaction

Represents a single purchase (or sale) event for an asset in an account.

| Field              | Business meaning                                                                              |
| ------------------ | --------------------------------------------------------------------------------------------- |
| `id`               | Unique identifier of the transaction.                                                         |
| `account_id`       | The account where the transaction occurred.                                                   |
| `asset_id`         | The financial asset involved in the transaction.                                              |
| `transaction_type` | Type of transaction: `Purchase`, `Sell`, or `OpeningBalance`. Immutable once saved (SEL-035). |
| `date`             | Date when the transaction was executed.                                                       |
| `quantity`         | Number of units acquired (positive, stored in micros: value \* 10^6).                         |
| `unit_price`       | Price per unit in asset's currency (stored in micros: value \* 10^6).                         |
| `exchange_rate`    | Exchange rate between asset currency and account currency (micros).                           |
| `fees`             | Transaction fees in the account's currency (stored in micros).                                |
| `total_amount`     | Total cost in account's currency (incl. fees, stored in micros).                              |
| `note`             | Optional user comment.                                                                        |

### Holding

Represents the current state of a position (asset held within an account). Computed from transactions.

| Field           | Business meaning                                                         |
| --------------- | ------------------------------------------------------------------------ |
| `id`            | Unique identifier of the holding.                                        |
| `account_id`    | The account holding the asset.                                           |
| `asset_id`      | The financial asset held.                                                |
| `quantity`      | Current number of units held (i64 micros).                               |
| `average_price` | Volume-weighted average purchase price in account currency (i64 micros). |

---

## Business Rules

### Eligibility and Initiation

**TRX-010 — Purchase entry point (frontend)**: A purchase can be initiated from three entry points: (1) the "Assets" table (contextual action on an asset) — navigates to the full transaction form page; (2) the "Account Details" view standalone button — navigates to the full transaction form page; (3) a holding row in "Account Details" — opens a `BuyTransactionModal` (inline modal, both account and asset pre-determined). Entry points (1) and (2) use page navigation because account or asset selection is still needed. Entry point (3) uses a modal because both contexts are already fixed, consistent with the sell flow (SEL-010).

**TRX-011 — Contextual pre-filling (frontend)**: When initiated from a specific asset (entry point 1), the asset is pre-selected and the user selects the account. When initiated from the account standalone button (entry point 2), the account is pre-selected and the user selects the asset. When initiated from a holding row (entry point 3), both account and asset are pre-filled and read-only — no selection needed.

### Creation

**TRX-020 — Field validation (backend)**: A transaction is valid if: `account_id` and `asset_id` exist, `date` is not in the future and not before `1900-01-01`, `quantity` is strictly positive, `unit_price` is positive or zero, `exchange_rate` is strictly positive, and the backend-computed `total_amount` is positive.

**TRX-021 — Multi-currency semantics (backend)**: The `unit_price` is stored in the asset's native currency. The `exchange_rate` is the conversion rate from the asset's currency to the account's currency, and is stored explicitly with the transaction.

**TRX-022 — Holding quantity update (backend)**: Creating a purchase transaction increases the `Holding.quantity` for the specified asset and account.

**TRX-023 — Form default values (frontend)**: The transaction date defaults to the current day. The `transaction_type` defaults to `Purchase` and is not displayed in the form while only `Purchase` is supported. `exchange_rate` defaults to 1.0.

**TRX-024 — Micro-unit representation (full stack)**: All financial amounts (quantity, price, fees, total) are represented as 64-bit integers (`i64`) using a micro-unit scale (×1,000,000) throughout the stack, as per [ADR-001](../adr/001-use-i64-for-monetary-amounts.md). The frontend stores and manipulates these values internally as micro-units. The only decimal↔micro conversion occurs at the UI boundary: user input (decimal string → `i64` micro) and display (`i64` micro → formatted decimal string, 3 decimal places).

**TRX-025 — Holding cost basis update (backend)**: Creating a purchase transaction updates the `Holding.average_price` using the VWAP method (TRX-030).

**TRX-026 — Total amount computation for purchases (backend)**: For `Purchase` transactions, `total_amount` is computed by the backend as `floor(floor(quantity × unit_price / MICRO) × exchange_rate / MICRO) + fees` (fees increase cost). All values are `i64` micro-units (TRX-024); arithmetic uses `i128` intermediates to prevent overflow. `total_amount` is never received from the frontend — the DTO (`CreateTransactionDTO`) intentionally omits it. The frontend computes the same formula locally for real-time display preview only (see TRX-024). For `Sell` transactions, the formula differs in fee sign — see SEL-023. In total-entry mode the user-typed total is stored instead and this formula does not apply — see TRX-060.

**TRX-027 — Atomicity of transaction and holding updates (backend)**: The transaction record insert and all associated `Holding` mutations (quantity and average_price) must be performed within a single database transaction. A failure in any step rolls back the entire operation.

**TRX-028 — Archived asset auto-unarchive on purchase (backend)**: For `Purchase` transactions only — if the referenced asset is archived at the time of transaction creation or modification, the use case atomically unarchives the asset and persists the transaction in a single database operation. The archived flag is reverted if the transaction fails. `Sell` transactions are explicitly excluded: selling an archived asset is rejected (see SEL-037).

**TRX-029 — Archived asset confirmation (frontend)**: If the selected asset is archived, a confirmation dialog is shown before form submission, informing the user that saving will automatically unarchive the asset. The transaction is submitted only upon explicit user confirmation.

### Update and Deletion

**TRX-030 — VWAP Calculation (backend)**: Average purchase price for a `Holding` is calculated using the Volume Weighted Average Price (VWAP) method: `average_price = Sum(total_amount_i) / Total Quantity`, where `total_amount_i` is computed per TRX-026 (fees included). This ensures the displayed transaction cost and the cost basis use the same value. Only `Purchase` transactions are included. _(Extended by TRX-048 to include `OpeningBalance` transactions.)_ This rule applies on creation (TRX-025) and on recalculation triggered by modification or deletion.

**TRX-031 — Transaction modification (backend)**: Modifying a transaction triggers a full recalculation of the `Holding` cost basis and quantity for the `(account_id, asset_id)` pair, processing all associated transactions in chronological order (TRX-036).

**TRX-032 — Modifiable fields (backend)**: All fields of a transaction are modifiable except `transaction_type`, which is immutable once saved (see SEL-035). Changing the `asset_id` or `account_id` is permitted and triggers a recalculation of Holdings for both the old and new `(account_id, asset_id)` pairs.

**TRX-033 — Update field validation (backend)**: When modifying a transaction, the same field constraints as TRX-020 apply, and the archived asset guard (TRX-028) is enforced. If `account_id` or `asset_id` is changed, the existence and non-archived status of the new values is verified before proceeding. When the `Sell` transaction type is active, editing a purchase transaction must also verify that no subsequent sell in the chronological sequence for the `(account_id, asset_id)` pair would become invalid (oversell) as a result — see SEL-032.

**TRX-034 — Transaction deletion (backend)**: Deleting a transaction triggers a recalculation of the `Holding` for the `(account_id, asset_id)` pair. If no transactions remain for that asset in the account, the `Holding` record is removed.

**TRX-035 — Deletion confirmation (frontend)**: Deleting a transaction requires a user confirmation dialog to prevent accidental data loss.

**TRX-036 — Chronological integrity (backend)**: Recalculations of `Holding` state following a transaction mutation must process all associated transactions for the specific `(account_id, asset_id)` pair in chronological order (`date ASC, created_at ASC`) to ensure the cost basis and quantity remain accurate. When two transactions share the same date, the one with the earlier `created_at` timestamp is processed first. The `transactions` table must include a `created_at TEXT NOT NULL` column (ISO 8601, default `datetime('now')`) to guarantee this ordering.

**TRX-037 — TransactionUpdated event (backend)**: After any successful transaction mutation (create, update, or delete), the `TransactionService` (owned by the `transaction/` bounded context) publishes a `TransactionUpdated` event on the event bus. The use case delegates to the service; the service owns the event publication (B8 compliance). This event is distinct from `AccountUpdated`: `AccountUpdated` signals structural account changes; `TransactionUpdated` signals position-data changes.

**TRX-038 — Holdings refresh on event (frontend)**: Upon receiving a `TransactionUpdated` event, the frontend refreshes the holdings data for the affected account so that the displayed portfolio state reflects the mutation. The implementation mechanism (store slice, local re-fetch, etc.) is left to the feature-planner. _(Resolved by ACD-039/040: `useAccountDetails` re-fetches on `TransactionUpdated` directly; no separate store layer needed.)_

### Lifecycle Management

**TRX-040 — Zero quantity handling (backend)**: If a `Holding.quantity` reaches zero due to `Sell` transactions, the `Holding` entity remains in the database to accommodate potential future purchase transactions. The `average_price` is maintained at its last known value until the next purchase transaction initiates a new VWAP calculation. _(Activated by the SEL spec.)_

**TRX-041 — Buy-from-holding-row modal (frontend)**: When a purchase is initiated from a holding row (TRX-010 entry point 3), the form opens as a `BuyTransactionModal`. Account and asset are pre-filled from the holding row context and are read-only (TRX-011). Default values follow TRX-023 (date=today, exchange_rate=1.0, fees=0). The exchange rate field is visible only when the asset currency differs from the account currency (consistent with SEL-036). On success the modal closes and a success snackbar is shown; the holdings view refreshes via the existing `TransactionUpdated` event (TRX-038).

### Opening Balance (042–059)

**TRX-042 — Opening balance transaction type (backend)**: A `TransactionType::OpeningBalance` variant allows users to seed an existing position — recording the quantity held and total cost paid before they began tracking in Folioneer — without re-entering the full purchase history. An `OpeningBalance` transaction is stored and processed alongside regular `Purchase` and `Sell` transactions in chronological recalculation.

**TRX-043 — Opening balance form fields (frontend)**: The opening balance form collects four fields: the target asset (selectable from all active assets), a date, a quantity (units held), and a total cost (total amount paid, in the account's currency). No fees field and no exchange rate field are shown.

**TRX-044 — Opening balance quantity validation (frontend + backend)**: Quantity must be greater than zero. The submit action is disabled while quantity is zero or empty.

**TRX-045 — Opening balance total cost validation (frontend + backend)**: Total cost must not be negative; a total cost of **zero is valid** — a zero-cost position (e.g. a mined, gifted, or airdropped asset seeded as a starting position) is the distinguishing case for the open-balance / "New position" flow versus a buy/sell transaction. Quantity must still be strictly positive (TRX-044). The submit action is disabled while total cost is empty or negative, and enabled at zero. A zero-cost position has `unit_price` and cost basis `0`.

**TRX-046 — Opening balance date validation (frontend + backend)**: Date is required and must not be in the future, consistent with TRX-020. The submit action is disabled while the date is empty, invalid, or in the future.

**TRX-047 — Opening balance stored fields (backend)**: An `OpeningBalance` transaction is persisted with `total_amount` = user-entered total cost (micro-units), `unit_price` = `floor(total_cost / quantity)` (micro-units, consistent with TRX-026 floor notation), `fees = 0`, `exchange_rate = 1_000_000`. The total amount formula defined in TRX-026 does not apply — `total_amount` is set directly from user input, both on creation and on edit.

**TRX-048 — Opening balance VWAP participation (backend)**: `OpeningBalance` transactions participate in VWAP recalculation identically to `Purchase` transactions: their `total_amount` and `quantity` are included in the Σ of TRX-030. This extends TRX-030 to cover both `Purchase` and `OpeningBalance` types.

**TRX-049 — Multiple opening balances allowed (backend)**: Multiple `OpeningBalance` transactions may exist for the same (account, asset) pair, and may coexist with `Purchase` and `Sell` transactions for the same pair. Each participates in chronological recalculation (TRX-036) in date order alongside regular transactions.

**TRX-050 — Opening balance archived asset guard (backend)**: If the target asset is archived when an opening balance is submitted, the backend rejects with `ArchivedAsset`. No auto-unarchive occurs. The asset selector (TRX-043) lists only active assets, so this guard is reached only via a race condition (asset archived between form load and submission); the frontend displays the error inline (TRX-028). The TRX-029 confirmation dialog does not apply to opening balance.

**TRX-051 — Opening balance edit form (frontend)**: When editing an `OpeningBalance` transaction, the edit form shows date, quantity, and total cost — no fees field, no exchange rate field. The `correct_transaction` mechanism (TRX-031) is used. TRX-047 applies on edit: `total_amount` is set directly from the entered total cost; TRX-026 does not apply.

**TRX-052 — Transaction list label (frontend)**: `OpeningBalance` entries are shown in the Transaction List with the label "Opening Balance", distinct from "Buy" and "Sell".

**TRX-053 — Opening balance amounts display (frontend)**: In the Transaction List, the unit price column for an `OpeningBalance` row displays the `unit_price` field stored by TRX-047. The total amount column shows the `total_amount` field (the entered total cost).

**TRX-054 — No realized P&L for opening balance (frontend)**: The realized P&L column is empty for `OpeningBalance` rows, as no gain or loss is crystallized at entry time.

**TRX-055 — Opening balance entry point (frontend)**: The opening balance form is accessible via a dedicated button in the Account Details page header, alongside the existing "Add Transaction" button. It is not available as a holding row action. The account is pre-filled from the current Account Details context; the user selects the asset in the form.

> **Superseded by DIV-012**: the standalone "Add a position" header button is consolidated into the header "Record" dropdown menu, relabeled "New position" (same modal, same in-form asset selection).

**TRX-056 — Opening balance error variants (backend)**: The opening balance command exposes the following error variants: `QuantityNotPositive` (quantity ≤ 0), `InvalidTotalCost` (total cost ≤ 0, new variant), `DateInFuture` (date is in the future), `DateTooOld` (date before 1900-01-01, consistent with TRX-020/TRX-046), `AccountNotFound`, `AssetNotFound`, `ArchivedAsset` (asset archived — race condition per TRX-050; displayed as inline error), `OpeningBalanceOnCashAsset` (asset has `class = AssetClass::Cash` — see CSH-061; the user must use `record_deposit` instead). `DbError` covers unexpected persistence failures.

**TRX-057 — Opening balance loading state (frontend)**: While the submission is in progress, the submit button is disabled and shows a loading indicator.

**TRX-058 — Opening balance success feedback (frontend)**: On success, the modal closes and a success snackbar is shown. The Account Details holdings view refreshes via the existing `TransactionUpdated` event (TRX-038).

### Total-Entry Mode (060–069)

**TRX-060 — Total-entry purchase (backend)**: `buy_holding` accepts an optional user-entered `total_amount` — the all-in amount debited by the broker, in account-currency micro-units, fees included. When provided, the typed total is ground truth: it is stored verbatim as `total_amount` (the TRX-026 formula does not apply) and the unit price is derived as `unit_price = round(((total_amount − fees) × MICRO × MICRO) / (quantity × exchange_rate))`, rounding half away from zero, with `i128` intermediates. Validation: `total_amount` must be strictly positive (`TotalAmountNotPositive`) and at least `fees` (`TotalAmountBelowFees`) — the securities part `total_amount − fees` must not be negative. When absent, TRX-026 applies unchanged. Downstream consumers — VWAP (TRX-030) and realized P&L (SEL-024) — read the stored `total_amount` unchanged. The entry mode is not persisted: a correction (TRX-031) operates on the stored decomposition (`quantity`, `unit_price`, `exchange_rate`, `fees`) and recomputes `total_amount` per TRX-026, unless the correction itself supplies a typed total (TRX-061). For sells, see SEL-050.

**TRX-061 — Total-entry correction (backend)**: `correct_transaction` accepts an optional user-entered `total_amount` with the exact TRX-060 semantics. When provided on a `Purchase` correction, the typed total is ground truth: it is stored verbatim as `total_amount` and the unit price is derived per the TRX-060 formula, with the same validation (`TotalAmountNotPositive`, `TotalAmountBelowFees`, `UnitPriceOutOfRange`); the caller-supplied `unit_price` is ignored. When provided on a `Sell` correction, the SEL-051 semantics apply. On every other transaction type the field is ignored — the type-specific recompute (TRX-026, TRX-051, CSH-022/032, DIV-040) applies as if it were absent. When absent, the existing recompute per TRX-026 applies unchanged.

---

## Workflow

```
[User initiates Purchase]
  → Modal: Add Purchase Transaction
          │
          ├─ [Select Account] (pre-filled if possible)
          ├─ [Select Asset] (pre-filled if possible)
          ├─ [Enter Date] (default: today)
          ├─ [Enter Quantity & Unit Price]
          ├─ [Enter Exchange Rate] (default: 1.0, hidden if same currency)
          ├─ [Enter Fees]
          ├─ [Enter Note] (optional)
          │
          └─ [Save] → Backend validates (TRX-020, TRX-033)
                     → Backend computes total_amount (TRX-026)
                     → Persists Transaction in micro-units (TRX-024)
                     → Updates Holding atomically (TRX-022, TRX-025, TRX-027)
                       using VWAP in chronological order (TRX-030, TRX-036)
                     → Publishes TransactionUpdated (TRX-037)
                     → Frontend refreshes holdings view (TRX-038)
```

---

## UX Draft

### Entry Point

- "Buy" action in the Assets table row.
- "Add Transaction" FAB in the Account Details view.

### Main Component

**FormModal** with the following fields:

- Account (Select)
- Asset (Combobox with fuzzy search)
- Date (Date picker)
- Quantity (Number field)
- Unit Price (Amount field with asset currency suffix)
- Exchange Rate (Number field, visible only if asset currency ≠ account currency)
- Fees (Amount field with account currency suffix)
- Total Amount (Amount field with account currency suffix, auto-calculated and read-only)
- Note (Textarea, optional)

_`transaction_type` is not shown in the form. It is hardcoded to `Purchase` until the `Sell` type is introduced._

### States

- **Empty**: Form fields empty or defaulted.
- **Loading**: Submitting the transaction.
- **Error**: Inline validation errors or backend rejection message.
- **Success**: Modal closes, success notification.

### User Flow

1. User clicks "Buy" on an asset.
2. Form opens with Asset pre-selected.
3. User selects the target Account.
4. User enters Quantity, Unit Price, and Fees.
5. Total Amount is auto-calculated and read-only.
6. User clicks "Save".

## Cross-amendments

- **CFR-041** — when the portfolio is shared between devices, `created_at` is carried verbatim with the transaction and never re-derived, and the TRX-036 order gains a third key — transaction identity — for same-date, same-`created_at` entries from different devices (see `sync-conflict-resolution.md`).
