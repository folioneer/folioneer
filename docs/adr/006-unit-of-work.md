# ADR-006: Unit of Work Pattern for Cross-Aggregate Atomicity

## Status

Accepted

## Context

Some operations must write to more than one aggregate in a single atomic DB transaction.
A naive approach — injecting `sqlx::Pool` directly into use cases — violates B18 (use cases
must not depend on infrastructure types) and makes atomicity untestable (no way to simulate
a mid-transaction failure without a real DB).

The project uses DDD bounded contexts (`Account`, `Asset`) each with their own repository
traits and SQLite implementations. When a future use case requires writing to both contexts
atomically, a clean abstraction is needed.

**Note:** The original motivation was the `auto_record_price` feature (writing a transaction
and an asset price in one DB transaction). This requirement was removed — the frontend now
calls the two commands independently. UoW is implemented as a foundation for future use.

## Decision

Adopt the **Unit of Work pattern** via two abstractions:

### 1. `TransactionManager` — `core/uow.rs`

A shared application infrastructure trait. Lives in `core/` alongside `event_bus/` — same
role: cross-cutting application tool with no domain knowledge.

```rust
pub type UoWFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

pub trait TransactionManager: Send + Sync {
    async fn run<T, F>(&self, operation: F) -> Result<T>
    where
        F: for<'a> FnOnce(&'a mut dyn AppUnitOfWork) -> UoWFuture<'a, T> + Send;
}
```

`SqlxTransactionManager` implements this by beginning a `sqlx::Transaction`, running the
closure, committing on `Ok`, rolling back on `Err`.

`SqlxTransactionManager` is created once at startup in `lib.rs` and injected only into
use cases that require cross-aggregate atomicity.

### 2. `AppUnitOfWork` — `use_cases/{uc}/uow.rs`

A use-case-specific super-trait combining the repository traits needed for that operation.
Lives in the use case folder — it is operation-specific and not globally reusable.

```rust
// Example for a hypothetical cross-BC operation:
pub trait AppUnitOfWork: AccountRepository + AssetPriceRepository + Send {}
```

`SqlxUnitOfWork` in infrastructure implements all combined traits over a shared
`sqlx::Transaction<'_, Sqlite>`.

### Execution flow

```
use_case.execute()
  │
  ├── load aggregate (read, outside UoW)
  ├── call aggregate root method (pure domain, no DB)
  │
  └── tx_manager.run(|uow| {
          uow.save_account(&account)        // AccountRepository::save()
          uow.upsert_asset_price(...)       // AssetPriceRepository::upsert()
      })
      │
      ├── Ok  → commit → delegate event notification to BC service notify methods
      └── Err → rollback → propagate error
```

### Event emission

After `tx_manager.run()` returns `Ok`, the use case calls each BC service's notify method.
The use case MUST NOT publish events directly (B12) — it delegates to the service that owns
the event:

```rust
let result = tx_manager.run(|uow| { ... }).await?;
account_service.notify_updated();
asset_service.notify_updated();
```

## Alternatives Considered

**1. Inject `sqlx::Pool` into use cases directly**
Rejected — violates B18 (infrastructure dependency in application layer). Makes unit
testing impossible without a real database.

**2. Event-driven (eventual consistency)**
Rejected for cases where atomicity is required by spec. If a future operation explicitly
allows best-effort price recording, event-driven is the preferred approach — simpler and
more DDD-correct for that case.

**3. Single global `AppUnitOfWork` combining all repos**
Rejected — couples all bounded contexts together. Each use case defines only the repo
combination it needs.

## Consequences

**Positive:**

- Use cases have no sqlx dependency — fully testable with `MockTransactionManager`
- Atomicity failure can be simulated in tests via fault injection
- `TransactionManager` is reusable across any future cross-aggregate use case
- Commit/rollback logic centralized in one place

**Negative:**

- HRTB lifetime bound (`for<'a> FnOnce(...)`) adds Rust complexity
- Each cross-aggregate use case must define its own `AppUnitOfWork` super-trait
- `SqlxUnitOfWork` must implement all combined repo traits — SQL may be duplicated
  from existing repo implementations

## References

- `docs/ddd-reference.md` — Unit of Work section
- `docs/backend-rules.md` — B22
- `core/uow.rs` — implementation
