# Business Rules — Global Performance (GPF)

## Context

The Global Performance feature presents how the user's whole portfolio — every account together, or one asset's positions across every account — has evolved over time, period by period, using the same table shape as the per-account performance view ([`account-performance.md`](account-performance.md), PRF). A single backend command, `get_global_performance(account_id?, asset_id?)`, serves every scope: when an `account_id` is supplied the read is exactly the single-account read of `get_account_performance`; when it is absent the read aggregates across accounts.

Accounts are denominated in different currencies, so a cross-account aggregation needs one reporting currency. The reference currency is fixed to **EUR** (user decision): every aggregated figure — end values, flows, bridge terms, gains — is converted to EUR before summation, using the same carry-forward rate resolution as holding valuation (FXR-035/042) and the same missing-rate degradation to `0` (FXR-034).

This is a **feature spec** spanning the `account`, `asset`, and `currency` bounded contexts, orchestrated by `use_cases/global_performance/` (ADR-003, ADR-004). The per-account series machinery is shared with `account_performance` through `use_cases/shared/` (B18); the aggregation reuses it per account and sums the converted results. All monetary values are `i64` micro-units per [ADR-001](../adr/001-use-i64-for-monetary-amounts.md); everything is recomputed on read per [ADR-013](../adr/013-recompute-account-performance-on-read.md).

---

## Business Rules

### Scope Matrix (010–019)

**GPF-010 — Scope matrix (backend)**: `get_global_performance` accepts an optional `account_id` and an optional `asset_id`:

| `account_id` | `asset_id` | Read                                                                                                                                                           |
| ------------ | ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| present      | absent     | The single-account read of `get_account_performance` (PRF-010–074) — response identical.                                                                       |
| present      | present    | The single-account asset-scoped read of `get_account_performance` (PRF-080–084) — response identical.                                                          |
| absent       | absent     | All accounts aggregated in the reference currency (GPF-011–041).                                                                                               |
| absent       | present    | The asset's positions across all accounts holding it, aggregated in the reference currency (GPF-011–041, with the PRF-080–084 position semantics per account). |

**GPF-011 — Reference currency and label (backend + frontend)**: Every cross-account aggregation is reported in **EUR**. The response reuses the `get_account_performance` shape with `currency = "EUR"` and an empty `account_name`; the backend carries no display label (no i18n in the backend) — the frontend resolves the aggregation's label (e.g. "All accounts").

**GPF-012 — Included accounts (backend)**: An account participates in an aggregation only when it has at least one dated in-scope transaction — any transaction for the all-accounts read, a transaction of the scoped asset for the asset-scoped read. Accounts with no in-scope transactions are excluded from the aggregation and from the month-view eligibility (GPF-014).

**GPF-013 — Data span (backend)**: Rows span from the period containing the earliest in-scope transaction across the included accounts through the current period (PRF-040 applied to the union). An account whose own history starts later contributes `0` end values before its first transaction.

**GPF-014 — Month view eligibility (backend)**: The yearly series is always computed. The monthly series is computed only when **every** included account satisfies the per-account month-view eligibility (PRF-013: `Automatic`, `ManualDay`, or `ManualWeek`); one ineligible included account disables the month view for the whole aggregation.

**GPF-015 — Empty portfolio (backend)**: When no account is included (no accounts, no in-scope transactions, or an asset scope no account holds), the read succeeds with the PRF-043-shaped empty result: empty yearly and monthly series, `currency = "EUR"`, empty `account_name`, `month_view_available = false`.

**GPF-016 — Remembered view granularity (frontend)**: The global view's month/year selection persists on the device (the global twin of PRF-014): the last choice is restored on the next visit, clamped to the year view when the month view is unavailable (GPF-014), and defaults to the month view when available and nothing is stored.

### Aggregated Values and Metrics (020–039)

**GPF-020 — Aggregated period end value (backend)**: A period row's end value is the sum over included accounts of the account's own end value at the period end — the account's Global Value (PRF-020–024) in the all-accounts read, the scoped position value (PRF-082) in the asset-scoped read, each computed with the account's own machinery in the account's own currency — converted at the account-currency → EUR rate as of the period end (carry-forward, FXR-042; identity for EUR accounts). An account with no usable rate as of that period end contributes `0` for that period (FXR-034).

**GPF-030 — Converted external flows (backend)**: The Simple Dietz flow set of the aggregation is the union of each included account's flows — the account-level external flows (PRF-030: Deposit `+`, Withdrawal `−`, OpeningBalance `+`) in the all-accounts read, the position trade flows (PRF-083: Purchase `+`, OpeningBalance `+`, Sell `−`) in the asset-scoped read — each converted at the account-currency → EUR rate as of the flow's own transaction date (carry-forward). A flow with no usable rate contributes `0`.

**GPF-031 — Aggregated metrics (backend)**: Period-over-period, year-to-date (month rows), since-inception, and the year-row annualized yield apply the PRF-031–035 Simple Dietz definitions over the aggregated EUR end values (GPF-020) and the converted flow set (GPF-030). In the asset-scoped read the assets' converted dividends are added to the gain and the percentage is absent when the Dietz denominator is not positive, per PRF-083. Baselines follow the unscoped definitions evaluated on the aggregated values: the previous period's aggregated end value, the prior 31 December aggregated value, and inception `0` at the global span start.

### Global Value Bridge (040–049)

**GPF-040 — Converted bridge terms (backend)**: The bridge columns mirror PRF-070–072 (all-accounts read) and PRF-084 (asset-scoped read), summed over included accounts in EUR. Per account: `cash_flow` (deposits − withdrawals plus cash-line interest; in asset scope Purchase + OpeningBalance − Sell) and `dividends` convert each transaction at its own transaction-date rate; within `asset_flow`, an opening balance is valued at its entry-date market value in the account currency (PRF-086, fallback typed cost) and converts at its transaction-date rate, and zero-cost in-kind credits (free shares, non-cash interest) are valued at their grant date in the account currency (PRF-071) and convert at the grant-date rate — every flow term thus converts at the rate of its own date. Any term with no usable rate contributes `0`.

**GPF-041 — Bridge identity and currency movement (backend)**: `pnl` is the bridge residual, so every aggregated row satisfies the PRF-074 identity to the cent — `end_value = previous_value + cash_flow + asset_flow + dividends + pnl` — and every asset-scoped row satisfies the PRF-084 identity (dividends outside: `end_value = previous_value + cash_flow + asset_flow + pnl`). Because end values convert at period-end rates while flows convert at transaction-date rates, the currency movement of foreign accounts between those dates lands in `pnl` — it is investment profit and loss from the EUR investor's viewpoint. Degradations to `0` (missing rates, GPF-020/030/040) likewise surface in the residual.

---

## Workflow

```
get_global_performance(account_id?, asset_id?)
          │
          ├─ account_id present → single-account read (PRF-010–084), response identical
          │
          └─ account_id absent:
                ├─ [load all accounts; keep those with in-scope transactions (GPF-012)]
                ├─ [derive the global span from the earliest in-scope transaction (GPF-013)]
                ├─ [month view = AND of per-account eligibility (GPF-014); none included → empty (GPF-015)]
                ├─ Per included account:
                │     ├─ load transactions (scoped when asset_id present) + priced assets
                │     ├─ resolve asset-currency → account-currency rates at valued dates (FXR-035/042)
                │     ├─ resolve account-currency → EUR rates at valued dates + transaction dates
                │     └─ convert the account's Dietz flows at their transaction dates (GPF-030)
                │
                └─ Per period in the global span (year rows; month rows when eligible):
                      ├─ end_value = Σ converted per-account end values at period end (GPF-020)
                      ├─ metrics over aggregated values + converted flows (GPF-031)
                      └─ bridge terms Σ converted per account; pnl = residual (GPF-040/041)
```

---

## Frontend Surface

- Route `/performance`, entered from the accounts overview header (`accounts-performance` icon button).
- Two scope selectors drive the GPF-010 matrix: an account selector defaulting to "All accounts" and an asset selector defaulting to "All assets" (catalog assets when unscoped; the scoped account's non-cash holdings otherwise). Changing the account scope resets the asset scope.
- The GPF-011 labels resolve in the frontend: the title carries the scoped account/asset names joined as "Account — Asset" (absent when unscoped) and the response currency (EUR for cross-account scopes).
