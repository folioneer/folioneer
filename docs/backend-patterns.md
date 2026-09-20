# Backend Patterns

Project-owned recipes for idiomatic backend code. Companion to the kit-managed `docs/backend-rules.md` (generic DDD rules) and `docs/ddd-reference.md` (general DDD vocabulary). Both kit docs stay generic; this file captures the **HOW** for this codebase.

Read this when:

- You're writing a new repository, use case, or row-to-domain mapping and want the established shape.
- A reviewer flags a structural choice (struct layout, file split, helper placement) — verify the existing pattern, then either follow it or propose an amendment here.
- Onboarding to a new module — skim once to internalise the conventions before reading any specific file.

> This file holds the project's own idioms, next to the generic rules in `backend-rules.md`. A pattern that stops being specific to this project moves into the rules file.

---

## 1. Repository row mapping — `FromRow` + `impl From<Row> for Domain`

**When to use**: any time a repository reads rows from SQLite and reconstructs a domain entity.

**Shape**:

```rust
#[derive(sqlx::FromRow)]
struct AccountRow {
    id: String,
    name: String,
    currency: String,
    update_frequency: String,
}

impl From<AccountRow> for Account {
    fn from(row: AccountRow) -> Self {
        let update_frequency = UpdateFrequency::from_str(&row.update_frequency).unwrap_or_else(|_| {
            tracing::warn!(
                target: BACKEND,
                value = %row.update_frequency,
                "unknown update_frequency value, falling back to default"
            );
            UpdateFrequency::default()
        });
        Account::restore(row.id, row.name, row.currency, update_frequency)
    }
}
```

Queries use `sqlx::query_as!(RowType, "SELECT ...")` and map via `.map(Domain::from)`:

```rust
let rows = sqlx::query_as!(
    AccountRow,
    r#"SELECT id, name, currency, update_frequency FROM accounts"#
).fetch_all(&self.pool).await?;
Ok(rows.into_iter().map(Account::from).collect())
```

**Rules**:

- The `Row` struct is repository-private (`struct AccountRow`, not `pub`). Domain types know nothing about its existence.
- Domain enums persisted as `TEXT` columns derive `strum_macros::Display + strum_macros::EnumString` (see `AssetClass`, `AssetPriceSource`). Repo writes call `enum.to_string()`; repo reads call `Enum::from_str(&row.field)`.
- Unknown-value fallback in `From<Row>` MUST log via `tracing::warn!(target: BACKEND, value = %..., "unknown ... value, falling back to default")` and pick a deterministic default. Never panic on storage drift — old rows from prior versions must keep loading.
- The `From` impl goes inside the repository module (`context/{bc}/repository/{aggregate}.rs`), directly under the `Row` struct.

**Anti-patterns**:

- ❌ Putting `as_str()` / `from_storage()` helpers on the domain enum itself. The domain doesn't know it lives in SQLite.
- ❌ Inline `.map(|r| Domain::restore(...))` closures that re-implement the row → domain mapping at every call site. Centralise in the `From` impl.
- ❌ Bare `sqlx::query!` returning anonymous row structs when the same shape is used in multiple methods. Use `query_as!(RowType, ...)`.
- ❌ Panicking on unknown enum values (`.unwrap()`, `.expect()`, `unreachable!()`). Migrations may leave old rows; always degrade with a logged fallback.

See: `src-tauri/src/context/account/repository/account.rs` (canonical), `src-tauri/src/context/asset/repository/asset_price.rs`.

---

## 2. Use-case orchestrator — one struct, multiple methods

**When to use**: a feature exposes multiple Tauri commands that share the same dependencies (services, guards, dispatchers, repos). The default split is **one orchestrator struct per cohesive feature**, with one method per command.

**Shape**:

```rust
pub struct AssetPriceFetchUseCase {
    account_service: Arc<AccountService>,
    asset_service: Arc<AssetService>,
    fetch_guard: Arc<FetchGuard>,
    dispatcher: Arc<Dispatcher>,
}

impl AssetPriceFetchUseCase {
    pub fn new(...) -> Self { ... }

    pub async fn fetch_all(&self) -> Result<(), FetchAllAssetPricesError> { ... }
    pub async fn fetch_for_account(&self, account_id: &str) -> Result<(), FetchAccountAssetPricesError> { ... }

    async fn build_scope(&self, asset_ids: HashSet<String>) -> Result<Vec<(Asset, String)>, AssetError> { ... }
}
```

Each Tauri command takes the same `State<'_, Arc<AssetPriceFetchUseCase>>` and delegates to one method. `lib.rs` calls `app_handle.manage(use_case)` exactly once.

**Rules**:

- Group methods that share **all** dependencies under a single struct. If two methods would need a strict subset of fields, that's still fine — over-injection on one method is cheaper than the wiring duplication of a split struct.
- Each public method returns its own wire-facing error composite (see `docs/error-model.md`). Multiple composites per orchestrator is normal and expected.
- Helpers shared across methods (`build_scope` above) live as **private** `impl` methods, not as `pub(super)` free functions in a sibling module.
- File naming: `use_cases/{feature}/orchestrator.rs` for the struct, `use_cases/{feature}/error.rs` for all composites + use-case-specific failure enums, `use_cases/{feature}/api.rs` for the Tauri commands.

**Anti-patterns**:

- ❌ One file per command method (`all.rs`, `account.rs`) when both methods take the same `new()` signature and share helpers. Duplicates the constructor, multiplies `lib.rs` `manage()` calls, leaks helpers as `pub(super)` between siblings.
- ❌ Splitting an orchestrator into multiple structs to "isolate concerns" when the concerns share state. The cohesion is the dependency set, not the method name.
- ❌ Exposing a private helper as `pub(super)` in a sibling module to share it across two orchestrators that should have been one.

See: `src-tauri/src/use_cases/asset_price_fetch/orchestrator.rs`, `src-tauri/src/use_cases/holding_transaction/` (the canonical multi-method orchestrator: `buy_holding`, `sell_holding`, `correct_transaction`, `cancel_transaction`, `record_deposit`, `record_withdrawal`, `open_holding`).

---

## What this file is NOT

- **Not a substitute for `docs/backend-rules.md`** (kit-managed). The kit doc carries generic DDD rules (B0–B43). This file carries this codebase's idiomatic shapes for applying them.
- **Not for divergences from textbook DDD** — those go to `docs/ddd-divergences.md`.
- **Not for architectural layout** — that's `ARCHITECTURE.md`.
- **Not for tech debt** — that's `docs/techdebt.md`.
