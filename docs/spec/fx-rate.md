# Business Rules — Foreign Exchange Rate (FXR)

## Context

The Foreign Exchange Rate feature gives the system a representation of a **current currency-pair rate detached from any trade**, so a holding whose asset currency differs from its account currency can be **valued live** in the account's currency. Today the system records an `exchange_rate` per transaction at trade time (a cost-basis input, frozen forever), but has no current rate for valuation. As a result every read model guards on `asset.currency == account.currency` and treats a mismatched holding as unvaluable — unrealized P&L, performance %, total return show "—" and the holding contributes `0` to the account's Global Value (MKT-033/034/035, MKT-040, DIV-071, CSH-094, ACC-021, PRF-020/024). This feature lifts those guards by converting the holding's current market price into the account currency at valuation time.

A rate is recorded per **directed currency pair** (`from_currency → to_currency`) and is timestamped: multiple entries accumulate over time, one per pair per date. Rates can be entered manually and fetched from an external provider. The Account Details and Account Performance read models use the most recently dated rate to convert foreign-currency market prices into the account currency.

A currency pair is a **persisted, durable entity** once it exists. A pair starts being followed either automatically — the moment a live holding (`quantity > 0`) exists whose asset currency differs from its account currency — or by manual declaration in the Currency Rates view. Buying a USD asset inside a EUR account auto-creates and follows the `USD → EUR` pair (FXR-013). Once created a pair is **maintained even if nothing currently uses it** (e.g. the position later closes); it is not auto-removed. Pairs are few, so keeping them costs little; an explicit archive affordance is deferred to a later iteration.

In V1 a currency rate exists **only to value foreign holdings** — it is a side-effect of managing assets, not a thing the user manages for its own sake. Treating currency as a first-class subject (valuing foreign-currency cash balances, FX as a tradeable position, standalone currency tracking) is explicitly out of scope and deferred.

The valuation rate (current market price → account currency) and the per-transaction `exchange_rate` (cost / realized side) are **two distinct concepts and never interact**: recording or refreshing an FX rate never re-rates historical cost basis or already-realized P&L, and editing a transaction's frozen `exchange_rate` never touches a `CurrencyRate` row.

This feature is owned by a new `currency` bounded context. The cross-context valuation that consumes rates lives in the existing `use_cases/account_details/` and `use_cases/account_performance/` use cases (per ADR-003 / ADR-004: use cases inject services, not repositories).

Decisions inherited and applied without re-asking:

- **Provider chain** — Frankfurter primary → ECB XML feed fallback → Manual, all keyless, EUR-base External tiers ([ADR-009](../adr/009-fx-rate-provider-chain.md)).
- **Write semantics** — latest write wins per `(from_currency, to_currency, date)`, regardless of source; `source` is metadata, not a precedence input ([ADR-012](../adr/012-latest-write-wins-source-as-metadata.md)).
- **Storage** — all rates and monetary values are `i64` micro-units ([ADR-001](../adr/001-use-i64-for-monetary-amounts.md)).

---

## Entity Definition

### CurrencyPair

Represents a directed currency pair the system follows for valuation. A durable record: created on first demand or by manual declaration, and retained thereafter regardless of whether any holding currently needs it.

| Field           | Business meaning                                                                             |
| --------------- | -------------------------------------------------------------------------------------------- |
| `from_currency` | ISO 4217 code of the source currency of the pair (e.g. the asset's native currency `"USD"`). |
| `to_currency`   | ISO 4217 code of the target currency of the pair (e.g. an account currency `"EUR"`).         |

> The combination `(from_currency, to_currency)` is unique. The two currencies must differ (FXR-011). A pair owns zero or more `CurrencyRate` observations over time. There is no archive flag in V1 — every persisted pair is active (the archive affordance is deferred).

### CurrencyRate

Represents the value of one currency expressed in another on a specific date — the rate that converts an amount in `from_currency` into `to_currency`.

| Field           | Business meaning                                                                                                                                                                                                           |
| --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `from_currency` | ISO 4217 code of the source currency — the currency an amount is converted **from** (e.g. the asset's native currency `"USD"`).                                                                                            |
| `to_currency`   | ISO 4217 code of the target currency — the currency an amount is converted **into** (e.g. the account's currency `"EUR"`).                                                                                                 |
| `date`          | The calendar date this rate observation applies to (ISO 8601, e.g. `2026-05-16`). The user's local calendar date at write time.                                                                                            |
| `rate`          | How many units of `to_currency` one unit of `from_currency` is worth, as i64 micros (ADR-001). Example: 1 USD = 0.92 EUR is stored as `920000` for the pair `(USD, EUR)`.                                                  |
| `source`        | Provenance of this rate (see FXR-100 for variants). `Manual` for user-entered values; a provider name (`Frankfurter`, `Ecb`) for fetched values. Metadata for traceability; never a read/write precedence input (ADR-012). |

> The combination `(from_currency, to_currency, date)` is unique: only one rate per directed pair per day. Recording a second rate for the same key overwrites the first regardless of either row's `source` (FXR-025, per ADR-012). The reverse pair `(to_currency, from_currency, date)` is a **separate** row and is not derived automatically.

### HoldingDetail (amended)

This feature does not add fields to `HoldingDetail`; it changes the **conditions** under which the existing MKT/DIV fields are populated. `unrealized_pnl` (MKT-034), `performance_pct` (MKT-035), and `total_return_pct` (DIV-071) cease to be forced to `None` purely because the asset and account currencies differ — they become computable whenever a usable rate exists (FXR-031/032/033). When no usable rate exists, the existing `None` / "—" behaviour is preserved (FXR-034).

### AccountDetailsResponse (amended)

`total_unrealized_pnl` (MKT-040) and `total_global_value` (CSH-094) now include converted foreign-currency holdings instead of excluding them (FXR-040, FXR-041).

---

## Business Rules

### Eligibility and Initiation (010–019)

**FXR-010 — Rate direction semantics (backend)**: A `CurrencyRate` is directional. `rate` is the multiplier that converts an amount in `from_currency` into `to_currency`: `amount_to = amount_from × rate`. Valuation always resolves the pair `(asset_currency → account_currency)`; the inverse pair is never inferred from a stored row.

**FXR-011 — Identity rate is implicit (backend)**: When `from_currency` equals `to_currency` the rate is `1` by definition. The identity rate is never stored, never fetched, and never surfaced for manual entry; same-currency holdings continue to value with no conversion (the existing MKT-033 path).

**FXR-012 — Manual entry point (frontend)**: The user declares pairs and records rates manually from a dedicated **Currency Rates** view, and can also reach a pre-filled manual-entry form directly from a foreign-currency holding row's "—" placeholder in Account Details. The holding-row shortcut mutates only URL search params and is wired by a shell-mounted handler (the `AssetEditModalMount` pattern), so `account_details` does not import the `currency` feature. Manual entry is always available regardless of provider reachability or the auto-fetch setting (ADR-009 tier 3).

**FXR-013 — Auto-follow a pair from a foreign holding (backend)**: When an active (`quantity > 0`), non-cash holding exists whose `asset_currency` differs from its account currency, the directed pair `(asset_currency → account_currency)` (FXR-010) is ensured to exist as a persisted `CurrencyPair`. Ensuring a pair is idempotent — a pair already present is left untouched, and no duplicate is created. Recording the first foreign-currency buy in an account therefore makes the system start following that pair. Same-currency holdings (FXR-011) and system cash assets never create a pair.

**FXR-014 — Pairs persist once created (backend)**: A `CurrencyPair` is retained once created, whether it arose by auto-follow (FXR-013) or manual declaration (FXR-012). It is **not** removed when the holdings that demanded it close (`quantity → 0`) or are deleted; the pair and its recorded rates remain available. V1 provides no removal or archive of a pair; an archive affordance is deferred to a later iteration.

### Recording a Rate Manually (020–029)

**FXR-020 — Required fields (frontend)**: The manual rate form requires `from_currency`, `to_currency`, a non-empty `date`, and a non-empty `rate`. The submit action is disabled while any field is empty.

**FXR-021 — Rate validation (frontend + backend)**: A valid rate is strictly greater than zero. The backend rejects a rate of zero or below with a specific error. The frontend validates inline and disables submit until corrected.

**FXR-022 — Date validation (frontend + backend)**: A valid date is a well-formed ISO 8601 calendar date (`YYYY-MM-DD`) not in the future. Any past date is accepted (users may backdate historical rates). The backend rejects an invalid or future date with a specific error; the frontend validates inline.

**FXR-023 — Currency validation (frontend + backend)**: `from_currency` and `to_currency` must each be a well-formed ISO 4217 code and must differ from each other. A currency code that is not a recognised ISO 4217 code is rejected with a single specific error — malformed and unknown codes are **not** distinguished (mirroring the account domain's single currency-validation error, TRX-021). An identity pair (`from == to`, FXR-011) is rejected with its own distinct error. The frontend validates inline.

**FXR-024 — i64 storage (backend)**: The rate is stored as i64 micro-units per ADR-001. The frontend transmits the human-readable decimal; the backend converts to micros at the IPC boundary.

**FXR-025 — Upsert by (from, to, date) (backend)**: If a rate already exists for the same `(from_currency, to_currency, date)`, it is overwritten with the new value regardless of either row's `source` (per ADR-012; latest-write-wins). Otherwise a new record is created. Transparent to the user; the form behaves identically for new and existing entries. The `source` written by this path is governed by FXR-101.

**FXR-026 — CurrencyRateUpdated event (backend)**: After a successful upsert the backend publishes a bare `CurrencyRateUpdated` event on the event bus (no payload), consistent with `AssetPriceUpdated` and the other `*Updated` signals. It is published by the `currency` bounded context. The Tauri event discriminant string is `"CurrencyRateUpdated"`.

**FXR-027 — In-flight state (frontend)**: While the upsert is in progress the submit button is disabled and shows a spinner to prevent double-submission.

**FXR-028 — Success feedback (frontend)**: On success the form closes and a snackbar confirms the rate was recorded.

**FXR-029 — Error feedback (frontend)**: On validation failure or backend rejection the form stays open with an inline error adjacent to the invalid field; the user can correct and resubmit without reopening.

### Valuation Effect — Lifting the Currency-Mismatch Guards (030–039)

> These rules amend the cited MKT / DIV rules. Where a cited rule previously forced `None` / `0` purely on a currency mismatch, the amended behaviour applies the conversion below when a usable rate exists, and preserves the prior behaviour when none does.

**FXR-030 — Convert current price to account currency (backend)**: When an active non-cash holding's `asset_currency` differs from the account currency, `AccountDetailsUseCase` resolves the rate for `(asset_currency → account_currency)` per FXR-035 and computes the holding's current value in account currency as `current_price × rate`, using i128 intermediates scaled back to i64 (consistent with ACD-024). Same-currency holdings are unchanged (no conversion; MKT-033).

**FXR-031 — Unrealized P&L across currencies (backend)**: Amends MKT-034. When the currencies differ **and** a usable rate exists, `HoldingDetail.unrealized_pnl = (converted_current_value − average_price) × quantity`, where `average_price` is already in account currency (cost basis). The result is a number (including `0`), no longer forced to `None`. When no usable rate exists, FXR-034 applies.

**FXR-032 — Performance % across currencies (backend)**: Amends MKT-035. When `unrealized_pnl` is computable (FXR-031) and `cost_basis` is non-zero, `performance_pct = unrealized_pnl × 100 / cost_basis` as i64 micros, identical formula to MKT-035. `None` only when `cost_basis` is zero or no usable rate exists.

**FXR-033 — Total return % across currencies (backend)**: Amends DIV-071. When `unrealized_pnl` is computable (FXR-031), `total_return_pct = (unrealized_pnl + dividends_received) × 100 / cost_basis` as i64 micros, under the same null conditions as FXR-032. `dividends_received` is already in account currency (DIV-070) and needs no conversion.

**FXR-034 — No usable rate — preserve mismatch behaviour (frontend + backend)**: When the currencies differ and **no** usable rate exists for the pair (none recorded, none fetched, all fetch tiers failed), `unrealized_pnl`, `performance_pct`, and `total_return_pct` are `None` and the frontend shows "—", exactly as before this feature. The converted current value contributes `0` to account totals (FXR-040/041). This is the pre-FXR state, now reachable only on genuine rate absence rather than on every mismatch.

**FXR-035 — Rate resolution — latest on or before valuation date (backend)**: For a given pair and valuation date, the system uses the `CurrencyRate` with the greatest `date` that is less than or equal to the valuation date (mirrors the most-recently-dated price resolution of MKT-031). If only future-dated rates exist for the pair, none is usable. The chosen rate's date drives the staleness indicator (FXR-090).

**FXR-036 — Reactivity (frontend)**: The Account Details event subscription adds `CurrencyRateUpdated` alongside `TransactionUpdated`, `AssetUpdated`, and `AssetPriceUpdated` (ACD-039, MKT-036). On receipt the view re-fetches so newly recorded or fetched rates and all derived values are reflected immediately.

**FXR-037 — CurrencyRateUpdated event registration (frontend + backend)**: `CurrencyRateUpdated` is added to the event-bus enum, published exclusively by the `currency` bounded context. The global store treats it as a locally-handled event (no global re-fetch). `ARCHITECTURE.md` must register `CurrencyRateUpdated` in the event-bus table and document the new subscription.

### Account Summary and Totals (040–049)

**FXR-040 — Total unrealized P&L includes converted holdings (backend)**: Amends MKT-040. `AccountDetailsResponse.total_unrealized_pnl` sums `unrealized_pnl` across all active holdings for which a value is computable — now including foreign-currency holdings with a usable rate (FXR-031). Holdings with no usable rate (FXR-034) or no recorded price are still excluded. `None` only when no holding qualifies.

**FXR-041 — Global Value includes converted holdings (backend)**: Amends CSH-094 and ACC-021. In `total_global_value = cash_holding.quantity + Σ_h (h.quantity × latest_price(h))`, a foreign-currency non-cash holding now contributes its **converted** value `h.quantity × latest_price(h) × rate(asset→account)` (FXR-030) instead of `0`. A foreign-currency holding with no usable rate still contributes `0` (FXR-034). The same amendment applies to `get_account_summaries()` (ACC-021), keeping the dashboard and Account Details totals consistent.

**FXR-042 — Account performance period value uses conversion (backend)**: Amends PRF-024 (and PRF-020). When reconstructing a period's `end_value`, a non-cash holding whose currency differs from the account's is valued at `quantity × (most-recent price ≤ period end) × (most-recent rate ≤ period end)` instead of contributing `0`. The rate is resolved as of the period end per FXR-035. When no usable rate exists as of the period end, the holding contributes `0`, consistent with FXR-034. This removes the since-inception discrepancy noted as a known limitation in the PRF spec.

### Rate History (050–069)

> The Currency Rates view mirrors the asset-price history surface (MKT-070+): list, add, edit, delete on a dedicated page. The backend query feeds both the view and the valuation read path.

**FXR-050 — Rate history query (backend)**: The `currency` bounded context exposes a query returning the recorded `CurrencyRate` rows **for a given pair** (`from_currency`, `to_currency`), ordered by `date` descending. It returns a successful empty list when the pair has no rates (and for an unknown pair — never a not-found error). The top-level pair list (FXR-051) is fed by a separate query that returns every pair with its most-recent rate.

**FXR-051 — Currency Rates view is pair-centric (frontend)**: The dedicated Currency Rates view mirrors the Asset catalog: its top level lists every persisted `CurrencyPair` (FXR-013/014) — those auto-followed from holdings and those manually declared. Each pair row shows `from_currency → to_currency`, its most-recent rate, that rate's date and source badge (FXR-102), and a staleness hint (FXR-090); a pair with no rate yet shows "—". An "Add pair" affordance lets the user **declare a new pair** by choosing its from/to currencies (the asset-creation analog); the pair persists immediately, with or without a rate. Selecting a pair drills into that pair's dated rate history (the asset-price-history analog) via a scoped per-pair query (FXR-050) — only that pair's rates are listed, date descending — where an "Add rate" action opens the manual form (FXR-020–029) and per-date rows offer Edit (FXR-052) and Delete (FXR-053). An empty state invites declaring the first pair.

**FXR-052 — Edit a recorded rate (frontend + backend)**: Each row has an Edit action opening the manual form pre-filled with that row's pair, date, and rate. Submitting applies the same validation as FXR-021/022/023 and persists via the FXR-025 upsert (re-recording the same `(from, to, date)` is an in-place overwrite). On success the backend publishes `CurrencyRateUpdated` and the view refreshes. Editing a fetched row makes it `source = Manual` (FXR-101).

**FXR-053 — Delete a recorded rate (frontend + backend)**: Each row has a Delete action guarded by a confirmation dialog identifying the pair, date, and rate. On confirmation the backend removes the row and publishes `CurrencyRateUpdated`; valuation falls back to the next most-recent rate for the pair (FXR-035) or to "—" if none remains (FXR-034). On failure the view stays open with an error and the row is retained. Deleting a rate never removes the pair itself (FXR-014).

**FXR-054 — Declare a pair (frontend + backend)**: The "Add pair" affordance (FXR-051) persists a new `CurrencyPair` from the chosen `from_currency` / `to_currency`. The two codes must each be a valid ISO 4217 code and must differ (FXR-023/011); the backend rejects an invalid or identity pair, and a pair that already exists is returned idempotently rather than duplicated. A pair may be declared with no rate; its rate(s) are added afterwards (FXR-020–029).

**FXR-055 — Declare-pair form behaviour (frontend)**: The "Add pair" form requires both `from_currency` and `to_currency` to be chosen; the submit action is disabled while either is empty or while the two are equal (the identity-pair guard, FXR-011). On backend rejection (invalid code, identity pair) the form stays open with an inline error and the user can correct and resubmit, consistent with FXR-029. Declaring a pair that already exists succeeds idempotently (FXR-054): no error is shown and no duplicate row appears — the view simply shows the existing pair (selecting it) so the user can proceed to add a rate.

**FXR-056 — CurrencyPairUpdated event registration (frontend + backend)**: `CurrencyPairUpdated` is added to the event-bus enum, published by the `currency` bounded context whenever a pair is declared on this device (FXR-054). The Currency Rates view re-fetches its pair list on receipt; a pair applied or removed from another device reaches it through `SyncCompleted` (SYN-064). The global store treats it as a locally-handled event (no global re-fetch). `ARCHITECTURE.md` registers it in the event-bus table.

### Auto-Fetch from External Provider (070–089)

**FXR-070 — Provider chain (backend)**: Fetching a pair's current rate follows the ADR-009 chain: **Frankfurter** primary → **ECB XML feed** fallback (when Frankfurter is unreachable) → on total External failure, no row is written and the pair falls back to its last cached rate (FXR-035) or to Manual entry. No keyed or market-spot provider is ever consulted (ADR-009 rejected Yahoo/Stooq spot rates and BYOK providers).

**FXR-071 — Fetch scope is the persisted pair set (backend)**: A fetch task first ensures every active (`quantity > 0`), non-cash foreign-currency holding in its scope has a persisted pair (FXR-013), then fetches **all persisted `CurrencyPair`s**. Because pairs persist (FXR-014), a pair whose holding has closed is still refreshed, keeping its rate current for when it is needed again and for the Currency Rates view. Pairs are few by construction (one per distinct cross-currency relationship the user has ever held or declared), so refreshing all of them is cheap. Example: on a EUR account, buying a USD asset ensures `(USD → EUR)`; that pair keeps refreshing even after the position is sold.

**FXR-072 — Empty scope (backend)**: When no persisted pair exists (no foreign holding has ever been held and none declared), the FX portion of the fetch task has nothing to do; it makes no external calls and is not treated as an error. (The surrounding price-fetch task, FXR-075, follows its own MKT-111 empty-scope behaviour independently.)

**FXR-073 — Per-pair failure is skipped silently (backend)**: Within a fetch task, a pair whose rate cannot be obtained from any External tier is skipped (no row written, logged as a warning); the task continues with the remaining pairs. Valuation for the skipped pair degrades per FXR-034.

**FXR-074 — CurrencyRateUpdated on fetch success (backend)**: Every successful rate write produced by a fetch publishes `CurrencyRateUpdated` (FXR-026).

**FXR-075 — Fetch trigger — piggyback on price refresh (frontend + backend)**: FX rate fetching piggybacks on the existing asset-price fetch tasks — launch auto-fetch (MKT-121/122), global refresh (MKT-130), and account refresh (MKT-131/132). The same user action ("Refresh prices") that fetches asset prices for a scope also fetches the FX pairs needed to value that scope's holdings (FXR-071). No separate FX trigger or button is introduced. The launch fetch obeys the same auto-fetch setting (MKT-120); manual refreshes are fire-and-forget and surface feedback consistent with MKT-115.

**FXR-076 — In-flight guard shared with price fetch (backend)**: Because FX fetch runs inside the existing price-fetch tasks (FXR-075), it is covered by the single-fetch-at-a-time guard already defined in MKT-113 — no separate FX in-flight guard exists. A pair fetch that fails does not abort the surrounding price-fetch task; it degrades per FXR-073.

### Cross-Rate Computation (080–089)

**FXR-080 — EUR-base cross-rate formula (backend)**: External tiers publish EUR-base rates only (ADR-009). For a requested pair `(from → to)`, the system fetches the EUR legs and computes `rate(from → to) = rate(EUR → to) / rate(EUR → from)`, using i128 intermediates. Degenerate legs collapse correctly: when `from = EUR`, `rate(EUR → from) = 1`; when `to = EUR`, `rate(EUR → to) = 1`.

**FXR-081 — Same-date legs (backend)**: Both EUR legs of a cross-rate must come from the **same** ECB daily snapshot (same `date`). A single daily fetch provides both legs, so they always share a date; the computed `CurrencyRate` is stored at that date.

**FXR-082 — Rounding (backend)**: The cross-rate is computed as `rate(EUR→to)_micros × 1_000_000 / rate(EUR→from)_micros` with i128 intermediates and integer division (truncation toward zero, consistent with MKT-035). Applying a rate to a price (FXR-030) truncates once more. The compounded truncation is acceptable for daily portfolio valuation and is not corrected.

**FXR-083 — Missing leg makes a pair unfetchable (backend)**: If either EUR leg is absent from the External snapshot (a currency ECB does not publish), the pair cannot be computed and is skipped per FXR-073; the user may still enter it manually (FXR-012).

### Staleness Display (090–099)

**FXR-090 — Staleness indicator (frontend)**: Where a converted value is shown, the frontend can surface how stale the underlying rate is, derived from the chosen rate's `date` (FXR-035): "Rate as of today" when it equals the user's local date, "Rate Nd old" otherwise. ECB publishes ~16:00 CET, so an early-morning launch legitimately values on yesterday's rate (ADR-009 consequence); the indicator makes this visible rather than erroneous.

**FXR-091 — No-rate indication (frontend)**: When a foreign-currency holding has no usable rate (FXR-034), the affected columns show the same "—" as a holding with no recorded price; "no FX rate" and "no market price" are not distinguished in v1 (parallels MKT-032's deferred disambiguation). The staleness label (FXR-090) still communicates freshness when a rate does exist.

### Historical Rate Backfill (110–119)

Historical valuations (yearly/monthly performance, as-of-date views) need a rate **at each historical date**; the fetch paths above only ever write current rates, so an account whose history predates its pairs values foreign holdings at 0 in the past (FXR-034) and its performance percentages degrade to "—" (PRF-032). The backfill fills the historical rate series in one action.

**FXR-110 — Backfill action (frontend)**: The Currency Rates view offers a "Download rate history" action. While the download is in progress the action is disabled with a pending indicator; on success a snackbar reports how many rates were written and the view refreshes; on failure a snackbar reports the error.

**FXR-111 — Range anchor (backend)**: The backfill covers the full span any valuation can ask for: from the **earliest transaction date across all accounts** through today. When no transaction exists, or no pair is persisted, the action succeeds quietly with zero writes.

**FXR-112 — Dated daily series (backend)**: For every persisted pair (FXR-071 scope), the backfill records one dated rate per day the provider published, via the same dated-series semantics as the scheduled fetch (SPF-035–038): absent days write nothing, a pair the provider cannot serve is silently skipped (FXR-073), cross-rates compute per FXR-080–083, and rows carry the provider source (FXR-102).

**FXR-113 — Existing rows follow latest-write-wins (backend)**: Backfilled rows upsert by `(pair, date)` per ADR-012 — a manual rate previously entered on a covered date is overwritten, exactly as the current-rate fetch overwrites a same-day manual rate.

**FXR-114 — Total provider failure is surfaced (backend)**: Unlike the piggybacked fetch (FXR-073/SPF-039, silent), a user-triggered backfill whose provider is entirely unreachable is rejected with a specific error so the frontend can report it (FXR-110). Per-pair skips within a reachable provider remain silent.

### Source Field on CurrencyRate (100–109)

**FXR-100 — CurrencyRateSource enum (backend)**: `CurrencyRate.source` is of type `CurrencyRateSource` with variants `Manual | Frankfurter | Ecb` (ADR-009), persisted as a SQLite text discriminant matching the variant name (consistent with `AssetPrice.source`). Exposed on the frontend wire surface.

**FXR-101 — source: Manual on user-driven paths (backend)**: Every user-driven write (manual record FXR-025, any manual edit FXR-052) sets `source = Manual`. A fetched row edited by the user therefore becomes `Manual`. The frontend never passes a source value.

**FXR-102 — source: provider on fetched paths (backend)**: A write produced by the Frankfurter tier sets `source = Frankfurter`; a write produced by the ECB XML fallback sets `source = Ecb` (FXR-070).

---

## Workflow

```
Foreign-currency holding in an account
    → AccountDetailsUseCase builds each HoldingDetail
        if asset_currency == account_currency:
            value with no conversion                                   (MKT-033)
        else:
            resolve rate(asset_ccy → account_ccy), latest date ≤ today (FXR-035)
            if rate found:
                converted_value = current_price × rate                 (FXR-030)
                unrealized_pnl  = (converted_value − avg_price) × qty   (FXR-031)
                performance_pct, total_return_pct computed              (FXR-032, FXR-033)
                contributes converted value to total_global_value       (FXR-041)
            else:
                unrealized_pnl / performance_pct / total_return_pct = None,
                contributes 0 to totals, "—" shown                      (FXR-034)
    → row shows staleness "Rate as of today" / "Rate Nd old"           (FXR-090)

Manual rate entry
    → user opens the rate form, enters from/to/date/rate
    → submit → validate (rate>0, date≤today, valid distinct ISO codes) (FXR-021/022/023)
             → upsert (from,to,date), latest-write-wins, source=Manual (FXR-025/101)
             → publish CurrencyRateUpdated                              (FXR-026)
    → Account Details / Performance re-fetch                            (FXR-036)

Auto-fetch (trigger per Open Questions)
    → ensure a pair exists for each active foreign holding              (FXR-013)
    → scope = all persisted CurrencyPairs                               (FXR-071)
    → if none persisted: nothing to fetch (not an error)                (FXR-072)
    → for each pair:
        fetch EUR legs (Frankfurter → ECB XML)                          (FXR-070)
        rate(from→to) = rate(EUR→to) / rate(EUR→from)                   (FXR-080)
        on any-tier failure or missing leg: skip silently               (FXR-073/083)
        on success: upsert at snapshot date, source=Frankfurter|Ecb     (FXR-081/102)
                    publish CurrencyRateUpdated                         (FXR-074)
```

---

## UX Draft

### Entry Point

A dedicated **Currency Rates** view (FXR-051) lets the user view, add, edit, and delete rates, mirroring the asset-price history surface. The conversion itself is invisible — foreign-currency holdings simply start showing real numbers in Account Details instead of "—". A foreign-currency holding row's "—" is also a shortcut into a pre-filled manual-entry form (FXR-012). FX rates refresh transparently whenever the user refreshes prices (FXR-075); no separate FX refresh button exists.

### Main Component

The Currency Rates view (a page), structured like the Asset catalog: a list of declared/demanded pairs at the top level, an "Add pair" action to declare a new pair, and a drill-in per pair showing that pair's dated rate history with per-row Edit/Delete. The add/edit form is a small modal (from-currency selector, to-currency selector, date, rate, with the target-currency label shown).

### States

- **Empty**: foreign-currency holdings show "—" until a rate exists (FXR-034); a hint points to recording or fetching a rate.
- **Loading**: spinner while a fetch is acknowledged (consistent with MKT-133).
- **Error**: inline validation errors on the manual form (FXR-029); a concurrent-fetch rejection surfaces via snackbar (FXR-076, MKT-115 style).
- **Success**: foreign-currency holdings show converted P&L, performance %, total return, and contribute to Global Value; a staleness label indicates the rate's age (FXR-090).

### User Flow

1. User opens an account holding a USD asset under a EUR account; the row currently shows "—" for P&L.
2. User records (or refreshes) the `USD → EUR` rate.
3. Backend stores the rate and publishes `CurrencyRateUpdated`.
4. Account Details re-fetches; the row now shows unrealized P&L, performance %, and total return in EUR, and the account's Global Value includes the holding.
5. A "Rate as of today / Nd old" label communicates the rate's freshness.

---

## Open Questions

- [x] **v1 UI scope** — resolved: FX mirrors the asset-price model — auto-fetch (same mechanism, piggybacked) + manual management on a dedicated page with full add/edit/delete (FXR-050–053).
- [x] **Fetch trigger** — resolved: FX fetch piggybacks on the existing price-refresh tasks and shares MKT-113's in-flight guard (FXR-075/076).
- [x] **Manual entry point** — resolved: a dedicated Currency Rates view plus a shortcut from a foreign-currency holding row's "—" via the shell URL-modal mount (FXR-012).
- [x] **No-rate vs no-price diagnostic** — resolved: merged into the existing "—" for v1, parallel to MKT-032's deferred disambiguation (FXR-091).

### Deferred

- **Direct currency management** — V1 scopes currency rates strictly as a side-effect of valuing foreign holdings. Valuing foreign-currency cash balances, treating FX as a tradeable position, and standalone currency tracking are out of scope; a future iteration may promote currency to a first-class managed subject.
- **Pair archive / removal** — V1 persists every pair and never removes one (FXR-014). When the pair list grows unwieldy or a pair is no longer wanted, an explicit archive (or delete) affordance can be added — analogous to asset archival (AST). The `CurrencyPair` record can gain an `is_archived` flag at that point without a model change.
- **No-rate vs no-price disambiguation (FXR-091)** — distinguishing "no FX rate" from "no market price" in the holding row is deferred until a user-pain signal warrants the extra typed diagnostic state, consistent with MKT-032's own deferral.

None — all questions have been resolved.
