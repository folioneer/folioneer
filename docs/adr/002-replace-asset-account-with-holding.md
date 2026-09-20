# ADR 002 — Replace `AssetAccount` with `Holding`

**Date**: 2026-04-12
**Status**: Accepted

## Context

The `AssetAccount` entity in `context/account/` was introduced as a junction table linking an account to an asset. It carries `quantity` (REAL) and `average_price` (REAL) fields representing the current state of a position. However:

- The name `AssetAccount` is a technical artefact (join table naming), not a domain concept.
- `REAL` storage for financial quantities violates the precision standard established by ADR-001.
- The introduction of the `Transaction` feature makes `AssetAccount` the target of computed state (VWAP, quantity aggregation), which demands a clearer domain identity.

`AssetAccount` is currently not used by any production feature (no frontend reads or writes target it directly).

## Decision

Replace the `AssetAccount` entity and its underlying `asset_accounts` table with a new `Holding` entity, owned by the `account/` bounded context.

- `Holding` represents the current state of a financial position: an asset held within an account.
- All financial fields (`quantity`, `average_price`) are stored as `i64` micro-units per ADR-001.
- The `asset_accounts` table and its `REAL`-typed columns are dropped via a schema migration.
- The `Holding` entity uses the same factory-method pattern (`new`, `update_from`, `from_storage`) as all other domain entities.

## Consequences

- **Schema migration**: Drop `asset_accounts`, create `holdings` with `i64` fields.
- **No data loss**: The `asset_accounts` table exists in schema but is never written to by any active user-facing feature. All `AssetAccount`-related gateway methods and hooks are tagged `TODO(R17)` and no UI component calls them. The `account_asset_details` feature exists as scaffolding but its data-fetching hook is entirely commented out.
- **Code removal**: The `AssetAccount` entity, `AssetAccountRepository` trait, `SqliteAssetAccountRepository`, service methods, 3 Tauri commands, and the `account_asset_details` placeholder feature are all deleted as part of this migration.
- **Domain clarity**: `Holding` maps directly to the business concept "a position held in an account."
- **ADR-001 alignment**: `holdings` becomes the first table in the `account/` context fully compliant with the i64 micro-unit standard.
- **Breaking change scope**: Internal only — no public API surface is affected.
