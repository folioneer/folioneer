# Business Rules — Cash Tracking (CSH)

## Context

Folioneer currently tracks asset positions per account but does not track the cash held in those accounts. A Buy implicitly conjures cash; a Sell vaporises the proceeds; opening a position seeds the asset but never the funds that paid for it. As a consequence, the "global value" of an account is undefined: there is no way to add cash-in-hand to `Σ holdings × current_price`, no way to compute year-over-year apport, and no way to keep the bookkeeping in sync with the user's real broker balance.

This spec closes the gap by treating **cash itself as a held position**. Cash is represented as a `Holding` whose asset is a system-seeded "Cash {CCY}" asset; one cash holding per account, denominated in the account's reference currency. New transaction types `Deposit` and `Withdrawal` move money in and out of the account from outside; existing `Purchase` and `Sell` transactions are re-linked so they debit/credit the cash holding by their already-computed `total_amount`. The existing `exchange_rate` and `fees` fields on `Transaction` already absorb all FX work — no FX-on-cash logic is added in v1.

The feature touches the `account` bounded context (Holding, Transaction, the Account aggregate root) and the `asset` bounded context (system Cash Asset seeding). It is a prerequisite for the paused Portfolio Dashboard spec (PFD).

> **Prerequisite (not part of this spec)**: this spec assumes a prior refactor has consolidated the existing transaction-recording use cases (`record_buy`, `record_sell`, `record_opening_balance`, `correct_transaction`, `cancel_transaction`) into a single cross-context module `use_cases/holding_transaction/`, and that the module exposes a shared `ensure_cash_asset(currency)` helper. That refactor is its own feature, scoped and shipped before this spec. The cash spec only declares **observable behaviours**; the orchestration mechanics live in the feature plan of the prerequisite refactor.

> **Migration note**: The original cash-tracking rollout shipped without a backfill (the app had no data yet). The eager-cash-line revision (CSH-010 / CSH-012 / CSH-013) ships a one-off data migration that seeds the Cash category + per-currency Cash Assets and inserts a 0-balance Cash Holding for every existing account.

---

## Entity Definition

### Cash Asset

A system-seeded `Asset` representing the cash position for a given currency. There is one Cash Asset per ISO currency that any account uses; multiple accounts in the same currency share a single Cash Asset record.

| Field         | Business meaning                                                                                                                                                                                                         |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `id`          | Stable system identifier, derived from the currency code (e.g. `system-cash-eur`). Never user-editable.                                                                                                                  |
| `name`        | Display label such as "Cash EUR". Localised for display; the canonical name is stored in English.                                                                                                                        |
| `class`       | Always `AssetClass::Cash`.                                                                                                                                                                                               |
| `currency`    | The ISO 4217 currency this Cash Asset represents (`EUR`, `USD`, …). Identifies which account currencies it can back.                                                                                                     |
| `category`    | A system "Cash" category with deterministic ID `system-cash-category`, distinct from `SYSTEM_CATEGORY_ID`. Seeded once on first Cash Asset creation. Reserved so cash never appears in user-defined category aggregates. |
| `risk_level`  | Always `1` (lowest). Not user-editable.                                                                                                                                                                                  |
| `reference`   | Equal to the currency code (`EUR`, `USD`, …). Not user-editable.                                                                                                                                                         |
| `is_archived` | Always `false`. Cash Assets cannot be archived.                                                                                                                                                                          |

> Cash Assets are not user-editable: the Edit Asset form, archive/unarchive actions, and the Delete Asset action are blocked when the target asset's `class` is `AssetClass::Cash` (see CSH-016).

### Cash Holding

A `Holding` whose `asset_id` points to a Cash Asset. There is at most one Cash Holding per `(account_id, account.currency)` pair.

| Field                | Business meaning                                                                            |
| -------------------- | ------------------------------------------------------------------------------------------- |
| `account_id`         | The account the cash sits in.                                                               |
| `asset_id`           | The Cash Asset matching the account's currency.                                             |
| `quantity`           | Current cash balance in account currency (i64 micros). Always ≥ 0 when CSH-080 is enforced. |
| `average_price`      | Always exactly `1` (i.e. `1_000_000` in i64 micros). Cash is its own unit.                  |
| `last_sold_date`     | Always `None`. Cash is not "sold" in the SEL sense.                                         |
| `total_realized_pnl` | Always `0`. Cash never realises a P&L of its own.                                           |

### Deposit Transaction (new `TransactionType` variant)

A cash inflow from outside the application's tracked world (salary, wire transfer, broker top-up).

| Field              | Business meaning                                                                                         |
| ------------------ | -------------------------------------------------------------------------------------------------------- |
| `transaction_type` | `TransactionType::Deposit`.                                                                              |
| `account_id`       | The account receiving the cash.                                                                          |
| `asset_id`         | The Cash Asset matching the account's currency. Filled by the backend; the user does not pick an asset.  |
| `date`             | Business date of the deposit (must not be in the future, must not be older than the existing TRX bound). |
| `quantity`         | The deposited amount in account currency (i64 micros). Equals `total_amount`.                            |
| `unit_price`       | Always `1` (i.e. `1_000_000`).                                                                           |
| `exchange_rate`    | Always `1`.                                                                                              |
| `fees`             | Always `0` in v1.                                                                                        |
| `total_amount`     | Equal to `quantity` in v1.                                                                               |
| `note`             | Free-text note (optional).                                                                               |
| `realized_pnl`     | Always `None`. A deposit never realises a P&L.                                                           |

### Withdrawal Transaction (new `TransactionType` variant)

A cash outflow to outside the application's tracked world (transfer to a bank account, fee paid externally).

Same fields as Deposit, with `transaction_type = TransactionType::Withdrawal`. Withdrawals reduce the cash holding's quantity.

---

## Tauri Commands

Two **new** commands are added; existing commands (`buy_holding`, `sell_holding`, `correct_transaction`, `cancel_transaction`, `open_holding` — already living in `use_cases/holding_transaction/api.rs` after the prerequisite refactor) gain new error variants but keep their existing signatures.

| Command             | Input DTO                                                                                      | Returns                                             | Error variants                                                                                                                                        |
| ------------------- | ---------------------------------------------------------------------------------------------- | --------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `record_deposit`    | `DepositDTO { account_id: String, date: String, amount_micros: i64, note: Option<String> }`    | `Result<Transaction, RecordDepositCommandError>`    | `AccountNotFound`, `AmountNotPositive`, `DateInFuture`, `DateTooOld`, `Unknown`                                                                       |
| `record_withdrawal` | `WithdrawalDTO { account_id: String, date: String, amount_micros: i64, note: Option<String> }` | `Result<Transaction, RecordWithdrawalCommandError>` | `AccountNotFound`, `AmountNotPositive`, `DateInFuture`, `DateTooOld`, `InsufficientCash { current_balance_micros: i64, currency: String }`, `Unknown` |

Edit and delete of Deposit/Withdrawal reuse the existing `correct_transaction(id, accountId, dto: CorrectTransactionDTO) -> Transaction` and `cancel_transaction(id, accountId)` commands. Their existing error enums are extended (see CSH-024 and CSH-051) with `InsufficientCash { current_balance_micros, currency }`.

`buy_holding`'s error enum gains `InsufficientCash { current_balance_micros, currency }` (CSH-041). `sell_holding` does **not** gain this variant — Sell only credits cash, never debits it (CSH-080). `open_holding`'s error enum gains `OpeningBalanceOnCashAsset` (CSH-061; cross-amends TRX-056).

The asset-mutation commands (`update_asset`, `archive_asset`, `unarchive_asset`, `delete_asset`) gain a new variant `CashAssetNotEditable` (CSH-016).

> The `total_global_value` field is added to the existing `get_account_details(account_id)` command's `AccountDetailsResponse`; no new command is introduced for it. The amendment to `AccountDetailsResponse` is registered in `docs/spec/account-details.md` (ACD spec).

---

## Business Rules

### Cash Asset and Cash Holding Lifecycle (010–019)

**CSH-010 — Cash Asset seeded at account creation (backend)**: The Cash Asset for an account's currency is seeded when the account is created, by the `use_cases/account_creation/` orchestrator via the shared `ensure_cash_asset(currency)` helper, which injects `AssetService` and idempotently upserts the Cash Asset (and the Cash category per CSH-017). `use_cases/holding_transaction/` still calls `ensure_cash_asset` before each cash-affecting transaction as an idempotent safety net, so a pre-existing Cash Asset is a no-op. No cross-context import from `context/account/` to `context/asset/` is introduced (B13 / ADR-003 / ADR-004 preserved).

**CSH-011 — One Cash Asset per currency (backend)**: Cash Asset IDs are deterministic from the currency code (`system-cash-{ccy_lower}`). The seeding helper (`ensure_cash_asset`) treats a primary-key collision as "already exists" and reuses the existing record — making the helper safe to call from every cash-affecting use case without prior existence checks.

**CSH-017 — Cash category seeding (backend)**: The system "Cash" category (id `system-cash-category`) is created on demand by `ensure_cash_asset` in the same code path, before the Cash Asset is upserted. Like the Cash Asset, it is created idempotently and never deleted. The Cash category is hidden from the user's category list in the frontend (it is filtered out alongside `SYSTEM_CATEGORY_ID`).

**CSH-018 — Cash Asset suppression in user-facing asset selectors (frontend)**: Cash Assets do not appear in any user-facing asset selector. The Asset Manager table, the Add Transaction asset combobox (used by Buy/Sell/OpeningBalance flows), and the OpenBalance asset combobox all filter out assets where `class === "Cash"` at the component level. The transaction-list asset filter (TXL) is the only place Cash Assets surface — and only in already-existing-transactions context (CSH-101).

**CSH-019 — Cash actions live on the cash row, not the header (frontend)**: The Account Details header carries **no** Deposit/Withdraw actions. Because every account has a persistent, always-visible Cash row (CSH-012 / CSH-097) whose inline actions are Deposit / Withdraw / View transactions (CSH-091), a header affordance would only duplicate them. Deposit and Withdrawal are reached exclusively from the Cash row's inline actions.

> **Revised for eager cash (this spec revision)**: the DIV-012 header "Record" dropdown no longer lists "Cash deposit" / "Cash withdrawal" — those two entries are removed. The dropdown keeps its non-cash entries (Open balance, Record dividend, Record free shares). The Cash-row inline Deposit/Withdraw actions (CSH-091) are the sole entry points; DIV-012 is cross-amended accordingly.

**CSH-012 — Cash Holding created eagerly at account creation (backend)**: Every account has a Cash Holding for its currency from the moment it is created — seeded at `quantity = 0` by the `use_cases/account_creation/` orchestrator (CSH-010) and persisted alongside the account. There is **no lazy creation**: Deposit (CSH-022), Sell (CSH-050), Purchase and Withdrawal all operate on the already-present Cash Holding. Accounts created before this spec revision are backfilled by a one-off data migration that is **insert-if-absent**: it seeds the Cash category and the per-currency Cash Asset (`system-cash-{ccy_lower}`, CSH-011) for every distinct existing-account currency, and inserts a 0-balance Cash Holding **only** for accounts that do not already have one — any Cash Holding created under the old lazy path (with its balance) is preserved untouched. After any cash-affecting transaction, the Cash Holding's `quantity` reflects the running cash balance (always ≥ 0 per CSH-080). The eager Cash Holding requires no new event: it surfaces through the normal `get_account_details` load (CSH-090), and ACC-022's `AccountUpdated` already fires on account creation.

**CSH-013 — Cash Holding persists for the account's lifetime (backend)**: Unlike other holdings (TRX-034), the Cash Holding is **never** removed by transaction cleanup — it persists at `quantity = 0` when fully withdrawn or spent. It is deleted only when its account is deleted (ACC cascade). The chronological replay (CSH-024 / CSH-034 / CSH-042 / CSH-051) always upserts the Cash Holding with the recomputed balance; it never deletes it.

**CSH-014 — Cash Holding currency invariant (backend)**: The `asset_id` of every Cash Holding for an account always matches `system-cash-{account.currency}`. Account currency is set at account creation (ACC) and is not editable; this invariant therefore holds for the life of the account.

**CSH-015 — Cash Asset is read-only in the catalog (frontend)**: Cash Assets do not appear in the Asset Manager table; the Asset Manager view filters them out at the frontend level (by `class === "Cash"`). The Add Asset form rejects an attempt to create an asset with `class = Cash` (backend `AssetClassNotAllowed` error in `add_asset`). The backend `get_assets()` and `get_assets_with_archived()` continue to return Cash Assets so the global `useAppStore` can populate transaction-list filters; per-feature suppression rules are stated in CSH-018.

**CSH-016 — Cash Asset is non-editable (backend + frontend)**: The backend commands `update_asset`, `archive_asset`, `unarchive_asset`, and `delete_asset` reject any input whose target asset has `class = AssetClass::Cash` with the error variant `CashAssetNotEditable` (added to each of `UpdateAssetCommandError`, `ArchiveAssetCommandError`, `UnarchiveAssetCommandError`, `DeleteAssetCommandError`). The frontend never surfaces Edit, Archive, Unarchive, or Delete row actions for Cash Assets (their rows are filtered out of the table by CSH-015, so the question doesn't arise in the Asset Manager; CSH-016 also blocks any direct invocation by ID).

### Deposit Transaction (020–029)

**CSH-020 — Deposit form fields (frontend)**: The Deposit form accepts `account_id` (read-only, pre-filled from the navigation context), `date` (default: today), `amount` (positive decimal), and an optional `note`. No asset selector, no unit price, no fees, no exchange rate.

**CSH-021 — Deposit input validation (frontend + backend)**: `amount` must parse as a positive decimal (`> 0`); `date` must not be in the future and must not be older than the TRX-020 lower bound. Backend re-validates and rejects with explicit error variants (`AmountNotPositive`, `DateInFuture`, `DateTooOld`).

**CSH-022 — Deposit creation effect (backend)**: Recording a Deposit, within a single Unit of Work (ADR-006), (a) idempotently ensures the Cash Asset for `account.currency` (a no-op when already seeded at account creation per CSH-010), (b) increases the account's always-present Cash Holding's `quantity` by `total_amount` (which equals the deposited amount in account currency), and (c) persists the Transaction with `transaction_type = Deposit`. All steps commit together or all roll back.

**CSH-023 — Deposit edit (backend + frontend)**: Editing a Deposit re-applies the chronological replay used by `Account` aggregate root (TRX-031). The cash holding is recomputed from scratch over all cash-affecting transactions for the account. Edits may change `date`, `amount`, or `note`.

**CSH-024 — Deposit delete (backend + frontend)**: Deleting a Deposit triggers a full chronological replay of every cash-affecting transaction for the account in `(date ASC, created_at ASC)` order with the deleted Deposit excluded. If, at any point during the replay, a remaining Withdrawal or Purchase would drive the running cash balance strictly negative, the deletion is rejected with the existing `cancel_transaction` `InsufficientCash { current_balance_micros, currency }` error (CSH-080). When the replay succeeds, the Cash Holding is upserted with the recomputed balance — it is never deleted, even when that balance is zero (CSH-013). The Cash Asset is likewise never deleted.

**CSH-025 — Deposit confirmation feedback (frontend)**: On successful Deposit recording or edit, a snackbar displays "Deposit recorded" / "Deposit updated"; on delete, "Deposit deleted". Failures display the validation or backend error inline in the form.

### Withdrawal Transaction (030–039)

**CSH-030 — Withdrawal form fields (frontend)**: Same fields as the Deposit form (CSH-020).

**CSH-031 — Withdrawal input validation (frontend + backend)**: Same as Deposit (CSH-021) plus an additional eligibility check: `amount` may not exceed the cash holding's current `quantity` (CSH-080). Backend rejects with `InsufficientCash` when violated.

**CSH-032 — Withdrawal creation effect (backend)**: Recording a Withdrawal, within a single Unit of Work (ADR-006), (a) requires the account's always-present Cash Holding to have `quantity ≥ amount`; insufficient quantity triggers `InsufficientCash { current_balance_micros, currency }` per CSH-080 (Withdrawals only debit the existing Cash Holding); (b) decreases the Cash Holding's `quantity` by `total_amount`; (c) persists the Transaction with `transaction_type = Withdrawal`. All steps commit together or all roll back.

**CSH-033 — Withdrawal edit (backend + frontend)**: Same replay semantics as CSH-023. The eligibility check is re-evaluated chronologically over the new transaction set; an edit that would leave any subsequent transaction in violation of CSH-080 is rejected with `InsufficientCash`.

**CSH-034 — Withdrawal delete (backend + frontend)**: Same chronological replay semantics as CSH-024. Deleting a Withdrawal increases the cash balance available to every subsequent transaction in the replay, so it never produces a `InsufficientCash` rejection. The deletion can still fail for unrelated reasons (account not found, persistence error) — those surface via the existing `cancel_transaction` error path.

**CSH-035 — Withdrawal confirmation feedback (frontend)**: Mirrors CSH-025 with "Withdrawal" wording.

### Purchase and Sell Re-linked to Cash (040–059)

**CSH-040 — Purchase debits cash (backend)**: Recording a Purchase via `buy_holding` (TRX-026) — in addition to its existing effect on the bought asset's holding — debits the account's Cash Holding by `total_amount` (already in account currency per TRX-021/TRX-026). The cash debit, the asset-holding mutation, and the Transaction persistence commit together within a single Unit of Work (ADR-006). The Cash Holding is always present (CSH-012); eligibility for the cash debit is governed by CSH-041.

**CSH-041 — Purchase eligibility on cash (backend)**: A Purchase is rejected when the account's Cash Holding `quantity` is strictly less than the Purchase's `total_amount` (a freshly created account with a 0-balance Cash Holding therefore rejects any Purchase until funded). The error variant is `InsufficientCash { current_balance_micros, currency }` (added to `BuyHoldingCommandError`). The guard is evaluated **after** the existing TRX-020/TRX-026 validations and **before** any mutation is persisted (CSH-080); on rejection, neither the asset holding nor the cash holding nor the Transaction is written.

**CSH-042 — Purchase edit re-applies cash effect (backend)**: Editing a Purchase (TRX-031) triggers the existing chronological replay for the asset holding **and** the Cash Holding. Both are recomputed end-to-end. The eligibility check (CSH-080) is re-evaluated chronologically; an edit that would leave any later Purchase in violation is rejected with `InsufficientCash`.

**CSH-043 — Purchase delete returns cash (backend)**: Deleting a Purchase (TRX-034) triggers replay; cash holding rises by the deleted total. Always safe — never violates CSH-080.

**CSH-050 — Sell credits cash (backend)**: Recording a Sell via `sell_holding` (SEL-024) — in addition to its existing effect on the sold asset's holding — credits the account's always-present Cash Holding by `total_amount` (CSH-012).

**CSH-051 — Sell delete and edit replay (backend)**: Edits and deletes of Sell transactions (SEL-031, SEL-033) trigger the same chronological replay across both the sold-asset holding and the Cash Holding. A delete that would leave any later Purchase in violation of CSH-080 is rejected with `InsufficientCash`, mirroring CSH-024.

### Opening Balance Cohabitation with Cash (060–069)

**CSH-060 — OpeningBalance for non-cash asset does not touch cash (backend)**: An `OpeningBalance` transaction whose `asset_id` is **not** a Cash Asset seeds only the asset's holding (per TRX-040). It does **not** create or modify the Cash Holding. The semantic is: "I already held this position when I started tracking — its prior cost is not visible to the app."

**CSH-061 — OpeningBalance for the Cash Asset is rejected (backend)**: The `open_holding` command rejects any `OpeningBalanceDTO` whose resolved asset has `class = AssetClass::Cash`, returning the error variant `OpeningBalanceOnCashAsset` (added to `OpenHoldingCommandError` — cross-amends TRX-056). The user records initial cash via the `record_deposit` command instead. Reason: keeping a single explicit entry point (Deposit) for "cash arriving in the account" simplifies the transaction list and avoids a redundant lifecycle.

### Insufficient Cash Guard (080–089)

**CSH-080 — Insufficient cash guard (backend)**: A transaction (or replay step during edit/delete) that would leave the Cash Holding's `quantity` strictly negative is rejected with the structured error `InsufficientCash { current_balance_micros: i64, currency: String }`. The payload's `current_balance_micros` carries the cash holding's balance **at the point of rejection** (i.e. before the rejected mutation would have applied) so the frontend can display it directly without a follow-up fetch. The same variant is shared by `record_withdrawal`, `buy_holding`, `correct_transaction`, and `cancel_transaction` error enums; it is **not** present on `record_deposit` or `sell_holding` (those only credit cash). Atomicity is provided by the existing Unit-of-Work (ADR-006).

**CSH-081 — Insufficient cash error display (frontend)**: When the backend returns `InsufficientCash { current_balance_micros, currency }`, the form (Deposit form is exempt — it doesn't trigger this; Withdrawal, Buy, Sell-via-edit, or any Edit modal that hits the guard) displays an inline error: "Not enough cash in this account. Current balance: {presenter.formatAmount(current_balance_micros, currency)}." The submit button stays enabled so the user can amend the amount.

### Cash Holding Display (090–099)

**CSH-090 — Cash Holding included in AccountDetailsResponse (backend)**: Every account has a Cash Holding (CSH-012), and `AccountDetailsUseCase` always includes it in `AccountDetailsResponse.holdings` (the active list) — **exempt from the ACD-020 `quantity > 0` filter** and never placed in `closed_holdings`, even at `quantity = 0`. Asset-metadata enrichment (ACD-022) treats the Cash Asset like any other asset — `asset_name` becomes "Cash EUR" / "Cash USD" / etc.

**CSH-091 — Cash row layout (frontend)**: The Cash row displays its quantity as a currency amount (e.g. `€1,250.00`), no average price column, no cost basis column, no realized P&L column, no Buy/Sell row actions. Its inline actions are a "Deposit" / "Withdraw" pair plus a "View transactions" (inspect) action (CSH-110) — replacing the standard holding-row actions.

**CSH-092 — Cash row position (frontend)**: The Cash Holding row is always rendered at the top of the active holdings table, ahead of other holdings, regardless of ACD-033 alphabetical sort.

**CSH-093 — Cash holding excluded from cost basis total (backend)**: `AccountDetailsResponse.total_cost_basis` (ACD-031) does **not** include the Cash Holding. The total cost basis is the sum of cost basis across non-cash active holdings only. Reason: cash has no "cost basis" — it is its own value.

**CSH-094 — total_global_value field (backend)**: `AccountDetailsResponse` is extended with a new field `total_global_value: i64` (account-currency micros, ADR-001), computed by `AccountDetailsUseCase` as `cash_holding.quantity + Σ_h (h.quantity × latest_price(h))` over **non-cash active holdings only** (`quantity > 0`). When no Cash Holding exists, the cash term is `0`. When a non-cash holding has no recorded `AssetPrice`, that holding contributes `0` (no fallback to `average_price`). When all non-cash holdings are unpriced and the account has no cash, `total_global_value = 0`. The amendment to `AccountDetailsResponse` is registered in `docs/spec/account-details.md`.

**CSH-095 — Cash row always visible (frontend)**: The Cash row is always rendered at the top of the active holdings table (CSH-092), including when its `quantity = 0` (e.g. a freshly created account, or one fully withdrawn/spent). There is **no** "No cash recorded yet" banner and no empty cash state — the persistent cash row, with its inline Deposit action, is the entry point for funding the account.

**CSH-097 — Cash row shown regardless of quantity (frontend)**: The Cash row overrides ACD-020's `quantity > 0` filter — it renders even at `quantity = 0` (the backend always supplies it per CSH-090). The Withdraw inline action is disabled when `quantity == 0` (cash is never negative — CSH-080); the Deposit inline action is always enabled.

**CSH-098 — ACD-034 empty-state count excludes cash (frontend)**: When determining whether to show the "No positions yet" or "All positions are closed" empty-state messages (ACD-034), the frontend excludes the Cash Holding from the active-holdings count. An account whose only non-zero holding is the Cash Holding still shows ACD-034's "No positions yet" message in the asset positions area (with the cash row visible above per CSH-091/092). The corresponding cross-reference is registered in `docs/spec/account-details.md` (ACD-034 amendment).

### Reactivity (100–109)

**CSH-100 — TransactionUpdated event scope (backend)**: Recording, editing, or deleting a Deposit or Withdrawal publishes the existing `TransactionUpdated` event (per the AccountService convention). Frontend reactivity (ACD-039, MKT-036) re-fetches without further changes.

**CSH-101 — Cash transaction list inclusion (frontend)**: Deposit and Withdrawal transactions appear in the existing transaction list view (TXL spec) when the user filters by the Cash Asset for that account. The Type column (TXL-023) displays "Deposit" / "Withdrawal" (cross-amends TXL-023). The Realized P&L column (TXL-022) renders the neutral placeholder `—` for both. Other columns (Quantity, Unit Price, Exchange Rate, Fees, Total Amount) render their stored values per the entity definition above. When filtering by a non-cash asset, Deposit and Withdrawal transactions do not appear. Backend behaviour is unchanged: the existing `get_transactions(accountId, assetId)` command returns Deposit/Withdrawal rows when queried with the Cash Asset's ID, by virtue of the new `TransactionType` variants being persisted.

### Cash transaction history & editing (110–119)

**CSH-110 — Cash row "View transactions" action (frontend)**: The Cash row exposes an inspect action (mirroring the non-cash holding-row inspect of ACD-042) that navigates to the per-asset transaction list `/accounts/:accountId/transactions/:assetId` (TXL-010) with the Cash Asset's id (`system-cash-{ccy}`) as `assetId`. This is the entry point that makes the cash-transaction inclusion of CSH-101 reachable, so the user can review and act on the account's Deposit/Withdrawal history. Navigation goes through the router; Account Details does not import from the `transactions` feature (TXL-010 / B13).

**CSH-111 — Cash transaction editing via the dedicated modals (frontend)**: In the transaction list, the edit action (TXL-030) on a Deposit row opens `DepositTransactionModal`, and on a Withdrawal row opens `WithdrawalTransactionModal`, each in an **edit mode**: pre-filled from the existing transaction (`date`, `amount`, `note` — the only editable cash fields per CSH-023 / CSH-033) and persisting via `correct_transaction` (TRX-031) instead of `record_deposit` / `record_withdrawal`. The generic `EditTransactionModal` remains cash-excluded (CSH-018) and is **not** used for cash transactions — it would present asset-shaped fields (asset, quantity, unit price, fees) that do not apply to cash. Because the transaction list lives in the `transactions` feature and the cash modals live in the account-details feature, the edit action opens the cash modal through the URL-driven modal-mount convention (a `?modal=…` search param handled by a shell-mounted mount), so the `transactions` feature does not import the account-details feature (B13). Delete is unchanged — the existing TXL-040 delete action and `cancel_transaction` already operate correctly on cash transactions (CSH-024 / CSH-034). On successful edit, the existing TXL-026 refresh and CSH-100 `TransactionUpdated` reactivity apply.

---

## Workflow

```
[User creates account in EUR]
  → ACC + use_cases/account_creation create the account
  → CSH-010: Cash Asset "system-cash-eur" seeded (idempotent)
  → CSH-012: Cash Holding created eagerly at quantity 0, persisted
  → Account Details shows the cash row at €0.00 (CSH-095)

[User opens "Deposit" from the cash row's inline action]
  → CSH-020 form: amount, date, optional note
  → Submit → CSH-022: cash holding quantity incremented by amount
  → TransactionUpdated event → ACD-039 re-fetch
  → Account Details header shows total_global_value

[User buys 10 AAPL @ $180, fee $5, exchange_rate 0.91]
  → TRX-026: total_amount in EUR ≈ €1,642.55
  → CSH-041: cash holding qty checked vs €1,642.55
  → If insufficient → InsufficientCash error (CSH-081)
  → Else → AAPL holding mutation + CSH-040 cash debit, atomic (ADR-006)

[User sells 5 AAPL @ $200, fee $3, exchange_rate 0.92]
  → SEL-024: total_amount in EUR ≈ €917.24
  → CSH-050: cash holding credited by €917.24
  → SEL holding mutation + cash credit, atomic

[User withdraws €500]
  → CSH-031 form, eligibility check vs current cash qty
  → CSH-032: cash holding qty decreased by €500
```

---

## UX Draft

### Entry Point

- The Cash Holding row in the active holdings table exposes "Deposit" and "Withdraw" inline actions (CSH-091), which open dedicated modals. This is the sole entry point — the Account Details header carries no cash actions (CSH-019).
- The cash row is always present (CSH-095), so the Deposit action is reachable even on a brand-new, unfunded account.

### Main Component

Two new modals — `DepositTransactionModal` and `WithdrawalTransactionModal` — both small `FormModal` instances:

- Account name (read-only, derived from route).
- Date (default today, `DateField`).
- Amount (positive decimal, `AmountField` with the account currency suffix).
- Note (optional `TextField`).
- Submit / Cancel.

The Account Details header gains a new total: **Global Value** (cash + market value of holdings, per CSH-094), shown alongside the existing Total Cost Basis and Total Realized P&L.

### States

- **Fresh account**: a just-created account shows the cash row at €0.00 (CSH-095) with its inline Deposit action enabled and Withdraw disabled (CSH-097). No banner, no empty cash state.
- **Loading**: skeleton row at the top of the active holdings table while the Cash Holding is fetched (handled by ACD-037).
- **Error (insufficient cash)**: per CSH-081 — inline error in form.
- **Error (validation)**: inline form errors per CSH-021 / CSH-031.
- **Success**: snackbar per CSH-025 / CSH-035; modal closes; account details re-fetches.

### User Flow

1. User creates a new account in EUR → cash row shows €0.00 immediately (CSH-012/095).
2. User clicks the cash row's "Deposit", enters €5 000, today's date → cash holding rises to €5 000, header updates.
3. User buys an asset for €1 600 → cash debited to €3 400, AAPL holding created. Both visible in the holdings table.
4. User sells half of the asset for €900 → cash credited to €4 300, AAPL holding quantity halved.
5. User clicks "Withdraw", enters €1 000 → cash drops to €3 300.
6. User attempts a Buy for €4 000 → form shows "Not enough cash in this account. Current balance: €3 300."

## Cross-amendments

- **CFR-042** — a merge of independently valid purchases may overdraw the cash holding; CSH-080's `quantity ≥ 0` then holds only once the user corrects the ledger, and the cash holding is marked inconsistent meanwhile (SYN-040, see `sync-conflict-resolution.md`).
