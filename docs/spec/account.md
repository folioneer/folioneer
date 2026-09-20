# Business Rules — Account Management (ACC)

## Context

An `Account` represents a financial account owned by the user (e.g. brokerage account, savings account, life insurance). Each account can hold positions on assets (`Holding`) created via transactions.

This spec covers the CRUD lifecycle of accounts. The Account Details view (drilling into holdings and cost basis) is covered by [`docs/spec/account-details.md`](account-details.md).

---

## Entity Definition

### Account

A financial account owned by the user.

| Field              | Business meaning                                                                                                       |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------- |
| `name`             | User-defined display name (e.g. "PEA Boursorama", "Livret A"). Unique among all accounts (case-insensitive).           |
| `update_frequency` | Frequency at which the user plans to update the account's data. Informational only — no automation is triggered today. |

### PortfolioTotal

The total of every account, read together with the account summaries (ACC-027). A derived value: recomputed on every read, never stored (ADR-013).

| Field                  | Business meaning                                                                                                  |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `total_global_value`   | Every account's Global Value, converted to the cross-account reference currency.                                  |
| `total_unrealized_pnl` | Every account's Unrealized P&L, converted the same way; absent when no convertible account carries one (ACC-029). |
| `currency`             | The cross-account reference currency both figures are stated in (GPF-011).                                        |
| `incomplete`           | Whether an account with no usable rate hid a non-zero figure from the total (ACC-028).                            |

---

## Business Rules

### Validation

**ACC-001 — Name normalisation (backend)** _(formerly R1)_: The `name` is normalised on receipt before any validation or storage: leading and trailing whitespace is stripped.

**ACC-002 — Account validation (backend)** _(formerly R2)_: An `Account` is valid if and only if its `name` (after ACC-001 normalisation) is non-empty, and its `update_frequency` is one of the five fixed values (`Automatic`, `ManualDay`, `ManualWeek`, `ManualMonth`, `ManualYear`). Any violation is rejected with an explicit error.

**ACC-003 — Name uniqueness (backend + frontend)** _(formerly R3)_: No two accounts may share the same name (comparison is performed on the normalised name per ACC-001, case-insensitive). Any create or edit operation resulting in a duplicate is rejected by the backend with an explicit error. ACC-001, ACC-002, and ACC-003 apply to both creation and modification.

**ACC-004 — UpdateFrequency fixed list (backend + frontend)** _(formerly R4)_: The update frequency is a fixed list of five non-customisable values. It is purely informational: no automation is triggered in the current version.

> Note: active scheduling behaviour is planned for a future release.

### Deletion

**ACC-005 — Cascade delete holdings (backend)** _(formerly R5)_: Deleting an `Account` is permanent and irreversible. All associated `Holding` records are deleted in cascade. The `Asset` records themselves are not affected.

**ACC-006 — Cascade delete transactions (backend)** _(formerly R6)_: Deleting an `Account` also deletes all transactions associated with that account. The `Asset` records referenced by those transactions are not affected.

> ⚠️ **Dependency**: ACC-006 requires the Transaction feature to be available ([`docs/spec/financial-asset-transaction.md`](financial-asset-transaction.md)). Until that table exists in the database, only the cascade on `Holding` (ACC-005) is active.

### Display

**ACC-007 — Default sort (frontend)** _(formerly R8 — sort behaviour)_: The account table is sorted by name ascending by default.

**ACC-008 — Table columns (frontend)** _(formerly R8 — column definition)_: The account table exposes the following columns:

| Column          | Content                                                                                                                 | Sortable                                                                                              |
| --------------- | ----------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| Name            | `account.name`                                                                                                          | Yes                                                                                                   |
| Bank            | Bank or broker name (ACC-026); "—" when unset                                                                           | Yes — unset sorts last in both directions                                                             |
| Frequency       | Human-readable label for `update_frequency`                                                                             | Yes — sorted on the logical enum order: Automatic → ManualDay → ManualWeek → ManualMonth → ManualYear |
| Global Value    | `total_global_value` formatted in the account's own currency (ACC-021); 0 shown for empty accounts                      | Yes                                                                                                   |
| Unrealized P&L  | `total_unrealized_pnl` formatted in the account's own currency (ACC-023); "—" when no holding has a computable value    | Yes — null values sort last                                                                           |
| YTD Performance | Year-to-date net-of-flows performance since Jan 1 as a percentage (ACC-024); "—" when the year-start baseline is absent | Yes — null values sort last                                                                           |
| Actions         | Edit button + Delete button                                                                                             | No                                                                                                    |

The account section title is displayed in the application shell header, not in the page body. A real-time search field (partial match, case-insensitive) filters the table by name. The table closes with a portfolio total row (ACC-030 to ACC-032).

**ACC-009 — Sort toggle (frontend)** _(formerly R9)_: Clicking a sortable column header toggles between ascending and descending order. A visual indicator on the header reflects the active sort direction.

### Navigation

**ACC-010 — Row navigation (frontend)**: The entire account table row is clickable and navigates to the Account Details view (`/accounts/:id`). Action buttons (Edit, Delete) use `stopPropagation` and do not trigger row navigation. A `ChevronRight` icon appears on hover as a visual affordance indicating the row is navigable. The row is keyboard-accessible: Tab to focus, Enter or Space to navigate.

### Empty and Error States

**ACC-011 — No search results (frontend)** _(formerly R10)_: If the search matches no accounts, the table displays a message distinct from the empty state. The message does not invite the user to create an account.

**ACC-012 — Empty state (frontend)** _(formerly R11)_: If no `Account` exists, the table displays an explicit empty state inviting the user to create their first account via the FAB.

**ACC-013 — Loading and error states (frontend)** _(formerly R12)_: The account table exposes a loading state while any read is in progress, the first one and every re-read (ACC-033), and an error state with a Retry button when a read fails.

**ACC-014 — Inline backend errors (frontend)** _(formerly R13)_: The create modal and the edit modal remain open on backend error and only signal success (close + refresh) after a positive response. Any failure (duplicate, network error, backend error) displays an inline error message inside the active modal or dialog.

### Mutations

**ACC-015 — Default UpdateFrequency (frontend)** _(formerly R7)_: The create form pre-selects `ManualMonth` as the default frequency.

**ACC-016 — Create via FAB (frontend)** _(formerly R14)_: A floating action button (bottom-right) opens a create modal with a Name field (required) and an `UpdateFrequency` selector. Submission is blocked if the name is empty or whitespace-only. After successful creation the modal closes and the table refreshes. On backend error, an inline error message is shown per ACC-014.

**ACC-017 — Edit (frontend)** _(formerly R15)_: The Edit button opens a modal with the Name and `UpdateFrequency` fields pre-filled. After saving, the modal closes and the table refreshes. On backend error, an inline error message is shown per ACC-014.

**ACC-018 — Delete empty account (frontend)** _(formerly R16)_: If the `Account` has no `Holding`, the Delete button opens a standard confirmation dialog. Confirmation triggers deletion (ACC-005, ACC-006); after deletion the dialog closes and the table refreshes.

**ACC-019 — Delete non-empty account (frontend)** _(formerly R17)_: If the `Account` has at least one `Holding`, the Delete button opens a reinforced confirmation dialog stating the number of holdings and transactions that will be permanently deleted. Counts are retrieved via the backend command defined in ACC-020. Confirmation triggers deletion and cascade (ACC-005, ACC-006); after deletion the dialog closes and the table refreshes.

> ⚠️ **Dependency**: ACC-019 depends on ACC-020 (pre-deletion count command), which crosses the `account/` and `transaction/` bounded contexts and requires a dedicated use case. Until ACC-020 is implemented, ACC-018 applies for all deletions (standard dialog without counts).

**ACC-020 — Pre-deletion count query (backend)** _(new)_: A backend command `get_account_deletion_summary(account_id)` returns the number of active holdings and the number of transactions associated with the account. Because this read spans `context/account/` (holdings) and `context/transaction/` (transactions), it must be implemented as a use case in `use_cases/` — not as a bounded-context command — per ADR-003 and ADR-004.

**ACC-021 — Account summaries with global value (backend + frontend)** _(new)_: A backend command `get_account_summaries()` returns a list of `AccountSummary` records — each `Account` enriched with its `total_global_value` (micros, computed in the account's own currency per CSH-094: `cash_holding.quantity + Σ_h (h.quantity × latest_price(h))` over same-currency priced active non-cash holdings; unpriced non-cash holdings, and foreign ones with no usable rate (FXR-041), contribute 0; empty accounts return 0). This read spans `context/account/` and `context/asset/`, so it lives as a use case in `use_cases/` per ADR-003 and ADR-004. The Accounts table (ACC-008) renders the value in a Global Value column formatted in the account's own currency.

**ACC-023 — Account-wide unrealized P&L on the summary (backend + frontend)** _(new)_: `AccountSummary` carries `total_unrealized_pnl` — the account-wide unrealized gain/loss in the account's own currency (micros), computed identically to `AccountDetailsResponse.total_unrealized_pnl` (MKT-040 as amended by FXR-040): the sum of per-holding unrealized P&L over active non-cash holdings for which the value is computable (same-currency, or foreign with a usable rate, and a recorded price), with the others excluded. It is `None` when no holding qualifies (empty account, or no priced/valuable holding). The Accounts table renders it in an "Unrealized P&L" column (ACC-008), formatted in the account's own currency, showing "—" when `None`.

**ACC-024 — Year-to-date performance on the summary (backend + frontend)** _(new)_: `AccountSummary` carries the account's year-to-date performance — the net-of-flows return spanning from the start of the current calendar year to the read date ("today"), expressed as a percentage (micro-percent). It is computed fresh on every read (no persistence, ADR-013) reusing the Simple-Dietz definition of the Account Performance feature (PRF-034): the start value is the account's Global Value at the prior 31 December, or inception (0) when the account began during the current year (a first-calendar-year account therefore still has a present YTD, consistent with PRF-034 — not `None`). The value is `None` only when the Simple-Dietz denominator is zero (PRF-032) — e.g. an account with no starting value and no net external flows in the year. The Accounts table renders it in a "YTD Performance" column (ACC-008), showing "—" when `None`.

> **Dependency**: ACC-023/ACC-024 build on the market-price valuation (MKT-040/FXR-040) and the Account Performance engine (PRF-034) respectively; both are implemented in code (`use_cases/account_summary/`, `use_cases/account_performance/`). ACC-024 runs a per-account performance recompute on the all-accounts list — the N-account read cost that ADR-003/ADR-013 flag for list views. This is accepted at the expected scale (a handful of accounts, modest transaction history); revisit with memoization or a bounded single-span valuation if account or transaction volume grows materially.

**ACC-025 — Account creation seeds the cash line (backend)** _(new)_: Creating an account also seeds its cash position, so every account has a Cash Holding from the outset (CSH-010 / CSH-012). Because this spans `context/account/` (the new account and its holding) and `context/asset/` (the per-currency Cash Asset), account creation is orchestrated by `use_cases/account_creation/`: it (a) idempotently ensures the Cash Asset and Cash category for the account's currency via the shared `ensure_cash_asset` helper (ADR-003 / ADR-004 — services injected, no cross-context import), then (b) creates the account together with a 0-balance Cash Holding within a single Unit of Work (ADR-006). The `add_account` command moves to `use_cases/account_creation/api.rs`, keeping its existing name, arguments, and return type (`Account`) — no frontend change. Accounts created before this revision are backfilled with a 0-balance Cash Holding by a one-off data migration (CSH-012). The cash holding is removed with its account on delete (ACC-005 cascade; CSH-013).

**ACC-026 — Bank name metadata (backend + frontend)** _(new)_: An `Account` MAY carry a bank name — the brand name of the bank or broker holding the account. It is free text with no validation beyond trimming; an empty string means unset. The bank name is provided on creation and editable on update (threaded through `add_account` / `update_account`), and the Accounts table displays it.

### Portfolio total

**ACC-027 — The account summaries carry a portfolio total (backend + frontend)**: The account summaries also carry a portfolio total: the sum of every account's Global Value and the sum of every account's Unrealized P&L, each converted from the account's currency to the cross-account reference currency (GPF-011) with the latest rate on or before the read date (FXR-035), or a rate of 1 when the account is already in that currency (FXR-011), together with the currency they are stated in. The application computes the total; the account table never computes any part of it.

**ACC-028 — An unconvertible account leaves the total incomplete when it hides a figure (backend)**: An account whose currency has no usable rate to the cross-account reference currency on the read date (FXR-034) contributes zero to both sums. The total is marked incomplete when that account holds a non-zero Global Value or a non-zero Unrealized P&L; an account that holds neither leaves it complete. The account's own summary is unaffected. Only the conversion of an account to the cross-account reference currency counts here: a holding with no usable rate to its account's currency already counts as zero within that account's Global Value (FXR-041) and does not mark the total incomplete. This differs on purpose from the price movement report, whose total is incomplete for any entry not read at its current price (PMV-032, PMV-043): the report judges one refresh's readings, while this total is a live valuation whose only gap is converting a whole account, each row already showing its own Global Value.

**ACC-029 — The portfolio Unrealized P&L can be absent (backend)**: The portfolio Unrealized P&L is absent when no account that can be converted carries an Unrealized P&L; the total is then incomplete when ACC-028's condition holds.

**ACC-030 — What the total row shows (frontend)**: The account table closes with a total row. Its label, "Total", spans the name, bank and frequency columns. It states the portfolio Global Value and the portfolio Unrealized P&L, both in the cross-account reference currency, which it names; the Unrealized P&L is presented as a gain when positive, a loss when negative, neither when zero, and "—" when absent (ACC-029). A "partial" marker follows the label when the total is incomplete (ACC-028). The year-to-date and actions cells stay blank.

**ACC-031 — Where the total row sits (frontend)**: The total row stays below every account whatever the sort column and direction (ACC-009). It is not an account row: it cannot be focused, clicked, or used to open Account Details (ACC-010).

**ACC-032 — When the total row is shown (frontend)**: The total row is shown only when every account is listed: it is hidden while a search filter is active (ACC-011), and absent while any read, first or re-read (ACC-013, ACC-033), is in progress, when a read fails (ACC-013), or when no account exists (ACC-012).

**ACC-033 — The account table follows rate changes (frontend)**: The account table re-reads its summaries when a currency rate is recorded, edited or removed, or a currency pair is declared, applied or removed, so the converted figures, the portfolio total and its incomplete marker reflect the rates in force.

### Events

**ACC-022 — AccountUpdated event (backend)** _(new)_: After any successful account mutation (create per ACC-001/002/003, update per ACC-001/002/003, delete per ACC-005/006), the `AccountService` (owned by the `account/` bounded context) publishes an `AccountUpdated` event on the event bus. The service owns the event publication (B8 compliance). This event is distinct from `TransactionUpdated`: `AccountUpdated` signals structural account changes; `TransactionUpdated` signals position-data changes.

---

## User Workflow

```
[User opens "Accounts"]
  → Table (default sort: Name asc) + FAB
          │
          ├─ [Search] → Real-time filter by name (ACC-008)
          │              → No match → distinct message (ACC-011)
          │
          ├─ [Row click] → Navigate to Account Details (ACC-010)
          │
          ├─ [Column header] → Toggle asc/desc sort + visual indicator (ACC-009)
          │
          ├─ [FAB] → Create modal (Name + Frequency, default ManualMonth)
          │            → Inline error if name empty or duplicate (ACC-014)
          │            → Account created → modal closed → table refreshed
          │
          ├─ [Edit] → Edit modal (Name + Frequency pre-filled)
          │            → Inline error if duplicate (ACC-014)
          │            → Saved → modal closed → table refreshed
          │
          └─ [Delete]
              ├─ Empty account → Standard dialog (ACC-018) → Confirm → Delete (ACC-005, ACC-006)
              └─ Non-empty account → Reinforced dialog (ACC-019, holding + tx count)
                                     → Confirm → Delete + cascade (ACC-005, ACC-006)
```

---

## UX Draft

### Entry point

**Accounts** — item in the main navigation drawer.

### Main component

Full-width table, default sort by Name ascending. Floating action button (bottom-right). Each row is fully clickable (navigates to Account Details, ACC-010) with a `ChevronRight` hover affordance. Edit and Delete buttons per row (stop propagation). The table closes with a portfolio total row in the cross-account reference currency (ACC-030).

### States

- **Empty**: message inviting the user to create their first account via the FAB (ACC-012)
- **Loading**: loading indicator in the table (ACC-013)
- **Load error**: error message + Retry button (ACC-013)
- **No search results**: message distinct from empty state (ACC-011)
- **Filtering**: the portfolio total row is hidden while a search filter is active (ACC-032)
- **Total incomplete**: "partial" beside the total's label when an unconvertible account hides a figure (ACC-028, ACC-030)
- **Inline error**: message displayed inside the active modal or dialog (ACC-014)
- **Delete empty account confirmation**: standard confirmation dialog (ACC-018)
- **Delete non-empty account confirmation**: reinforced dialog — "This account contains X holding(s) and Y transaction(s). All data will be permanently deleted." (ACC-019)
- **Mutation success**: silent today (modal/dialog closes, table refreshes); snackbar feedback planned (see Future Improvements)

### User flow

1. User opens the Accounts page (section title shown in the shell header).
2. User clicks a row → navigates to Account Details for the selected account (ACC-010).
3. User clicks FAB → modal → enters name and selects frequency → submits → account created.
4. User clicks Edit → pre-filled modal → modifies → saves → modal closes.
5. User clicks Delete (empty account) → standard dialog → confirms → account deleted.
6. User clicks Delete (account with positions) → reinforced dialog → confirms → account + positions + transactions deleted.

---

## Future Improvements

- **Archiving**: Implement account archiving (soft-delete) instead of permanent deletion to preserve transaction history.
- **Success feedback**: Implement snackbar notifications for all mutations (ACC-014 currently only covers error feedback).

## Cross-amendments

- **SYN-040** — the accounts list marks an account that contains an inconsistent holding (see `multi-device-sync.md`).
- **CFR-035** — after a merge two accounts may carry the same name; ACC-003's uniqueness check binds the name being set by the user, not names that already clash (see `sync-conflict-resolution.md`).

## Open Questions

None — all questions have been resolved.
