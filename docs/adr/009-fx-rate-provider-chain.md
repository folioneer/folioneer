# ADR 009 — FX rate provider chain: Frankfurter primary, ECB XML fallback, Manual override

**Date**: 2026-05-16
**Status**: Accepted

## Context

The asset price fetch ([ADR-017](017-yahoo-finance-keyless-price-source.md)) lets the system value each holding in its listing currency. To roll holdings up across currencies — first to the account's `report_currency`, then to the user's `dashboard_currency` for PFD — current FX rates are required. Today the system captures `exchange_rate` per transaction at trade time (good for cost basis), but has no representation of a current FX rate detached from a trade.

FX has stricter "what is the authoritative rate" semantics than equity prices: a foreign-exchange rate at portfolio-valuation time is reference data, not market data. A reasonable user expectation is that two consecutive launches of the app on the same day produce consistent valuations. That semantics drives the provider choice as much as availability does.

## Decision

Use a three-tier source chain for `CurrencyRate`, captured by a new `source: CurrencyRateSource` enum (`Manual | Frankfurter | Ecb`):

1. **Frankfurter API** (`https://api.frankfurter.dev/v1/latest?from=…&to=…`) — primary auto-fetch source. Open-source JSON wrapper around ECB daily reference rates. No API key.
2. **ECB XML feed direct** (`https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml`) — fallback when Frankfurter is unreachable. Same upstream data, different hosting.
3. **Manual** — user-entered rates via the FXR spec; always available.

Write semantics per `(from_currency, to_currency, date)`: latest write wins regardless of source (per ADR-012). The `source` enum is metadata for traceability and is not a precedence input. The `source` column is persisted as a SQLite text discriminant matching the enum variant name (consistent with ADR-008's `AssetPrice.source`).

Both External tiers use EUR as the base currency. Non-EUR pairs (e.g. USD/GBP) must therefore be computed via EUR cross-rate. The exact cross-rate algorithm and edge-case behaviour (same-date rate availability, rounding) will be specified in the future FXR spec; this ADR commits to the EUR-base architectural choice, not to the formula.

Alternatives considered:

- **Yahoo Finance currency quotes** (e.g. `EURUSD=X`) — rejected as fallback. Yahoo returns market spot rates, which disagree with ECB reference rates by 5–20 bps depending on volatility. Mixing the two sources across launches would surface as small, unexplained P&L drift to the user — exactly the consistency property we want to preserve. The 2026 Yahoo-blocking concerns documented in ADR-008 reinforce this rejection.
- **Open Exchange Rates / Fixer.io / ExchangeRate-API / Twelve Data FX** — all require API keys. Free tiers offer at most hourly granularity and EUR or USD base only. **None of these provide anything a daily-cadence portfolio valuation needs beyond what Frankfurter already gives for free**: ECB-reference rates are the right semantics, daily is the right cadence, EUR base with cross-rate computation covers every pair. Adding a keyed provider here would impose BYOK setup friction (per ADR-011) with zero functional gain.
- **ECB XML direct as primary** — rejected for now. The XML feed is reliable but parsing XML in the first implementation adds friction over Frankfurter's JSON. ECB-direct is retained as fallback so the data source is unchanged when Frankfurter is down.

## Consequences

- **Pros**: no API key on any tier; both External tiers share the same authoritative source, so values are consistent across fallback transitions and across consecutive launches; manual override respected; deterministic cross-rate math from a documented base; one fetch per pair per day suffices for a portfolio-valuation use case.
- **Cons**: EUR-base only — every non-EUR pair requires cross-rate computation, adding test cases; both tiers depend on ECB's ~16:00 CET publication, so early-morning launches operate on yesterday's rate (acceptable but worth surfacing as a staleness indicator in PFD); daily granularity only — disqualifies the system for trading or intraday use; if both Frankfurter and ECB are simultaneously unreachable (rare), the dashboard falls back to last cached rate or to Manual entry, never to a third-party best-effort rate.
