# ADR 001 — Use i64 for Monetary Amounts

**Date**: 2026-04-12
**Status**: Accepted

## Context

Existing financial values in the database (e.g., `average_price`, `quantity` in `asset_accounts`, `price` in `asset_prices`) are stored as `REAL` (floating-point numbers). Introducing a new `Transaction` entity requires precise storage for quantities, unit prices, fees, and total amounts to avoid floating-point inaccuracies inherent to `REAL` or `f64` types. This is critical for financial calculations, especially for long-term performance tracking and aggregation.

## Decision

All new financial amounts within the `Transaction` entity, and any future financial entities or fields, will be stored as `i64` (64-bit integers). A consistent micro-unit scale (e.g., multiplying by 1,000,000 for 6 decimal places of precision) will be applied consistently at the application boundary (e.g., frontend-to-backend and vice-versa) to convert between human-readable decimal values and the `i64` micro-unit representation. This approach ensures exact precision for all monetary calculations.

## Consequences

- **Pros**:
  - **Precision**: Eliminates floating-point inaccuracies, ensuring exact financial calculations.
  - **Consistency**: Establishes a clear standard for handling money within the application.
  - **Safety**: Reduces the risk of subtle bugs related to financial rounding or comparisons.
- **Cons**:
  - **Migration**: Existing `REAL` fields in `asset_accounts` and `asset_prices` will eventually need to be migrated to `i64` to maintain consistency across the entire financial domain. This is an anticipated future refactoring.
  - **Conversion Overhead**: Requires explicit conversion logic at the application layer when interacting with external systems or displaying values to users.
  - **Storage Size**: `i64` might use slightly more storage than `REAL` for very small numbers, but this is negligible for typical financial values and outweighed by precision benefits.
