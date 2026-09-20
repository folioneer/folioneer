# ADR 014 — Per-asset price-refresh lock via fetch-scope exclusion

**Date**: 2026-05-30
**Status**: Accepted

## Context

[ADR-012](012-latest-write-wins-source-as-metadata.md) established latest-write-wins for source-qualified entities: the repository upserts unconditionally, `source` is metadata only, and there is no write-time precedence check. Its decision point 4 deliberately shipped **no "lock"/"pin" flag in v1**, stating that "if the rare 'I want this Manual entry to survive auto-fetch' case becomes a recurring user pain, a future ADR can introduce an explicit pin mechanism." It also pre-considered a `pinned: bool` flag and deferred it as v1 YAGNI, noting it "can be added later without breaking the simpler model."

That pain has now surfaced from a concrete case. For the Amundi MSCI World ETF (`DCAM.PA`), the provider's end-of-day feed serves a close of `6.00` while the official Euronext closing-auction price is `5.993` (a continuous-session last-trade vs the 17:35 auction print — a per-symbol data-quality gap). Under ADR-012, every refresh re-clobbers the user's manual `5.993` correction with the fetched `6.00`, because the manual entry and the fetch land on the same `(asset_id, date)` row and the latest write wins. Re-typing after every refresh — ADR-012's documented workaround — is the recurring pain its decision point 4 anticipated.

## Decision

Add a persisted boolean `price_refresh_blocked` on the `Asset` aggregate. When it is true, the asset is **excluded from fetch scope** in every fetch task (launch, global refresh, account refresh) when the task builds its candidate set — the same kind of exclusion already applied to system cash assets (MKT-116). For a locked asset no provider symbol is derived, no provider call is made, and no `AssetPrice` row is written. The lock governs the asset everywhere it is held (spec rules MKT-150–158).

The defining property: this is achieved **upstream of the write path, by scope exclusion — not by reintroducing a write-time source-precedence check.** ADR-012's decision points 1–3 (blind unconditional upsert, `source` as metadata, date-ordered read) remain untouched. Because the locked asset never enters scope, there is simply no fetch write to lose; the pin needs no precedence logic at the repository. This is the additive, contained change ADR-012's Consequences section foresaw — and it is even narrower than predicted, since the write path itself is unchanged and only scope-building gains a skip.

This ADR **fulfills** ADR-012 decision point 4; it does **not** supersede it. Latest-write-wins remains the default for every unlocked asset.

The lock is asset-level. `AssetPrice` rows are keyed by `asset_id` alone and are shared across every account holding the asset, so a per-holding lock would be incoherent — a refresh from another account would still overwrite the shared price.

Alternatives considered:

- **Write-time precedence ("Manual wins")** — rejected. This is the rule ADR-012 rejected; it brings back the source-aware read query, the write-time existence check, and the precedence test matrix that ADR-012 eliminated. Scope exclusion gives the user-facing pin without any of that complexity.
- **Per-holding lock** — rejected. Prices are per-asset; a per-holding flag would not actually protect the shared price from another account's refresh.
- **Do nothing (ADR-012 status quo: re-type each day)** — rejected now that the recurring-pain precondition ADR-012 set for revisiting has been met.

## Consequences

- **Pros**: a manual correction on a locked asset survives every refresh with zero re-typing; ADR-012's simpler write/read model is fully preserved (no precedence check, no source-aware ordering); the mechanism reuses the existing MKT-116 scope-exclusion shape, so the fetch pipeline gains one predicate, not a new code path; the flag is additive and reversible by an unlock.
- **Cons**: a locked asset's price goes stale silently — its staleness label keeps aging and the user must remember to unlock it when they again want live updates; the lock is asset-wide, so a user holding the same asset in multiple accounts cannot lock it in one and fetch it in another (an accepted limit given the per-asset price model); adds one persisted column and a small surface (two commands + a row toggle) to maintain.
