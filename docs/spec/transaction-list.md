# Business Rules — Transaction List (TXL)

## Context

The `Transaction List` feature lets a user browse, edit, and delete all transactions recorded for a specific financial asset within a specific account. It is the primary surface for reviewing the history of a position and the only place where individual transactions can be deleted.

The feature lives within the `transaction` bounded context (`features/transactions/`). It is accessed via a dedicated route (`/accounts/:accountId/transactions/:assetId`) so that `account_details` triggers navigation without importing from `transactions`. The page exposes two filter dropdowns (account + asset) so the user can switch context without leaving the view.

> Cross-spec dependency: the entry-point inspect action on Account Details holding rows is defined in ACD-042 and references TXL-010. The `account-details.md` spec has been updated accordingly.

---

## Entity Definition

> This feature does not introduce a new persisted entity. It reads `Transaction` records defined in [Financial Asset Transaction](financial-asset-transaction.md).

---

## Business Rules

### Navigation and Filters (010–019)

**TXL-010 — Entry point from Account Details (frontend)**: In the Account Details view, each holding row exposes an inspect action (ACD-042). Clicking it navigates to `/accounts/:accountId/transactions/:assetId`, where `accountId` and `assetId` are taken from the holding row. Navigation goes through the router; Account Details does not import from the `transactions` feature.

**TXL-011 — Route parameters as initial filter state (frontend)**: On mount, the view reads `accountId` and `assetId` from the route. The account dropdown is pre-selected to `accountId`; the asset dropdown is pre-selected to `assetId`. Both filters are changeable by the user after mount.

**TXL-012 — Account dropdown population (frontend)**: The account dropdown lists all accounts available to the user. Changing the selected account resets the asset dropdown to no selection and clears the transaction list until a new asset is selected.

**TXL-013 — Asset list for account — backend (backend)**: A dedicated backend command returns the list of asset IDs that have at least one transaction recorded for a given account. This command queries the transactions data only. If the `account_id` is unknown or has no transactions, the command returns an empty list rather than an error.

**TXL-014 — Asset dropdown population (frontend)**: The asset dropdown is filtered to show only assets returned by TXL-013 for the currently selected account. The frontend resolves asset names by cross-referencing the returned IDs with the global asset list. When the account changes, the asset list is refreshed by re-calling the TXL-013 command.

**TXL-015 — Back navigation (frontend)**: The view provides a "Back" link that navigates to `/accounts/:accountId`, where `accountId` is the value currently selected in the account dropdown (which may differ from the route param if the user has changed the filter).

**TXL-016 — Sort direction reset on filter change (frontend)**: When the account or asset filter changes, the sort direction resets to the default (date descending). The sort direction is preserved when the transaction list re-fetches after an edit or delete (TXL-025) — only a filter change causes a reset.

### Data Display (020–029)

**TXL-020 — Transaction fetch — backend (backend)**: The backend exposes an existing command that returns all transactions for a given `(accountId, assetId)` pair, ordered chronologically.

**TXL-021 — Transaction list loading — frontend (frontend)**: The transaction list is loaded on mount using the `accountId` and `assetId` from the route params, and reloaded whenever either filter changes to a complete selection.

**TXL-022 — Displayed columns (frontend)**: The transaction table displays the following columns for each row: Type, Date, Quantity, Unit Price, Exchange Rate, Fees, Total Amount, Realized P&L. The Realized P&L column displays the `realized_pnl` value for sell rows (per SEL-041) and a neutral placeholder (`—`) for purchase, opening-balance, deposit, withdrawal, dividend, and free-shares rows (dividend rows cross-amended by DIV-050; free-shares rows by FSD-050, where the Unit Price and Total Amount columns also render `—`).

**TXL-023 — Type column value (frontend)**: The Type column displays the actual transaction type for each row: "Purchase" for purchase transactions, "Sell" for sell transactions (per SEL-040), "Opening balance" for opening-balance entries, "Deposit" for deposit transactions (per CSH-101), and "Withdrawal" for withdrawal transactions (per CSH-101). Sell rows are visually distinguished from Purchase rows.

**TXL-024 — Default sort order (frontend)**: Transactions are displayed with the most recent date first (descending). The user can toggle the sort direction by clicking the Date column header.

**TXL-025 — Financial value formatting (frontend)**: All financial fields (unit price, exchange rate, fees, total amount, quantity) are formatted as decimal strings with three decimal places, per TRX-024 (micro-unit → decimal conversion at the display boundary, ADR-001).

**TXL-026 — Reactivity to transaction mutations (frontend)**: After any successful edit or delete operation within this view, the transaction list is re-fetched for the current `(accountId, assetId)` pair to reflect the updated state. The current sort direction is preserved across this re-fetch.

### Edit (030–039)

**TXL-030 — Edit action (frontend)**: Each transaction row exposes an edit action. Clicking it opens an edit modal pre-filled with the transaction's data, per TRX-031 and TRX-033. For Purchase / Sell / OpeningBalance rows this is the generic Edit Transaction modal. For **Deposit / Withdrawal rows** (Cash Asset) it is instead the dedicated `DepositTransactionModal` / `WithdrawalTransactionModal` in edit mode (CSH-111) — the generic modal is cash-excluded (CSH-018). The TXL view owns the trigger and the post-success refresh; the modal owns all validation, submission logic, and success snackbar. The cash-modal trigger goes through the URL-driven modal-mount convention so the `transactions` feature does not import the account-details feature (CSH-111 / B13).

**TXL-031 — Edit success (frontend)**: On successful edit, the modal closes and the transaction list refreshes (TXL-026).

### Delete (040–049)

**TXL-040 — Delete action (frontend)**: Each transaction row exposes a delete action. Clicking it opens a confirmation dialog before proceeding, per TRX-035.

**TXL-041 — Delete confirmation content (frontend)**: The confirmation dialog uses the existing `transaction.delete_confirm_title` and `transaction.delete_confirm_message` i18n keys (already defined in EN and FR locales). It does not display individual transaction details.

**TXL-042 — Delete success feedback (frontend)**: On successful deletion, the confirmation dialog closes, a success snackbar is shown, and the transaction list refreshes (TXL-026).

**TXL-043 — Position closure on last delete (frontend)**: After the re-fetch triggered by TXL-026, if the backend returns zero records for the `(accountId, assetId)` pair, the view navigates to `/accounts/:accountId`. This relies on the same empty-state detection as TXL-051 and requires no additional backend signal.

**TXL-044 — Delete failure feedback (frontend)**: If the backend delete call fails, the confirmation dialog closes and a snackbar error is shown. The transaction list is not refreshed.

### States (050–059)

**TXL-050 — Loading state (frontend)**: While the transaction list is being fetched, the view displays a skeleton or loading indicator in place of the table.

**TXL-051 — Empty state (frontend)**: If no transactions exist for the selected `(accountId, assetId)` pair, the view displays a "No transactions" message and a shortcut to add a transaction. The shortcut opens the Add Transaction modal pre-filled with the current `accountId` and `assetId`, per TRX-011.

**TXL-052 — Incomplete filter state (frontend)**: If the asset dropdown has no selection (e.g. after an account change), the table area displays a prompt inviting the user to select an asset.

**TXL-053 — Transaction fetch error state (frontend)**: If the transaction fetch (TXL-020/TXL-021) fails, the view displays a generic error message and a "Retry" button that re-triggers the fetch.

**TXL-054 — Asset list fetch error state (frontend)**: If the TXL-013 backend call fails, the asset dropdown displays a generic error state and a retry action. The transaction table is not shown until the asset list is successfully loaded.

---

## Workflow

```
[User clicks inspect icon on a holding row in Account Details (ACD-042)]
  → Navigate to /accounts/:accountId/transactions/:assetId (TXL-010)
          │
          ├─ Account dropdown pre-selected (TXL-011)
          ├─ Asset list fetched for account (TXL-013/014)
          ├─ Asset dropdown pre-selected (TXL-011)
          ├─ Fetch transactions for (accountId, assetId) (TXL-020/021)
          │
          └─ [Table renders, sorted by date desc (TXL-024)]
              │
              ├─ [Edit row] → EditTransactionModal → success → refresh (TXL-030/031)
              │
              └─ [Delete row] → ConfirmationDialog (TXL-040/041)
                               → confirmed → delete
                                   → success → refresh (TXL-042)
                                       → 0 records → navigate /accounts/:accountId (TXL-043)
                                   → failure → snackbar error (TXL-044)
```

---

## UX Draft

### Entry Point

- Inspect icon (e.g. magnifier) on each holding row in the Account Details view (ACD-042).

### Main Component

**Page** at route `/accounts/:accountId/transactions/:assetId`, using `ManagerLayout`.

### Header

- Back link: "← Back to account" navigating to `/accounts/:accountId` (current filter value).
- Account dropdown (full list, pre-selected from route).
- Asset dropdown (filtered to assets with transactions in selected account, pre-selected from route).

### Transaction Table

Columns: Type | Date | Quantity | Unit Price | Exchange Rate | Fees | Total Amount | Realized P&L | Actions

Actions per row: Edit icon button + Delete icon button.

### States

- **Loading**: Skeleton rows in the table area.
- **Incomplete filter**: Prompt "Select an asset to view transactions."
- **Empty**: "No transactions recorded for this position." + "Add Transaction" shortcut (TRX-011 pre-fill).
- **Asset list error**: Asset dropdown shows error + retry action.
- **Transaction fetch error**: Generic error message + "Retry" button.
- **Success (edit)**: Modal closes; table refreshes in place (sort direction preserved).
- **Success (delete)**: Snackbar shown; table refreshes; navigates back if 0 records remain (TXL-043).
- **Delete failure**: Confirmation dialog closes; snackbar error shown (TXL-044).

### User Flow

1. User clicks the magnifier icon on a holding row in Account Details.
2. Route navigates to `/accounts/:accountId/transactions/:assetId`; loading skeleton appears.
3. Asset list and transactions load; table renders sorted by date descending.
4. User can change the account or asset filter to browse other positions.
5. User clicks Edit on a row → Edit modal opens → saves → table refreshes.
6. User clicks Delete on a row → Confirmation dialog → confirms → row removed → table refreshes (or navigates back if last transaction).
