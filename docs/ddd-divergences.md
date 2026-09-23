# DDD Divergences

Catalog of intentional divergences from textbook DDD. We follow the structural pillars strictly (aggregate roots, factory/aggregate-method conventions, layered errors, ubiquitous language, repository pattern, application services, use-case orchestrators, BC isolation at the module level). This doc captures the **purity-tax** patterns we deliberately don't implement, with the trade we accepted.

Read this when:

- A reviewer flags a divergence as a missing pattern — verify it's listed here, then move on.
- You're adding a new feature and unsure whether to introduce a textbook pattern (typed IDs, DTOs, ACL) — check whether the trade reasoning still holds.
- The constraint that justified a divergence changes (e.g. we add a non-Tauri consumer) — re-litigate that entry, not the whole catalog.

---

## 1. IDs are `String`, not value-object wrappers

**Pattern**: Identity is a value object — `struct AccountId(uuid::Uuid)`. The type system prevents passing an `AssetId` where an `AccountId` is expected.

**Practice**: `id: String` everywhere. Tauri commands take and return strings.

**Trade**: Tauri's wire model is string-based to TypeScript; round-tripping through typed wrappers costs ceremony at every IPC boundary. Type confusion is caught fast in tests because the wrong-context lookup returns `None`. Going pure means ~100+ touch points and ergonomic friction at every command site for marginal real-world safety.

**When to revisit**: If we ever ship a non-Tauri consumer (CLI, server, library), wrapper types become cheaper and prevent a real class of cross-context bugs.

---

## 2. Currency / amount / date are primitives, not value objects

**Pattern**: `Currency`, `Money`, `MicroAmount`, `IsoDate` as immutable VOs. Construction validates; the type prevents mixing units.

**Practice**: `currency: String` (validated at the factory — TRX-021), `amount_micros: i64` (no wrapper), `date: String` (parsed at the factory — TRX-046, MKT-024). Validation lives in the aggregate constructor.

**Trade**: Specta wants flat wire types — wrapping `i64` in `MicroAmount` either generates an opaque TS branded type (annoying on the FE) or unwraps to plain `number` (no real type safety). TypeScript has no date primitive. Partial protection via factory validation is enough for our domain complexity.

**When to revisit**: If we add a second currency-conversion path beyond the existing exchange-rate field, or if mixing micro-units across currencies becomes a recurring bug class.

---

## 3. Domain types serialize to the FE directly

**Pattern**: Domain entities live inside the BC. The API layer translates to/from DTOs. Domain has zero `serde` derives.

**Practice**: `Account`, `Asset`, `Transaction` derive `Serialize + Deserialize + specta::Type` and ship over the wire as-is. No parallel DTO layer.

**Trade**: Hand-maintaining a parallel DTO type for every domain entity (and a mapper for each) drifts the moment a field changes. Specta-generated bindings collapse this duplication. Cost: any field added to a domain entity is FE-visible immediately, even if it shouldn't be — the discipline lives in code review, not the type system.

**When to revisit**: If we ship a non-Tauri consumer that needs different field visibility, or if FE-visible-by-default starts causing privacy regressions.

---

## 4. Domain events fire at the service layer, not from inside the aggregate

**Pattern**: `Account::buy_holding(...)` raises a domain event into a `pending_events` list; the application service dispatches them after persistence.

**Practice**: `Account` accumulates `pending_changes: Vec<AccountChange>` (which the repo applies on save). The actual `Event::AccountUpdated` is emitted by the service after `save_account` succeeds.

**Trade**: Tauri's event bus is a service-layer concern (it bridges to the FE). Routing events through a `pending_events` list inside the aggregate would just add a re-emit step. Current shape separates concerns by audience: pending changes for the **repo** (persistence), service emits events for the **FE** (notification).

**When to revisit**: If we add domain-internal subscribers (e.g. a domain policy reacting to `BuyHoldingApplied` before persistence), the event must originate inside the aggregate.

---

## 5. Write commands return data (no CQS separation)

**Pattern**: Write commands return void or just an ID. Reads return DTOs. Separation of read and write models.

**Practice**: `buy_holding` returns the persisted `Transaction`. `add_account` returns the new `Account`. Reads return domain types directly (with the Specta serialization noted in #3).

**Trade**: The FE almost always needs the result of a write to update local state (the new transaction's ID, computed `total_amount`, etc.). Round-tripping with a separate read after every write is a wasted query. Single-user desktop app — no scaling pressure to separate read/write models.

**When to revisit**: If we ever need a separate read model (materialized view, denormalized aggregate, search index), CQS becomes worth it for the affected slice.

---

## 6. Sub-aggregate repositories (Holding, Transaction)

**Pattern**: One repository per aggregate root. The aggregate's children are accessed only through the root — `account.holdings`, `account.transactions`. The repo doesn't expose them independently.

**Practice**: `SqliteHoldingRepository` and `SqliteTransactionRepository` are separate from `SqliteAccountRepository`. `get_transactions(account_id, asset_id)` queries them directly. Writes still go through the Account aggregate (which produces `pending_changes` applied across all three repos atomically via the Unit of Work).

**Trade**: Loading the entire Account aggregate to display a transaction list would mean materializing every transaction and holding for every read. For large accounts (years of trades) that's hundreds of rows on every UI render. The sub-repos are a **read optimization**; write integrity stays at the aggregate.

**When to revisit**: Probably never — the read-path performance argument compounds as accounts age. The risk to watch: a write-path that bypasses the aggregate by using the sub-repo directly. Reviewer-arch should flag any direct sub-repo write.

---

## 7. Bounded contexts are logical, not physical

**Pattern**: Each BC is its own deployable, often its own database, communicating via published events. Strong runtime isolation.

**Practice**: Single Tauri binary, single SQLite database, foreign keys spanning BCs (`Asset.category_id` → `AssetCategory.id`, `Transaction.asset_id` → `Asset.id`). The BC boundary is enforced only at the Rust module level (`context/account/` does not import from `context/asset/`).

**Trade**: Desktop app, single user, single process. Splitting into separate processes / DBs would multiply complexity for zero user-visible benefit. Logical BC separation keeps modules independent and testable; that's enough.

**When to revisit**: Never for this product. If we ever extract a BC into a service (e.g. a portfolio analytics backend), the boundary is already drawn — the migration is mechanical.

---

## 8. No anti-corruption layer for cross-BC use cases

**Pattern**: When one BC consumes another's model, an anti-corruption layer (ACL) translates the foreign types so the consumer doesn't depend on them.

**Practice**: `delete_asset` use case directly imports `AccountService::transaction_count_for_asset(...)` and `AccountError`. No translation layer.

**Trade**: Both BCs are first-party, evolving together in the same monorepo. An ACL would protect against the foreign BC changing its types — but we control both. If `AccountService` changes, the use case changes. Pure ceremony otherwise.

**When to revisit**: If a BC ever becomes externally maintained (third-party plugin, separate team's library), the ACL becomes load-bearing.

---

## 9. No domain services (BC-internal, non-aggregate)

**Pattern**: Logic that doesn't fit on a single aggregate (multi-aggregate calculation, stateless policy) lives in a `domain/services/` namespace.

**Practice**: Such logic goes to application services or use-case orchestrators. We don't have a `domain/services/` folder.

**Trade**: Most cross-aggregate logic IS use-case-shaped (queries multiple repos, decides). Stateless domain rules are rare in our domain — when they show up (e.g. cash replay computation), they live as instance methods on the aggregate (`Account::replay_cash_holding`). A `domain/services/` folder with no clear residents is worse than not having it.

**When to revisit**: If a logic chunk repeatedly fails the "is this an instance method on an aggregate?" question and ends up smeared across the application service, that's the signal to extract a domain service.

---

## 10. `anyhow::Error` in repo trait error type

**Pattern**: Domain-owned trait should not reference `anyhow` (an infra-flavored crate).

**Practice**: `type Error = anyhow::Error;` on every repo trait.

**Trade**: Typing the repo error opaquely (`enum { Storage(Box<dyn Error>) }`) gives nothing — anyhow with extra steps. Typing it semantically (`UniqueViolation`, `ForeignKeyViolation`) IS valuable but is a real refactor (~400 LOC, plus migration verification, plus dropping service-level pre-checks). Status quo is a documented small leak; the application layer translates infra failures to per-BC `*ApplicationError::DatabaseError` (per `docs/error-model.md`), so the FE wire surface is unaffected.

**When to revisit**: If race conditions on uniqueness pre-checks become observable (two simultaneous `add_account` with the same name → one returns `DatabaseError` instead of `NameAlreadyExists`), or if we want to drop FK violation handling onto DB constraints.

---

## 11. Aggregate methods returning `anyhow::Result` in the account BC

**Pattern**: Aggregate methods raise typed domain errors that the application service consumes directly.

**Practice**: Several aggregate methods (buy/sell/correct/cancel/open_holding) return `anyhow::Result` and box `AccountError` values. Service-layer bridges (`to_holding_tx_error`, `to_open_holding_error` in `context/account/service.rs`) downcast the `AccountError` out and translate the rest to `DatabaseError`.

**Trade**: Splitting these aggregate methods into typed factory + apply pairs (so they return `Result<_, AccountError>` directly) is a real refactor across the account BC. The error-enum collapse into a single flat `AccountError` is in place; this aggregate-method retrofit is the remaining step. The bridges are intentional until it happens.

**When to revisit**: Now that the flat `AccountError` has landed, fold the aggregate methods into typed `Result<_, AccountError>` and delete the bridges (`to_holding_tx_error`, `to_open_holding_error`). This is also the prerequisite for the `replay_cash_holding` factory-call fix tracked in `docs/techdebt.md` (2026-06-20), whose `Holding::new`/`with_id` calls need typed `AccountError` rather than `anyhow::Result`.

---

## 12. `shared/` seeded infrastructure-first (no empty layer trio)

**Pattern**: B43 keeps the `application/ domain/ infrastructure/` layer folders present even when empty, to document the layering and reserve the spot.

**Practice**: `shared/` holds `shared/infrastructure/` (first resident `http::read_capped_text`) and `shared/domain/` (created with its first resident, `record_change.rs` — the change-log vocabulary every bounded context records in). `shared/application/` does not exist until a cross-BC application helper needs it.

**Trade**: Empty placeholder modules are speculative scaffolding (YAGNI) — they add module-tree noise with no consumer. Each layer folder appears together with its first resident, so the tree documents what exists rather than what might.

**When to revisit**: Add `shared/application/` the moment the first cross-BC application helper lands there — created together with its resident, not ahead of it.

---

## 13. Startup side-effect hooks live in `shell/`, not in their domain feature

**Pattern**: F0/F28 scope the `shell/` bucket to layout chrome (AppShell, header, sidebar, overlay hosts); behaviour belongs to its feature module.

**Practice**: `useFeeGeneration` — the app-mount hook that fires `applyDueFeeDeductions` (FEE-040 lazy catch-up) — lives in `src/features/shell/fee_generation/`, and `applyDueFeeDeductions` is wrapped by `shell/gateway.ts`, not by `account_details/gateway.ts`.

**Trade**: A cross-account, app-startup operation is not owned by any single feature. Routing it through `account_details` would force the shell-mounted hook to import the `account_details` gateway — a sibling-feature behaviour import that F26 flags. Keeping the startup effect + its gateway in `shell/` keeps the dependency direction clean. (`useUpdateBanner` is the same app-mount-effect shape but lives in `@/lib/update`; the two precedents diverge until the `lib/` → `infra/` migration lands.)

**When to revisit**: If more startup effects accumulate in `shell/`, factor a dedicated `shell/startup/` group; if the `lib/` → `infra/` migration lands first, reconcile with the `useUpdateBanner` placement.

---

## 14. The scheduled-fetch use case owns its own persistence

**Pattern**: Use cases orchestrate across bounded contexts; persisted aggregates and their repositories live inside a bounded context.

**Practice**: `use_cases/scheduled_fetch/repository.rs` persists `ScheduledFetchConfiguration` (device singleton) and `ScheduledFetchRun` (execution history) directly, with no owning bounded context. `use_cases/shared/price_fetch_log.rs` does the same for the price fetch log (device singleton, MKT-201), and sits in `use_cases/shared/` because two use cases write it (`asset_price_fetch`, `scheduled_fetch`) and a third reads it (`price_freshness`), which may not import one another (B18).

**Trade**: These are operational records of the orchestration itself — a run sweeps `asset` prices and `currency` rates, so neither BC can own its configuration or its outcome without inverting the dependency; the fetch log records that a sweep happened, which no BC knows. A dedicated bounded context for three small records fails the ceremony/benefit test. Precedent: `update_checker`'s use-case-scoped `UpdateState`.

**When to revisit**: If scheduling grows more aggregates (per-asset schedules, notification preferences), promote the trio to a real `context/scheduling/` bounded context.

---

## 15. Repository capture tests import the sync bounded context

**Pattern**: A bounded context never imports another bounded context (B13); tests of a repository exercise that repository's own dependencies only.

**Practice**: The inline `mod tests` of the synced repositories in `context/account/`, `context/asset/`, and `context/currency/` import `crate::context::sync::SqliteChangeRecorder` (test-only) to assert that a write leaves real `changes` / `tombstones` rows and stamps the rank columns.

**Trade**: The production code of those repositories depends on `shared::infrastructure::change_recorder::ChangeRecorder` only — the port — so B13 holds where it matters. A mock recorder can prove the port was called, but not that the change and the record exist together in one transaction (SYN-020); that needs the real implementation, which the `sync` bounded context owns. The import is confined to `#[cfg(test)]`.

**When to revisit**: If a test-support crate or a shared in-memory recorder implementation appears, point the capture tests at it and drop the cross-context test import.

---

## 16. Ports that take the live database connection

**Pattern**: A repository port exposes domain-level operations only; persistence handles (connections, transactions) never cross the port boundary, and a use case never touches a connection.

**Practice**: `ChangeRecorder` (`shared/infrastructure/change_recorder.rs`), `RankStamper` (`context/sync/domain/rank_stamper.rs`), `ChangeLogRepository` (`context/sync/domain/change_log.rs`) and the `stamp_sync_rank(conn, rank)` method on the account, asset and currency repository traits all take a `&mut sqlx::SqliteConnection`; `use_cases/portfolio_sync/rank_stamper.rs` forwards that connection to the three services. `ChangeRecorder` sits under `shared/infrastructure/`, not `shared/domain/`, for the same reason: its signature names a persistence handle.

**Trade**: A record and its change row must commit together (SYN-020), and the first publish must write the header's first segment, stamp every existing row in three bounded contexts and enrol the device in **one** transaction (SYN-013: "rolled back on failure"). Without a unit of work (ADR-006 is unimplemented — see `docs/techdebt.md`), the only way to keep those writes atomic is to hand the live transaction through the ports. Each context still owns its own SQL; the use case only carries the handle.

**When to revisit**: When ADR-006's unit of work lands, the ports take the unit of work instead of a raw connection and the use case stops seeing sqlx types.

---

## 17. Test functions carry descriptive names, not a `test_` prefix

**Pattern**: The kit's test convention and its reviewer lane expect Rust test functions to start with `test_`.

**Practice**: Tests are named for the behaviour they pin — `missing_header_reports_the_volume_unmounted_and_does_not_pause`, `apply_removal_of_a_pair_publishes_only_currency_pair_updated` — with the rule ID in a comment above. 315 of 391 test functions had this shape when it was recorded (2026-05-24), and every test written since does.

**Trade**: `#[test]` already marks the function; a `test_` prefix repeats it and pushes the behaviour out of view in `cargo test` output. A reviewer finding of "missing `test_` prefix" is rejected on sight.

**When to revisit**: If the kit codifies descriptive naming, drop this entry; if it ever hard-fails on the prefix, rename project-wide in one chore.

---

## 18. A use case with no orchestrator

**Pattern**: A use case has an orchestrator as its entry point after `api.rs` (B22), which coordinates the bounded contexts it spans.

**Practice**: `use_cases/capabilities/` is `mod.rs` and `api.rs` only. `get_capabilities` returns a `Capabilities` value the composition root managed when it decided which External provider the build has (ADR-020, MKT-211).

**Trade**: There is nothing to coordinate: the answer is a fact about how the application was composed, settled before the first window opens and constant for the run. An orchestrator would hold one field and forward it. Precedent: `update_checker`'s `UpdateState`, also managed at composition and read by commands.

**When to revisit**: If a capability ever needs computing — from a subscription, a licence check, a reachable service — give the use case its orchestrator then.

---

## What we follow strictly (not divergences)

For reference, the patterns this codebase enforces tightly:

- Aggregate roots — every state mutation goes through a root method (`Account::buy_holding`, `Asset::archive`, `AssetCategory::ensure_renameable`).
- Factory / aggregate-root method conventions — `new()`, `with_id()`, `from_storage()` for construction; `update_from(self, ...)` / `archive(self)` / `ensure_<predicate>(&self)` for mutation. See `CLAUDE.md` § Domain Entities.
- Repository pattern — traits in domain, impls in infrastructure (repo). Single direction of dependency.
- Application services are thin — load → mutate → save. No business logic outside the aggregate.
- Use-case orchestrators for cross-BC operations. They never reach into another BC's repo directly.
- Layered errors with the rejection-layer rule — see `docs/error-model.md` and `docs/ddd-reference.md` § Errors.
- Bounded context isolation at the module level — `context/{account,asset}/` cannot cross-import.
- Ubiquitous language — `docs/ubiquitous-language.md` is the single source of truth.
- Persistence ignorance — domain types know nothing about sqlx or SQL; the closest concession is `Asset::from_storage(...)` factories (named for the use case, not the storage tech).

The divergences above are **deliberately scoped** — they don't compound into "we're doing service-oriented design wearing a DDD hat". The structural backbone is intact.
