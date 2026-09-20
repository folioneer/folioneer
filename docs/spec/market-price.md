# Business Rules — Market Price (MKT)

## Context

The Market Price feature allows users to record the current market value of a financial asset manually. It is the first phase of market price support; automated price feeds are a future feature that will slot into the same data model.

A price is recorded per asset (not per holding) and is timestamped: multiple entries can accumulate over time, one per date per asset. The Account Details view uses the most recently dated price to display the current value, unrealized gain/loss, and performance percentage for each active holding.

This spec is a **feature spec** spanning two domains: price recording belongs to the `asset` bounded context; display of current price and derived values extends the `use_cases/account_details/` use case; the price history backfill (MKT-190+) reads the account's transactions through its own cross-context use case, `use_cases/price_history_backfill/`. See `docs/spec/account-details.md` for the baseline Account Details behaviour that this spec extends.

By default, recording a buy or sell transaction does **not** automatically create a price record. `Transaction.unit_price` is the price transacted at (a cost-basis input); `AssetPrice.price` is the current market value of the asset. Conflating them by default would show cost as current price, making unrealized P&L meaningless. As an explicit opt-in (see MKT-050+), the user can choose — globally or per transaction — to also persist the transacted unit price as the asset's market price for the transaction date.

All financial values are stored as `i64` micro-units per [ADR-001](../adr/001-use-i64-for-monetary-amounts.md).

---

## Entity Definition

### AssetPrice

Represents a manually recorded market price for a financial asset on a specific date. Owned by the `asset` bounded context.

| Field      | Business meaning                                                                                                                                                                                                                                                                                                              |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `asset_id` | The asset whose market price this record describes.                                                                                                                                                                                                                                                                           |
| `date`     | The calendar date this price observation applies to (ISO 8601, e.g. `2026-04-26`). Date is the user's local calendar date at write time, not the asset market's timezone.                                                                                                                                                     |
| `price`    | Market price per unit in the asset's native currency (i64 micros, ADR-001).                                                                                                                                                                                                                                                   |
| `source`   | Provenance of this price record (see MKT-100 for variants). `Manual` for user-entered values (including those auto-recorded from a transaction's `record_price=true` flag); a provider name (e.g. `YahooFinance`) for auto-fetched values. Metadata for traceability; does not influence read/write precedence (per ADR-012). |

> The combination `(asset_id, date)` is unique: only one price per asset per day. Recording a second price for the same `(asset_id, date)` pair overwrites the first (MKT-025), regardless of source (per ADR-012) — except in a price history backfill, which fills only dates without a price (MKT-192). Correction by re-recording remains valid. Standalone edit and delete of individual entries are also supported via the price history view (MKT-070+).

### PriceFreshness

The two figures behind the header's price item (MKT-202). A read model, never persisted as such.

| Field               | Business meaning                                                                                                                                              |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `newest_price_date` | Latest observation date among the prices of the held assets (ISO 8601 date, e.g. `2026-09-15`); absent without a held, priced asset (MKT-200).                |
| `last_fetch_at`     | Moment this device last fetched prices, as a local date and time without offset (e.g. `2026-09-19T08:14:00`); absent when the device never fetched (MKT-201). |

### HoldingDetail (extended)

The `HoldingDetail` DTO defined in the ACD spec gains five new fields populated by this feature.

| Field                  | Business meaning                                                                                                                                                                                                                            |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `asset_currency`       | ISO 4217 currency code of the asset (e.g. `"USD"`). Required for the price label in the modal (MKT-023) and for the currency-mismatch check (MKT-034). Always present.                                                                      |
| `current_price`        | Most recently dated `AssetPrice.price` for this asset, in asset currency (i64 micros). `None` if no price has ever been recorded.                                                                                                           |
| `current_price_date`   | ISO date string of the price observation used as `current_price`. `None` when `current_price` is `None`.                                                                                                                                    |
| `current_price_source` | Provenance of the price observation used as `current_price` (see `AssetPriceSource` — `Manual` or `YahooFinance`). `None` when `current_price` is `None`. Surfaced so the FE can render the source badge per MKT-142 without a per-row IPC. |
| `unrealized_pnl`       | Unrealized gain or loss in account currency (i64 micros). `None` when no price exists or when asset and account currencies differ (MKT-034). `0` when current price equals average price (not `None`).                                      |
| `performance_pct`      | `unrealized_pnl / cost_basis × 100`, expressed as i64 micros (e.g. 5.25 % = 5 250 000). `None` when `unrealized_pnl` is `None` or `cost_basis` is zero. `0` when `unrealized_pnl` is zero.                                                  |

### AccountDetailsResponse (extended)

The `AccountDetailsResponse` DTO gains one new field.

| Field                  | Business meaning                                                                                                                                                                  |
| ---------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `total_unrealized_pnl` | Sum of `unrealized_pnl` (i64 micros) across all active holdings where a value is computable (same-currency with a recorded price). `None` when no holding has a computable value. |

### Asset (extended)

The `Asset` entity (owned by the AST spec) gains one field for this feature.

| Field                   | Business meaning                                                                                                                                                                                                                                                                    |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `price_refresh_blocked` | Whether automated price fetches are blocked for this asset (the asset is "locked"). When true, every fetch task skips the asset (MKT-151), preserving its most recently recorded price. Defaults to `false` (not locked). Independent of archive state; unchanged by an asset edit. |

---

## Business Rules

### Eligibility and Initiation (010–019)

**MKT-010 — Entry point (frontend)**: An "Enter price" action is available on each **active** holding row in the Account Details view, alongside the existing Buy and Sell actions. It is not shown on closed holdings.

**MKT-011 — Modal asset + date (frontend)**: The "Enter price" form's asset field is a fuzzy-search combobox over the account's active non-cash holdings, **pre-selected** to the holding the form was launched from (and switchable, so prices for several holdings can be entered without reopening). The date field seeds from the account's stored last-operation date (editable, e.g. to backdate to yesterday's closing price). Switching the asset clears the price (a price belongs to one asset) and updates the currency label (MKT-023).

**MKT-012 — Price opens empty (frontend)**: The price field always opens empty. (The earlier today's-price pre-fill is incompatible with the selectable-asset combobox — the holdings list carries no per-asset current price — and with the stored-date default; the user types the price.)

**MKT-013 — No additional backend call (frontend)**: The asset list and date default are sourced from data already loaded by the Account Details view (the holdings and the stored last-operation date). Opening the modal requires no additional IPC request.

**MKT-014 — Save & add another (frontend)**: A secondary "Save & add another" button records the price, refreshes the price history, and keeps the modal open with the price cleared (asset + date retained) for rapid entry across multiple holdings. The account holdings table refreshes via the `AssetPriceUpdated` event subscription, so no explicit account re-fetch is needed.

### Recording a Price (020–029)

**MKT-020 — Required fields (frontend)**: The price form requires a non-empty date and a non-empty price. The submit button is disabled while either field is empty. The asset is implicit from the entry point and is not modifiable by the user.

**MKT-021 — Price validation (frontend + backend)**: A valid price is strictly greater than zero. The backend rejects a submitted price of zero or below with a specific error. The frontend validates inline and disables the submit button until corrected.

**MKT-022 — Date validation (frontend + backend)**: A valid date is a well-formed ISO 8601 calendar date (`YYYY-MM-DD`) that is not in the future. Any past date is accepted; no lower bound applies (users may backdate historical prices). The backend rejects an invalid or future date with a specific error. The frontend validates inline and disables the submit button until corrected.

**MKT-023 — Price currency (frontend + backend)**: The price is stored in the asset's native currency. No currency conversion is applied at recording time. The asset's currency code is displayed as a read-only label next to the price input field so the user knows which currency they are entering.

**MKT-024 — i64 storage (backend)**: The price is stored as i64 micro-units per ADR-001. The frontend transmits the human-readable decimal; the backend converts to micros at the IPC boundary.

**MKT-025 — Upsert by (asset, date) (backend)**: If a price record already exists for the same `(asset_id, date)` pair, it is overwritten with the new value regardless of either row's `source` (per ADR-012; latest-write-wins). Otherwise a new record is created. This is transparent to the user; the form behaves identically for new and existing entries. The `source` value written by this command is governed by MKT-101.

**MKT-026 — AssetPriceUpdated event (backend)**: After a successful upsert, the backend publishes an `AssetPriceUpdated` event on the event bus. This event carries no payload; it is a bare signal consistent with `AssetUpdated`, `CategoryUpdated`, `AccountUpdated`, and `TransactionUpdated`. It is published by the `asset` bounded context per B4. The Tauri frontend event discriminant string is `"AssetPriceUpdated"` — the variant name is forwarded as-is by the event forwarder, matching the convention for all existing events.

**MKT-027 — In-flight state (frontend)**: While the upsert request is in progress, the submit button is disabled and displays a spinner to prevent double-submission.

**MKT-028 — Success feedback (frontend)**: On success, the modal closes and a snackbar confirms the price was recorded.

**MKT-029 — Error feedback (frontend)**: On validation failure or backend rejection, the modal remains open. An inline error message is shown adjacent to the invalid field. The user can correct and resubmit without reopening the form.

### Display in Account Details (030–039)

**MKT-030 — Current price column (frontend + backend)**: The Account Details active holdings table gains a "Current Price" column. For each holding row, it displays `HoldingDetail.current_price` formatted in the asset's native currency. When `current_price_date` is available, it is shown as a secondary label indicating the age of the data — formatted as "Updated today" when the date equals the user's local date, or "Updated Nd ago" otherwise. See MKT-140 for the staleness-label rules introduced alongside auto-fetch.

**MKT-031 — Latest price resolution (backend)**: The `AccountDetailsUseCase` retrieves the most recently dated `AssetPrice` for each active holding's asset via `AssetService`, per ADR-004 (use cases inject services, not repositories). If no record exists for an asset, `current_price` and `current_price_date` are `None`. A failure in the price lookup does not abort the overall `get_account_details` response; it degrades gracefully by returning `None` for the affected holding's price fields.

**MKT-032 — No-price diagnostic (frontend)**: When `current_price` is `None` for a holding, the "Current Price" column displays a typed diagnostic state derived from the holding's data, so the user can see _why_ a price is unavailable. Two states:

- **"Missing ticker"** — when `asset_reference` is empty (no identifier the price-fetch path can query). Signals that adding a ticker is the action that would unlock price fetch.
- **"No price available"** — when `asset_reference` is non-empty but `current_price` is `None`. The two upstream causes ("provider returned N/D" and "no fetch has run yet under a manual-update-frequency account") are intentionally merged into this single state in this phase; `update_frequency` is not consulted by the presenter. Disambiguation is deferred — see Open Questions.

**Cell composition.** The diagnostic string occupies the primary price slot. No secondary staleness label (MKT-140 is skipped — there is no `current_price_date` to label) and no source badge (MKT-142 is skipped — there is no `AssetPrice.source` to badge) are rendered alongside it. Both states render as subdued text.

**Interactivity.** The "Missing ticker" diagnostic is clickable — activating it opens the Edit Asset modal with the reference (ticker) input focused (other fields pre-filled per AST-012), so the user can supply the missing identifier and unlock price fetch without leaving the holdings view. After save, `AssetUpdated` fires and the holdings view re-fetches per ACD-040; the diagnostic transitions to a real price or to "No price available" on the next fetch cycle. The "No price available" diagnostic is non-interactive (no user action would change the outcome — both identifiers are present and fetch has yielded no data). The Edit Asset modal is mounted by a shell-mounted `AssetEditModalMount` that watches the URL for an `edit-asset` request; the click handler only mutates URL search params, keeping it inside `account_details` and avoiding a direct import of the `assets` feature.

**Other columns unchanged.** The "Unrealized P&L" and "Performance %" columns continue to display "—" when `current_price` is `None`, regardless of which diagnostic state the Current Price column shows (consistent with MKT-034's currency-mismatch placeholder behavior).

**MKT-033 — Unrealized P&L — same currency (backend)**: When the asset's native currency equals the account's currency (the gate condition defined in MKT-034), the backend computes `unrealized_pnl = (current_price − average_price) × quantity` using i128 intermediates before scaling back to i64, consistent with ACD-024. Both `current_price` and `average_price` are expressed in the same currency under this condition, making the subtraction valid. The result is included in `HoldingDetail.unrealized_pnl`. A zero result is returned as `0`, not `None`.

**MKT-034 — Unrealized P&L — currency mismatch (frontend + backend)**: When the asset currency differs from the account currency, `HoldingDetail.unrealized_pnl` and `HoldingDetail.performance_pct` are `None`. The frontend displays "—" in those columns. No exchange-rate conversion is attempted in this phase; multi-currency unrealized P&L is deferred to a future iteration.

**MKT-035 — Performance % (backend)**: When `unrealized_pnl` is available and `cost_basis` is non-zero, the backend computes `performance_pct = unrealized_pnl × 100 / cost_basis` as i64 micros using i128 intermediates and Rust integer division (truncation toward zero). Example: 5.25 % = 5 250 000 micros; −3.7 % = −3 700 000 micros. A zero result is returned as `0`, not `None`. When `cost_basis` is zero, `performance_pct` is `None`.

**MKT-036 — Reactivity (frontend)**: The Account Details event subscription adds `AssetPriceUpdated` alongside the existing `TransactionUpdated` and `AssetUpdated` subscriptions (ACD-039). Upon receiving `AssetPriceUpdated`, the view re-fetches account details, ensuring that newly recorded prices and all derived values (unrealized P&L, performance %, totals) are reflected immediately without a manual page refresh.

**MKT-037 — AssetPriceUpdated event registration (backend + frontend)**: The `AssetPriceUpdated` event is added to the event bus enum alongside `AssetUpdated`, `CategoryUpdated`, `AccountUpdated`, and `TransactionUpdated`. It is published exclusively by the `asset` bounded context. The global store treats it as a locally-handled event (no global data re-fetch triggered). `ARCHITECTURE.md` must be updated to register `AssetPriceUpdated` in the event bus table and to document that `useAccountDetails` subscribes to it alongside `TransactionUpdated` and `AssetUpdated`.

### Account Summary (040–049)

**MKT-040 — Total unrealized P&L (backend)**: `AccountDetailsResponse.total_unrealized_pnl` is the sum of `unrealized_pnl` across all active holdings for which the value is computable (same-currency holdings with a recorded price). Holdings with a currency mismatch or no recorded price are excluded from the sum and contribute nothing. When no holding qualifies, the field is `None`.

**MKT-041 — Total unrealized P&L display (frontend)**: The Account Details summary row displays `total_unrealized_pnl`. When the value is `None`, the summary shows "—". When the value is a number (including zero), it is displayed as-is; per-row "—" placeholders already communicate which individual holdings were excluded from the sum.

**MKT-042 — Correction by re-recording (backend)**: An `AssetPrice` record can be corrected by re-recording a price for the same `(asset_id, date)`, which overwrites the existing entry (MKT-025). Standalone edit (MKT-083, MKT-084) and delete (MKT-090) via the price history view are also supported.

**MKT-043 — Unknown asset rejection (backend)**: The backend rejects `record_asset_price` with a specific error if `asset_id` does not refer to a known asset. In normal use the asset is always selected from active holdings, making this case unreachable from the UI; the guard exists for API-level correctness.

### Auto-record from Transactions (050–069)

This section extends the buy/sell transaction flow defined in `docs/spec/financial-asset-transaction.md` (TRX) and `docs/spec/sell-transaction.md` (SEL). When the user opts in, recording a buy or sell transaction also writes an `AssetPrice` record so the transacted unit price becomes the asset's market price for the transaction date. Standalone edit and delete of `AssetPrice` records are now supported via the price history view (MKT-070+).

**MKT-050 — Global auto-record toggle (frontend)**: The Settings page exposes a toggle "Automatically record transaction price as market price". The toggle defaults to OFF. The user's choice persists across sessions on the current device. The toggle controls only the default state of the per-transaction checkbox (MKT-052); it never bypasses or replaces that checkbox.

**MKT-051 — Per-transaction checkbox (frontend)**: The buy and sell transaction forms (both creation forms — modal and the standalone /transactions/new page — and their edit variants) display a checkbox "Use this price as the market price for {transaction date}" placed immediately before the form's primary submit action, after all data fields. The label's date placeholder reflects the form's current `date` field value and updates live when the user changes it.

**MKT-052 — Checkbox default state (frontend)**: When the form opens to **create** a new transaction, the checkbox initial state equals the current value of the global toggle (MKT-050). When the form opens to **edit** an existing transaction, the checkbox initial state is always OFF, regardless of the global toggle.

**MKT-053 — Checkbox snapshot semantics (frontend)**: The checkbox initial state (MKT-052) is read from the global toggle once at form open. Subsequent changes to the global toggle do not propagate into already-open forms; the user keeps whatever state they have already set in the open form.

**MKT-054 — Submit payload (frontend + backend)**: The frontend forwards the checkbox state as a `record_price: bool` field added to the existing `CreateTransactionDTO` already used by `add_transaction` and `update_transaction` (per the project's Specta single-DTO convention). The backend never reads the global toggle directly — the per-call flag carried in the DTO is the only signal that determines whether a price is recorded.

**MKT-055 — Auto-write as a separate frontend call (frontend)**: When `record_price` is `true` and `tx.unit_price > 0` (see MKT-061 for the zero-price exception), the frontend calls `record_asset_price(asset_id, tx.date, tx.unit_price)` as a separate Tauri command **after** the transaction command (`add_transaction` / `update_transaction`) has returned successfully. The write targets `(asset_id = tx.asset_id, date = tx.date, price = tx.unit_price)` and uses the same upsert semantics as MKT-025 (insert on absence, replace on `(asset_id, date)` collision). The price is taken in the asset's native currency; `tx.unit_price` already excludes fees per the TRX domain definition, so no fee adjustment is applied. Validation rules MKT-021 (price > 0) and MKT-022 (date not in future) hold by construction: TRX-020 enforces `tx.date` not in the future, and the `tx.unit_price > 0` precondition is enforced by MKT-061.

**MKT-056 — No atomicity between transaction and price write (frontend)**: The transaction insert/update and holding recomputation (TRX-027 / SEL-025) are committed atomically by the backend command. The `record_asset_price` call (MKT-055) is a separate, independent command issued by the frontend after the transaction commits. There is no shared database transaction between the two operations. If the transaction succeeds and the price write subsequently fails, the transaction remains committed; no rollback occurs. This is a deliberate trade-off documented in ADR-006.

**MKT-057 — AssetPriceUpdated event on auto-record (backend)**: After `record_asset_price` commits successfully (i.e. MKT-061 did not skip the call), the backend publishes the `AssetPriceUpdated` event defined in MKT-026 via `AssetService::notify_asset_price_updated()`. This event is in addition to the `TransactionUpdated` event published by the transaction command. The two events are independent signals; their relative order is unspecified and irrelevant because subscribers refetch idempotently. When `record_price` is `false` or MKT-061 skipped the call, no `AssetPriceUpdated` event is published.

**MKT-058 — Conflict — silent overwrite (backend)**: If an `AssetPrice` record already exists at `(tx.asset_id, tx.date)` when `record_price` is `true`, it is silently overwritten with `tx.unit_price` via the same upsert semantics as MKT-025. No prompt or warning is shown to the user; the form behaves identically whether or not a same-day price already exists.

**MKT-059 — Edit lifecycle — price independence (backend)**: Editing a transaction does not modify or remove any `AssetPrice` record previously written by that transaction. When the user re-saves an edited transaction with `record_price = true`, the upsert (MKT-055) targets the transaction's _current_ `tx.date` and _current_ `tx.unit_price`. If the user changed the transaction date during the edit, the price record at the prior date is left untouched and remains in storage; the upsert lands at the new date as a separate `(asset_id, date)` row. The same applies if the user changed the unit price: only the row at the current date is overwritten.

**MKT-060 — Delete lifecycle — price independence (backend)**: Deleting a transaction does not remove any `AssetPrice` record previously written by that transaction. `AssetPrice` records are independent of the transaction lifecycle: once persisted, they are governed solely by MKT rules (upsert via MKT-025; standalone delete via MKT-090).

**MKT-061 — Zero unit_price skip (backend)**: If `record_price` is `true` and `tx.unit_price` is `0` (a valid transaction per TRX-020 / SEL-020 — gifted or inherited assets), the orchestrator silently skips the `AssetPrice` write. The transaction itself proceeds normally and commits per its own validation rules; no `AssetPriceUpdated` event is published; no error is surfaced to the user. Rationale: a zero market price would conflict with MKT-021 (price > 0) and is not a meaningful signal of the asset's market value.

**MKT-062 — Auto-record failure is best-effort (frontend)**: A failure of the `record_asset_price` call (MKT-055) does not surface an error to the user. The transaction is already committed (MKT-056) and the frontend treats the price write as fire-and-forget: failures are logged as warnings and silently dropped. No inline error, snackbar, or retry affordance is shown specifically for the price write. The user may still record the price manually via the price history modal (MKT-070+).

### Price History CRUD (070–092)

**MKT-070 — Price history entry point (frontend)**: A "Price history" action is available on each active holding row in Account Details, alongside the existing Buy, Sell, and "Enter price" actions. It is not shown on closed holdings.

**MKT-071 — Price history list contents (frontend)**: The price history view lists all `AssetPrice` records for the selected asset, sorted by date descending (most recent first). Each row displays the date and the price formatted in the asset's native currency. Edit and Delete affordances are shown on each row. `AssetPrice` is per asset (not per holding), so the history shows all recorded prices regardless of which account the entry point was reached from.

**MKT-072 — Backend query for price history (backend)**: The `asset` bounded context exposes a `get_asset_prices(asset_id)` command that returns all `AssetPrice` records for a given `asset_id`, ordered by date descending. The returned list includes all historical entries, not only the most recent. The command returns a specific error if `asset_id` does not refer to a known asset; a successful response with an empty list is returned when the asset exists but has no recorded prices.

**MKT-073 — Empty state (frontend)**: When no `AssetPrice` records exist for the asset, the price history view displays a message indicating no prices have been recorded yet, and offers an affordance to add the first price.

**MKT-074 — Loading and error states (frontend)**: While price records are being fetched, the price history view displays a loading indicator. A fetch failure surfaces an inline error with a retry affordance; the view remains open rather than closing.

**MKT-075 — In-view "Add price" affordance (frontend)**: The price history view includes an action to add a new price entry, opening the same recording form governed by MKT-020–MKT-029 (same validation, same upsert semantics, same `AssetPriceUpdated` publication). After a successful add, the history list refreshes to include the new entry.

**MKT-076 — View reactivity after mutations (frontend)**: After a successful add (MKT-075), edit (MKT-086), or delete (MKT-092), the price history view re-fetches its list via MKT-072. The `AssetPriceUpdated` event emitted by the backend (MKT-026, MKT-085, MKT-091) independently causes the Account Details view to re-fetch, keeping the current-price column and derived values in sync without any additional coordination by the price history view.

**MKT-080 — Edit action (frontend)**: Each row in the price history list has an Edit action that opens an edit form pre-filled with that row's date and price.

**MKT-081 — Edit form fields (frontend)**: The edit form displays the asset name as a read-only label, an editable date field, and an editable price field, both pre-filled with the current values of the selected entry. The asset's currency code is shown as a read-only label next to the price field, consistent with MKT-023.

**MKT-082 — Edit validation (frontend + backend)**: The edit form applies the same validation as price recording: price must be strictly greater than zero (MKT-021) and date must be a well-formed ISO 8601 calendar date not in the future (MKT-022). The backend rejects invalid values with the same specific errors. The frontend validates inline and disables the submit button until all fields are valid.

**MKT-083 — Edit semantics — same date (backend)**: The `asset` bounded context exposes an `update_asset_price(asset_id, original_date, new_date, new_price)` command that returns `()` on success. When `original_date` equals `new_date`, the backend updates the price in place at the existing `(asset_id, date)` row; a single-row in-place update is inherently atomic. The command returns `NotFound` if the record at `(asset_id, original_date)` does not exist, or `Unknown` for any other failure.

**MKT-084 — Edit semantics — date changed (backend)**: When `original_date` differs from `new_date` in an `update_asset_price` call (MKT-083), the backend deletes the record at `(asset_id, original_date)` and upserts a new record at `(asset_id, new_date)`. If a record already exists at the new date, it is silently overwritten, consistent with MKT-025. The deletion and the upsert are atomic within a single database transaction; a failure in either step rolls back the entire operation.

**MKT-085 — AssetPriceUpdated after successful edit (backend)**: After a successful edit (MKT-083 or MKT-084), the backend publishes an `AssetPriceUpdated` event consistent with MKT-026. No event is published if the edit fails.

**MKT-086 — Edit success feedback (frontend)**: On a successful edit, the edit form closes, the price history list refreshes (MKT-076), and a snackbar confirms the price was updated.

**MKT-087 — Edit error feedback (frontend)**: On a validation failure or backend rejection, the edit form remains open. An inline error message is shown adjacent to the invalid field. The user can correct and resubmit without reopening the form.

**MKT-094 — Edit in-flight state (frontend)**: While the edit submit request is in progress, the submit button is disabled and displays a spinner to prevent double-submission, consistent with MKT-027.

**MKT-095 — asset_id is immutable on edit (backend)**: The `update_asset_price` command (MKT-083) does not accept a new `asset_id`. The asset an `AssetPrice` belongs to cannot be changed after creation; only the date and price are modifiable.

**MKT-088 — Delete action (frontend)**: Each row in the price history list has a Delete action.

**MKT-089 — Delete confirmation dialog (frontend)**: Triggering the Delete action opens a confirmation dialog that identifies the date and price of the record to be removed. The user must explicitly confirm before deletion proceeds.

**MKT-090 — Delete command (backend)**: The `asset` bounded context exposes a `delete_asset_price(asset_id, date)` command that returns `()` on success. If no record exists at `(asset_id, date)`, the command returns `NotFound`. Any other failure returns `Unknown`.

**MKT-091 — AssetPriceUpdated after deletion (backend)**: After a successful deletion, the backend publishes an `AssetPriceUpdated` event consistent with MKT-026. The Account Details view re-fetches via MKT-036; if the deleted entry was the most recently dated price for the asset, the holding row falls back to the next most recent price or shows "—" if no records remain.

**MKT-092 — Delete success feedback (frontend)**: On a successful deletion, the price history list refreshes (MKT-076) and a snackbar confirms the price record was removed.

**MKT-093 — In-flight state for delete (frontend)**: While the delete request is in progress (after the user confirms in MKT-089), the Delete action for the targeted row is disabled to prevent double-submission.

**MKT-096 — Delete error feedback (frontend)**: If the delete request fails, an error banner is shown at the top of the price history list. The view remains open and the targeted entry is not removed from the list. The user may retry.

### Source field on AssetPrice (100–109)

These rules apply to all paths that write `AssetPrice` (manual entry MKT-020+, transaction auto-record MKT-050+, auto-fetch — see "Auto-Fetch from External Provider" — and the price history backfill MKT-190+).

**MKT-100 — `AssetPriceSource` enum (backend)**: `AssetPrice.source` is of type `AssetPriceSource`, with variants `Manual | YahooFinance`. Exposed on the frontend wire surface. (Per ADR-017 the provider is keyless Yahoo Finance; the former `Stooq` / `Finnhub` variants are removed.)

**MKT-101 — `source: Manual` on user-driven paths (backend)**: Every user-driven write sets `source = Manual` — both `record_asset_price` (manual entry MKT-020+, transaction auto-record MKT-050+) and `update_asset_price` (price-history edit MKT-083, MKT-084). An auto-fetched row edited via the price-history flow therefore becomes `Manual`. The frontend never passes a source value.

**MKT-102 — `source: YahooFinance` on fetched paths (backend)**: Every write produced by a fetch path (launch MKT-122, global refresh MKT-130, account refresh MKT-132) or by a price history backfill (MKT-192) sets `source = YahooFinance`.

### Auto-Fetch from External Provider (110–149)

This section adds an automated price-update mechanism that complements the existing manual entry paths (MKT-020+, MKT-050+). Auto-fetch retrieves current prices from an external provider on app launch and on user demand. The choice of external provider is captured in [ADR-017](../adr/017-yahoo-finance-keyless-price-source.md): the keyless Yahoo Finance `/v8/finance/chart/` JSON endpoint, the sole automated source (no API key, no proof-of-work).

#### Fetch task definitions

- **Auto-fetch**: a task called at application launch.
- **Global refresh**: a task manually triggered by the user to refresh all values (button on global dashboard).
- **Account refresh**: a task triggered by the user on an account detail page to refresh all values for that account (button on account detail).

#### Shared behaviors (110–119)

**MKT-110 — Symbol derivation (backend)**: The Yahoo Finance provider symbol is resolved per ADR-017 with the following precedence:

1. If `Asset.exchange` is set, the symbol is `Asset.reference` joined by `.` to the Yahoo venue suffix of the exchange (e.g. `VOD` + LSE → `VOD.L`, `BMW` + XETRA → `BMW.DE`, `MC` + Euronext Paris → `MC.PA`); the suffix is produced by a per-provider mapper from the canonical `Exchange`. US venues (NYSE/Nasdaq) map to an **empty** suffix, so the symbol is the bare reference (`AAPL`) — Yahoo addresses US listings without a suffix.
2. If `Asset.exchange` is unset, the symbol is the bare `Asset.reference`. This branch preserves the US-ticker happy path and covers legacy assets created before the exchange field existed.
3. If the mapper returns no suffix for a non-US exchange it does not recognise, or the resolved string is empty, the asset is skipped per MKT-114.

In all branches a class-share `/` separator in the reference is translated to Yahoo's `-` convention: OpenFIGI spells Berkshire Hathaway B as `BRK/B`, but Yahoo resolves `BRK-B`. The reference itself is left unchanged — the translation is local to the provider symbol.

**MKT-111 — Empty-holdings rejection (backend)**: When the task's scope contains no holding asset that is both active (quantity > 0) and has a derivable provider symbol (MKT-110), the fetch task is rejected with a specific error so the frontend can give feedback. No external calls are made. Applies to every fetch task path (launch MKT-122, global refresh MKT-130, account refresh MKT-132).

**MKT-112 — `AssetPriceUpdated` on fetch success (backend)**: Every successful `AssetPrice` write produced by a fetch publishes `AssetPriceUpdated` per MKT-026.

**MKT-113 — In-flight guard, one fetch at a time (backend)**: Only one fetch task may run at a time across all three paths (launch MKT-122, global refresh MKT-130, account refresh MKT-132). If a fetch task is already running, a subsequent task call is rejected with a specific error.

**MKT-114 — Asset silently skipped on per-asset failure (backend)**: In a fetch task (launch MKT-122, global refresh MKT-130, account refresh MKT-132), an individual asset is silently skipped (no row written, no error surfaced) when either its symbol cannot be derived OR the provider fetch fails (network/HTTP/parse error, logged as warning). The task continues with the remaining assets.

**MKT-115 — Manual refresh feedback (frontend)**: For user-triggered fetch actions (global refresh MKT-130, account refresh MKT-131), the frontend surfaces feedback via snackbar:

- On successful dispatch by the backend: a snackbar acknowledges the fetch has started (e.g. "Fetching prices…").
- On in-flight rejection (MKT-113): a snackbar indicates a fetch is already in progress; the user's action is rejected without disrupting the ongoing fetch.
- On no-fetchable-holdings rejection (MKT-111): a snackbar indicates there are no holdings to fetch.

The launch auto-fetch (MKT-121) shows no dispatch snackbar; its outcome is silent on success but surfaces failures via the completion snackbar (MKT-145).

**MKT-116 — System cash assets excluded (backend)**: System cash assets (per CSH spec, identified by their `system-cash-*` reference) are excluded from every fetch task scope (launch MKT-122, global refresh MKT-130, account refresh MKT-132). They have no external market price.

**MKT-117 — Provider returns the observation date (backend)**: `PriceProvider::fetch_price` returns the provider's observation date alongside the price (the date the quote is _for_, not the time of the fetch). The Yahoo adapter derives this from the chart response's regular-market timestamp (epoch seconds, converted to the exchange-local ISO date). A provider that does not supply a date returns it as absent.

**MKT-118 — Fetched price is dated by the observation date, with a today fallback (backend)**: When a fetch writes an `AssetPrice`, it uses the provider's observation date (MKT-117) as `AssetPrice.date` — keyed and upserted by `(asset_id, observation_date)` per MKT-025 — provided that date is a well-formed ISO `yyyy-mm-dd` not in the future. When the observation date is absent, malformed, or in the future, it falls back to the current local date. The price is always recorded; an unusable observation date never causes a skip (contrast MKT-114, which skips only on price/network/parse failure). Effect: a fetch on a non-trading day dates the row at the last trading day, so the staleness label (MKT-140) reads honestly (e.g. "Updated 2d ago" on a Sunday), and repeated non-trading-day fetches are idempotent on that row rather than minting a new current-dated row.

**MKT-119 — Fetch task-completion signal (backend)**: When a fetch task finishes, the backend publishes an `AssetPriceFetchCompleted { ok, skipped }` event carrying the outcome counts — `ok` = assets whose price was updated, `skipped` = assets with no data or a fetch/upsert failure (MKT-114). It is published once per task, after the per-asset loop, for every entry point (launch MKT-122, global refresh MKT-130, account refresh MKT-132). Distinct from the per-asset `AssetPriceUpdated` (MKT-026).

#### Auto-fetch (120–125)

**MKT-120 — Auto-fetch setting (frontend)**: An auto-fetch setting is present on the Settings page. The setting defaults to `OFF` and persists across sessions on the current device. When `ON`, the auto-fetch feature is enabled; when `OFF`, it is disabled.

**MKT-121 — Auto-fetch call (frontend)**: If the setting (MKT-120) is `ON`, the frontend calls the auto-fetch task once per session, after initial app mount. The call is fire-and-forget (the frontend does not await the backend response).

**MKT-122 — Auto-fetch start (backend)**: The auto-fetch task scope is all active holdings across all accounts (subject to MKT-111, MKT-116). Auto-fetch is acknowledged synchronously; per-asset results are signaled via `AssetPriceUpdated` (MKT-112).

**MKT-125 — Sub-unit (pence) quotes normalized to the major ISO unit (backend)**: Applies to every fetch-write path (launch MKT-122, global refresh MKT-130, account refresh MKT-132) and to the price history backfill (MKT-192). Some venues quote in a currency's minor unit — Yahoo reports London (LSE) prices in `GBp` (pence), Johannesburg in `ZAc` (cents), Tel Aviv in `ILA` (agorot). When the provider's quoted currency is one of the recognised minor-unit codes (`GBp`, `ZAc`, `ILA`), the adapter divides the price by 100 and persists it under the corresponding major ISO currency (`GBp → GBP`, `ZAc → ZAR`, `ILA → ILS`). Any currency code **not** in that recognised minor-unit set — including every major ISO code — is treated as already major and stored unchanged (no division). A minor-unit code is never persisted as a currency. (Known limitation: a minor-unit code outside the recognised set would be stored unscaled; the recognised set is widened if such a venue surfaces.)

#### Manual refresh (130–134)

**MKT-130 — Global refresh (frontend)**: The global refresh action is triggered by the user on the global dashboard. It uses the same backend entry point as the auto-fetch call (MKT-122) and therefore shares its scope (all active holdings across all accounts). The call is fire-and-forget.

**MKT-131 — Account refresh (frontend)**: The account refresh action is triggered by the user on an account detail page. The account identifier is transmitted to the backend. The call is fire-and-forget.

**MKT-132 — Account refresh (backend)**: Account refresh on an unknown account is rejected with a specific error. Otherwise account refresh is acknowledged synchronously; per-asset results are signaled via `AssetPriceUpdated` (MKT-112). The task scope is the active / derivable holdings for the specified account (subject to MKT-111, MKT-116).

**MKT-133 — Refresh button in-flight state (frontend)**: While a manual refresh is being acknowledged (between the user click and the dispatch-success snackbar of MKT-115, or the in-flight error snackbar), the corresponding refresh button is disabled and displays a spinner to prevent double-clicks.

#### Display (140–149)

**MKT-140 — Staleness indicator (frontend)**: The "Current Price" column's secondary label shows "Updated today" when the most recent `AssetPrice.date` for the asset equals today's local date, "Updated Nd ago" otherwise (N integer day delta). When no price exists, the secondary label is omitted and MKT-032's diagnostic state occupies the primary cell instead.

**MKT-141 — Source badge in price history (frontend)**: Each row in the price-history modal (MKT-071) displays a badge with the row's `source` value.

**MKT-142 — Source badge in Current Price column (frontend)**: The Account Details "Current Price" column displays a badge alongside the price (or near the staleness label MKT-140) showing the source of the most recent `AssetPrice` record. Same styling as MKT-141.

**MKT-143 — Current Value column (frontend)**: The Account Details active-holdings table includes a "Current Value" column, placed immediately after "Current Price", showing the holding's market value as `current_price × quantity` in the asset's native currency. When `current_price` is `None`, the column shows the same neutral placeholder as the other market columns ("—"). The system cash row leaves the cell blank, consistent with its other market columns. This is a presentation-only derivation — no new `HoldingDetail` field is introduced.

**MKT-145 — Fetch-outcome snackbar (frontend)**: On `AssetPriceFetchCompleted` (MKT-119), the frontend shows a snackbar only when `skipped > 0`: an error snackbar ("Couldn't update prices (N)") when `ok == 0`, otherwise an info snackbar summarizing the partial result ("Updated N · M couldn't be updated"). A fully successful fetch (`skipped == 0`) shows nothing, so the launch auto-fetch (MKT-121) stays silent on the happy path while failures surface from any entry point. Handled globally (the event is not correlated to a specific trigger). This snackbar is superseded by the unupdated-prices modal whenever that modal auto-opens for the same signal (MKT-173) — the two never appear together for one completion event.

### Price Refresh Lock (150–169)

This section lets the user pin an asset's recorded price against automated overwrites. By default every fetch task (MKT-122/130/132) overwrites the asset's same-day price under the latest-write-wins policy ([ADR-012](../adr/012-latest-write-wins-source-as-metadata.md)); a manual correction is therefore replaced on the next refresh (see the MKT-100+ "manual override" flow). Locking an asset excludes it from all fetch tasks, so its most recently recorded price — typically a manual correction — is preserved until the user unlocks it. The lock is a property of the asset (prices are per asset, not per holding), so it applies across every account that holds the asset. The decision to implement the pin as fetch-scope exclusion (rather than reintroducing the write-time precedence ADR-012 removed) is recorded in [ADR-014](../adr/014-price-refresh-lock-scope-exclusion.md), which fulfills ADR-012's deferral.

**MKT-150 — Lock flag on Asset (backend)**: The `Asset` entity gains a `price_refresh_blocked` boolean (owned by the `asset` bounded context; see AST spec). It defaults to not-locked for new and existing assets, is persisted, and is exposed on the asset wire surface so the frontend can render the lock state. It is independent of `is_archived` and is not modified by an asset edit (MKT-155).

**MKT-151 — Locked asset excluded from fetch scope (backend)**: In every fetch task (launch MKT-122, global refresh MKT-130, account refresh MKT-132), an asset whose `price_refresh_blocked` is true is excluded from the task scope — no symbol is derived (MKT-110), no provider call is made, and no `AssetPrice` row is written for it. This is the same kind of scope exclusion as the system-cash rule (MKT-116); all other assets in the task proceed normally. A locked asset does not count toward the fetchable set of MKT-111: a task whose only candidates are locked (or system-cash) assets is rejected with the no-fetchable-holdings error, exactly as for a cash-only scope. Because the lock is a property of the asset, locking an asset held in one account also excludes it from another account's refresh (MKT-132).

**MKT-152 — Lock applies only to provider fetches (frontend + backend)**: The lock suppresses only automated provider fetches (MKT-151). It does not restrict user-driven price writes: manual entry (MKT-020+), price-history add / edit / delete (MKT-070+), and transaction auto-record (MKT-050+) all remain available on a locked asset — the backend accepts those writes unchanged, and on the frontend the Enter price (MKT-010) and Price history (MKT-070) actions stay visible and usable on a locked holding row. Locking is the mechanism for keeping a deliberately-entered price; it never prevents the user from changing that price themselves.

**MKT-153 — Lock toggle entry point (frontend)**: A lock / unlock action is available on each active, non-cash holding row in Account Details, alongside the existing Buy, Sell, Enter price, and Price history actions. The action's icon reflects the asset's current `price_refresh_blocked` state (locked vs unlocked). It is not shown on the system cash row (MKT-154) nor on closed holdings.

**MKT-154 — System cash cannot be locked (backend)**: Setting or clearing the lock on the system Cash Asset is rejected with a specific error, consistent with the cash-not-editable invariant (CSH-016). The cash row does not expose the toggle (MKT-153), so this guard is for API-level correctness.

**MKT-155 — Lock independent of edit and archive (backend)**: Editing an asset (the AST update flow) leaves `price_refresh_blocked` unchanged, and archiving or unarchiving an asset does not change it either. Notwithstanding AST-005 ("all asset fields are editable after creation"), `price_refresh_blocked` is not part of the Edit Asset form's editable field set — it is toggled exclusively by its dedicated action (MKT-156), mirroring how `is_archived` is governed by the archive / unarchive actions rather than the edit form.

**MKT-156 — Toggle commands (backend)**: The `asset` bounded context exposes commands to set and to clear `price_refresh_blocked` for a given `asset_id`, each acknowledged synchronously and returning `()` on success. On success the backend publishes the `AssetUpdated` event, consistent with every other asset-state write (create, update, archive, unarchive, delete). A command targeting an unknown asset is rejected with a specific error (consistent with MKT-043). Locking an already-locked asset, or unlocking an already-unlocked one, is idempotent and succeeds without error.

**MKT-157 — Toggle reactivity and feedback (frontend)**: After a successful toggle, the frontend re-reads the asset list once the command returns (mirroring the archive / unarchive flow; the `AssetUpdated` event published per MKT-156 keeps other views in sync). The holding row sources `price_refresh_blocked` from that asset slice — the same store it already uses for archive state — so its lock icon reflects the new state, and `HoldingDetail` is not extended with the flag. A snackbar confirms the change, and a subsequent refresh (MKT-130 / MKT-131) then skips or includes the asset per MKT-151.

**MKT-158 — Locked state preserves the displayed price (frontend + backend)**: While an asset is locked, its Current Price column continues to display the most recently recorded `AssetPrice` (MKT-030); because fetches skip the asset (MKT-151), the staleness label (MKT-140) may age ("Updated Nd ago") without being refreshed. The source badge (MKT-142) keeps reflecting the recorded row's source (typically `Manual`).

### Unupdated-Price Manual Fill (170–189)

This section lets the user hand-enter prices for the assets a fetch task could not update. A fetch silently skips an asset whose price the provider cannot supply (MKT-114); those assets keep a stale price or none at all. Rather than leave the gap invisible, the fetch's completion signal now also names the unpriced assets, and the frontend presents them in a single modal where the user can enter a value per asset or skip it. Manual entries reuse the existing recording path (MKT-020+), so no new write behavior is introduced — only a richer completion signal and a new presentation surface.

**MKT-170 — Unpriced-asset list on the completion signal (backend)**: The fetch task-completion signal (MKT-119) carries, in addition to the `ok` / `skipped` counts, the list of assets that were counted in `skipped` — i.e. every in-scope asset not in the `ok` set, defined by the outcome counter rather than by data availability (the rare "provider returned data but the upsert failed" case is therefore included, while a today-fallback write per MKT-118 is an `ok` and is excluded). Each entry identifies the asset by name, ticker (`reference`), and ISIN (when the asset has one), and carries the asset's native currency together with its most recently recorded price and that price's observation date (both absent when the asset has never had a price recorded). The carried price and date are a point-in-time snapshot for modal display only — not an authoritative source; the modal's accuracy after a manual fill comes from the MKT-179 re-fetch, not from this snapshot. The list is published once per task, after the per-asset loop, for every fetch path (launch MKT-122, global refresh MKT-130, account refresh MKT-132), consistent with MKT-119.

**MKT-171 — Scope of the unpriced list (backend)**: The unpriced list (MKT-170) contains exactly the in-scope assets the fetch could not price — provider returned no data, the provider fetch failed, the symbol could not be derived, or the upsert failed (the full MKT-114 skip set). Assets excluded from the fetch scope entirely — system cash (MKT-116) and refresh-locked (MKT-151) — never enter the list, since they are never counted as skipped. The list length equals the `skipped` count (MKT-119).

**MKT-172 — Modal auto-opens on a fetch with unpriced assets (frontend)**: When a completion signal (MKT-170) reports a non-empty unpriced list, the frontend automatically opens the unupdated-prices modal listing those assets. When the list is empty (a fully successful fetch), no modal opens. This applies to every fetch path (launch MKT-122, global refresh MKT-130, account refresh MKT-132), handled globally like the completion-snackbar (MKT-145) — the modal is not correlated to a specific trigger.

**MKT-173 — Modal supersedes the fetch-outcome snackbar (frontend)**: When the unupdated-prices modal auto-opens (MKT-172), the partial-result / failure snackbar of MKT-145 is not also shown for that same completion signal — the modal is the richer surface and the snackbar would duplicate it. A fully successful fetch remains silent on both surfaces.

**MKT-174 — Modal list contents (frontend)**: The modal shows one row per unpriced asset. Each row displays the asset name, its most recently recorded price formatted in the asset's native currency (or a "no previous price" indicator when the asset has never had a price), the ticker (`reference`), the ISIN (when present), and an empty price input for the new value. The asset's currency code is shown next to the input, consistent with MKT-023.

**MKT-175 — Manual entry per row (frontend + backend)**: Entering a value in a row and confirming it records a market price for that asset through the existing manual-recording path (MKT-020+, MKT-025): the price is written in the asset's native currency, dated to the fetch day (the user's current local date), with `source = Manual` (MKT-101). Each row is recorded independently as it is confirmed; there is no batch write. The same price and date validation applies (MKT-021, MKT-022): the date is valid by construction (today is never in the future), while a price that fails MKT-021 (≤ 0 or non-finite) is rejected and surfaced inline on that row per MKT-178.

**MKT-176 — Skip a row (frontend)**: A row can be skipped without entering a value. Skipping records nothing; the asset's price is left unchanged (stale, or absent if it never had one). Skipping is always available and never produces an error.

**MKT-177 — Row resolution and modal dismissal (frontend)**: A row that has been recorded (MKT-175) or skipped (MKT-176) leaves the list. When every row has been resolved, the modal closes automatically. The user may also dismiss the whole modal at any time; any rows still unresolved are treated as skipped (MKT-176).

**MKT-178 — Per-row in-flight, success, and error feedback (frontend)**: While a row's record request is in flight, that row's confirm action is disabled to prevent double-submission (consistent with MKT-027). On success, a per-row confirmation is shown and the row leaves the list. On a validation failure or backend rejection, the row remains in the list with an inline error adjacent to its input so the user can correct and retry, consistent with MKT-029; other rows are unaffected.

**MKT-179 — Reactivity after a manual fill (frontend + backend)**: Each successful per-row record (MKT-175) publishes `AssetPriceUpdated` (MKT-026), so the Account Details and dashboard views re-fetch and reflect the newly entered price and its derived values, consistent with MKT-036. No additional coordination is required from the modal.

**MKT-180 — Fetch progress in the shell (backend + frontend)**: The fetch task publishes `AssetPriceFetchProgress { done, total }` — once at task start (`done = 0`, `total` = scope size) and once after each attempted asset, successful or skipped (`done` = attempts so far). The frontend tracks the in-flight state in the global store and renders a thin determinate progress bar in the shell, below the header and visible on every page, while a fetch is active. `AssetPriceFetchCompleted` (MKT-119) clears the state and hides the bar.

**MKT-181 — Coalesced view refresh during a bulk fetch (frontend)**: While a fetch task is in flight (per the MKT-180 store state), the views that re-fetch on `AssetPriceUpdated` (MKT-036 — account details, account performance, account summaries — and the header's price item, MKT-203) suppress those per-asset re-fetches and reload once on `AssetPriceFetchCompleted`. Outside an active fetch, single `AssetPriceUpdated` events (manual price entry, manual-fill modal per MKT-179) keep triggering an immediate re-fetch.

### Price History Backfill (190–199)

This section fills a holding's price history in one action. Account history, as-of views and performance value a holding at the price recorded for each date; an asset bought before the scheduled fetch existed, or held while it was off, has dates with no price. A price history backfill asks the provider for the daily closes of the held period and records them on the dates that carry no price.

**MKT-190 — Price history backfill action (frontend)**: A "Fill missing price history" action is available on each active, non-cash holding row in Account Details, placed with the Price history action (MKT-070), and on each closed position row (ACD-049). It is not shown on the system cash row nor in the read-only as-of view. It stays visible on a refresh-locked holding and on a closed position of an archived asset: the rejection (MKT-196) is the feedback.

**MKT-191 — Held period (backend)**: A price history backfill covers the held period: from the date of the account's earliest transaction on the asset through yesterday while the holding is active, or through the date of its latest transaction on the asset once the holding is closed — never later than yesterday (the day before today's local date); today's date is left to the fetch tasks and the scheduled fetch. A provider close for a session still in progress is not a close and is not recorded (SPF-033). Every transaction type on the asset counts toward both bounds, and a gap between selling out and buying back lies inside the period and is filled like any other date. A held period that would start after yesterday (a holding opened today) is empty: once the rejections made before any request have passed — except the provider-symbol check, which needs a period to request (MKT-196) — the backfill requests nothing, records nothing and succeeds with both counts at zero (MKT-194).

**MKT-192 — Recorded prices are never overwritten (backend)**: A price history backfill records a daily close only on a date that carries no `AssetPrice` for the asset; a date that already carries one keeps it unchanged, whatever its source (MKT-100), so a backfill over a complete history writes nothing. This is the one write path exempt from latest-write-wins (ADR-012 decision 1, MKT-025): the user asks to fill gaps, not to refresh what exists, unlike the rate backfill (FXR-113). The promise holds on the device that runs the backfill; when a multi-device merge brings another write for the same date, CFR-050 decides as for any record. Recorded closes carry `source = YahooFinance` (MKT-102) with the sub-unit normalization of every fetch (MKT-125).

**MKT-193 — Ranged requests (backend)**: The daily closes come from the provider's ranged daily-close request (SPF-030/031), never from one request per date. The held period is split into consecutive windows of at most 365 days — the application's cap on the range of a single request — one request each.

**MKT-194 — Outcome (backend)**: A successful price history backfill returns two counts: `written`, the closes recorded on dates that had no price, and `already_priced`, the closes the provider served for dates that already had one. When `written` is at least one, `AssetPriceUpdated` is published once (MKT-026), so the views re-fetch (MKT-036).

**MKT-195 — Failures (backend)**: Every window is requested before anything is recorded; when any request fails (network, HTTP or parse error), the price history backfill is rejected with a provider-unreachable error and records nothing. A storage failure while recording rejects it with a database error, keeps the closes already recorded and publishes no `AssetPriceUpdated`; running the price history backfill again records the rest, since a priced date is never rewritten (MKT-192).

**MKT-196 — Rejections (backend)**: A price history backfill is rejected before any request, recording nothing, when the account is unknown, the asset is unknown, the asset is the system Cash Asset (MKT-116), the asset is archived (AST-006), the account has no transaction on the asset, the asset is refresh-locked (MKT-151 — the lock keeps the provider away from the asset), or no provider symbol can be derived (MKT-110) — the last as an unresolvable ticker. After the requests, it is rejected as an unresolvable ticker, recording nothing, when the provider reports the symbol unknown, or when it knows the symbol but serves no close over a held period of 7 days or more — such a period always holds a trading day, so the provider has no history for the asset. A known symbol served no close over a shorter held period — a weekend or a market holiday — is not a rejection: the backfill succeeds with both counts at zero (MKT-194). The requests stop at the first window that fails or reports the symbol unknown, and that window decides the rejection (MKT-195). The scheduled fetch skips an unknown symbol quietly instead (SPF-041): here the user asked for the backfill, so the user is told.

**MKT-197 — Feedback (frontend)**: When the price history backfill succeeds with `written` at least one, a success snackbar states both counts; with `written` at zero — an empty held period (MKT-191) and a held period the provider serves no close for (MKT-196) included — an info snackbar states that there is no missing price to fill. A rejection shows an error snackbar naming its cause.

**MKT-198 — Pending state (frontend)**: While a price history backfill runs for a holding, that holding's price history backfill action is disabled and shows a pending state; every other action stays usable, and several holdings may run at once.

**MKT-199 — Independent of fetch tasks (backend)**: A price history backfill is not subject to the fetch in-flight guard (MKT-113) and may run while a fetch task or a scheduled fetch (SPF-023) runs. A date a fetch priced before the backfill read the recorded dates keeps the fetch's price (MKT-192); a fetch that writes the date afterwards overwrites the backfilled close (MKT-025).

### Price Freshness (200–209)

The header tells how recent the portfolio's prices are. Prices travel with the sync, so the date of the newest price is meaningful on every device; when a given device last fetched prices is not, and is kept apart.

**MKT-200 — Newest price date (backend)**: The application computes the latest observation date among the prices recorded for the assets currently held (`quantity > 0`) in any account, whatever each price's source. It reflects prices fetched on any device, once a sync has applied them here. With no held asset, or no price recorded for any of them, there is none.

**MKT-201 — Last fetch on this device (backend)**: The application keeps, as device-local data that is never synced (SYN-023), the moment this device last fetched prices: the end of an in-app fetch task (MKT-122, MKT-130, MKT-132) that priced at least one asset (MKT-119 `ok > 0`), or the execution of a scheduled download (SPF-050) that updated at least one asset, whichever is later. A task or a run that priced nothing — nothing in scope, provider unreachable, every asset skipped — leaves the moment unchanged. An in-app task records the moment before it announces its completion (MKT-119). A device that ran scheduled downloads before this record existed starts from its last successful one; a device that has never fetched has no such moment.

**MKT-202 — Header display (frontend)**: The header shows, on every view and whether or not sync is enabled, "Prices as of" the newest price date (MKT-200) with its own icon; the item's tooltip tells when this device last fetched prices (MKT-201), or that it never did. With no newest price date the item says that no price is recorded yet. The item is absent until its figures have been read, and stays absent when the read fails.

**MKT-203 — Reactivity (frontend)**: The header item reads its two figures when the application starts, and again after a fetch task completes (`AssetPriceFetchCompleted`), after a price is recorded outside a fetch task (`AssetPriceUpdated`, ignored while a fetch task runs — MKT-181), after a transaction changes what is held (`TransactionUpdated`), and after a sync applied changes (`SyncCompleted`). A scheduled download runs outside the application and raises no event in it: when it runs while the application is open, the item shows its result at the next of these events or at the next start.

---

## Workflow

```
Account Details (active holding row)
    → "Enter price" button
    → PriceModal opens (no extra fetch — uses HoldingDetail data)
        date = today (editable)
        price = current_price if current_price_date == today, else empty
        → user enters price, adjusts date if needed
        → submit
            backend: validate price > 0 and date ≤ today
            backend: upsert AssetPrice(asset_id, date, price)
            backend: publish AssetPriceUpdated (bare signal)
        → modal closes + snackbar
        → Account Details re-fetches on AssetPriceUpdated
        → holding row: current price, unrealized P&L, performance % updated
```

### Workflow — Price history CRUD (MKT-070+)

```
Account Details (active holding row)
    → "Price history" button (MKT-070)
    → PriceHistoryView opens; list fetched via MKT-072
        → sorted date descending; Edit + Delete per row
        → "Add price" action → same flow as "Enter price" (MKT-020–029)

Edit row:
    → Edit form opens pre-filled with (date, price)
    → user changes date and/or price
    → submit
        same date → in-place update (MKT-083)
        new date  → delete old + upsert at new date (MKT-084)
        on failure → rollback, form stays open (MKT-087)
    → success: form closes + snackbar + list refreshes + AssetPriceUpdated (MKT-085, MKT-086)
    → Account Details re-fetches on AssetPriceUpdated (MKT-076)

Delete row:
    → confirmation dialog identifies (date, price) (MKT-089)
    → user confirms
        backend: delete_asset_price(asset_id, date) (MKT-090)
        backend: publish AssetPriceUpdated (MKT-091)
    → list refreshes; Account Details re-fetches (MKT-076)
    → if deleted entry was most recent price: holding row shows "—" or next price
```

### Workflow — Auto-record from a buy/sell transaction (MKT-050+)

```
Settings page
    → "Automatically record transaction price as market price" toggle
    → choice persisted to localStorage (default OFF)

Buy/Sell transaction form (create or edit)
    → checkbox "Use this price as the market price for {date}"
        create mode → default = global toggle snapshot at open (MKT-052)
        edit mode   → default = OFF (MKT-052)
    → user submits
        frontend: record_price: bool added to CreateTransactionDTO (MKT-054)
        backend (atomic DB transaction):
            ├─ insert/update Transaction
            └─ recompute Holding (TRX-027 / SEL-025)
        → commit; on any failure rollback the transaction DB operation
            transaction context publishes TransactionUpdated              (B8)
        → if record_price && tx.unit_price > 0 (MKT-055, MKT-061):
            frontend calls record_asset_price(asset_id, date, price)     (separate command, best-effort)
            on success: asset context publishes AssetPriceUpdated         (MKT-057, B8)
            on failure: logged as warning, silently dropped               (MKT-056, MKT-062)
        → Account Details re-fetches via AssetPriceUpdated                (MKT-036)
```

### Workflow — Auto-fetch on launch and on user demand (MKT-100+)

```
App launch
    → frontend completes initial mount
    → frontend reads auto-fetch setting from FE store                      (MKT-120)
        if OFF (default): no launch call; user can still trigger refresh
        if ON: frontend calls auto-fetch task once per session             (MKT-121, fire-and-forget)
    → backend (sync part):
        ├─ if a fetch task is already running: reject (MKT-113)
        ├─ load scope (active/derivable holdings)
        ├─ if scope is empty: reject (MKT-111)
        └─ dispatch background job, return                                 (MKT-122)
    → background job:
        for each active holding asset:
            ├─ derive provider symbol from Asset.reference                (MKT-110, ADR-017)
            ├─ if symbol unmappable OR provider fetch fails: skip silently (MKT-114, logged warning)
            └─ on success: upsert (asset_id, date, price, source=YahooFinance) (MKT-025, MKT-102, MKT-125)
                          publish AssetPriceUpdated                       (MKT-112)
    → subscribers re-fetch on AssetPriceUpdated                            (MKT-036)

User clicks "Refresh prices" on the global dashboard
    → frontend calls global refresh                                        (MKT-130, fire-and-forget)
    → backend (sync part — same entry point as launch MKT-122):
        ├─ if a fetch task is already running: reject (MKT-113)
        ├─ load scope (active/derivable holdings across all accounts)
        ├─ if scope is empty: reject (MKT-111)
        └─ dispatch background job, return
    → background job: same per-asset behavior as launch                    (MKT-110, MKT-114, MKT-112, MKT-116)

User clicks "Refresh prices" on an account detail page
    → frontend calls account refresh with account_id                       (MKT-131, fire-and-forget)
    → backend (sync part):
        ├─ if account_id is unknown: reject (MKT-132)
        ├─ if a fetch task is already running: reject (MKT-113)
        ├─ load scope (active/derivable holdings for this account)
        ├─ if scope is empty: reject (MKT-111)
        └─ dispatch background job, return                                 (MKT-132)
    → background job: same per-asset behavior as launch                    (MKT-110, MKT-114, MKT-112, MKT-116)

In-flight guard (all fetch paths)
    → if any fetch task (launch, global, account) is already running,
      a subsequent task call is rejected with a specific error            (MKT-113)

User-triggered refresh feedback (FE)
    → button disabled + spinner while awaiting BE ack                     (MKT-133)
    → snackbar on dispatch success ("Fetching prices…")                   (MKT-115)
    → snackbar on in-flight rejection ("Fetch already in progress")       (MKT-115)
    → snackbar on no-fetchable-holdings rejection ("No holdings to fetch") (MKT-115)
```

### Workflow — Price refresh lock (MKT-150+)

```
Account Details (active, non-cash holding row)
    → lock / unlock icon button (MKT-153), icon reflects price_refresh_blocked
    → user clicks to lock
        backend: set price_refresh_blocked = true for asset_id          (MKT-156)
                 reject if asset unknown (MKT-156) or system cash (MKT-154)
                 publish AssetUpdated                                    (MKT-156)
    → frontend re-reads assets; row icon flips to "locked"; snackbar     (MKT-157)

Next refresh (global MKT-130 / account MKT-131)
    → build scope across active, derivable, non-cash holdings
        └─ skip every asset with price_refresh_blocked = true           (MKT-151)
    → locked asset is not fetched; its recorded price is preserved       (MKT-158)
        (staleness label keeps aging; source badge unchanged)

User unlocks (same toggle)
    → backend: set price_refresh_blocked = false                         (MKT-156)
    → asset re-enters fetch scope on the next refresh                    (MKT-151)
```

### Workflow — Manual fill of unupdated prices (MKT-170+)

```
Any fetch task finishes (launch MKT-122 / global MKT-130 / account MKT-132)
    → backend publishes completion signal with counts + unpriced list    (MKT-119, MKT-170)
        list = in-scope assets the fetch could not price                 (MKT-171, MKT-114)
    → frontend receives the signal (handled globally)
        if unpriced list is empty: nothing (silent success)              (MKT-172)
        if unpriced list is non-empty:
            → unupdated-prices modal auto-opens                          (MKT-172)
            → MKT-145 partial/failure snackbar suppressed for this signal (MKT-173)

Unupdated-prices modal (one row per unpriced asset)
    each row: asset name | last-known value (or "no previous price") |
              ticker | ISIN | empty price input + currency label         (MKT-174)
    → user enters a value and confirms a row
        frontend: record_asset_price(asset_id, today, price)             (MKT-175, MKT-020+)
            source = Manual; upsert by (asset_id, today)                 (MKT-101, MKT-025)
        on success: row leaves the list; AssetPriceUpdated published     (MKT-178, MKT-179)
        on failure: row stays with an inline error for retry             (MKT-178)
    → user skips a row
        nothing recorded; row leaves the list                           (MKT-176)
    → all rows resolved → modal closes                                  (MKT-177)
    → user dismisses modal → remaining rows treated as skipped          (MKT-177)
    → Account Details / dashboard re-fetch on AssetPriceUpdated          (MKT-179, MKT-036)
```

### Workflow — Price history backfill (MKT-190+)

```
Account Details (active non-cash holding row, or closed holding row)
    → "Fill missing price history" button                                (MKT-190)
    → that row's action disabled, pending state                          (MKT-198)
    → backend: backfill_holding_price_history(account_id, asset_id)
        reject: unknown account / asset, system cash, archived,
                refresh-locked, never held, no derivable symbol          (MKT-196)
        held period = earliest transaction on the asset
                      → yesterday (active) | latest transaction ≤ yesterday (closed)  (MKT-191)
            period starts after yesterday → succeed, both counts zero    (MKT-191)
        request each window of ≤ 365 days                                (MKT-193)
            any request fails → reject, nothing recorded                 (MKT-195)
        provider reports the symbol unknown → reject as unresolvable ticker (MKT-196)
        no close over 7 days or more → reject as unresolvable ticker    (MKT-196)
        no close over a shorter period → succeed, both counts zero     (MKT-196)
        record closes on dates without a price; count the others         (MKT-192, MKT-194)
        written ≥ 1 → publish AssetPriceUpdated once                     (MKT-194)
    → snackbar: both counts / nothing to fill / rejection cause          (MKT-197)
    → Account Details re-fetches on AssetPriceUpdated                    (MKT-036)
```

---

## UX Draft

### Entry Point

"Enter price" icon button on each active holding row in Account Details, in the actions column alongside Buy and Sell. Not shown on closed holdings.

### Main Component

Small modal dialog. No navigation — stays within Account Details.

### Form Fields

| Field          | Default                                   | Editable |
| -------------- | ----------------------------------------- | -------- |
| Asset name     | Pre-filled from holding row               | No       |
| Date           | Today                                     | Yes      |
| Price          | Today's existing price if any, else empty | Yes      |
| Currency label | Asset's native currency code              | No       |

### States

- **Submit in-flight** (MKT-027): Submit button disabled + spinner while persisting.
- **Validation / backend error** (MKT-029): Inline error adjacent to the invalid field; modal stays open.
- **Success** (MKT-028): Modal closes; snackbar "Price recorded."
- **No price — missing ticker (holding row)** (MKT-032): "Missing ticker" diagnostic in Current Price; "—" in Unrealized P&L, Performance %.
- **No price — fetch unavailable (holding row)** (MKT-032): "No price available" diagnostic in Current Price; "—" in Unrealized P&L, Performance %.
- **Currency mismatch (holding row)**: Current Price shown in asset currency; Unrealized P&L and Performance % show "—".

### User Flow

1. User views Account Details for an account.
2. User clicks "Enter price" on a holding row.
3. Modal opens immediately (no fetch) with asset name, today's date, and price pre-filled if a same-day entry exists.
4. User types the current market price (in asset currency, shown as a label).
5. User optionally changes the date (e.g. to use yesterday's closing price).
6. User submits.
7. Backend validates, upserts the price, publishes `AssetPriceUpdated`.
8. Modal closes, snackbar confirms.
9. Account Details re-fetches: the holding row now shows current price, unrealized P&L, performance %.

### UX Draft — Auto-record from Transactions (MKT-050+)

#### Settings page

A new toggle row "Automatically record transaction price as market price" sits alongside the existing language preference. Default OFF. State persists across sessions on the current device.

#### Buy and sell transaction forms

A checkbox is added directly above the submit button in:

- the buy creation modal,
- the sell creation modal,
- the standalone /transactions/new form (whether the transaction is a buy or a sell),
- the edit variants of all the above.

| Field             | Default                                                 | Editable |
| ----------------- | ------------------------------------------------------- | -------- |
| Auto-record price | Snapshot of global toggle on create; always OFF on edit | Yes      |

The label updates live with the form's date field: "Use this price as the market price for 2026-04-27".

#### States

- **Checkbox unchecked**: no behaviour change; the form behaves exactly as before this feature.
- **Checkbox checked + submit success**: the standard transaction success path (snackbar, modal close, list refresh) is unchanged. The new `AssetPriceUpdated` event causes Account Details to refresh its market-price columns transparently.
- **Submit failure with checkbox checked**: the form remains open with the standard inline transaction error feedback. No price is written because the transaction did not commit. The user can untick the checkbox or correct the inputs and retry.
- **Price write failure after successful transaction**: the transaction is committed and the modal closes normally. The price write failure is silent (MKT-062); the user can record the price manually via the price history modal (MKT-070+).
- **Same-day price already recorded**: no warning shown; the existing entry is silently overwritten on submit (MKT-058).
- **Zero `unit_price`**: when the buy/sell unit price is `0` (gifted asset, TRX-020), the auto-record step is silently skipped per MKT-061. The transaction itself succeeds normally; the checkbox state has no observable effect in this case.

#### User flow — global default

1. User opens Settings.
2. User flips "Automatically record transaction price as market price" ON.
3. User opens a buy form anywhere in the app — the auto-record checkbox is pre-checked.
4. User submits; the unit price is recorded as the day's market price in addition to the transaction.

#### User flow — per-transaction override

1. Global toggle is OFF (default).
2. User opens a sell form for an asset they want to also stamp a market price for today.
3. User ticks the auto-record checkbox manually before submitting.
4. User submits; the price is recorded for this transaction only. The next form opens with the box unchecked again.

### UX Draft — Price History CRUD (MKT-070+)

#### Entry Point

"Price history" icon button on each active holding row in Account Details, in the actions column alongside Buy, Sell, and "Enter price". Not shown on closed holdings.

#### Main Component

Modal or side panel listing all recorded `AssetPrice` entries for the selected asset (per asset, not per account).

#### States

- **Loading**: spinner while prices are fetched on open (MKT-074).
- **Empty**: "No prices recorded yet" message with an "Add price" button (MKT-073).
- **Populated**: date-descending list; each row shows date, price (in asset currency), Edit button, Delete button.
- **Edit form** (inline within modal, or nested modal): date + price fields, pre-filled; same inline error feedback as "Enter price" form (MKT-087).
- **In-flight (edit)**: Submit button disabled + spinner while the edit request is in progress (MKT-094).
- **Delete confirmation** (standard ConfirmationDialog): identifies date + price of the record to be removed (MKT-089).
- **In-flight (delete)**: Delete button for the targeted row is disabled while the request is in progress (MKT-093).
- **Delete error**: error banner at the top of the list; entry remains visible (MKT-096).
- **Fetch error**: inline error with retry (MKT-074).

#### User Flow — View and delete a stale price

1. User views Account Details and notices a price in the current-price column that looks wrong.
2. User clicks "Price history" on that holding row.
3. Modal opens; list is fetched and displayed date-descending.
4. User locates the stale entry, clicks Delete.
5. Confirmation dialog appears identifying the date and price.
6. User confirms.
7. Backend deletes the record, publishes `AssetPriceUpdated`.
8. History list refreshes; Account Details re-fetches — holding row now shows the next most recent price or "—".

#### User Flow — Correct a price with a wrong date

1. User opens "Price history" for a holding.
2. User finds an entry recorded on the wrong date (e.g. recorded on 2026-04-28 but should be 2026-04-27).
3. User clicks Edit on that row.
4. Edit form opens pre-filled with the wrong date and the price.
5. User changes the date to 2026-04-27.
6. User submits.
7. Backend deletes the 2026-04-28 record and upserts at 2026-04-27 (MKT-084).
8. Edit form closes; history list refreshes; snackbar confirms (MKT-086).

### UX Draft — Auto-Fetch from External Provider (MKT-100+)

#### Settings page

The existing Settings page (host of MKT-050's transaction auto-record toggle) gains a second toggle: "Automatically fetch prices on launch" (MKT-120). Default OFF. Sits above the existing transaction-related toggle to group all price-related settings together.

#### Global dashboard — Refresh action

A "Refresh prices" button is added to the global dashboard, placed in the page header. Pressing it triggers the global refresh (MKT-130). The button is disabled with a spinner while the refresh is being acknowledged (MKT-133); on dispatch, a snackbar acknowledges the fetch (MKT-115). Per-asset failures during the fetch degrade silently per MKT-114. On in-flight collision (MKT-113), a snackbar indicates a fetch is already in progress (MKT-115); the user's action is rejected without disrupting the ongoing fetch.

#### Account Details — Refresh action

A "Refresh prices" button is added to the Account Details view, placed in the page header. Pressing it triggers the account refresh (MKT-131) scoped to that account's holdings. Same feedback and in-flight semantics as the global refresh (MKT-113, MKT-114, MKT-115, MKT-133).

#### Account Details — Current Price column secondary label and source badge

The "Current Price" column displays the price as before (MKT-030), with a secondary label below showing the staleness phrasing per MKT-140: "Updated today" or "Updated Nd ago". A small source badge (per MKT-142) sits alongside the price or near the staleness label so the user can tell auto-fetched values from manual entries without opening the price history. Subdued typography on both label and badge to avoid competing with the price value itself.

#### Price-history modal — Source badge

Each row in the price-history list (MKT-071) gains a small badge to the right of the date showing the row's `source` value: "Manual", "Yahoo Finance", etc. (MKT-141). The badge uses neutral styling — it's informational, not a status pill.

#### States

- **Initial launch with auto-fetch OFF (default)**: the dashboard renders with whatever stored prices exist; missing prices show "—". The user can hit "Refresh prices" on the global dashboard or on an account detail page to fetch on demand.
- **Initial launch with auto-fetch ON (user-enabled)**: prices appear as the background job completes; per-row updates are reactive via `AssetPriceUpdated` events. No global spinner or progress UI — the UI fills in row-by-row as data arrives.
- **Refresh button awaiting ack**: button disabled + spinner until BE acknowledges dispatch (MKT-133).
- **Refresh dispatched**: snackbar "Fetching prices…" (MKT-115); rows update reactively as the background job completes.
- **Concurrent refresh attempt**: BE returns the in-flight error (MKT-113); snackbar "Fetch already in progress" (MKT-115); ongoing fetch undisturbed.
- **Refresh on empty scope**: BE returns the no-fetchable-holdings rejection (MKT-111); snackbar "No holdings to fetch" (MKT-115).
- **Account refresh on unknown account**: BE returns a specific error (MKT-132); FE surfaces it via the standard error pipeline.
- **Asset with no provider coverage**: row's "Current Price" stays at "—" indefinitely. The user can enter a manual value via "Enter price" (MKT-010).
- **System cash holding**: excluded from fetch scope (MKT-116); no warning, no badge change.

#### User flow — first launch of the day (auto-fetch enabled by the user)

1. User opens the app.
2. Frontend mounts. Auto-fetch setting is `ON` (MKT-120), so the frontend calls the auto-fetch task (MKT-121). Backend returns immediately and dispatches the background job (MKT-122).
3. The dashboard renders immediately with stored prices (likely yesterday's for active holdings).
4. As each asset's fetch completes in the backend job, `AssetPriceUpdated` events fire; the Current Price column updates reactively; the staleness label flips to "Updated today".
5. User sees a fully refreshed dashboard within seconds (typical case: ~5–50 holdings).

#### User flow — enable auto-fetch

1. User opens Settings.
2. User toggles "Automatically fetch prices on launch" to ON.
3. Setting persists in the FE store; no immediate fetch (the launch trigger fires once per session at mount).
4. On the next launch, the auto-fetch task runs (MKT-121).
5. User can still hit "Refresh prices" (global or account) at any time to fetch on demand.

#### User flow — manual override of a fetched value

1. Auto-fetch writes `AssetPrice(AAPL, 2026-05-17) = $192, source: YahooFinance`.
2. User notices the "Yahoo Finance" badge and disagrees with the value (e.g. corporate-action edge case).
3. User opens "Enter price" or "Price history", enters $189.
4. Backend writes `AssetPrice(AAPL, 2026-05-17) = $189, source: Manual` — overwrites the fetched row (per ADR-012; MKT-025, MKT-101).
5. Account Details shows $189; the badge becomes "Manual".
6. On the next launch, auto-fetch will overwrite $189 with the new day's Yahoo Finance value (per ADR-012). The user's correction is for today; tomorrow brings tomorrow's price.

### UX Draft — Price refresh lock (MKT-150+)

#### Entry Point

A lock / unlock icon button on each active, non-cash holding row in Account Details, in the actions column alongside Buy, Sell, Enter price, and Price history (MKT-153). The icon shows the asset's current state: an open padlock when fetches are allowed, a closed padlock when locked. Not shown on the system cash row nor on closed holdings.

#### States

- **Unlocked (default)**: open-padlock icon; the asset participates in every fetch (MKT-151 does not skip it). Tooltip conveys "Block automatic price updates".
- **Locked**: closed-padlock icon; the asset is skipped by every fetch (MKT-151) and its recorded price is preserved (MKT-158). Tooltip conveys "Allow automatic price updates".
- **Toggle in-flight**: the button is briefly disabled while the command is acknowledged; on success the icon flips and a snackbar confirms (MKT-157).
- **Asset-wide scope reminder**: because the lock is per asset (not per holding), the tooltip notes that locking affects the asset everywhere it is held.

#### User flow — pin a manual correction

1. A fetch wrote `AssetPrice(DCAM, today) = 6.000, source: YahooFinance`, but the official close was `5.993`.
2. The user records `5.993` manually via "Enter price" (MKT-010) → the row now shows the manual value with a "Manual" badge.
3. The user clicks the lock icon on the holding row (MKT-153). The asset is now locked (MKT-156); a snackbar confirms (MKT-157).
4. On every subsequent refresh, the asset is skipped (MKT-151); the `5.993` value persists.
5. When the user no longer needs the pin, they click the icon again to unlock; the asset re-enters fetch scope (MKT-151).

### UX Draft — Manual fill of unupdated prices (MKT-170+)

#### Entry Point

No explicit entry point: the modal auto-opens (MKT-172) after any fetch task that left one or more assets unpriced. It is not reachable from a button — it is a reaction to the fetch outcome.

#### Main Component

A modal dialog listing the unpriced assets, one per row. Each row is a self-contained mini-form (value input + confirm + skip) so the user can fill the assets they know and skip the rest.

| Column / control | Content                                                                 | Editable |
| ---------------- | ----------------------------------------------------------------------- | -------- |
| Asset name       | Asset display name                                                      | No       |
| Last-known value | Most recent recorded price in asset currency, or "no previous price"    | No       |
| Ticker           | `reference`                                                             | No       |
| ISIN             | ISIN when present, otherwise blank                                      | No       |
| Price input      | Empty; user types the new value (currency label alongside, per MKT-023) | Yes      |
| Confirm / Skip   | Per-row confirm (records) and skip (dismisses) actions                  | —        |

#### States

- **Auto-open with unpriced assets** (MKT-172): modal appears listing every unpriced asset; the MKT-145 snackbar is suppressed (MKT-173).
- **Row in-flight** (MKT-178): the row's confirm action is disabled while its record request is in progress.
- **Row recorded** (MKT-175, MKT-178): the row shows brief confirmation and leaves the list.
- **Row error** (MKT-178): inline error adjacent to the row's input; the row stays for retry; other rows unaffected.
- **Row skipped** (MKT-176): the row leaves the list with nothing recorded.
- **All rows resolved** (MKT-177): the modal closes automatically.
- **Modal dismissed** (MKT-177): remaining rows are treated as skipped.

#### User Flow — fill some, skip the rest

1. A launch auto-fetch finishes; the provider could not price 3 of the user's holdings.
2. The unupdated-prices modal auto-opens listing those 3 assets, each with its last-known value, ticker, and ISIN.
3. The user knows today's price for two of them; they type each value and confirm the row. Each confirm records a `Manual` price dated today and the row disappears.
4. The third is an illiquid asset with no figure to hand; the user skips it. The row disappears.
5. With all rows resolved, the modal closes. Account Details and the dashboard already reflect the two new prices (reactive via `AssetPriceUpdated`).

### UX Draft — Price history backfill (MKT-190+)

#### Entry Point

A "Fill missing price history" icon button in the actions of each active, non-cash holding row, directly under Price history, and of each closed holding row, after the transactions button (MKT-190). The pinned Actions column is five buttons wide to fit it. Not shown on the cash row nor in the as-of view.

#### States

- **Idle**: the button is enabled.
- **Pending** (MKT-198): the button is disabled and pulses; the other rows stay usable.
- **Written** (MKT-197): success snackbar with both counts ("412 prices added · 38 days already had one").
- **Nothing to fill** (MKT-197): info snackbar.
- **Rejected** (MKT-195, MKT-196): error snackbar naming the cause — never held, locked, ticker not resolved, provider unreachable, or the shared messages for an unknown account or asset, the system cash asset, an archived asset and a database error.

#### User Flow

1. The user opens an account whose history shows gaps for a fund bought in 2021, before the scheduled fetch existed.
2. They click the fill button on the fund's row; it pulses while the closes download.
3. A snackbar reads "412 prices added · 38 days already had one"; the history and as-of views now value the fund on every trading day since 2021, up to yesterday.

---

## Open Questions / Deferred

**MKT-032 — disambiguating the "No price available" state.** The current rule merges two upstream causes (provider returned N/D vs no fetch has run yet under a manual-update-frequency account) into one diagnostic. Distinguishing them requires BE telemetry (per-asset last-fetch-attempt + outcome) not currently exposed on `HoldingDetail`. Defer until a real user-pain signal warrants the BE surface change.

**MKT-153 — toggling the lock from the Asset management table.** This phase exposes the lock only from the Account Details holding row, where the price discrepancy is observed. Surfacing the same toggle in the Asset management table (assets view) — so an asset can be locked without holding it in an open account — is deferred until a need surfaces; the flag and commands (MKT-150, MKT-156) already support it.

- [x] **Companion ADR for the price-refresh lock** — resolved: [ADR-014](../adr/014-price-refresh-lock-scope-exclusion.md) records the decision, fulfilling [ADR-012](../adr/012-latest-write-wins-source-as-metadata.md)'s decision-point-4 deferral. The pin is implemented as fetch-scope exclusion, leaving ADR-012's latest-write-wins write path intact.

- [x] **MKT-171 — scope of the unpriced list.** Resolved: the list includes the full MKT-114 skip set (no-data, fetch error, and symbol-underivable). List length equals the `skipped` count.
- [x] **MKT-175 — save model.** Resolved: per-row immediate record on confirm, reusing `record_asset_price`; no batch command.

None — all questions have been resolved.
