# ADR 003 — Cross-Context Use Case Orchestration via Sequential Service Calls

**Date**: 2026-04-16
**Status**: Accepted — amended by ADR-005 (TransactionService added to account_details)

## Context

The `account_details` use case (introduced by ACD) requires data from three bounded contexts in a single backend call: holdings from `account/`, latest asset prices from `asset/`, and exchange rates from `transaction/`. Because bounded contexts must not import each other directly (B2), a use case in `use_cases/` must orchestrate these reads.

Three strategies were considered:

1. **Sequential service calls** — the use case injects each context's service and calls them in sequence, assembling the result in application code.
2. **Single SQL JOIN** — the use case writes a hand-crafted query joining tables across context schemas directly.
3. **Read model** — a dedicated repository owned by the use case layer wraps a custom query.

## Decision

Use **sequential service calls**. The use case injects the services it requires and calls them in order to assemble the response.

For `use_cases/account_details/`: injects `AccountService`, `AssetService`, and `TransactionService` (amended by ADR-005 — `TransactionService` was added to provide realized P&L aggregation per holding).

Rationale:

- Keeps each context's data access logic inside its own service, consistent with the existing architecture.
- The performance overhead of multiple queries is acceptable for a per-account detail view (bounded result set, not a list of thousands of rows).
- Avoids coupling the use case to raw SQL or to table schemas owned by other contexts.
- Easiest to test: each service can be mocked independently.

## Consequences

- **Pros**: clean context boundaries; each service remains the single source of truth for its data; straightforward to test with service mocks; no raw SQL in the use case layer.
- **Cons**: N+1 query risk if the pattern is applied naively to list views in the future (not a concern here given the single-account scope); slightly more round-trips than a JOIN.
