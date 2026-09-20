# Contract — Scheduled Fetch

> Domain: `scheduled_fetch` (use case — orchestrates `asset` prices + `currency` rates, like `asset_price_fetch`)
> Last updated by: `scheduled-price-fetch` spec

> **Error model on the wire**: each command's error serializes as a flat `{ code: "VariantName", ...payload }` object. The FE matches on `code`. Per-command reachable codes are listed in the "Errors" column of the table below. Infrastructure failures surface as `{ code: "DatabaseError" }` (no payload; diagnostic chain preserved server-side via `tracing::error!`).

---

## Commands

| Command                      | Args                                                                  | Return                 | Errors                                                                                       |
| ---------------------------- | --------------------------------------------------------------------- | ---------------------- | -------------------------------------------------------------------------------------------- |
| `configure_scheduled_fetch`  | `ConfigureScheduledFetchArgs { enabled: bool, trigger_time: String }` | `()`                   | `InvalidTriggerTime`, `ScheduleRegistrationFailed`, `ScheduleRemovalFailed`, `DatabaseError` |
| `get_scheduled_fetch_status` | —                                                                     | `ScheduledFetchStatus` | `DatabaseError`                                                                              |

Traceability: `configure_scheduled_fetch` ← SPF-010, SPF-011, SPF-012, SPF-013 (`InvalidTriggerTime` guards a malformed time; `ScheduleRegistrationFailed` / `ScheduleRemovalFailed` carry the SPF-013 OS-schedule failure, per direction). `get_scheduled_fetch_status` ← SPF-010 (section render), SPF-052 (last-run status line).

The scheduled run itself, the once-per-day guard, catch-up, backfill, and start-time self-heal (SPF-015, SPF-020–SPF-033, SPF-040–SPF-053) are internal-only — no frontend caller, so no commands.

---

## Shared Types

```rust
struct ScheduledFetchConfiguration {
    enabled: bool,          // daily download active on this device (SPF-010)
    trigger_time: String,   // local wall-clock time of day "HH:MM" (SPF-014)
}

struct ScheduledFetchRun {
    executed_at: String,    // when the run actually executed (SPF-050)
    trigger_date: String,   // the calendar day this run settles (SPF-021, SPF-022)
    outcome: ScheduledFetchOutcome,
    updated_count: u32,     // assets whose prices were written (SPF-050)
    skipped_count: u32,     // in-scope assets silently skipped (SPF-041)
}

enum ScheduledFetchOutcome {
    Succeeded,          // run completed its sweep (including zero-update empty scope, SPF-042)
    Failed,             // provider unreachable after bounded retries (SPF-051)
    SkippedAlreadyRun,  // once-per-day guard exit (SPF-021)
}

struct ScheduledFetchStatus {
    configuration: ScheduledFetchConfiguration,
    last_run: Option<ScheduledFetchRun>,   // None when no run has ever executed (SPF-052)
}
```

---

## Events

None — SPF-024: the scheduled run never live-notifies a running app (separate execution, no event bridge); the settings section re-reads status via `get_scheduled_fetch_status` after `configure_scheduled_fetch` returns.

---

## Changelog

- 2026-07-12 — Added by `scheduled-price-fetch` spec: `configure_scheduled_fetch`, `get_scheduled_fetch_status`
- 2026-07-12 — contract-reviewer fix: `ScheduledFetchStatus.last_run` → `Option<ScheduledFetchRun>` (fresh-install state)
- 2026-07-12 — SPF-017: all three desktop platforms ship adapters; no platform-support flag needed on the wire
