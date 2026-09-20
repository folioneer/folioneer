# Backend Rules

> For DDD concept definitions, see [docs/ddd-reference.md](ddd-reference.md).

> Rule numbers (B0, B1, …) are stable IDs — once assigned, they never change. New rules are appended; deprecated rules keep their number with a note.

## Folder Structure

**B0** — The backend source tree MUST follow this layout. Three layer-named folders (`application/`, `domain/`, `infrastructure/`) appear symmetrically inside `shared/` and every `context/{bc}/` — the DDD canonical trio is visible everywhere DDD layering applies.

```
src-tauri/src/
├── shared/                                 ← cross-cutting (was: core/ pre-v4.4)
│   ├── application/                        ← shared application types (reserved; empty in projects with no cross-BC application concern)
│   ├── domain/                             ← shared kernel (cross-BC domain — see ddd-reference.md § Shared Kernel)
│   │   └── *.rs                            ← cross-BC constants, IDs, value objects
│   └── infrastructure/                     ← shared concrete infra
│       ├── db.rs
│       ├── event_bus/
│       ├── logger.rs
│       ├── specta_builder.rs
│       └── uow.rs
│
├── context/{bc}/                           ← bounded contexts
│   ├── api.rs                              ← boundary (Tauri commands, single-BC scope)
│   ├── error.rs                            ← {BC}Error (one flat enum — every variant the BC can raise)
│   ├── application/                        ← Application layer
│   │   └── service.rs                      ← {BC}Service (orchestrates aggregate)
│   ├── domain/                             ← Domain layer (pure, no infra deps)
│   │   ├── {aggregate_root}.rs
│   │   └── {entity_or_vo}.rs
│   └── infrastructure/                     ← Infrastructure layer (was: repository/ pre-v4.4)
│       └── {aggregate}.rs                  ← repo impls today; future external/cache adapters as siblings
│
└── use_cases/{flow}/                       ← cross-BC orchestrators
    ├── api.rs                              ← cross-BC Tauri commands
    ├── orchestrator.rs
    └── error.rs                            ← {UseCase}Error composite (wraps BC enums + flat guards)
```

**B1** — `shared/` MUST only contain cross-cutting code (infrastructure utilities, shared application types, shared kernel domain) with no BC-specific knowledge.

**B2** — `context/{bc}/infrastructure/{aggregate}.rs` MUST only contain the database implementation of the repository trait declared in the same BC's `domain/{aggregate_root}.rs`. No business logic.

**B3** — `shared/infrastructure/specta_builder.rs` is the ONLY place where Tauri commands are registered.

**B4** — A bounded context MAY contain multiple aggregate roots. Each aggregate root lives as a file in `context/{bc}/domain/{aggregate_root}.rs`. Aggregates within the same BC reference each other by ID only — never by direct object reference.

**B38** — Layer folders symmetric. The three layer-named folders (`application/`, `domain/`, `infrastructure/`) MUST appear everywhere DDD layering applies — both inside each BC and inside `shared/`. The DDD canonical trio is visible at every level.

**B39** — `api.rs` MUST be a single file at the boundary (BC root, use-case root). Don't fold into a `presentation/` folder — the boundary surface (Tauri commands + DTOs + error mapping) is small enough that one file at the root is more discoverable than a folder.

**B40** — Infrastructure folder MUST be named `infrastructure/`, not `repository/`. `repository/` overpromises — it names one _type_ of infrastructure (repo impls) and forces awkward sibling folders the day a BC adds an external API client, cache adapter, or message-queue subscriber. `infrastructure/` is the layer name and accommodates all of those as flat siblings.

**B41** — Flat-first inside `infrastructure/`. Until a BC's infrastructure has 5+ files of distinct concerns, all files MUST sit flat at the root of `infrastructure/` (e.g. `infrastructure/{aggregate}.rs` for repo impls, `infrastructure/openfigi_client.rs` for an external client). Don't pre-create a `repository/` sub-folder for one repo impl. Nest only when the count grows.

**B42** — Top-level cross-cutting folder MUST be named `shared/`, not `core/`. `core/` overpromises (it implies "central business logic" but BCs ARE the business). `shared/` is direct, accurate, and DDD-agnostic for newcomers.

**B43** — Keep layer folders even when small or empty. `shared/application/` is typically empty in projects following the flat-`{BC}Error` model (per-BC translation lives in each BC's own `error.rs` — see [`error-model.md`](error-model.md)); keep the folder anyway. It documents the layering and reserves the spot for cross-BC application-layer growth (e.g. a future cross-cutting orchestration helper). The alternative is a deceptively flat `shared/` that hides the layer structure.

## Ubiquitous Language

**B5** — Domain vocabulary (entity names, aggregate method names, event names, domain concepts)
MUST be defined and validated by the user before use in code, tests, or documentation.
The agent MUST NOT unilaterally decide on domain terms — it MUST propose and wait for
explicit confirmation. All confirmed terms MUST be recorded in `docs/ubiquitous-language.md`
and used consistently everywhere.

**B6** — All new code MUST use the vocabulary confirmed in `docs/ubiquitous-language.md`.
If a confirmed term differs from the current code name (recorded as a code discrepancy in
the UL doc), new code uses the confirmed term and a rename of the existing code is scheduled.
The UL doc is the source of truth — not the current codebase.

## Domain Object

**B7** — Domain objects MUST be created with a factory method:

- `new()` — validates fields and generates id (use in service or use case)
- `with_id()` — validates fields, uses provided id (use in service, use case, or api)
- `restore()` — direct restore from database, no validation (use in repository only)

Exception: internal aggregate entities have factory methods that are called ONLY from within
the Aggregate Root's methods — never from services, use cases, or api.rs directly.

Immutable domain concepts with no identity SHOULD be modelled as Value Objects (no ID, no factory method — constructed directly).

## Aggregate

**B8** — The BC's root entity (named after the BC folder, e.g. `Order` in `context/order/`) is the Aggregate Root. External code MUST NOT mutate internal entities directly. Reading internal entities for query purposes is acceptable (CQRS-lite).

**B9** — All mutations to internal entities MUST go through the Aggregate Root methods or its BC Application Service. No external code constructs or mutates internal entities directly.

**B10** — One database transaction SHOULD modify at most one aggregate. Cross-aggregate writes require the UnitOfWork pattern.

**B11** — Aggregate Root methods MUST use domain/business vocabulary — they describe what
happens to the aggregate, not the internal mechanism.

> ✅ `root.perform_action()` — `root.cancel(reason)`
> ❌ `root.status = Status::Cancelled` — `root.with_status(...)`

**B12** — Boy scout rule: when a use case or service needs to mutate an aggregate field
directly, extract an Aggregate Root method for that mutation first, then call the method.
Never add a new direct field mutation to an aggregate from outside its own type.
Existing direct mutations are tracked in `docs/ubiquitous-language.md` as code discrepancies
and MUST be refactored incrementally.

**B37** — Aggregates own their state-dependent invariants. Before adding a `if loaded.state == X { return Err(...) }` check inside an application service, ask: "could this be enforced inside the aggregate or value-object that owns the state?" If yes, move it.

Service-layer state-dependent pre-checks are an "anemic domain" anti-pattern: the service ends up encoding rules about the aggregate's lifecycle, leaving the aggregate as a pure data carrier. Concrete forms that respect this rule:

- **State-mutating actions:** `aggregate.action_from(self, ...) -> Result<Self, {BC}Error>` consuming `self`, returning the new state for the caller to persist. The aggregate enforces its own invariants in the constructor of the result.
- **Pre-conditions for non-constructive ops** (e.g. delete): `aggregate.ensure_<predicate>(&self) -> Result<(), {BC}Error>`. The service calls it just before invoking the destructive action.

Service-layer checks are appropriate ONLY for cross-aggregate invariants (uniqueness across the BC) or application-layer concerns (`NotFound`, cross-BC preconditions). See the rejection-layer rule in `ddd-reference.md` § Errors for the disambiguation.

## Bounded Context (`/context`)

**B13** — MUST never import from another context.

**B14** — MUST share its external API directly through its main `mod.rs`.

- Outside the context, never import `crate::context::{domain}::domain::{Entity}` — always import `crate::context::{domain}::{Entity}`.

**B15** — SHOULD always publish a `{Domain}Updated` event when its state changes (create, update, delete, etc.). The BC Application Service (`service.rs`) is responsible for event emission. If no Application Service exists, the `api.rs` handler is responsible.

**B16** — `api.rs` is the framework boundary — the only layer that knows Tauri exists.
Its sole responsibilities are:

1. **Deserialize** — translate Tauri command arguments into domain types
2. **Delegate** — make exactly one call to its own BC Application Service
3. **Return** — propagate the typed `Result<T, {BC}Error>` directly (the BC enum, or the `{UseCase}Error` composite for cross-BC orchestrators, IS the FE-facing contract — see [`error-model.md`](error-model.md)); no mapper, no boundary type

It MUST only call the Application Service of its own bounded context.
It MUST NOT call another BC's service, another BC's repository, or a use case.
Cross-BC coordination belongs in a use case with its own `api.rs`.

**B17** — MUST declare its Tauri commands in the `api.rs` file.

## Use Cases (`/use_cases`)

**B18** — MAY import from contexts, MUST NOT import from another use case.

**B19** — MUST share its external API directly through its main `mod.rs`.

**B20** — MUST NOT publish a `{Domain}Updated` event directly (orchestrators do not own state).
For cross-aggregate UoW operations, MUST delegate notification to each BC service's notify
method after commit — the service owns the event, not the use case.

**B21** — MUST declare its Tauri commands in its own `api.rs` file. This `api.rs` follows
the same framework boundary role as for Bounded Context: deserialize → delegate to the use case orchestrator
→ return the typed composite. It MUST NOT contain coordination logic — that belongs in the orchestrator.

**B22** — SHOULD have an orchestrator as its main entry point (after api) that handles the global logic.

## Application Service (BC)

**B23** — A bounded context service (`service.rs`) is a BC-scoped Application Service. Its
primary role is to emit domain events after state changes: load via repository → call
Aggregate Root method → save → emit event. All domain logic (invariants, calculations,
state transitions) MUST live in the Aggregate Root — the service is a thin coordinator.
It MUST only exist when event emission or aggregate coordination adds value; trivial CRUD
with no event does not justify a service. A service MUST NOT expose repository types or
sqlx types in its public signature.

## Use Case Orchestrator

**B24** — Use cases MAY depend on any domain abstraction: repository traits, domain entities,
or bounded context services. They MUST NOT depend on infrastructure: concrete repository
implementations, `sqlx::Pool`, `sqlx::Transaction`, `sqlx::query!`, or any other sqlx type.

**B25** — For write operations that must emit an event, use cases SHOULD go through the BC Application Service rather than the repository trait directly to ensure the event is properly fired.

**B26** — For cross-aggregate writes (operations that must write to more than one aggregate
atomically), the use case orchestrator MUST use the UnitOfWork pattern (`TransactionManager`
from `shared/infrastructure/uow.rs`). Single-aggregate writes do NOT use UoW — the aggregate's own repository
handles atomicity internally via its `save()` method.

## Repository

**B27** — MUST use sqlx macros for queries. Use your project's DB reset command to wipe and re-migrate if needed.

## Logging

**B28** — MUST use `tracing::{info, debug, warn, error}` with structured fields. Never use `println!`.

**B29** — MUST use `target:` field when adding a new backend specific log.

**B30** — When using the `target:` field in tracing calls, MUST use a named constant instead of a string literal. Define `BACKEND` / `FRONTEND` constants in a shared `shared::infrastructure::logger` module and reference them:

```rust
// Define once in shared/infrastructure/logger.rs:
pub const BACKEND: &str = "backend";

// Use everywhere:
tracing::info!(target: BACKEND, field = value, "message");
```

## General

**B31** — Application services and use-case orchestrators MUST return typed `Result<T, {BC}Error>` (BC-scoped) or `Result<T, {UseCase}Error>` (cross-BC composite) per [`error-model.md`](error-model.md) — one flat enum per BC, plus use-case composites via `#[serde(untagged)]` + `#[from]` wrapping each BC enum and a tagged `{UseCase}Task` sub-enum that carries any use-case-specific codes (bare unit variants directly on the untagged composite serialize to `null` and collapse on the wire). Repositories MAY use `anyhow::Error` as their trait error type; the application layer translates infra failures to the BC's `{BC}Error::DatabaseError` variant at the call site, logging the diagnostic chain via `tracing::error!`. Tauri commands return the typed enum / composite directly — no `Result<T, String>` boundary translation, no `anyhow::Result<T>` on a wire-visible signature.

**B32** — MAY use `#[allow(clippy::too_many_arguments)]` on domain factory methods and production constructors (e.g. orchestrator or service new() with many injected dependencies). MUST NOT use on test helpers — use a builder struct instead.

## Tests

**B33** — Tests MUST NOT be trivial. A trivial test is one that verifies:

- A constructor does not panic
- An empty input returns empty output (no logic traversed)
- A getter returns what was just passed in
- A test helper disguised as a test

**B34** — Unit tests & mock

- Tests for services and orchestrators (inline #[cfg(test)] in src/) SHOULD mock external dependencies using mockall-generated mocks.
- Exception: tests for concrete repository implementations MUST use a real database (in-memory or isolated test instance) instead of mocks.

**B35** — Integration tests (tests/ folder) MUST use real database repos. They test cross-layer behavior end-to-end and MUST NOT use mocks.

**B36** — e2e tests MUST use an ephemeral database.
