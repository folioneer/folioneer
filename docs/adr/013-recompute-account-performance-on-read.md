# ADR 013 — Recompute Account Performance on Read

**Date**: 2026-05-29
**Status**: Accepted

## Context

Account Performance (PRF) needs per-period historical values — the end-of-month or end-of-year value of an account plus its net-of-flows performance. These derive from two time-varying sources already in the system: the transaction history (which determines units held and the cash balance at any past date, via chronological replay) and the `AssetPrice` history (which determines each asset's price at any past date). Crucially, both can be **backdated** — MKT allows recording a price for a past date, and transactions can be created or edited with past dates — so any value persisted for an already-closed period can be invalidated after the fact.

## Decision

Recompute period values and performance metrics on demand in the account-performance use case; persist nothing. Alternatives considered: (a) a persisted period-snapshot table recomputed on every transaction or price mutation — rejected for the cache-invalidation burden (a single backdated price or transaction invalidates every later period) plus the schema and migration cost; (b) snapshots frozen at period close — rejected because backdated data silently makes frozen periods stale. Recompute-on-read matches the project's established model where holdings are always rebuilt by full chronological replay rather than cached, is correct by construction, and is fast at personal-portfolio scale (hundreds of transactions, tens of periods).

## Consequences

- **Pros**: no new table or migration; no cache-invalidation logic; always consistent with source data including backdated edits; aligns with the existing replay-everywhere architecture; the simplest correct implementation for v1.
- **Cons**: read cost scales with transaction count × number of periods (acceptable at personal scale, but not unbounded); the computation repeats on every page load with no cross-load memoization; if data volume grows or sub-daily granularity is later introduced, a cached or persisted approach may become necessary — re-incurring the invalidation work deferred here.
