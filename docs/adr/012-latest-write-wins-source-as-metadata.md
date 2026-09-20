# ADR 012 — Latest-write-wins for source-qualified entities; source field is metadata, not precedence

**Date**: 2026-05-16
**Status**: Accepted

## Context

`AssetPrice` and `CurrencyRate` carry a `source` field (entered by the user, or fetched from a provider). Two writers can therefore target the same `(business_key, date)`: the user and the automated fetch. A rule has to say which one stands.

A "manual entry overrides a fetched one" precedence rule protects one case — the user who overrides a fetched value that looks wrong — and gets the common cases wrong:

- **Onboarding** — a user types a last-known price when adding an asset; the next fetch should _replace_ that placeholder, not be blocked by it.
- **Backfill** — a user types a historical price for a date the provider does not cover; this is a different `(asset, date)` row and never conflicts with the fetch in the first place.
- **No-coverage assets** — the provider does not cover the asset; manual is the only source ever written; no conflict.
- **Trade-derived prices** (`record_price=true`) — the user just executed a real trade at that price; this is more current than any prior manual entry on the same day.

Manual entries in this application are usually placeholders or fills for what the fetch will eventually provide, not deliberate pins.

## Decision

For any source-qualified entity (`AssetPrice`, `CurrencyRate`, future entities of the same shape):

1. **Latest write wins per `(business_key, date)`**, regardless of source. Repository upserts unconditionally — no source-based skip, no precedence check at write time.
2. **The `source` enum field is metadata**: it lets the price-history UI render a per-row badge ("entered by you" vs fetched), supports debugging, and is available for future audit or filtering features. It does NOT influence which row wins on read or write.
3. **Repository read** returns the row at `(business_key, date)` via the primary-key lookup (no `ORDER BY source` expression). For "most recent value for an asset," reads pick the row with the latest `date` — date is the dimension that orders price history, not source.
4. **No "lock" / "pin" flag in v1**. If the rare "I want this Manual entry to survive auto-fetch" case becomes a recurring user pain, a future ADR can introduce an explicit pin mechanism. Until then, the user re-typing is the documented workflow.

Alternatives considered:

- **Manual wins** — rejected on the common-case argument above. It also costs a source-aware read query, a write-time existence check and a precedence test matrix.
- **A separate `pinned: bool` flag distinct from `source`** — rejected for v1 as YAGNI. It can be added later without breaking the simpler model defined here.
- **Latest-write-wins only between user writes; the fetch still blocked by a manual row** — rejected as a half-measure: two cases with subtle differences, the complexity of "manual wins" with a thinner rationale.

## Consequences

- **Pros**: the fetch is a single unconditional upsert; onboarding and "the fetch fills in what I placeholder-typed" work naturally; every entry persists at write time, so no "my new manual entry didn't persist" surprise; the `source` field still does real work (history badges, debugging); no precedence matrix to test.
- **Cons**: a user who intentionally overrode a fetched value must re-override after each subsequent fetch on the same date (typically once per day at most, often zero times because the fetch only writes today); when a pin mechanism is later needed, the data model gains a flag and the write path gains a check — contained and additive, not a model reversal.
