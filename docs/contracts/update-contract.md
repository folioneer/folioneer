# Contract — Update

> Domain: `update` (use case — `update_checker`)
> Last updated by: `update` spec

> **Error model on the wire**: each command's error serializes as a flat `{ code: "VariantName" }` object. The FE matches on `code`. Per-command reachable codes are listed in the "Errors" column of the table below. This use case has no repository: a technical failure surfaces as `{ code: "OperationFailed" }` (no payload; the cause is logged server-side via `tracing::error!`).
>
> Rust-internal type organization (per-BC enums, use-case composites, serde tagging) is out of scope for this contract — it documents the BE↔FE frontier, not Rust internals.

---

## Commands

| Command               | Args | Return                     | Errors                                                                                                                                                                                                                                                                                                          |
| --------------------- | ---- | -------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `check_for_update`    | —    | `Option<UpdateInfo>`       | `AccessRefused` _(UPD-028 — the update server refuses this build's access; also emitted as `update:error`)_, `OperationFailed` _(any other failure: updater initialisation, network, server — UPD-027A; the startup check's silence is the window's, UPD-021)_                                                  |
| `download_update`     | —    | `()`                       | _(none — returns immediately (UPD-006, UPD-007); failures are emitted as `update:error` with `OperationFailed` (UPD-023) or `AccessRefused` (UPD-028); re-invoke to retry per UPD-024; concurrent calls silently ignored per UPD-010)_                                                                          |
| `install_update`      | —    | `()`                       | `NoDownloadedUpdate` _(precondition guard — UPD-013A: install requires a completed download)_, `AccessRefused` _(the re-check is refused — UPD-013C, UPD-028)_, `OperationFailed` _(updater initialisation, the release is no longer found or cannot be asked for or the installation itself fails — UPD-013A)_ |
| `get_startup_failure` | —    | `Option<MigrationFailure>` | _(infallible — reads what is kept in memory; `None` while no failure is kept: the portfolio opened, or is still opening and `db:migration_error` brings it; UPD-018A)_                                                                                                                                          |

---

## Shared Types

```rust
struct UpdateInfo {
    version: String,  // semantic version of available update (e.g. "1.2.3")
}

enum UpdateError {        // serialized as { code: "..." }
    NoDownloadedUpdate,   // install asked before a completed download
    OperationFailed,      // technical failure; cause logged server-side
    AccessRefused,        // the update server refuses this build's access (UPD-028)
}

struct MigrationFailure {         // UPD-018, UPD-035
    detail: String,               // the error, as the log has it
    backups_dir: Option<String>,  // the copies' folder when the portfolio was put back
}
```

---

## Events

| Event                | Payload                | Rule                                                                               |
| -------------------- | ---------------------- | ---------------------------------------------------------------------------------- |
| `update:available`   | `UpdateInfo`           | UPD-001, UPD-025                                                                   |
| `update:progress`    | `u64` — percent, 0–100 | UPD-008                                                                            |
| `update:complete`    | —                      | UPD-011                                                                            |
| `update:error`       | `UpdateError`          | UPD-009, UPD-023, UPD-028, #873's mock                                             |
| `db:migration_error` | `MigrationFailure`     | UPD-018, UPD-035 — sent once, during startup; `get_startup_failure` reads it again |

---

## Changelog

- 2026-11-21 — #873 (UPD-027A, UPD-028): `check_for_update` returns `OperationFailed` for a failure that is not a refusal, instead of `None`; `install_update` returns `AccessRefused` when its re-check is refused.
- 2026-11-21 — #732 (UPD-018A): new `get_startup_failure -> Option<MigrationFailure>`; the `db:migration_error` event and `MigrationFailure` are listed.
