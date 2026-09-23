# Contract — Asset

> Domain: `asset`
> Last updated by: market-price (MKT-190–213)

> **Error model on the wire**: each command's error serializes as a flat `{ code: "VariantName", ...payload }` object. The FE matches on `code`. Per-command reachable codes are listed in the "Errors" column of each table below. Infrastructure failures surface as `{ code: "DatabaseError" }` (no payload; diagnostic chain preserved server-side via `tracing::error!`).
>
> Rust-internal type organization (per-BC enums, use-case composites, serde tagging) is out of scope for this contract — it documents the BE↔FE frontier, not Rust internals.

---

## Commands

### Asset CRUD

| Command                       | Args                                                                                                                                                                                                | Return          | Errors                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ----------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `get_assets`                  | —                                                                                                                                                                                                   | `Vec<Asset>`    | `DatabaseError` _(returns active assets only)_                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `get_assets_with_archived`    | —                                                                                                                                                                                                   | `Vec<Asset>`    | `DatabaseError` _(returns all assets including archived)_                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `add_asset`                   | `CreateAssetDTO { name: String, class: AssetClass, category_id: String, currency: String, risk_level: i32, reference: String, isin: Option<String>, exchange: Option<Exchange> }`                   | `Asset`         | `NameEmpty` (R1), `ReferenceEmpty` (R1), `InvalidIsinFormat` (AST-023, WEB-016), `InvalidRiskLevel { received: i32 }` (AST-002), `InvalidCurrency { currency }` (TRX-021), `InvalidExchange { exchange_code: String }` (AST-001), `CategoryNotFound { id }` (when `category_id` missing), `DatabaseError`                                                                                                                                                                                              |
| `update_asset`                | `UpdateAssetDTO { asset_id: String, name: String, reference: String, isin: Option<String>, class: AssetClass, currency: String, risk_level: i32, category_id: String, exchange: Option<Exchange> }` | `Asset`         | `AssetNotFound { id }` (asset missing), `CategoryNotFound { id }` (category missing — including a category that stands removed after a multi-device merge, CFR-030: the user picks another), `Archived` (R18 — archived asset cannot be edited), `CashAssetNotEditable` (CSH-016), `NameEmpty`, `ReferenceEmpty`, `InvalidIsinFormat` (AST-023, WEB-016), `InvalidRiskLevel { received: i32 }`, `InvalidCurrency { currency }`, `InvalidExchange { exchange_code: String }` (AST-001), `DatabaseError` |
| `unarchive_asset`             | `id: String`                                                                                                                                                                                        | `()`            | `AssetNotFound { id }`, `CashAssetNotEditable` (CSH-016), `DatabaseError`                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `block_asset_price_refresh`   | `id: String`                                                                                                                                                                                        | `()`            | `AssetNotFound { id }` (MKT-156), `CashAssetNotEditable` (CSH-016 / MKT-154), `DatabaseError` _(MKT-150/156 — sets `price_refresh_blocked = true`; idempotent; publishes `AssetUpdated`)_                                                                                                                                                                                                                                                                                                              |
| `unblock_asset_price_refresh` | `id: String`                                                                                                                                                                                        | `()`            | `AssetNotFound { id }` (MKT-156), `CashAssetNotEditable` (CSH-016 / MKT-154), `DatabaseError` _(MKT-150/156 — clears `price_refresh_blocked`; idempotent; publishes `AssetUpdated`)_                                                                                                                                                                                                                                                                                                                   |
| `get_supported_exchanges`     | —                                                                                                                                                                                                   | `Vec<Exchange>` | _(infallible — returns the canonical curated set from the BE constant; FE picker source per AST-021)_                                                                                                                                                                                                                                                                                                                                                                                                  |

### Categories

> All category commands are owned by the asset BC. The system default category cannot be renamed (`SystemReadonly`) or deleted (`SystemProtected`).

| Command           | Args                        | Return               | Errors                                                                                                                                                                                                             |
| ----------------- | --------------------------- | -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `get_categories`  | —                           | `Vec<AssetCategory>` | `DatabaseError` _(read-only)_                                                                                                                                                                                      |
| `add_category`    | `label: String`             | `AssetCategory`      | `LabelEmpty`, `DuplicateName` (binds the label being set, CFR-035), `DatabaseError`                                                                                                                                |
| `update_category` | `id: String, label: String` | `AssetCategory`      | `CategoryNotFound { id }`, `LabelEmpty`, `DuplicateName` (binds the label being set; two categories may share a label after a multi-device merge until one is renamed, CFR-035), `SystemReadonly`, `DatabaseError` |
| `delete_category` | `id: String`                | `()`                 | `CategoryNotFound { id }`, `SystemProtected`, `DatabaseError`                                                                                                                                                      |

### Archive / Delete (use cases)

> Both are cross-BC use cases that check asset existence + cash-asset guard, then check the account BC for holdings/transactions referencing the asset.

| Command         | Args         | Return | Errors                                                                                             |
| --------------- | ------------ | ------ | -------------------------------------------------------------------------------------------------- |
| `archive_asset` | `id: String` | `()`   | `AssetNotFound { id }`, `CashAssetNotEditable` (CSH-016), `ActiveHoldings` (OQ-6), `DatabaseError` |
| `delete_asset`  | `id: String` | `()`   | `AssetNotFound { id }`, `CashAssetNotEditable` (CSH-016), `ExistingTransactions`, `DatabaseError`  |

### Asset Prices

> All `date` / `original_date` / `new_date` arguments use ISO 8601 calendar format (e.g. `"2026-04-29"`), matching the `AssetPrice.date` shared-type convention.

| Command              | Args                                                                        | Return            | Errors                                                                                                                                                                                                                        |
| -------------------- | --------------------------------------------------------------------------- | ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `record_asset_price` | `asset_id: String, date: String, price: f64`                                | `()`              | `AssetNotFound { id }` (MKT-043), `Archived` (AST-006), `NotPositive` (MKT-021), `NonFinite` (MKT-021), `DateInFuture` (MKT-022), `InvalidDateFormat { date }`, `DatabaseError`                                               |
| `get_asset_prices`   | `asset_id: String`                                                          | `Vec<AssetPrice>` | `AssetNotFound { id }` (MKT-072), `DatabaseError`                                                                                                                                                                             |
| `update_asset_price` | `asset_id: String, original_date: String, new_date: String, new_price: f64` | `()`              | `AssetNotFound { id }` (AST-006), `Archived` (AST-006), `PriceNotFound { asset_id, date }` (MKT-083), `NotPositive` (MKT-082), `NonFinite` (MKT-082), `DateInFuture` (MKT-082), `InvalidDateFormat { date }`, `DatabaseError` |
| `delete_asset_price` | `asset_id: String, date: String`                                            | `()`              | `AssetNotFound { id }` (AST-006), `Archived` (AST-006), `PriceNotFound { asset_id, date }` (MKT-090), `DatabaseError`                                                                                                         |

### Asset Price Fetch Tasks

> `fetch_all_asset_prices` is the single BE entry point shared by auto-fetch on launch (MKT-121, MKT-122) and global refresh on the dashboard (MKT-130). Both commands are acknowledged synchronously (return `()` once dispatched); per-asset results are signaled asynchronously via `AssetPriceUpdated` (MKT-112). Per-asset failures during the fetch degrade silently per MKT-114; the in-flight guard (MKT-113) rejects concurrent calls across both commands. System cash assets are excluded from scope per MKT-116.
>
> A fetch task that priced at least one asset records the moment it ends in this installation's fetch log before it publishes `AssetPriceFetchCompleted` (MKT-201); a failed write never fails the task.
>
> Both commands are keyless (ADR-017): they fetch from Yahoo Finance with no API key and no fetch-mode argument. The former `use_api_key: bool` parameter was removed when the BYOK/Stooq path was retired.

| Command                      | Args                    | Return | Errors                                                                                                                                        |
| ---------------------------- | ----------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `fetch_all_asset_prices`     | `trigger: FetchTrigger` | `()`   | `FetchAlreadyRunning` (MKT-113), `NoFetchableHoldings` (MKT-111), `DatabaseError`, `UnknownError`                                             |
| `fetch_account_asset_prices` | `account_id: String`    | `()`   | `AccountNotFound { account_id }` (MKT-132), `FetchAlreadyRunning` (MKT-113), `NoFetchableHoldings` (MKT-111), `DatabaseError`, `UnknownError` |

### Price History Backfill

> `backfill_holding_price_history` is implemented in `use_cases/price_history_backfill/`. It fills the dates of one holding's held period (MKT-191) that carry no recorded price with the provider's daily closes, and never overwrites a recorded price (MKT-192). Every window is fetched before anything is written, so a provider failure writes nothing (MKT-195).

| Command                          | Args                                   | Return                        | Errors                                                                                                                                                                                                                                                                                                                                                                                                                              |
| -------------------------------- | -------------------------------------- | ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `backfill_holding_price_history` | `account_id: String, asset_id: String` | `PriceHistoryBackfillOutcome` | `AccountNotFound { account_id }` (MKT-196), `AssetNotFound { id }` (MKT-196), `CashAssetNotEditable` (MKT-116/196), `Archived` (AST-006), `AssetNeverHeld` (MKT-196 — no transaction on the asset; a closed holding is accepted), `PriceRefreshBlocked` (MKT-151/196), `TickerNotResolved` (MKT-110/196 — no symbol can be derived, or the provider reports it unknown), `ProviderUnreachable` (MKT-195), `DatabaseError` (MKT-195) |

### Price Freshness

> `get_price_freshness` is implemented in `use_cases/price_freshness/`. It reads the newest price date among the assets held in any account (MKT-200) and the moment this installation last fetched prices, kept outside the synced data (MKT-201).

| Command               | Args | Return           | Errors          |
| --------------------- | ---- | ---------------- | --------------- |
| `get_price_freshness` | —    | `PriceFreshness` | `DatabaseError` |

### Capabilities

> `get_capabilities` is implemented in `use_cases/capabilities/`. It returns the answer the composition root settled for the run (MKT-211): whether the build has an External provider. Infallible — the value exists before any window opens.
>
> In a build without one, `fetch_all_asset_prices`, `fetch_account_asset_prices` and `backfill_holding_price_history` are refused before any work is done (MKT-210): nothing is fetched or written, and the refusal carries no typed code, since no use case exists to answer. The interface never calls them there (MKT-212).

| Command            | Args | Return         | Errors                                           |
| ------------------ | ---- | -------------- | ------------------------------------------------ |
| `get_capabilities` | —    | `Capabilities` | _(none — infallible; settled at start, MKT-211)_ |

### Web Lookup

> `lookup_asset` is implemented in `use_cases/asset_web_lookup/` — it reads from an external web
> API and returns transient value objects; it does not persist anything. Owned here as the asset
> aggregate is the primary subject. The frontend selects which path to invoke via the explicit
> `mode` parameter (WEB-014); the backend does not infer it from the query shape.

| Command        | Args                              | Return                   | Errors                                                                                    |
| -------------- | --------------------------------- | ------------------------ | ----------------------------------------------------------------------------------------- |
| `lookup_asset` | `query: String, mode: LookupMode` | `Vec<AssetLookupResult>` | `InvalidIsinFormat` (WEB-016, WEB-025), `RateLimited` (WEB-025), `NetworkError` (WEB-025) |

---

## Shared Types

```rust
struct Asset {
    id: String,
    name: String,
    class: AssetClass,           // AST-003
    category: AssetCategory,     // nested category (id + name); read responses include the resolved category, not just the id. When the stored category stands removed on another device (a tombstone after a multi-device merge), the default category is resolved instead — derived on read, nothing rewritten (CFR-030)
    currency: String,            // ISO 4217 (TRX-021)
    risk_level: u8,              // 1..=5 (AST-002); DTOs accept `i32` input and the backend validates with `InvalidRiskLevel { received: i32 }`
    reference: String,           // ticker / freeform reference (mandatory — R1); for quoted assets carrying an ISIN, the ISO 6166 identity lives in `isin` (AST-023)
    isin: Option<String>,        // ISIN (ISO 6166, 12 chars, Luhn-validated — AST-023); absent for non-quoted or keyword-discovered assets (WEB-046)
    exchange: Option<Exchange>,  // canonical trading venue (AST-021); absent for legacy / non-listed assets
    is_archived: bool,           // R18
    price_refresh_blocked: bool, // MKT-150 — when true the asset is excluded from every price-fetch task (ADR-014); default false; independent of is_archived
}

struct AssetCategory {
    id: String,
    name: String,
}

// Canonical reference to a trading venue, independent of any market-data provider.
// AST Entity Definition. Provider symbols (Yahoo venue suffixes, OpenFIGI exchange
// codes) are resolved by per-provider mappers at the boundary, NOT stored here.
struct Exchange {
    code: String,                // ISO 10383 Market Identifier Code (MIC), e.g. "XPAR", "XNAS"
    label: String,               // human-readable display name, e.g. "Euronext Paris"
}

// Note on write DTOs: CreateAssetDTO and UpdateAssetDTO carry `category_id: String`
// (the FK), not the nested `AssetCategory`. The service resolves the id to the
// aggregate at write time and the read shape returns the resolved category.
```

```rust
// AssetClass variants (AST-003) — Derivatives added by WEB-023
enum AssetClass { Cash, Bonds, RealEstate, MutualFunds, ETF, Stocks, DigitalAsset, Derivatives }
// Derivatives maps from securityType: "Warrant" | "Option" | "Future" | "Rights" (WEB-023)
// default_risk for Derivatives = 5 (AST-003)
```

```rust
// Transient value object — not persisted (WEB-020)
// Fields marked "optional" may be absent per spec rules
struct AssetLookupResult {
    name: String,
    reference: Option<String>,       // ticker symbol; absent when OpenFIGI returns no ticker (WEB-046)
    isin: Option<String>,            // ISIN (ISO 6166); populated only on the ISIN path (WEB-046); absent on the keyword path
    currency: Option<String>,        // absent when OpenFIGI returns no currency (WEB-024)
    asset_class: Option<AssetClass>, // absent when securityType unrecognised (WEB-023)
    exchange: Option<Exchange>,      // canonical Exchange resolved via per-provider mapper (WEB-049); absent when the venue is not in the curated set
}

// Explicit lookup path selector (WEB-014).
// Isin → /v3/mapping with WEB-016 format validation before the HTTP call.
// Keyword → /v3/search → /v3/mapping with WEB-015 diacritics normalization.
enum LookupMode { Isin, Keyword }
```

```rust
// Input prices are transmitted as f64 decimal; backend converts to i64 micros at the IPC boundary (MKT-024).
// ADR-001 (i64 micros) applies to storage and read responses — f64 on write input is the intentional
// transport-layer exception; the f64 → i64 conversion inside the command handler is the ADR-001 compliance point.

struct AssetPrice {
    asset_id: String,            // asset this price belongs to
    date: String,                // ISO 8601 calendar date (e.g. "2026-04-29")
    price: i64,                  // market price in asset's native currency, i64 micros (ADR-001)
    source: AssetPriceSource,    // MKT-100 — provenance qualifier
}
```

```rust
// MKT-200/201 — the two figures behind the header's price item (MKT-202).
struct PriceFreshness {
    newest_price_date: Option<String>, // YYYY-MM-DD; None without a held, priced asset
    last_fetch_at: Option<String>,     // local wall-clock YYYY-MM-DDTHH:MM:SS; None if this device never fetched
}
```

```rust
struct Capabilities {
    external_provider: bool, // whether this build has an External provider (MKT-211); constant for the run
}
```

```rust
// AssetPriceSource variants (MKT-100) — keyless Yahoo Finance is the sole provider (ADR-017)
enum AssetPriceSource { Manual, YahooFinance }
// Manual:       every user-driven write — manual entry (MKT-020+), transaction
//               auto-record follow-up (MKT-050+), price-history edit (MKT-083+);
//               set by record_asset_price / update_asset_price per MKT-101.
// YahooFinance: fetch-task write (fetch_all_asset_prices, fetch_account_asset_prices)
//               per MKT-102, and price history backfill write
//               (backfill_holding_price_history) per MKT-192.
```

```rust
// PMV-010 — which action started a global fetch. The frontend states what the user did
// (a fact only it holds); the backend decides what that means — only `Manual` produces a
// movement report. `fetch_account_asset_prices` needs no trigger: it never reports movement.
enum FetchTrigger { Launch, Manual }
```

```rust
// PMV-020+ — what a manual global refresh did to the portfolio's value. Both readings are
// computed over the same holdings, quantities and rates, so only prices differ between them
// (PMV-020). Both readings use the rates in force when the refresh STARTED — the same refresh
// also fetches FX (FXR-075), and the later reading deliberately ignores what it obtained, so the
// reference-currency total will briefly differ from the dashboard's freshly converted one.
// Account values come from the application's own Global Value computation (PMV-023,
// account-contract AccountSummary.total_global_value) — never a second valuation path. Produced only when `trigger == Manual`; carried in AssetPriceFetchCompleted.
// Asset counts are NOT repeated here — the event's existing `ok` / `skipped` carry them.
struct PriceMovementReport {
    rows: Vec<PriceMovementRow>,          // one per account, by account name ascending (PMV-030/033)
    total_before: i64,                    // portfolio value before, reference-currency micros (PMV-040, GPF-011, ADR-001)
    total_after: i64,                     // portfolio value after, reference-currency micros (PMV-040)
    total_currency: String,               // the reference currency both totals are in (PMV-040) — on the
                                          // wire rather than assumed, matching AccountPerformanceResponse.currency
    total_movement_pct: Option<i64>,      // micro-percent from the two totals (PMV-041); absent when the
                                          // earlier total is not positive (PMV-044) or the two are equal (PMV-045)
    total_movement_amount: Option<i64>,   // total_after − total_before, reference-currency micros (PMV-046);
                                          // absent when the two are equal, present even when
                                          // total_before is not positive (PMV-044)
    observed_from: Option<String>,        // ISO date carried before the fetch (PMV-050); absent per PMV-052
    observed_to: Option<String>,          // ISO date this fetch produced (PMV-050); absent when the fetch
                                          // produced none LATER than observed_from (PMV-051) — note this says
                                          // nothing about whether values moved, which PMV-060 decides.
                                          // Independent of observed_from: when nothing was priced before
                                          // (observed_from absent, PMV-052) a date this fetch produced is
                                          // still carried here.
    incomplete: bool,                     // any row incomplete (PMV-043)
}

// PMV-030 — one account's share of the report. Present for every account, including those
// that did not move and those holding no priced asset.
struct PriceMovementRow {
    account_id: String,
    name: String,
    currency: String,                     // the account's own currency — both values are in it (PMV-034)
    before: i64,                          // account value before, account-currency micros (PMV-021)
    after: i64,                           // account value after, account-currency micros (PMV-022)
    movement_pct: Option<i64>,            // micro-percent (PMV-024); absent when unmoved (PMV-031)
                                          // or when `before` is not positive (PMV-025). `before` and `after`
                                          // are both on the wire, so "unmoved" stays distinguishable from
                                          // "undefined" without a discriminant.
    movement_amount: Option<i64>,         // after − before, account-currency micros (PMV-027); absent when
                                          // unmoved, present even when `before` is not positive
    incomplete: bool,                     // a holding that was MEANT to be read at its current price could
                                          // not be (PMV-032): the MKT-171 skip set, or one contributing 0 for
                                          // want of a usable rate (FXR-034/GPF). Deliberate exclusions never
                                          // set it — system cash (MKT-116) and refresh-locked holdings
                                          // (MKT-151), whose stale price is the point of the lock (MKT-158).
}
```

```rust
// MKT-170 — one entry per asset a fetch task could not price (the full MKT-114 skip
// set: no-data, fetch error, symbol-underivable, upsert failure). Carried in the
// AssetPriceFetchCompleted payload so the FE can present the manual-fill modal (MKT-172+).
// last_price / last_price_date are absent when the asset has never had a price recorded.
struct UnpricedAsset {
    asset_id: String,                 // target of the per-row record_asset_price call (MKT-175)
    name: String,                     // display name (MKT-174)
    reference: String,                // ticker (MKT-174)
    isin: Option<String>,             // ISIN when the asset has one (MKT-174)
    currency: String,                 // asset native currency — formatting + the manual write (MKT-023)
    last_price: Option<i64>,          // most recent recorded price, i64 micros (ADR-001); absent if never priced
    last_price_date: Option<String>,  // ISO date of last_price; absent if never priced
}
```

```rust
// MKT-194 — what a price history backfill recorded over the held period.
struct PriceHistoryBackfillOutcome {
    written: u32,                     // closes recorded on dates that had no price (MKT-192)
    already_priced: u32,              // closes the provider served for dates that already had one
}
```

---

## Events

| Event                      | Payload                                                                                          | Direction                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| -------------------------- | ------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `AssetUpdated`             | none                                                                                             | published — fired after any successful asset CRUD write or archive/unarchive/delete (R18, R23), including the price-refresh lock toggle `block_asset_price_refresh` / `unblock_asset_price_refresh` (MKT-156)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `AssetPriceUpdated`        | none                                                                                             | published — fired after successful `record_asset_price` (MKT-026), `update_asset_price` (MKT-085), `delete_asset_price` (MKT-091), or per-asset write success during a fetch task — `fetch_all_asset_prices` / `fetch_account_asset_prices` (MKT-112), or once by `backfill_holding_price_history` when it recorded at least one close (MKT-194). The transaction auto-record path (MKT-055/057) emits via the same `record_asset_price` call the FE issues after the transaction commits — no separate producer.                                                                                                                                                                                                                                                                                                                  |
| `AssetPriceFetchCompleted` | `{ ok: u32, skipped: u32, unpriced: Vec<UnpricedAsset>, movement: Option<PriceMovementReport> }` | published — once per fetch task after the per-asset loop (MKT-119), for every fetch path (`fetch_all_asset_prices` / `fetch_account_asset_prices`). `ok` = assets repriced, `skipped` = assets the fetch could not price; `unpriced` carries one `UnpricedAsset` per skipped asset (MKT-170/171) so the FE can open the manual-fill modal (MKT-172). `unpriced.len() == skipped`. `movement` is present only for a `fetch_all_asset_prices` call with `trigger == Manual` (PMV-010/011); it is absent on the launch auto-fetch and on every account-scoped fetch, and absent when the report could not be produced (PMV-014). A refresh rejected before any asset is attempted (`FetchAlreadyRunning`, `NoFetchableHoldings`) publishes **no** `AssetPriceFetchCompleted` at all, so the FE never faces an empty report (PMV-012). |
| `SyncCompleted`            | none                                                                                             | subscribed — the asset, category and account slices of the global store re-fetch once for everything a sync applied (SYN-064); applied assets, categories and prices raise no `AssetUpdated` / `AssetPriceUpdated` of their own                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `TransactionUpdated`       | none                                                                                             | subscribed — the header's price item reads again, since a transaction changes which assets are held (MKT-203); it also reads again on `AssetPriceUpdated`, `AssetPriceFetchCompleted` and `SyncCompleted`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |

---

## Changelog

- 2026-09-19 — Amended by `market-price` spec (MKT-200–203): new `get_price_freshness` command and `PriceFreshness` shared type. No event added; the header item re-reads on `AssetPriceFetchCompleted`, `AssetPriceUpdated`, `TransactionUpdated` and `SyncCompleted`.
- 2026-09-19 — SYN-064 amended: applied assets, categories and prices raise no event of their own; `SyncCompleted` is the re-fetch trigger for what a sync applied
- 2026-08-22 — Amended by `sync-conflict-resolution` spec: `Asset.category` resolves to the default category when the stored category stands removed (CFR-030); `update_asset.CategoryNotFound` and `update_category.DuplicateName` notes (CFR-030/035).
- 2026-05-30 — Added by `market-price` spec (price-refresh-lock amendment, ADR-014): `block_asset_price_refresh`, `unblock_asset_price_refresh`; `Asset.price_refresh_blocked` field; `AssetUpdated` note extended.
- 2026-06-12 — Amended by `api-key-management` spec (KEY-050–054, keyless fetch mode): `fetch_all_asset_prices` and `fetch_account_asset_prices` gain a `use_api_key: bool` arg carrying the device-local Stooq fetch-mode preference. No new types or errors.
- 2026-06-12 — Amended by `market-price` spec under ADR-017 (Yahoo Finance keyless price source): `fetch_all_asset_prices` and `fetch_account_asset_prices` drop the `use_api_key: bool` arg (BYOK retired); `AssetPriceSource` variant `Stooq` renamed to `YahooFinance`.
- 2026-06-16 — Amended by `market-price` spec (MKT-170+, unupdated-price manual fill): new `UnpricedAsset` shared type; `AssetPriceFetchCompleted` event registered with its `{ ok, skipped, unpriced }` payload (the `unpriced` list is the new part). No new command — per-row manual fill reuses `record_asset_price`.
- 2026-09-11 — Amended by `price-movement` spec (PMV): `fetch_all_asset_prices` gains a `trigger: FetchTrigger` arg so the backend knows which action started it (PMV-010); new `FetchTrigger`, `PriceMovementReport` and `PriceMovementRow` shared types; `AssetPriceFetchCompleted` payload gains `movement: Option<PriceMovementReport>`. No new command and no new error variant — a report that cannot be produced leaves the fetch's own outcome untouched (PMV-014). Both readings use the rates in force at refresh start, so the reference-currency total intentionally diverges from the freshly converted dashboard total (PMV-020, FXR-075).
- 2026-09-14 — Amended by `price-movement` spec (PMV-027, PMV-028, PMV-046): `PriceMovementRow.movement_amount` and `PriceMovementReport.total_movement_amount` carry the signed amount moved. No new command, type or error.
- 2026-09-15 — Amended by `market-price` spec (MKT-190–199): new `backfill_holding_price_history` command and `PriceHistoryBackfillOutcome` shared type; not subject to the fetch in-flight guard (MKT-199); `AssetPriceUpdated` also published by a backfill that recorded at least one close.
- 2026-09-23 — Amended by `market-price` spec (MKT-210–213): new `get_capabilities` command and `Capabilities` shared type. In a build without an External provider the fetch and backfill commands are refused before any work, with the runtime's untyped refusal; the interface never calls them (MKT-212).
