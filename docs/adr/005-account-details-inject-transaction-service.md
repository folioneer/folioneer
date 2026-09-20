# ADR 005 — Inject TransactionService into account_details Use Case for Realized P&L

**Date**: 2026-04-19
**Status**: Accepted — amends ADR-003

## Context

ADR-003 established that `use_cases/account_details/` injects `AccountService` and `AssetService` only, explicitly excluding `TransactionService` on the grounds that "holdings already carry the pre-computed VWAP `average_price`; no raw transaction data is needed."

The SEL spec introduces `realized_pnl` — a value stored per sell transaction in `context/transaction/`. The Account Details view must display cumulative realized P&L per holding (sum of `realized_pnl` for all sell transactions in a given `(account_id, asset_id)` pair). This data lives in the `transaction/` bounded context and is not available through `AccountService` or `AssetService`.

## Decision

Inject `TransactionService` into `use_cases/account_details/` alongside the existing `AccountService` and `AssetService`. The use case calls `TransactionService` to fetch the summed realized P&L per asset for the account, then enriches the `AccountDetailsResponse` with a `realized_pnl` field per holding.

The sequential service call pattern from ADR-003 is preserved. This is an extension of the service set, not a replacement of the orchestration strategy.

## Consequences

- **Pros**: Account Details remains the single cross-context use case that assembles all holding-level data; no new use case or data denormalization required; consistent with ADR-004 (services only, no repository access).
- **Cons**: `account_details` now depends on three services instead of two; one additional query per account detail load.
