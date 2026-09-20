# Contract — Update

> Domain: `update` (use case — `update_checker`)
> Last updated by: `update` spec

> **Error model on the wire**: each command's error serializes as a flat `{ code: "VariantName" }` object. The FE matches on `code`. Per-command reachable codes are listed in the "Errors" column of the table below. This use case has no repository: a technical failure surfaces as `{ code: "OperationFailed" }` (no payload; the cause is logged server-side via `tracing::error!`).
>
> Rust-internal type organization (per-BC enums, use-case composites, serde tagging) is out of scope for this contract — it documents the BE↔FE frontier, not Rust internals.

---

## Commands

| Command            | Args | Return               | Errors                                                                                                                                                                                                                                 |
| ------------------ | ---- | -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `check_for_update` | —    | `Option<UpdateInfo>` | `AccessRefused` _(UPD-028 — the update server refuses this build's access; also emitted as `update:error`. Other network/server errors are silent per UPD-021: the command returns None)_                                              |
| `download_update`  | —    | `()`                 | _(none — returns immediately (UPD-006, UPD-007); failures are emitted as `update:error` with `OperationFailed` (UPD-023) or `AccessRefused` (UPD-028); re-invoke to retry per UPD-024; concurrent calls silently ignored per UPD-010)_ |
| `install_update`   | —    | `()`                 | `NoDownloadedUpdate` _(precondition guard — UPD-013: install requires a completed download)_, `OperationFailed` _(updater initialisation, the release is no longer found, or the installation itself fails)_                           |

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
```

---

## Events

| Event              | Payload                | Rule                      |
| ------------------ | ---------------------- | ------------------------- |
| `update:available` | `UpdateInfo`           | UPD-001, UPD-025          |
| `update:progress`  | `u64` — percent, 0–100 | UPD-008                   |
| `update:complete`  | —                      | UPD-011                   |
| `update:error`     | `UpdateError`          | UPD-009, UPD-023, UPD-029 |
