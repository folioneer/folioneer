# Contract — Currency

> Domain: `currency`
> Last updated by: `fx-rate` spec

> **Error model on the wire**: each command's error serializes as a flat `{ code: "VariantName", ...payload }` object. The FE matches on `code`. Per-command reachable codes are listed in the "Errors" column. Infrastructure failures surface as `{ code: "DatabaseError" }` (no payload; diagnostic chain preserved server-side via `tracing::error!`), consistent with the `account` and `asset` contracts.
>
> Rust-internal type organization (per-BC enums, serde tagging) is out of scope — this documents the BE↔FE frontier, not Rust internals.
>
> **Wire convention for `rate`**: mutation commands accept `rate: f64` (the human-readable decimal the FE holds); the backend converts to `i64` micros at the IPC boundary per FXR-024 / ADR-001 — mirroring `record_asset_price(price: f64)` in the `asset` contract. Returned `CurrencyRate.rate` is `i64` micros.

---

## Commands

### Currency Pairs

> A `CurrencyPair` is a durable record (FXR-013/014). `declare_currency_pair` is the manual
> "Add pair" path (FXR-054); pairs are also auto-followed from foreign holdings (FXR-013) — that
> path is internal to `use_cases/` and has no distinct FE command (see Notes). `get_currency_pairs`
> feeds the pair-centric Currency Rates view (FXR-051).

| Command                 | Args                                         | Return                     | Errors                                                                                                                                                         |
| ----------------------- | -------------------------------------------- | -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `declare_currency_pair` | `from_currency: String, to_currency: String` | `CurrencyPair`             | `InvalidCurrency { currency }` (FXR-023), `IdentityPair` (FXR-011/023), `DatabaseError` _(FXR-054 — idempotent: an existing pair is returned, not duplicated)_ |
| `get_currency_pairs`    | —                                            | `Vec<CurrencyPairSummary>` | `DatabaseError` _(FXR-051 — empty list when no pair persisted; each pair enriched with its most-recent rate, FXR-035)_                                         |

### Currency Rates

> `record_currency_rate` upserts by `(from_currency, to_currency, date)`, latest-write-wins
> regardless of source (FXR-025, ADR-012), and sets `source = Manual` (FXR-101). It **ensures the
> pair exists** before writing (FXR-013 ergonomics) — recording a rate for a not-yet-declared pair
> is accepted and persists the pair as a side-effect; no separate declare call is required.
> `update_currency_rate` and `delete_currency_rate` mirror the `asset` contract's
> `update_asset_price` / `delete_asset_price` shapes (same-date edit = in-place overwrite; changed
> date = delete-old + upsert-new); neither ever removes the pair (FXR-014).

| Command                          | Args                                                                                                 | Return              | Errors                                                                                                                                                                                                                                                                                                                                                                                                         |
| -------------------------------- | ---------------------------------------------------------------------------------------------------- | ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `record_currency_rate`           | `from_currency: String, to_currency: String, date: String, rate: f64`                                | `CurrencyRate`      | `NotPositive` (FXR-021), `NonFinite` (FXR-021), `DateInFuture` (FXR-022), `InvalidDateFormat { date }` (FXR-022), `InvalidCurrency { currency }` (FXR-023), `IdentityPair` (FXR-011/023), `DatabaseError`                                                                                                                                                                                                      |
| `update_currency_rate`           | `from_currency: String, to_currency: String, original_date: String, new_date: String, new_rate: f64` | `()`                | `RateNotFound { from_currency, to_currency, date }` (FXR-052), `NotPositive` (FXR-021), `NonFinite` (FXR-021), `DateInFuture` (FXR-022), `InvalidDateFormat { date }` (FXR-022), `InvalidCurrency { currency }` (FXR-023), `IdentityPair` (FXR-011/023), `DatabaseError`                                                                                                                                       |
| `delete_currency_rate`           | `from_currency: String, to_currency: String, date: String`                                           | `()`                | `RateNotFound { from_currency, to_currency, date }` (FXR-053), `DatabaseError`                                                                                                                                                                                                                                                                                                                                 |
| `get_currency_rates`             | `from_currency: String, to_currency: String`                                                         | `Vec<CurrencyRate>` | `DatabaseError` _(FXR-050 — ordered by `date` descending; empty list for an unknown pair, never NotFound)_                                                                                                                                                                                                                                                                                                     |
| `backfill_currency_rate_history` | —                                                                                                    | `u32`               | `ProviderUnreachable`, `DatabaseError` _(FXR-110–114, "Update rates" — ensures the pairs of active foreign holdings (FXR-071), then a dated daily series for all persisted pairs, from the earliest transaction date across accounts through today; returns rows written; zero when nothing anchors the range. Implemented by `use_cases/rate_history_backfill` with its own flat `RateHistoryBackfillError`)_ |
| `refresh_currency_rates`         | —                                                                                                    | `()`                | `DatabaseError` _(FXR-075 — the launch rate refresh: ensures the pairs of active foreign holdings, then records the current rate of every persisted pair; a provider failure is silent (FXR-070/073). Implemented by `use_cases/rate_refresh` with its own flat `RateRefreshError`)_                                                                                                                           |

---

## Shared Types

```rust
// A directed currency pair the system follows (FXR-013/014). Durable; (from, to) unique; from != to.
struct CurrencyPair {
    from_currency: String,   // ISO 4217 source currency (e.g. "USD")
    to_currency: String,     // ISO 4217 target currency (e.g. "EUR")
}

// One dated rate observation for a pair (FXR entity). Unique by (from_currency, to_currency, date).
struct CurrencyRate {
    from_currency: String,        // ISO 4217 source currency
    to_currency: String,          // ISO 4217 target currency
    date: String,                 // ISO 8601 date YYYY-MM-DD
    rate: i64,                    // micros: units of to_currency per 1 from_currency (FXR-010, ADR-001)
    source: CurrencyRateSource,   // provenance (FXR-100); metadata only, never precedence (ADR-012)
}

// CurrencyRateSource (FXR-100) — text discriminant matching the variant name, like AssetPriceSource.
enum CurrencyRateSource {
    Manual,        // user-driven write (record/update) — FXR-101
    Frankfurter,   // Frankfurter fetch tier — FXR-102
    Ecb,           // ECB XML fallback fetch tier — FXR-102
}

// Row returned by get_currency_pairs (FXR-051): a pair enriched with its most-recent rate
// (resolved per FXR-035). The latest_* fields are None for a pair that has no rate yet.
struct CurrencyPairSummary {
    from_currency: String,
    to_currency: String,
    latest_rate: Option<i64>,                  // micros; most-recent CurrencyRate.rate; None when no rate recorded
    latest_rate_date: Option<String>,          // ISO date of the most-recent rate; None when latest_rate is None
    latest_rate_source: Option<CurrencyRateSource>, // provenance of the most-recent rate; None when latest_rate is None
}
```

---

## Events

### Published

| Event                 | Payload | Rule                                                                                                                                                    |
| --------------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `CurrencyRateUpdated` | —       | FXR-026 (record), FXR-052 (edit), FXR-053 (delete), FXR-074 (fetch) — bare signal, discriminant `"CurrencyRateUpdated"`, published by the `currency` BC |
| `CurrencyPairUpdated` | —       | FXR-054 (declare) — bare signal, discriminant `"CurrencyPairUpdated"`, published by the `currency` BC                                                   |

### Subscribed (frontend re-fetch triggers)

| Event                 | Payload | Rule                                                                                                                                                                     |
| --------------------- | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `CurrencyRateUpdated` | —       | FXR-036 — Currency Rates view re-fetches; **also** consumed by the `account` domain's `account_details` / `account_performance` views (cross-contract wiring, see Notes) |
| `SyncCompleted`       | —       | SYN-064 — Currency Rates view re-fetches its pair list and the selected pair's rates once for everything a sync applied                                                  |
| `CurrencyPairUpdated` | —       | FXR-056 — Currency Rates view re-fetches its pair list                                                                                                                   |

---

## Notes

- **Valuation effect is not a command.** Lifting the currency-mismatch guards (FXR-030–042) changes how `use_cases/account_details/` and `use_cases/account_performance/` value foreign holdings by consuming an injected currency-rate service (ADR-003/004). It rides the **existing** `account` contract's `get_account_details` / `get_account_summaries` / `get_account_performance` — **no new command and no change to the `account` contract's command list**. The planner wires the use cases to a `currency` service; this introduces no FE-visible command here.
- **Cross-contract event wiring (FXR-036/037).** `CurrencyRateUpdated` must be added to the event-bus enum and the `account` contract's _Subscribed_ events table (`account_details` + `account_performance` re-fetch on it), and `ARCHITECTURE.md` updated. This is a follow-up the planner schedules; it is not a command change.
- **Auto-follow (FXR-013) has no command of its own.** A foreign holding's pair is ensured inside `refresh_currency_rates` and `backfill_currency_rate_history` (FXR-071). Apart from those, the only command that creates a pair is `declare_currency_pair` (FXR-054).
- **Rate refresh (FXR-070–076) is its own command.** The frontend calls `refresh_currency_rates` once at launch (FXR-075) and `backfill_currency_rate_history` from "Update rates" (FXR-110); the price fetch tasks fetch no rate, and no in-flight guard covers rates (FXR-076).

---

## Changelog

- 2026-09-25 — #042 (FXR-071, FXR-074–077, FXR-110–114): `refresh_currency_rates` added (launch rate refresh, every build); `backfill_currency_rate_history` backs "Update rates", ensures holding pairs first and publishes one `CurrencyRateUpdated` when it wrote a rate; the price fetch tasks no longer fetch rates
- 2026-09-19 — SYN-064 amended: applied pairs, rates and removals raise no event of their own (`CurrencyPairUpdated` / `CurrencyRateUpdated` are published by local writes only); the Currency Rates view subscribes to `SyncCompleted`
- 2026-09-12 — `CurrencyPairUpdated` event (FXR-056): published by `declare_currency_pair`, `apply_currency_pair` and an applied pair removal (SYN-064); the Currency Rates view subscribes to it instead of the bare `SyncCompleted` marker
- 2026-07-14 — Added by `fx-rate` spec (FXR-110–114): `backfill_currency_rate_history` (historical dated-series download); `CurrencyError` gains `ProviderUnreachable`
- 2026-06-01 — Added by `fx-rate` spec (FXR): `declare_currency_pair`, `record_currency_rate`, `update_currency_rate`, `delete_currency_rate`, `get_currency_pairs`, `get_currency_rates`; types `CurrencyPair`, `CurrencyRate`, `CurrencyRateSource`, `CurrencyPairSummary`; event `CurrencyRateUpdated`. First contract for the new `currency` bounded context. Valuation effect (FXR-030–042) and provider fetch (FXR-070–083) introduce **no** command — they ride existing `account`/`asset` surfaces internally (see Notes).
