# Architecture

Folioneer is a single-user Tauri 2 desktop app. React 19 + TypeScript on the frontend, Rust + SQLite on the backend, Specta-generated bindings for IPC, DDD for backend layering.

> **For Claude Code**: this file is a **router**, not a code catalog. Per-module file lists, command signatures, and entity field shapes are NOT documented here — they drift the moment code changes and are one `ls` or `grep` away. For implementation questions, start with the doc pointers below and read the actual source.

---

## Stack

| Layer         | Tech                                                                                                              |
| ------------- | ----------------------------------------------------------------------------------------------------------------- |
| Desktop shell | Tauri 2 (single executable)                                                                                       |
| Frontend      | React 19 + TypeScript, Zustand store, react-i18next                                                               |
| Backend       | Rust, async via tokio, SQLite via sqlx (compile-time query checking)                                              |
| IPC           | Specta-generated bindings — `src/bindings.ts` (auto-generated, do not edit; regenerate via `just generate-types`) |

---

## Where to look

| You need                                                                | Read                                                                                                                 |
| ----------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| Backend implementation patterns (row mapping, orchestrator shape, etc.) | [`docs/backend-patterns.md`](docs/backend-patterns.md)                                                               |
| Generic DDD rules (kit-managed)                                         | [`docs/backend-rules.md`](docs/backend-rules.md), [`docs/ddd-reference.md`](docs/ddd-reference.md)                   |
| Intentional divergences from textbook DDD                               | [`docs/ddd-divergences.md`](docs/ddd-divergences.md)                                                                 |
| Error model (per-BC enum + per-use-case composite)                      | [`docs/error-model.md`](docs/error-model.md)                                                                         |
| Frontend rules + visual proof workflow                                  | [`docs/frontend-rules.md`](docs/frontend-rules.md), [`docs/frontend-visual-proof.md`](docs/frontend-visual-proof.md) |
| i18n + a11y rules                                                       | [`docs/i18n-rules.md`](docs/i18n-rules.md)                                                                           |
| E2E + test conventions                                                  | [`docs/e2e-rules.md`](docs/e2e-rules.md), [`docs/test_convention.md`](docs/test_convention.md)                       |
| Business rules per feature                                              | [`docs/spec/{feature}.md`](docs/spec/)                                                                               |
| Architectural decisions                                                 | [`docs/adr/{NNN}-*.md`](docs/adr/)                                                                                   |
| Domain terms (canonical)                                                | [`docs/ubiquitous-language.md`](docs/ubiquitous-language.md)                                                         |
| Tech debt                                                               | [`docs/techdebt.md`](docs/techdebt.md)                                                                               |
| Open items                                                              | [`docs/todo.md`](docs/todo.md)                                                                                       |
| Roadmap                                                                 | [`docs/roadmap.md`](docs/roadmap.md)                                                                                 |
| Design system tokens                                                    | [`docs/design-system.md`](docs/design-system.md), [`docs/spec/theme.md`](docs/spec/theme.md)                         |
| How work moves (Workflow C)                                             | [`docs/workflow-c.md`](docs/workflow-c.md)                                                                           |

For "what does use case X do?" → read `src-tauri/src/use_cases/{name}/mod.rs` (module doc) and its spec at `docs/spec/`.
For "what does feature X do?" → read `src/features/{name}/` and its spec.

---

## Backend layout (`src-tauri/src/`)

```
context/{bc}/         bounded contexts — strict module isolation, no cross-BC imports
  domain/             entities + repository traits (persistence-ignorant)
  repository/         SQLite repository implementations (FromRow + impl From<Row> for Domain)
  service.rs          application service (thin: load → mutate → save → publish event)
  api.rs              BC-owned Tauri commands
  error.rs            flat {BC}Error enum (per docs/error-model.md)
  mod.rs              public API surface (use only this from outside)

use_cases/{name}/     cross-BC orchestrators (read or write paths spanning multiple BCs)
  orchestrator.rs     single struct, one method per Tauri command
  error.rs            per-use-case composite(s) + use-case-specific flat failures
  api.rs              use-case-owned Tauri commands
  mod.rs

use_cases/shared/     stateless helpers shared by use cases without importing one another (B18)
  global_value.rs     the single Global Value arithmetic + REFERENCE_CURRENCY (PMV-023, GPF-011)
  price_movement.rs   pure Price Movement report builder (PMV-020/022/030-060)

core/                 shared infra (legacy bucket — new shared code goes in shared/, see B0)
  db.rs               sqlite pool + migrations
  event_bus/          SideEffectEventBus + Event enum (cross-BC pub/sub)
  logger.rs           tracing setup, FRONTEND / BACKEND targets
  specta_builder.rs   THE Tauri command registry (every command registered here, nowhere else)
  cash.rs             system cash asset id helpers (shared without crossing a BC boundary)

shared/               gold-layout shared infra (docs/backend-rules.md B0/B37)
  domain/
    record_change.rs  RecordChange / RecordKind / Operation / Origin — the per-device change-log vocabulary every BC writes in (SYN-020/021, ADR-019)
  infrastructure/
    app_directories.rs where a build keeps its data and logs — the installed application's folders for a release build, development folders of their own for a debug build
    change_recorder.rs ChangeRecorder port — records one change on the live write connection so the record and its change row commit together (divergence #16)
    container.rs      AppContainer — boxes SQLite repos into the application services; consumed by both entry points (lib.rs setup, headless scheduled fetch)
    e2e_run.rs        the isolated data folder an E2E run injects through an environment variable (debug builds only)
    http.rs           shared reqwest client construction + capped body reads
    scheduler/        DailyFetchScheduler trait + per-OS adapters (systemd/schtasks/launchd)
                      registering the daily scheduled fetch (SPF); NoopScheduler in every debug build

lib.rs                composition root — wires services, use cases, dispatchers; calls app_handle.manage().
                      Also exposes run_scheduled_fetch_headless() — the OS-triggered `--scheduled-fetch`
                      invocation (main.rs branch) that runs the daily download without a window (SPF-020)
```

**Hard rules:**

- Tauri commands are registered ONLY in `core/specta_builder.rs`. Adding a `#[tauri::command]` without registering it there silently breaks IPC.
- Bounded contexts cannot import from each other. Cross-BC logic lives in `use_cases/`.
- Repositories return `Result<T, anyhow::Error>`. Services translate to typed `{BC}Error`. Use cases compose via `#[from]`. See `docs/error-model.md`.
- See [`docs/backend-patterns.md`](docs/backend-patterns.md) for the row-mapping recipe and orchestrator shape.

> Contexts: `account/`, `asset/`, `currency/`, `sync/`. The newer `currency/` and `sync/` BCs follow the gold `application/domain/infrastructure` trio with `api.rs` + `error.rs`, rather than the `repository/` + `service.rs` shape shown in the template above. `sync/` owns the device identity, the encrypted shared folder (Argon2id-derived key, XChaCha20-Poly1305 segments), the change-log repository, and the single conflict-resolution engine `domain/resolution.rs` (ADR-019, spec CFR); `use_cases/portfolio_sync/` applies the resolved changes through the other three BCs' services and owns the eleven sync commands. Asset prices are auto-fetched from the keyless Yahoo Finance `/v8/chart/` endpoint (`asset/repository/yahoo_client.rs`, ADR-017) — no API key, so no credential bounded context.

---

## Event bus

Backend publishes events on every state change. Frontend listens via a single `events.event.listen()` subscription in `src/lib/store.ts:init()` and dispatches to the right fetcher.

| Event                      | Published by                                                                                                                                                                                          | Frontend re-fetches                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `AssetUpdated`             | `context/asset/` writes                                                                                                                                                                               | `assets`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `CategoryUpdated`          | `context/asset/` category writes                                                                                                                                                                      | `categories`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `AssetPriceUpdated`        | `context/asset/` price writes (including a price history backfill, MKT-194) + `use_cases/asset_price_fetch/`                                                                                          | `account_details`, `performance` (per-page); shell price item (`shell/price_freshness/`, MKT-203)                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `AccountUpdated`           | `context/account/` account writes                                                                                                                                                                     | `accounts`, `performance` (per-page)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `TransactionUpdated`       | `context/account/` holding / transaction writes                                                                                                                                                       | `account_details`, `transactions`, `performance` (per-page); shell price item (`shell/price_freshness/`, MKT-203)                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `CurrencyRateUpdated`      | `context/currency/` rate writes + provider fetch                                                                                                                                                      | `account_details`, `currency_rates_view`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `FeeScheduleUpdated`       | `context/account/` fee-schedule CRUD                                                                                                                                                                  | `account_details`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `HoldingNoteUpdated`       | `context/account/` holding-note upsert / delete                                                                                                                                                       | `account_details`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `CurrencyPairUpdated`      | `context/currency/` pair declare                                                                                                                                                                      | `currency_rates_view`, `account_details`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `AssetPriceFetchProgress`  | `use_cases/asset_price_fetch/` dispatcher (task start + per asset, MKT-180)                                                                                                                           | shell progress bar via the global store (`lib/store.ts`); no re-fetch                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `AssetPriceFetchCompleted` | `use_cases/asset_price_fetch/` dispatcher (bulk fetch end, MKT-181)                                                                                                                                   | `account_details`, `accounts`, `performance` (per-page); progress bar cleared + unpriced list refreshed via `lib/store.ts`. Carries a `PriceMovementReport` when a user-started Global refresh produced one (PMV-010); the accounts-list dialog reads it through its own mount-scoped listener, NOT `lib/store.ts`, so the report is discarded on navigation (PMV-016). `AccountManager` reads `lib/store.ts` only for the unpriced slice, to hold the report back while the manual-fill modal is up (PMV-018); shell price item (`shell/price_freshness/`, MKT-203) |
| `SyncCompleted`            | `context/sync/` `SyncService::remember_run` — a run that applied changes or changed the device's failures / paused state — and `announce_join` after a join; the only event an apply raises (SYN-064) | bare marker: `lib/store.ts` re-fetches assets, categories, accounts; every feature view listed above re-fetches once; `shell/sync_indicator` re-reads `get_sync_status`; shell price item (`shell/price_freshness/`, MKT-203)                                                                                                                                                                                                                                                                                                                                        |

Adding a new event: declare the variant in `core/event_bus/event.rs`, publish from the service after persistence (`bus.publish(Event::Foo)`), subscribe in the relevant feature hook.

---

## Frontend layout (`src/`)

```
bindings.ts          AUTO-GENERATED Tauri bindings — do not edit
features/{name}/     feature-first layout (see "Feature layout convention" below)
infra/               cross-feature plumbing (storage helpers, logger wrapper, fuzzy search)
ui/                  M3 design primitives (Button, Field, Modal, Layout)
shared/              cross-feature utilities (Result helpers, presenter primitives)
i18n/                react-i18next config + locales (fr default, en fallback)
lib/                 legacy bucket — migration to infra/ + shared/ in progress (see docs/techdebt.md)
```

### Data flow

```
Component
  └─ Hook (state, useMemo, callbacks)
       └─ Gateway (commands.* — positional args, matches bindings.ts exactly)
            └─ Tauri IPC
                 └─ Rust api.rs handler (Result<T, {Command}Error>)
                      └─ Use case / Service
                           └─ Repository
                                └─ SQLite

Backend publishes Event
  └─ src/lib/store.ts:init() listener
       └─ store re-fetches the affected slice → subscribed components re-render
```

### Feature layout convention (gold)

All new features MUST follow this. Reference: `features/assets/`.

```
features/{domain}/
├── gateway.ts                  # ONLY file that calls commands.* for this feature
├── {sub_feature}/
│   ├── {SubFeature}.tsx        # Component
│   ├── use{SubFeature}.ts      # Colocated hook
│   └── use{SubFeature}.test.ts # Colocated test
├── shared/
│   ├── presenter.ts            # Domain → UI transformations
│   ├── validate{X}.ts          # Pure validation
│   └── constants.ts
└── index.ts                    # Public re-exports
```

Hard rules:

- `gateway.ts` at the feature root — never `api/` wrappers, never inline `commands.*` calls in components or hooks.
- Sub-features are directories grouped by **concern**, not by layer (no `components/`, `hooks/`).
- Hooks colocated next to their component inside the sub-feature folder.
- `presenter.ts` is pure — no `commands.*`, no `useEffect`.
- See [`docs/frontend-rules.md`](docs/frontend-rules.md) for the full rule set (F1–F28).

> The `features/currency/` feature (FX-rate manual entry CRUD + the Currency Rates view) follows this layout: `gateway.ts` at root, `declare_pair/`, `record_rate/`, and `currency_rates_view/` sub-features, and `shared/`.

### Cross-feature modal opens — URL-driven shell mounts

A feature must not import a sibling feature's modal. Instead the opener sets a `?modal=…` URL param (`openModalSearch` / `patchModalSearch` in `src/lib/modalSearch.ts`) and a mount component in `features/shell/` (e.g. `CashTransactionEditMount`, `AssetEditModalMount`, `FreeSharesEditModalMount`, `ManagementFeeEditModalMount`, `InterestEditModalMount`, `SplitEditModalMount`) renders the dialog while the param is present. The shell is the only layer that imports across features.

---

## Decisions to know about

These are the load-bearing architectural choices that aren't obvious from reading code:

- **Single SQLite database** with foreign keys spanning BCs. Logical BC isolation only (Rust module level). See [`docs/ddd-divergences.md`](docs/ddd-divergences.md) #7.
- **Domain types serialize directly to the FE** via Specta (no DTO layer). See [`docs/ddd-divergences.md`](docs/ddd-divergences.md) #3.
- **IDs are `String`, not value-object wrappers.** See [`docs/ddd-divergences.md`](docs/ddd-divergences.md) #1.
- **Repository folder is still named `repository/`** (gold standard is `infrastructure/`). Bit-by-bit migration tracked in [`docs/techdebt.md`](docs/techdebt.md).
- **`anyhow::Error` in repo trait error type** is intentional. See [`docs/ddd-divergences.md`](docs/ddd-divergences.md) #10.
- **Update lifecycle state (`UpdateState`)** is managed before the DB is ready so the frontend can show a migration-failure screen.
- **Multi-device sync is a per-device append-only change log merged by one resolution engine** — no CRDT library, no SQLite triggers, no server; transport is a user-owned cloud folder. See [ADR-019](docs/adr/019-per-device-change-log-multi-device-sync.md), specs `SYN` + `CFR`, and divergences #15/#16.

---

## Maintenance

This file describes what doesn't change often. Per-feature / per-use-case / per-migration details belong in source code, spec docs, or the kit pattern docs — not here.

Update this file when:

- A new top-level module appears under `src-tauri/src/` or `src/`
- A new event is added to the event bus
- An architectural decision changes (DB swap, layering reshape, IPC rework)
- A new specialized doc joins `docs/` and is worth routing to

Do NOT update this file when adding a use case, a BC method, a Tauri command, or a feature module — those live in source code and their own specs.
