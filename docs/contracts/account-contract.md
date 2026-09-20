# Contract — Account

> Domain: `account`
> Last updated by: `account` spec, `financial-asset-transaction` spec, `sell-transaction` spec, `transaction-list` spec, `account-details` spec, `cash-tracking` spec, `cash-dividend` spec, `free-share-distribution` spec, `management-fee-deduction` spec

> **Error model on the wire**: each command's error serializes as a flat `{ code: "VariantName", ...payload }` object. The FE matches on `code`. Per-command reachable codes are listed in the "Errors" column of each table below. Infrastructure failures surface as `{ code: "DatabaseError" }` (no payload; diagnostic chain preserved server-side via `tracing::error!`).
>
> Rust-internal type organization (per-BC enums, use-case composites, serde tagging) is out of scope for this contract — it documents the BE↔FE frontier, not Rust internals.

---

## Commands

### Account CRUD

| Command                        | Args                                                                                                 | Return                   | Errors                                                                                                                                                                                                                                       |
| ------------------------------ | ---------------------------------------------------------------------------------------------------- | ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `get_accounts`                 | —                                                                                                    | `Vec<Account>`           | `DatabaseError`                                                                                                                                                                                                                              |
| `add_account`                  | `CreateAccountDTO { name: String, currency: String, update_frequency: UpdateFrequency }`             | `Account`                | `NameEmpty` (ACC-002), `NameAlreadyExists` (ACC-003 — binds the name being set, CFR-035), `InvalidCurrency { currency }` (TRX-021), `DatabaseError`                                                                                          |
| `update_account`               | `UpdateAccountDTO { id: String, name: String, currency: String, update_frequency: UpdateFrequency }` | `Account`                | `NameEmpty` (ACC-002), `NameAlreadyExists` (ACC-003 — binds the name being set; editing other fields of an account whose name already clashes after a merge is accepted, CFR-035), `InvalidCurrency { currency }` (TRX-021), `DatabaseError` |
| `delete_account`               | `id: String`                                                                                         | `()`                     | `DatabaseError` _(ACC-005, ACC-006 — plain DELETE, silent on missing row)_                                                                                                                                                                   |
| `get_account_deletion_summary` | `account_id: String`                                                                                 | `AccountDeletionSummary` | `DatabaseError` _(read-only; counts are 0 if account has no data — no NotFound raised)_                                                                                                                                                      |

### Account Details

> `get_account_details` is implemented in `use_cases/account_details/` — it reads from both the
> account and asset BCs but mutates neither; owned here as the account aggregate is the primary subject.

| Command               | Args                                                                                                                                | Return                   | Errors                                                                                                                            |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | ------------------------ | --------------------------------------------------------------------------------------------------------------------------------- |
| `get_account_details` | `account_id: String, as_of_date: Option<String>` — `None` = live view (today); `Some("YYYY-MM-DD")` = read-only past reconstruction | `AccountDetailsResponse` | `AccountNotFound { account_id }` (ACD-012), `DatabaseError` (ACD-038); price lookup failures silently degrade to `None` (MKT-031) |

### Account Summaries

> `get_account_summaries` is implemented in `use_cases/account_summary/` — it reads from both the
> account and asset BCs (price lookups for each account's holdings) but mutates neither.

| Command                 | Args | Return             | Errors                                                                              |
| ----------------------- | ---- | ------------------ | ----------------------------------------------------------------------------------- |
| `get_account_summaries` | —    | `AccountSummaries` | `DatabaseError`; price lookup failures silently contribute 0 to the value (MKT-031) |

### Account Performance

> `get_account_performance` is implemented in `use_cases/account_performance/` — it reads from
> both the account BC (transaction replay → as-of-date holdings + cash) and the asset BC (price
> history) but mutates neither; owned here as the account aggregate is the primary subject. Period
> values and metrics are recomputed on read per ADR-013; nothing is persisted.

| Command                   | Args                                                                                                        | Return                       | Errors                                                                                                                       |
| ------------------------- | ----------------------------------------------------------------------------------------------------------- | ---------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `get_account_performance` | `account_id: String, asset_id: Option<String>` — `Some` scopes the series to one asset's position (PRF-080) | `AccountPerformanceResponse` | `AccountNotFound { account_id }` (PRF-016), `DatabaseError` (PRF-027); price-lookup failures silently contribute 0 (PRF-022) |

### Holdings & Transactions

> Read paths (`get_asset_ids_for_account`, `get_transactions`) and mutation paths (`buy_holding`,
> `sell_holding`, `correct_transaction`, `cancel_transaction`, `open_holding`) live behind a
> single FE-visible surface. Mutation commands coordinate across the account and asset BCs
> (cash-asset seeding, archived-asset guards, etc.).

| Command                      | Args                                                 | Return             | Errors                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ---------------------------- | ---------------------------------------------------- | ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `get_asset_ids_for_account`  | `account_id: String`                                 | `Vec<String>`      | `DatabaseError` (TXL-054) — returns empty list for unknown or empty account, never NotFound (TXL-013)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `get_transactions`           | `account_id: String, asset_id: String`               | `Vec<Transaction>` | `DatabaseError` (TXL-020)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `get_holding_snapshot_as_of` | `account_id: String, asset_id: String, date: String` | `HoldingSnapshot`  | `InvalidDate` (TDI-012), `DatabaseError` — unknown account/asset yields an empty snapshot `{ quantity: 0, average_price: 0 }`, never NotFound (TDI-010)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `buy_holding`                | `BuyHoldingDTO`                                      | `Transaction`      | `AccountNotFound { account_id }` (TRX-020), `InvalidDate` (TRX-020), `DateInFuture` (TRX-020), `DateTooOld` (TRX-020), `QuantityNotPositive` (TRX-020), `UnitPriceNegative` (TRX-020), `ExchangeRateNotPositive` (TRX-020), `FeesNegative` (TRX-020), `TotalAmountNotPositive` (TRX-020), `InsufficientCash { current_balance_micros, currency }` (CSH-041), `DatabaseError`                                                                                                                                                                                                                                                                                            |
| `sell_holding`               | `SellHoldingDTO`                                     | `Transaction`      | `AccountNotFound { account_id }` (TRX-020), `InvalidDate` (TRX-020), `DateInFuture` (TRX-020), `DateTooOld` (TRX-020), `QuantityNotPositive` (TRX-020), `UnitPriceNegative` (TRX-020), `ExchangeRateNotPositive` (TRX-020), `FeesNegative` (SEL-020), `TotalAmountNotPositive` (TRX-020), `ClosedPosition` (SEL-012), `Oversell { available, requested }` (SEL-021), `DatabaseError`                                                                                                                                                                                                                                                                                    |
| `correct_transaction`        | `CorrectTransactionDTO`                              | `Transaction`      | `TransactionNotFound` (TRX-031), `AccountNotFound { account_id }` (TRX-031), `InvalidDate` (TRX-033), `DateInFuture` (TRX-033), `DateTooOld` (TRX-033), `QuantityNotPositive` (TRX-033), `UnitPriceNegative` (TRX-033), `ExchangeRateNotPositive` (TRX-033), `FeesNegative` (TRX-033), `TotalAmountNotPositive` (TRX-033), `CascadingOversell` (SEL-032 / FSD-040 — shrinking a free-share distribution can leave a later sell oversold on replay; FEE-063 — increasing a management-fee removal likewise), `InsufficientCash { current_balance_micros, currency }` (CSH-042 / CSH-051 / DIV-040 — dividend edit re-applies the cash credit on replay), `DatabaseError` |
| `cancel_transaction`         | `CancelTransactionDTO`                               | `()`               | `TransactionNotFound` (TRX-034), `AccountNotFound { account_id }` (TRX-034), `CascadingOversell` (SEL-033 / FSD-041 — replay after cancel can leave a later sell oversold, incl. removing a free-share distribution), `InsufficientCash { current_balance_micros, currency }` (CSH-024 / CSH-051 / DIV-041 — deleting a dividend removes a cash credit, which can underflow a later debit on replay), `DatabaseError`                                                                                                                                                                                                                                                   |
| `open_holding`               | `OpenHoldingDTO`                                     | `Transaction`      | `AccountNotFound { account_id }` (TRX-056), `AssetNotFound` (TRX-056), `ArchivedAsset` (TRX-050), `OpeningBalanceOnCashAsset` (CSH-061), `QuantityNotPositive` (TRX-044), `InvalidTotalCost` (TRX-045), `InvalidDate` (TRX-046), `DateInFuture` (TRX-046), `DateTooOld` (TRX-046), `DatabaseError`                                                                                                                                                                                                                                                                                                                                                                      |

### Cash Transactions

> Both commands record cash-only movements (no asset selector, no unit price, no exchange rate)
> and return the persisted `Transaction` so the frontend can mirror the buy/sell flow's success
> path. Edit / delete of Deposit and Withdrawal reuse `correct_transaction` and
> `cancel_transaction` (CSH-023 / CSH-033 / CSH-024 / CSH-034) — those commands accept any
> `transaction_type` and run the chronological replay across all cash-affecting transactions for
> the account.

| Command             | Args                                                                                           | Return        | Errors                                                                                                                                                                                                                                   |
| ------------------- | ---------------------------------------------------------------------------------------------- | ------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `record_deposit`    | `DepositDTO { account_id: String, date: String, amount_micros: i64, note: Option<String> }`    | `Transaction` | `AccountNotFound { account_id }` (CSH-021), `AmountNotPositive` (CSH-021), `InvalidDate` (CSH-021), `DateInFuture` (CSH-021), `DateTooOld` (CSH-021), `DatabaseError`                                                                    |
| `record_withdrawal` | `WithdrawalDTO { account_id: String, date: String, amount_micros: i64, note: Option<String> }` | `Transaction` | `AccountNotFound { account_id }` (CSH-031), `AmountNotPositive` (CSH-031), `InvalidDate` (CSH-031), `DateInFuture` (CSH-031), `DateTooOld` (CSH-031), `InsufficientCash { current_balance_micros, currency }` (CSH-080), `DatabaseError` |

### Dividend

> `record_dividend` records a cash dividend attributed to the **paying asset** (`asset_id` is that
> asset, not the Cash Asset) and credits the account's Cash Holding by the account-currency total —
> mirroring how Sell proceeds re-link to cash (CSH-050/012), but leaving the paying asset's holding
> quantity and cost basis untouched (DIV-024). It only ever credits cash, so it carries **no**
> `InsufficientCash` variant. Edit / delete reuse `correct_transaction` / `cancel_transaction`
> (DIV-040/041) — a dividend **delete** can surface `InsufficientCash` on replay (removing the credit
> may underflow a later debit), via `cancel_transaction`'s existing variant.

| Command           | Args          | Return        | Errors                                                                                                                                                                                                                                                                                                                                                                     |
| ----------------- | ------------- | ------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `record_dividend` | `DividendDTO` | `Transaction` | `AccountNotFound { account_id }` (DIV-011), `AssetNotFound` (DIV-011), `AssetNotHeld` (DIV-011 — no active holding, `quantity = 0` or never held), `DividendOnCashAsset` (DIV-011 — asset is a Cash Asset), `AmountNotPositive` (DIV-021), `InvalidDate` (DIV-021), `DateInFuture` (DIV-021), `DateTooOld` (DIV-021), `ExchangeRateNotPositive` (DIV-022), `DatabaseError` |

### Free Share Distribution

> `record_free_shares` records a zero-cost quantity-only corporate event attributed to the
> **distributing asset**: the holding's `quantity` increases by the distributed amount, the cost
> basis is unchanged (average cost dilutes — FSD-022/023), and **no cash moves** (no
> `InsufficientCash` variant possible). Edit / delete reuse `correct_transaction` /
> `cancel_transaction` (FSD-040/041) — both can surface `CascadingOversell` on replay, since
> shrinking or removing the free shares can leave a later sell oversold.

| Command              | Args            | Return        | Errors                                                                                                                                                                                                                                                                                                                                    |
| -------------------- | --------------- | ------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `record_free_shares` | `FreeSharesDTO` | `Transaction` | `AccountNotFound { account_id }` (FSD-011), `AssetNotFound` (FSD-011), `AssetNotHeld` (FSD-011 — no active holding, `quantity = 0` or never held), `FreeSharesOnCashAsset` (FSD-011 — asset is a Cash Asset), `QuantityNotPositive` (FSD-021), `InvalidDate` (FSD-021), `DateInFuture` (FSD-021), `DateTooOld` (FSD-021), `DatabaseError` |

### Interest Credit

> `record_interest` records a zero-cost quantity credit attributed to the **credited asset** —
> the crediting mirror of the fee deduction, reusing the FreeShares mechanics (INT-024): the
> holding's `quantity` rises, the cost basis is unchanged (average cost dilutes), and **no cash
> moves** — except that the account's **Cash Asset is a valid target** (INT-023): crediting it
> raises the cash balance by `quantity` with no Deposit recorded. The amount is entered as
> exactly one of `percent_micros` (of the holding/balance as of the date, INT-022) or
> `quantity_micros` (INT-021). Edit / delete reuse `correct_transaction` / `cancel_transaction`
> (INT-040/041) — a correction that shrinks the credit can surface `CascadingOversell` on replay.

| Command           | Args                | Return        | Errors                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ----------------- | ------------------- | ------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `record_interest` | `RecordInterestDTO` | `Transaction` | `AccountNotFound { account_id }` (INT-011), `AssetNotFound` (INT-011), `AssetNotHeld` (INT-011 — non-cash asset with no active holding; the Cash Asset is exempt), `InterestAmountInvalid` (INT-021 — both or neither of percent/quantity), `PercentageNotPositive` / `PercentageAboveHundred` (INT-021), `QuantityNotPositive` (INT-021/022), `InvalidDate` / `DateInFuture` / `DateTooOld` (INT-021), `DatabaseError` |

### Management Fee

> `record_management_fee` records a one-off quantity-reducing fee attributed to the **charged
> asset**, entered as a percentage or as the resulting quantity — exactly one (FEE-021): the
> holding's `quantity` drops by `floor(holding_as_of(date) × percent)`, or by exactly
> `holding_as_of(date) − resulting_quantity` (FEE-028; `preview_management_fee` returns that removal
> without writing, FEE-029), the cost basis
> is unchanged (average cost concentrates — FEE-022/023), and **no cash moves**. Because it removes
> shares, it can leave a later sell oversold on replay (`CascadingOversell`, FEE-027) — the
> quantity-reducing inverse of the FSD/Sell guards. `create_fee_schedule` / `update_fee_schedule` /
> `delete_fee_schedule` / `get_fee_schedule` manage one recurring per-`(account, asset)` schedule;
> `update_fee_schedule`'s DTO omits `frequency` and `start_date`, making them structurally immutable
> (cadence change = delete + recreate, FEE-060). `apply_due_fee_deductions` is invoked by the
> frontend on app startup to materialize every due **completed** period since each schedule's cursor
> (FEE-040/044), skipping any period that would oversell (FEE-047). Edit / delete of an individual
> fee deduction reuse `correct_transaction` / `cancel_transaction` (FEE-063): a correction that
> increases the removal can surface `CascadingOversell`; a delete restores shares and is always
> replay-safe. The cumulative Management Fees figures ride on `get_account_details`
> (`HoldingDetail.management_fees`, `AccountDetailsResponse.total_management_fees`).

| Command                    | Args                                                                                 | Return                 | Errors                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| -------------------------- | ------------------------------------------------------------------------------------ | ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `record_management_fee`    | `ManagementFeeDTO`                                                                   | `Transaction`          | `AccountNotFound { account_id }` (FEE-012), `AssetNotFound` (FEE-012), `AssetNotHeld` (FEE-012), `ManagementFeeOnCashAsset` (FEE-012), `ManagementFeeAmountInvalid` (FEE-021 — both or neither of percent / resulting quantity), `PercentageNotPositive` (FEE-021), `PercentageAboveHundred` (FEE-021), `ResultingQuantityNegative` (FEE-021), `ResultingQuantityNotBelowHeld { held_quantity }` (FEE-028), `QuantityNotPositive` (FEE-022 — a percentage that floors to a zero removal), `ManagementFeesDisabled` (FEE-077), `InvalidDate` (FEE-021), `DateInFuture` (FEE-021), `DateTooOld` (FEE-021), `CascadingOversell` (FEE-027), `DatabaseError` |
| `preview_management_fee`   | `account_id: String, asset_id: String, date: String, resulting_quantity_micros: i64` | `ManagementFeeRemoval` | `AccountNotFound { account_id }` (FEE-012), `AssetNotFound` (FEE-012), `AssetNotHeld` (FEE-012), `ManagementFeeOnCashAsset` (FEE-012), `ManagementFeesDisabled` (FEE-077), `InvalidDate` / `DateInFuture` / `DateTooOld` (FEE-021), `ResultingQuantityNegative` (FEE-021), `ResultingQuantityNotBelowHeld { held_quantity }` (FEE-028), `DatabaseError` (FEE-029 — no `CascadingOversell`: the replay-integrity guard stays with the recording)                                                                                                                                                                                                         |
| `create_fee_schedule`      | `CreateFeeScheduleDTO`                                                               | `FeeSchedule`          | `AccountNotFound { account_id }` (FEE-012), `AssetNotFound` (FEE-012), `AssetNotHeld` (FEE-012), `ManagementFeeOnCashAsset` (FEE-012), `ManagementFeesDisabled` (FEE-077), `RateNotPositive` (FEE-032), `RateAboveHundred` (FEE-032), `InvalidDate` (FEE-032), `EndBeforeStart` (FEE-032), `ScheduleAlreadyExists` (FEE-031), `DatabaseError`                                                                                                                                                                                                                                                                                                           |
| `update_fee_schedule`      | `UpdateFeeScheduleDTO`                                                               | `FeeSchedule`          | `ScheduleNotFound` (FEE-060), `RateNotPositive` (FEE-032), `RateAboveHundred` (FEE-032), `InvalidDate` (FEE-032), `EndBeforeStart` (FEE-032), `DatabaseError`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `delete_fee_schedule`      | `account_id: String, asset_id: String`                                               | `()`                   | `DatabaseError` (FEE-062 — silent on missing schedule, mirrors `delete_account`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `get_fee_schedule`         | `account_id: String, asset_id: String`                                               | `Option<FeeSchedule>`  | `DatabaseError` (FEE-030 — returns `None` when no schedule exists for the pair)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `apply_due_fee_deductions` | —                                                                                    | `()`                   | `DatabaseError` (FEE-040/044/047 — frontend-triggered on app startup; when multi-device sync is enabled and not paused the launch sync runs first inside this command, SYN-060, so generation sees the merged ledger; then generates due completed periods for every active schedule, skipping any that would oversell)                                                                                                                                                                                                                                                                                                                                 |

---

## Shared Types

```rust
struct Account {
    id: String,                          // unique identifier
    name: String,                        // user-defined display name (normalised; unique among names set on this device — after a multi-device merge two accounts may share a name until the user renames one, CFR-035)
    currency: String,                    // ISO 4217 currency code (TRX-021)
    update_frequency: UpdateFrequency,   // how often the user plans to update data
}

enum UpdateFrequency {
    Automatic,
    ManualDay,
    ManualWeek,
    ManualMonth,
    ManualYear,
}

struct AccountDeletionSummary {
    holding_count: u32,       // active holdings in the account
    transaction_count: u32,   // transactions associated with the account
}

struct HoldingSnapshot {
    quantity: i64,        // units held as of the queried date (micro-units), 0 when not held (TDI-010)
    average_price: i64,   // VWAP cost basis per unit, account currency (micro-units), 0 when never held
}
```

```rust
// Purchase: type is implicit in the command — no transaction_type field
struct BuyHoldingDTO {
    account_id: String,
    asset_id: String,
    date: String,           // ISO date YYYY-MM-DD
    quantity: i64,          // micro-units; strictly positive (TRX-020)
    unit_price: i64,        // micro-units, asset currency; zero or positive (TRX-020)
    exchange_rate: i64,     // micro-units; strictly positive (TRX-020)
    fees: i64,              // micro-units, account currency; zero or positive (TRX-020)
    note: Option<String>,
}

// Sell: identical fields, separate type — may diverge as sell-specific rules grow
struct SellHoldingDTO {
    account_id: String,
    asset_id: String,
    date: String,
    quantity: i64,
    unit_price: i64,
    exchange_rate: i64,
    fees: i64,              // micro-units, account currency; zero or positive (SEL-020)
    note: Option<String>,
}

// Correction: no account_id / asset_id / type — those are immutable on an existing transaction
struct CorrectTransactionDTO {
    date: String,
    quantity: i64,
    unit_price: i64,
    exchange_rate: i64,
    fees: i64,
    note: Option<String>,
}
```

> `total_amount` intentionally absent from input DTOs — computed by backend (TRX-026, SEL-023).
> `realized_pnl` intentionally absent — computed by backend (SEL-024).

```rust
// Opening balance: total_cost set directly by user; no fees, no exchange_rate (TRX-047); no note (TRX-043)
struct OpenHoldingDTO {
    account_id: String,
    asset_id: String,
    date: String,       // ISO date YYYY-MM-DD; must not be future or before 1900-01-01 (TRX-046)
    quantity: i64,      // micro-units; strictly positive (TRX-044)
    total_cost: i64,    // micro-units, account currency; strictly positive (TRX-045)
}

// Cash inflow from outside the application (CSH-020/022). Backend resolves the Cash Asset
// for `account.currency`; user does not pick an asset.
struct DepositDTO {
    account_id: String,
    date: String,           // ISO date YYYY-MM-DD; same TRX-020 / CSH-021 bounds as buy/sell
    amount_micros: i64,     // micro-units, account currency; strictly positive (CSH-021)
    note: Option<String>,
}

// Cash outflow to outside the application (CSH-030/032). Same shape as Deposit; eligibility
// (CSH-080) checked against current Cash Holding balance.
struct WithdrawalDTO {
    account_id: String,
    date: String,
    amount_micros: i64,
    note: Option<String>,
}

// Cash dividend paid by a held asset (DIV-020/022). `asset_id` is the PAYING asset; the backend
// credits the account's Cash Holding by `amount_micros × exchange_rate` (account currency) and
// leaves the paying asset's holding untouched (DIV-023/024). No quantity, no unit price, no fees.
struct DividendDTO {
    account_id: String,
    asset_id: String,       // the paying asset (must be an active, non-cash holding — DIV-011)
    date: String,           // ISO date YYYY-MM-DD; same TRX-020 / DIV-021 bounds as buy/sell
    amount_micros: i64,     // micro-units, ASSET currency; net dividend received; strictly positive (DIV-021)
    exchange_rate: i64,     // micro-units, asset→account rate; strictly positive; 1_000_000 when currencies match (DIV-022)
    note: Option<String>,
}

// Free shares received at zero cost (FSD-020). `asset_id` is the DISTRIBUTING asset; the backend
// adds `quantity` to that holding, leaves the cost basis unchanged (average cost dilutes —
// FSD-022/023), and touches no cash. No amount, no unit price, no exchange rate, no fees.
struct FreeSharesDTO {
    account_id: String,
    asset_id: String,       // the distributing asset (must be an active, non-cash holding — FSD-011)
    date: String,           // ISO date YYYY-MM-DD; same TRX-020 / FSD-021 bounds as buy/sell
    quantity: i64,          // micro-units; free shares received; strictly positive (FSD-021)
    note: Option<String>,
}
```

```rust
enum TransactionType {
    Purchase,
    Sell,
    OpeningBalance,  // TRX-042
    Deposit,         // CSH-022 — cash inflow
    Withdrawal,      // CSH-032 — cash outflow
    Dividend,        // DIV-023 — cash income; attributed to the paying asset, credits cash
    FreeShares,      // FSD-022 — zero-cost quantity event; attributed to the distributing asset, no cash leg
    ManagementFee,   // FEE-022 — quantity-reducing fee; attributed to the charged asset, no cash leg
}

// Returned by buy_holding, sell_holding, correct_transaction, open_holding, record_deposit,
// record_withdrawal, and get_transactions.
//
// For Deposit/Withdrawal: asset_id is always the Cash Asset for account.currency;
// quantity == total_amount; unit_price == 1_000_000 (cash is its own unit); exchange_rate ==
// 1_000_000; fees == 0 (v1); realized_pnl is None.
// For Dividend (DIV-023): asset_id is the PAYING asset (not the Cash Asset); total_amount is the
// account-currency cash credited; exchange_rate is the asset→account rate (DIV-022); fees == 0;
// realized_pnl is None (income, not a capital gain — DIV-024). quantity/unit_price carry no
// business meaning (fixed convention).
// For FreeShares (FSD-022): asset_id is the DISTRIBUTING asset; quantity is the distributed
// shares; unit_price == 0, exchange_rate == 1_000_000, fees == 0, total_amount == 0 (no money
// moved — FSD-023); realized_pnl is None.
struct Transaction {
    id: String,
    account_id: String,
    asset_id: String,
    transaction_type: TransactionType,
    date: String,                   // ISO date YYYY-MM-DD
    quantity: i64,                  // micro-units (TRX-024)
    unit_price: i64,                // micro-units, asset currency (TRX-021)
    exchange_rate: i64,             // micro-units, asset→account rate (TRX-021)
    fees: i64,                      // micro-units, account currency
    total_amount: i64,              // micro-units, account currency — computed by backend (TRX-026, SEL-023)
    realized_pnl: Option<i64>,      // micros; Some only for Sell (SEL-024); None for Purchase/OpeningBalance
    note: Option<String>,           // optional user comment; None when absent
    created_at: String,             // ISO 8601 timestamp; chronological tie-breaking (TRX-036, SEL-024)
}
```

```rust
// Active position — quantity > 0 (ACD-020)
struct HoldingDetail {
    asset_id: String,
    asset_name: String,
    asset_reference: String,
    quantity: i64,                      // micros, always > 0
    average_price: i64,                 // micros, VWAP
    cost_basis: i64,                    // micros, quantity × average_price (ACD-023)
    realized_pnl: i64,                  // micros, cumulative from partial sells; 0 if none (SEL-042)
    asset_currency: String,             // ISO 4217 code of the asset's native currency (MKT-023)
    current_price: Option<i64>,         // micros in asset currency; None when no price ever recorded (MKT-031)
    current_price_date: Option<String>, // ISO date of the price observation; None when current_price is None (MKT-031)
    unrealized_pnl: Option<i64>,        // micros in account currency; None on currency mismatch or no price; 0 (not None) when price == avg_price (MKT-033/034)
    performance_pct: Option<i64>,       // micros (5.25% = 5_250_000); None when unrealized_pnl is None or cost_basis = 0; 0 (not None) when unrealized_pnl is 0 (MKT-035)
    dividends_received: i64,            // micros, account currency; sum of dividend cash for this (account, asset); 0 when none; always computable (DIV-070)
    total_return_pct: Option<i64>,      // micros; (unrealized_pnl + dividends_received) × 100 / cost_basis; None under the same conditions as performance_pct (DIV-071)
    management_fees: i64,               // micros, account currency; cumulative fee value (Σ removed_qty × price_as_of(date)) for this (account, asset); 0 when none (FEE-052)
    inconsistency: Option<HoldingInconsistency>, // derived on read; Some when the merged ledger oversells the position or overdraws cash (SYN-040, CFR-042); None otherwise — see sync-contract.md
}

// Closed position — quantity = 0 (ACD-044)
struct ClosedHoldingDetail {
    asset_id: String,
    asset_name: String,
    asset_reference: String,
    realized_pnl: i64,      // micros, total gain/loss for this position (ACD-045)
    last_sold_date: String, // ISO date "YYYY-MM-DD"; non-optional in this DTO (ACD-043)
}

// Top-level response for get_account_details
struct AccountDetailsResponse {
    account_name: String,
    holdings: Vec<HoldingDetail>,              // active (quantity > 0), includes Cash Holding when present and qty > 0 (CSH-090, CSH-097); includes archived assets (ACD-020, ACD-021), sorted by asset_name asc (ACD-033)
    closed_holdings: Vec<ClosedHoldingDetail>, // closed, sorted by asset_name asc (ACD-046); empty list when none
    total_holding_count: i64,                  // all holdings regardless of quantity (ACD-034)
    total_cost_basis: i64,                     // micros, sum of cost_basis across active non-cash holdings (ACD-031, CSH-093)
    total_realized_pnl: i64,                   // micros, sum of total_realized_pnl across all holdings (ACD-045)
    total_unrealized_pnl: Option<i64>,         // micros; sum across same-currency priced active holdings; None when none qualify (MKT-040)
    total_global_value: i64,                   // micros, account currency: cash_holding.quantity + Σ_h (h.quantity × latest_price(h)) over non-cash active holdings; unpriced non-cash holdings contribute 0 (CSH-094)
    total_dividends_received: i64,             // micros, account currency: sum of dividend cash across all the account's dividend transactions; 0 when none (DIV-073)
    total_management_fees: i64,                // micros, account currency: sum of per-holding management_fees across the account; 0 when none (FEE-053)
}

// Row returned by get_account_summaries (ACC-021)
struct AccountSummary {
    id: String,
    name: String,
    currency: String,                          // ISO 4217 currency code; same as Account.currency
    update_frequency: UpdateFrequency,
    total_global_value: i64,                   // micros, account currency: same algorithm as AccountDetailsResponse.total_global_value (CSH-094)
    total_unrealized_pnl: Option<i64>,         // micros, account currency: account-wide unrealized P&L, same algorithm as AccountDetailsResponse.total_unrealized_pnl (MKT-040 / ACC-023); None when no holding qualifies
    ytd_performance_pct: Option<i64>,          // micro-percent: year-to-date net-of-flows performance since Jan 1, reusing PRF-034 (ACC-024); first-year accounts use an inception baseline (present, not None); None only when the Simple-Dietz denominator is 0 (PRF-032)
    has_inconsistent_holding: bool,            // derived on read; true when any holding of the account carries an inconsistency (SYN-040, CFR-042)
}

// Response of get_account_summaries (ACC-021, ACC-027)
struct AccountSummaries {
    summaries: Vec<AccountSummary>,
    total: PortfolioTotal,
}

// Portfolio total over every account (ACC-027/028/029)
struct PortfolioTotal {
    total_global_value: i64,                   // micros, reference currency: Σ account Global Value converted at the read date's rate
    total_unrealized_pnl: Option<i64>,         // micros, reference currency: Σ account Unrealized P&L converted; None when no convertible account carries one (ACC-029)
    currency: String,                          // the reference currency (GPF-011)
    incomplete: bool,                          // an account with no usable rate to the reference currency (FXR-034) held a non-zero Global Value or Unrealized P&L, which the total leaves out (ACC-028)
}
```

```rust
// Net-of-flows performance for one period; Simple Dietz percentage (PRF-031, PRF-032)
struct PerformanceMetric {
    gain: i64,          // micros, account currency
    pct: Option<i64>,   // micro-percent (8.00% = 8_000_000); None when the Dietz denominator is 0 (PRF-032)
}

// One calendar period row — a month or a year (PRF-020, PRF-040)
struct PerformancePeriod {
    year: i32,
    month: Option<u8>,                              // Some(1..=12) for month rows; None for year rows (PRF-011)
    end_value: i64,                                 // micros, account currency; Global Value at period end (PRF-020)
    period_over_period: Option<PerformanceMetric>,  // None when no preceding period exists (PRF-033, PRF-042)
    year_to_date: Option<PerformanceMetric>,        // None for year rows (PRF-037); always Some for month rows — inception-year months use baseline 0, equal to since-inception (PRF-034)
    since_inception: Option<PerformanceMetric>,     // measured from net invested; inception baseline value 0 (PRF-035)
}

// Top-level response for get_account_performance — recomputed on read (ADR-013)
struct AccountPerformanceResponse {
    account_name: String,
    currency: String,                   // account's own ISO 4217 currency
    month_view_available: bool,         // true only for Automatic/ManualDay/ManualWeek (PRF-013)
    yearly: Vec<PerformancePeriod>,     // one per year, most-recent first (PRF-041); month is None
    monthly: Vec<PerformancePeriod>,    // one per month over the full span, most-recent first; empty when month_view_available is false (PRF-013, PRF-015)
}
```

```rust
// Recurring-fee cadence (FEE-034). periods_per_year: Monthly 12, Quarterly 4, Annually 1.
enum FeeFrequency {
    Monthly,
    Quarterly,
    Annually,
}

// One-off management fee (FEE-020/022). `asset_id` is the CHARGED asset (active, non-cash holding).
// Exactly one of `percent_micros` / `resulting_quantity_micros` (FEE-021): the backend removes
// floor(holding_as_of(date) × percent), or exactly holding_as_of(date) − resulting quantity (FEE-028).
struct ManagementFeeDTO {
    account_id: String,
    asset_id: String,
    date: String,                            // ISO date YYYY-MM-DD; same TRX-020 / FEE-021 bounds as buy/sell
    percent_micros: Option<i64>,             // micro-percent of the holding (1% = 1_000_000); strictly positive, ≤ 100_000_000 (FEE-021)
    resulting_quantity_micros: Option<i64>,  // quantity held after the fee, as of `date`; ≥ 0 and below the held quantity (FEE-028)
    note: Option<String>,
}

// What a fee entered by its resulting quantity removes (FEE-029). Micro-units; percent floored.
struct ManagementFeeRemoval {
    held_quantity: i64,
    removed_quantity: i64,
    percent_micros: i64,
}

// Recurring per-(account, asset) fee schedule (FEE-030). `annual_rate_percent_micros` scaled to
// `frequency` per period (FEE-041). `last_applied_period` is the boundary date of the most recent
// generated deduction — the catch-up cursor (FEE-043); None until the first deduction is generated.
struct FeeSchedule {
    id: String,
    account_id: String,
    asset_id: String,
    annual_rate_percent_micros: i64,     // micro-percent per year (0.20% = 200_000); strictly positive, < 100_000_000 (FEE-032)
    frequency: FeeFrequency,
    start_date: String,                  // ISO date YYYY-MM-DD; immutable after first generation (FEE-060)
    end_date: Option<String>,            // ISO date; None = open-ended (FEE-045)
    active: bool,                        // false while paused — no deductions generated (FEE-061)
    last_applied_period: Option<String>, // ISO date of last generated period boundary; None initially (FEE-043). Between devices this is its own synced record identified by (account_id, asset_id), merged by maximum (CFR-044); exposed here as a derived read field
}

// Create a fee schedule (FEE-030). At most one per (account, asset) — ScheduleAlreadyExists otherwise.
struct CreateFeeScheduleDTO {
    account_id: String,
    asset_id: String,
    annual_rate_percent_micros: i64,
    frequency: FeeFrequency,
    start_date: String,
    end_date: Option<String>,
}

// Edit a fee schedule (FEE-060/061). `frequency` and `start_date` intentionally absent — structurally
// immutable; cadence change = delete + recreate. Identified by (account_id, asset_id).
struct UpdateFeeScheduleDTO {
    account_id: String,
    asset_id: String,
    annual_rate_percent_micros: i64,
    end_date: Option<String>,
    active: bool,
}
```

---

## Events

### Published

| Event                | Payload | Rule                               |
| -------------------- | ------- | ---------------------------------- |
| `AccountUpdated`     | —       | ACC-022                            |
| `TransactionUpdated` | —       | TRX-037, DIV-026, FSD-026, FEE-026 |
| `FeeScheduleUpdated` | —       | FEE-064                            |
| `HoldingNoteUpdated` | —       | HNO-020, HNO-021                   |

### Subscribed (frontend re-fetch triggers)

| Event                 | Payload | Rule                                                                                                                           |
| --------------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `AccountUpdated`      | —       | ACC-021, PRF-060                                                                                                               |
| `TransactionUpdated`  | —       | ACD-039, ACC-021, PRF-060, FEE-026                                                                                             |
| `AssetUpdated`        | —       | ACD-040                                                                                                                        |
| `AssetPriceUpdated`   | —       | MKT-036, PRF-060                                                                                                               |
| `CurrencyRateUpdated` | —       | FXR-037, ACC-033                                                                                                               |
| `FeeScheduleUpdated`  | —       | FEE-064 (Account Details re-fetch)                                                                                             |
| `HoldingNoteUpdated`  | —       | HNO-043 (Account Details re-fetch)                                                                                             |
| `CurrencyPairUpdated` | —       | FXR-056 (Account Details re-fetch — a removed pair takes its rates with it), ACC-033                                           |
| `SyncCompleted`       | —       | SYN-064 — every account view (details, list, performance, transactions, journal) re-fetches once for everything a sync applied |

---

## Changelog

- 2026-09-19 — FEE-028/029: `ManagementFeeDTO.percent_micros` becomes optional beside the new `resulting_quantity_micros` (exactly one); `preview_management_fee` + `ManagementFeeRemoval { held_quantity, removed_quantity, percent_micros }` added; errors `ManagementFeeAmountInvalid`, `ResultingQuantityNegative`, `ResultingQuantityNotBelowHeld { held_quantity }`; `ManagementFeesDisabled` (FEE-077) and `QuantityNotPositive` now listed where the code already raised them
- 2026-09-19 — SYN-064 amended: applied changes raise no event of their own (`HoldingNoteUpdated` is published by local note writes only); every account view subscribes to `SyncCompleted` and re-fetches once per sync
- 2026-09-12 — `HoldingNoteUpdated` event (HNO-043): published by note upsert / delete and by `apply_holding_note` (SYN-064); Account Details subscribes to it instead of the bare `SyncCompleted` marker
- 2026-08-22 — Amended by `multi-device-sync` + `sync-conflict-resolution` specs: `HoldingDetail.inconsistency` and `AccountSummary.has_inconsistent_holding` (derived, SYN-040 / CFR-042); `FeeSchedule.last_applied_period` documented as the derived read of the synced catch-up record (CFR-044); `apply_due_fee_deductions` runs the launch sync first (SYN-060); `Account.name` / `update_account.NameAlreadyExists` bind the name being set (CFR-035).
- 2026-05-29 — Added by `account-performance` spec: `get_account_performance` (+ `AccountPerformanceResponse`, `PerformancePeriod`, `PerformanceMetric` types; PRF-060 re-uses existing subscribed events)
- 2026-05-31 — Added by `cash-dividend` spec: `record_dividend` (+ `DividendDTO`); `TransactionType::Dividend` variant; `HoldingDetail.dividends_received` + `.total_return_pct`; `AccountDetailsResponse.total_dividends_received`; edit/delete reuse `correct_transaction`/`cancel_transaction` (DIV-040/041)
- 2026-05-31 — `cash-transaction-history` (CSH-110/111): no command change — cash Deposit/Withdrawal edit reuses `correct_transaction` and delete reuses `cancel_transaction` (both already accept the cash `TransactionType`s); the feature is a frontend entry point (cash-row inspect action + dedicated cash modals in edit mode). Contract surface unchanged.
- 2026-06-02 — Added by `fx-rate` spec: `CurrencyRateUpdated` subscribed event (FXR-037) — the `account_details` and `account_performance` views re-fetch when an FX rate changes so foreign-currency holdings revalue. No command change.
- 2026-06-16 — Amended by `account` spec (ACC-023/024, accounts-overview metrics): `AccountSummary` gains `total_unrealized_pnl: Option<i64>` (account-wide unrealized P&L, MKT-040 algorithm) and `ytd_performance_pct: Option<i64>` (year-to-date performance reusing PRF-034). No new command — `get_account_summaries` returns the enriched rows.
- 2026-06-11 — Added by `free-share-distribution` spec: `record_free_shares` (+ `FreeSharesDTO`); `TransactionType::FreeShares` variant + its `Transaction` packing convention; FSD-040/041 cross-refs on `correct_transaction`/`cancel_transaction`'s `CascadingOversell`; edit/delete reuse those commands.
- 2026-06-30 — Added by `management-fee-deduction` spec: `record_management_fee`, `create_fee_schedule`, `update_fee_schedule`, `delete_fee_schedule`, `get_fee_schedule`, `apply_due_fee_deductions` (+ `ManagementFeeDTO`, `FeeFrequency`, `FeeSchedule`, `CreateFeeScheduleDTO`, `UpdateFeeScheduleDTO`); `TransactionType::ManagementFee` variant; `HoldingDetail.management_fees` + `AccountDetailsResponse.total_management_fees`; `FeeScheduleUpdated` event (published + Account Details subscribes); FEE-063 cross-ref on `correct_transaction`'s `CascadingOversell`; edit/delete of a deduction reuse `correct_transaction`/`cancel_transaction`.
- 2026-07-04 — Added by `interest-credit` spec: `record_interest` (+ `RecordInterestDTO`, `InterestError`); `TransactionType::Interest` variant + its zero-cost `Transaction` packing (INT-024); the Cash Asset as a valid target (INT-023); `AccountError::InterestAmountInvalid`; INT-040/041 cross-refs — edit/delete reuse `correct_transaction`/`cancel_transaction`. Same session: `add_account`/`update_account` DTOs gain `management_fees_enabled` (FEE-075) and `get_account_details` gains `HoldingDetail.market_value` + `fee_rate_percent_micros` and `AccountDetailsResponse.total_net_cash_input` (ACD-052/053, FEE-074).
- 2026-07-05 — Input-column refresh: `get_account_details` gains `as_of_date: Option<String>` (as-of read-only view) and `get_account_performance` gains `asset_id: Option<String>` (position-scoped series, PRF-080) — both shipped earlier, now reflected in the tables above. No new command.
- 2026-09-14 — Amended by `account` spec (ACC-027/028): `get_account_summaries` returns `AccountSummaries` — the rows plus a `PortfolioTotal` in the reference currency, flagged incomplete when an account with no usable rate held a non-zero figure; the accounts list re-fetches on `CurrencyRateUpdated` and `CurrencyPairUpdated` (ACC-033). No new command or error.
