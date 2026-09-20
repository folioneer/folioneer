# Business Rules — Scheduled Price Fetch (SPF)

## Context

Today, market prices are downloaded only while the app is running (launch auto-fetch and manual refreshes, MKT-110+). The user wants prices to arrive **once per day at a chosen time — even when the app is closed** — so that opening the app any morning shows yesterday's closing values without any manual action.

The feature schedules a daily download with the operating system: at the configured time the application runs invisibly, records the day's closing prices (and refreshes exchange rates), and exits. There is no resident process, no tray icon, and no OS setup by the user — everything is configured from the app's Settings page. Recorded values are always **daily closes**, never live intraday quotes: a run that executes late (machine off at the trigger) still records the close of each missed trading day, backfilled up to 30 days.

This spec is a **feature spec**: it extends the price-write model owned by the `asset` bounded context (MKT spec) and the rate-fetch path owned by the `currency` bounded context (FXR spec), and adds a scheduling/configuration surface of its own. The two entities below (`ScheduledFetchConfiguration`, `ScheduledFetchRun`) are owned by the **scheduled-fetch use case** — a deliberate lightweight divergence from the "use cases orchestrate, contexts own persistence" norm: they are operational records of the orchestration itself, belong to no existing bounded context, and are too small to justify a new one (to be recorded in `docs/ddd-divergences.md` at implementation).

**Accepted limitation — uninstall leftover**: uninstalling the application without disabling the schedule first may leave the OS entry registered; it then fails harmlessly once per day. Disabling before uninstall removes it cleanly.

All financial values are stored as i64 micro-units per [ADR-001](../adr/001-use-i64-for-monetary-amounts.md). Price writes follow latest-write-wins per [ADR-012](../adr/012-latest-write-wins-source-as-metadata.md); the provider is keyless Yahoo Finance per [ADR-017](../adr/017-yahoo-finance-keyless-price-source.md).

---

## Entity Definition

### ScheduledFetchConfiguration

The device-wide configuration of the daily download. Exactly one configuration exists.

| Field          | Business meaning                                                                                               |
| -------------- | -------------------------------------------------------------------------------------------------------------- |
| `enabled`      | Whether the daily download is active on this device. Off by default.                                           |
| `trigger_time` | Local wall-clock time of day at which the download runs (e.g. `19:00`), interpreted in the machine's timezone. |

### ScheduledFetchRun

The record of one execution of the scheduled download. Runs accumulate as an auditable history and power the once-per-day guard and the settings status line.

| Field           | Business meaning                                                                                                              |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `executed_at`   | When the run actually executed (may be later than the trigger when catching up).                                              |
| `trigger_date`  | The calendar day whose trigger this run settles — always the **latest pending trigger** at execution time (SPF-021, SPF-022). |
| `outcome`       | Whether the run succeeded, failed, or was skipped by the once-per-day guard.                                                  |
| `updated_count` | Number of assets whose prices were written by this run.                                                                       |
| `skipped_count` | Number of in-scope assets the run could not price (silently skipped).                                                         |

---

## Business Rules

### Configuration & Scheduling (010–019)

**SPF-010 — Settings surface (frontend)**: The Settings page gains a "Daily price download" section with an enable toggle (default OFF) and a time-of-day field for the trigger time. The time field is editable only while the toggle is on.

**SPF-011 — Configuration lives with the application data (backend)**: The configuration persists with the application's data — not the interface session — so it remains authoritative for runs that happen while the application is closed.

**SPF-012 — Zero OS setup (frontend + backend)**: Enabling registers the daily schedule with the operating system's task-scheduling facility at the configured local time; disabling removes it; changing the time re-registers it. The user never configures anything outside the app.

**SPF-013 — Registration failure surfaced (frontend + backend)**: When the schedule cannot be registered or removed, the action is rejected with a specific error, the toggle remains in its previous state, and an inline error is shown. The configuration is not persisted in a state that contradicts the OS schedule.

**SPF-014 — Local time and DST (backend)**: The trigger time is a local wall-clock time in the machine's timezone. Daylight-saving shifts do not change the wall-clock trigger.

**SPF-015 — Self-heal on app start (backend)**: On every app start, when the configuration is enabled, the OS schedule is verified and silently repaired if missing or stale (e.g. the application moved after an update). When disabled, a leftover schedule is silently removed.

**SPF-016 — No resident presence (backend)**: Outside its brief daily execution, nothing runs and nothing is visible — no tray icon, no background service, no window.

**SPF-017 — All desktop platforms supported (backend)**: The schedule registration works on Linux, Windows, and macOS through each system's native scheduling facility. Behavior is identical across platforms; only Linux is covered by automated end-to-end verification (Windows and macOS registrations are verified by unit-level checks on the generated scheduling definitions).

**SPF-018 — Default trigger time (frontend)**: When the user enables the feature without choosing a time, the trigger time defaults to **22:15** local time (after the same-day NYSE close as seen from Europe; a machine off at that hour is covered by the catch-up, SPF-022).

**SPF-019 — Trigger time validation (frontend + backend)**: A valid trigger time is a well-formed time of day (hours 00–23, minutes 00–59). The backend rejects a malformed trigger time with a specific error; the frontend's time field constrains input so the case is unreachable from the UI.

### Execution Model (020–029)

**SPF-020 — Invisible execution (backend)**: At the trigger, the application executes invisibly — no window is opened or focused, even when the app is already open — performs the download, records the run, and exits. It behaves identically whether or not the app is open at that moment.

**SPF-021 — Once-per-day guard (backend)**: Every run settles the **latest pending trigger**: the most recent calendar day whose trigger time has already passed at execution. Before any external call, the run checks whether a successful run has already settled that same trigger day; if so, it records a skipped run and exits without downloading.

**SPF-022 — Missed-trigger catch-up (backend)**: A trigger missed because the machine was off or asleep executes at the next opportunity (machine start or wake). Multiple missed days coalesce into a single catch-up run that settles the latest pending trigger (SPF-021); the earlier missed days are recovered by the backfill (SPF-031), not by additional run records.

**SPF-023 — Independent of in-app fetch tasks (backend)**: The scheduled run is a separate, short-lived execution of the application alongside any open instance — neither replaces, focuses, nor blocks the other. It is not subject to the in-app single-fetch guard (MKT-113), and vice versa. Simultaneous execution converges safely: all price writes are per-`(asset, date)` upserts (MKT-025), and concurrent access to the shared data store must be safe by construction.

**SPF-024 — Open app reflects values on its next read (frontend)**: A running app is not live-notified by the scheduled run. New values appear the next time the app naturally re-reads its data (navigation, manual refresh, relaunch).

### Values Recorded (030–039)

**SPF-030 — Close-of-day semantics (backend)**: The scheduled download records **daily closing prices**, never a live intraday quote. Each recorded price is dated to the trading day it closes — including during catch-up: a run executing the morning after a missed trigger records the previous day's close, not the morning's price.

**SPF-031 — Backfill window (backend)**: Each run retrieves, per asset, the **daily close series** covering the days missing since the last successful scheduled run — a capability the price provider already exposes (its chart data carries dated daily history, ADR-017) — and records every completed trading-day close in that window, up to a maximum of 30 days back. Gaps older than 30 days are left untouched.

**SPF-032 — Non-trading days produce no rows (backend)**: Weekends and market holidays have no close; the run writes nothing for those dates. This is not a skip or an error.

**SPF-033 — Markets still open at the trigger (backend)**: An asset whose trading day is not yet complete when the run executes contributes only closes up to its previous completed day. The in-progress day's close is written by the next run through the backfill window — the schedule self-corrects regardless of the chosen trigger time.

**SPF-034 — Writes follow the established price-write model (backend)**: Every price written by a scheduled run uses the `(asset, date)` upsert (MKT-025, latest-write-wins per ADR-012), carries the provider source (MKT-102), and applies sub-unit normalization (MKT-125).

**SPF-035 — Exchange rates in the same run (backend)**: The run also records **daily reference exchange rates** for every persisted currency pair (the same scope as the in-app rate refresh, FXR-071), each rate dated to its day — never a live snapshot re-dated to the run day.

**SPF-036 — Exchange-rate backfill (backend)**: Missing rate days since the last successful scheduled run are backfilled up to 30 days, mirroring SPF-031. The rate providers expose dated daily history for this (the primary provider serves date-range queries; verified live 2026-07-12).

**SPF-037 — Exchange-rate non-trading days (backend)**: Days for which the rate provider publishes no reference rate (weekends, ECB holidays) produce no rate rows, mirroring SPF-032. This is not a skip or an error.

**SPF-038 — Per-pair silent skip (backend)**: A currency pair the provider cannot serve is silently skipped — nothing written, no error surfaced — and the run continues with the remaining pairs, mirroring SPF-041.

**SPF-039 — Price/rate failure independence (backend)**: Rate failures do not fail the price portion of the run, and vice versa; each portion's outcome is reflected in the run record independently of the other.

### Scope & Skips (040–049)

**SPF-040 — Scope mirrors the in-app fetch tasks (backend)**: The run's scope is every active holding across all accounts with a derivable provider symbol (MKT-110), excluding system cash assets (MKT-116) and refresh-locked assets (MKT-151).

**SPF-041 — Per-asset silent skip (backend)**: An in-scope asset is silently skipped — nothing written, no error surfaced — when its symbol cannot be derived, the provider has no data for it, the provider call fails, or the write fails (same skip set as MKT-114). The run continues with the remaining assets, and the skip is counted in the run record.

**SPF-042 — Empty scope is a quiet success (backend)**: When the scope is empty, the run records a successful execution with zero updates and exits. Unlike the in-app rejection (MKT-111), no error is raised — there is no user watching a scheduled run.

### Outcome & Visibility (050–059)

**SPF-050 — Every run is recorded (backend)**: Every execution — successful, failed, or skipped by the once-per-day guard — records a `ScheduledFetchRun` with its trigger date, execution time, outcome, and update/skip counts.

**SPF-051 — Transient retry within a run (backend)**: A run that cannot reach the provider at all retries up to **3 attempts** (with increasing delay) within the same execution before recording a failed run. Per-asset skips (SPF-041) and per-pair skips (SPF-038) are not retried.

**SPF-052 — Status visible in Settings, never nagging (frontend)**: The Settings section displays the most recent run's status — when it ran, its outcome, and its counts (e.g. "Last download: yesterday 19:00 — 12 updated, 2 skipped"). A failed run appears there; no popup, snackbar, or dialog interrupts the user on the next app open.

**SPF-053 — Failure is covered by the next opportunity (backend)**: After a failed run, nothing is lost: the next trigger or catch-up records the missing closes through the backfill window (SPF-031).

### Settings Feedback (060–069)

**SPF-060 — Configure in-flight state (frontend)**: While a configuration change (enable, disable, or time change) is being acknowledged, the toggle and time field are disabled and a pending indicator is shown, preventing double submission — consistent with MKT-027.

**SPF-061 — Status loading state (frontend)**: While the section's status is being loaded, a loading indicator is shown in place of the status line. A load failure shows an inline error within the section; the rest of the Settings page is unaffected.

---

## Workflow

```
Settings page
    → "Daily price download" toggle ON + trigger time (default OFF)      (SPF-010)
        backend: persist configuration with app data                     (SPF-011)
        backend: register daily OS schedule at local trigger time        (SPF-012)
        on registration failure: reject, toggle reverts, inline error    (SPF-013)

Every day at trigger time (app open or closed)
    → OS launches the application invisibly                              (SPF-020)
        ├─ once-per-day guard: already settled today? → record skip, exit (SPF-021)
        ├─ scope = active holdings, minus cash + locked                  (SPF-040)
        ├─ for each asset: fetch daily closes missing since last
        │  successful run (≤ 30 days), dated to their trading day        (SPF-030, SPF-031)
        │    ├─ non-trading days: no row                                 (SPF-032)
        │    ├─ unfinished trading day: previous close only; next run
        │    │  completes it                                             (SPF-033)
        │    └─ no data / failure: silent skip, counted                  (SPF-041)
        ├─ record dated daily rates for all persisted pairs,
        │  backfilled like prices                                        (SPF-035–039)
        ├─ record ScheduledFetchRun (outcome + counts)                   (SPF-050)
        └─ exit                                                          (SPF-016)

Machine was off at the trigger
    → next machine start/wake: OS fires the missed trigger once          (SPF-022)
    → run records the missed days' closes via the backfill               (SPF-030, SPF-031)

App opened later
    → views read the new prices from storage as usual                    (SPF-024)
    → Settings shows "Last download: … — N updated, M skipped"           (SPF-052)
```

---

## UX Draft

### Entry Point

Settings page — new "Daily price download" section, alongside the existing price-related settings (auto-fetch on launch MKT-120, auto-record toggle MKT-050).

### Main Component

Settings section (no modal): enable toggle, time-of-day field, read-only status line for the last run.

### States

- **Disabled (default)**: toggle off, time field hidden or inert, no status line.
- **Enabled**: toggle on, time field editable, status line shows the last run or "No download yet".
- **Loading** (SPF-061): loading indicator in place of the status line while the section's data loads; inline error on load failure.
- **Configure in-flight** (SPF-060): toggle and time field disabled with a pending indicator while a change is acknowledged.
- **Registration error** (SPF-013): toggle reverts, inline error below the section.
- **Last run failed** (SPF-052): status line shows the failure — no popup anywhere else.

### User Flow

1. User opens Settings and enables "Daily price download".
2. User picks a time (e.g. 19:00) — or keeps the default.
3. The app registers the OS schedule; the section confirms it's active.
4. Every day at that time, prices and rates arrive silently — even with the app closed.
5. The next morning the user opens the app: yesterday's closes are already displayed; the Settings status line reads "Last download: yesterday 19:00 — 12 updated, 2 skipped".

---

## Open Questions

None — all questions have been resolved.
