# Architecture

Folioneer is a single-user Tauri 2 desktop app: React 19 + TypeScript on the frontend, Rust + SQLite on the backend, Specta-generated bindings for IPC, DDD for backend layering.

This file is a **map** — where code lives and what each place holds. Rules live in the rules docs, decisions in the ADRs; [`docs/README.md`](docs/README.md) says which document holds what.

## Stack

| Layer         | Tech                                                                                           |
| ------------- | ---------------------------------------------------------------------------------------------- |
| Desktop shell | Tauri 2 (single executable)                                                                    |
| Frontend      | React 19 + TypeScript, Zustand store, react-i18next                                            |
| Backend       | Rust, tokio, SQLite via sqlx (compile-time checked queries, offline cache in `.sqlx/`)         |
| IPC           | `src/bindings.ts`, generated from Rust by Specta (`just generate-types`, never edited by hand) |

## Backend (`src-tauri/src/`)

```
context/{bc}/        bounded contexts: account/, asset/ (older layout: domain/, repository/, service.rs)
                     currency/, sync/ (gold layout: application/, domain/, infrastructure/) — each with
                     api.rs (its Tauri commands), error.rs, mod.rs (its public surface)
use_cases/{name}/    cross-context orchestrators: orchestrator.rs, api.rs, error.rs, mod.rs
use_cases/shared/    stateless helpers shared by use cases (Global Value, price movement, fetch scope)
core/                legacy shared bucket: db.rs, event_bus/ (Event enum), logger.rs,
                     specta_builder.rs (the command registry), cash.rs
shared/              gold shared code: domain/record_change.rs (change-log vocabulary),
                     infrastructure/ (app_directories, change_recorder, container, e2e_run, http, scheduler/, window_lock)
lib.rs               the crate root: modules, the headless entry, tracing — the application core
app.rs               the Tauri shell (feature `app`, B46): wires services, use cases and dispatchers
                     into the window
command_line/        the command line (CLI): an interface beside the shell, calling the same use cases without a window
agent_connection/    the agent connection (AGT, ADR-023): the `--mcp` bridge, which holds no data, the local channel,
                     the owner's consent and the tools the running application serves to an agent
../cli/main.rs       `folioneer-cli`, the command line as a console program (installed beside the main one on Windows)
extensions.rs        the one file a build differs by: external data sources and update channel (ADR-020)
main.rs              entry point; `--scheduled-fetch` runs the daily download, `holding …` a command (CLI), `--mcp` the
                     agent bridge (AGT), all without a window
```

`sync/` owns device identity, the encrypted shared folder and the conflict-resolution engine (`domain/resolution.rs`, ADR-019); `use_cases/portfolio_sync/` applies resolved changes through the other contexts' services.

## Frontend (`src/`)

```
bindings.ts          generated Tauri bindings
features/{name}/     one folder per feature: gateway.ts (the only caller of commands.*), sub-feature
                     folders (component + hook + test), shared/ (presenter, validation)
features/shell/      composition root: layout chrome and the modal mounts other features open
ui/                  M3 design primitives (Button, Field, Modal, Layout)
lib/                 cross-feature plumbing (legacy bucket; new code goes to infra/, F0)
i18n/                react-i18next config and locales (fr default, en fallback)
```

Data flow: component → hook → gateway → Tauri command (`api.rs`) → use case or service → repository → SQLite. Every write from the window publishes an event (`core/event_bus/event.rs`) — the headless starts (scheduled download, command line) publish none; the store listener in `src/lib/store.ts` re-fetches the affected slice.

## Elsewhere

- `e2e/` — WebDriver specs against the real app (`docs/e2e-rules.md`).
- `scripts/` — the harness, the architecture and rule-home checks, release and merge tooling.
- `src-tauri/migrations/` — SQLite migrations; `src-tauri/tests/` — integration tests and the golden portfolio.

Update this map when a top-level folder appears or moves; not for a new feature, command or use case.
