# Contract — Update

> Domain: `update` (use case — `update_checker`)
> Last updated by: `update` spec

> **Error model on the wire**: each command's error serializes as a flat `{ code: "VariantName", ...payload }` object. The FE matches on `code`. Per-command reachable codes are listed in the "Errors" column of the table below. Infrastructure failures surface as `{ code: "DatabaseError" }` (no payload; diagnostic chain preserved server-side via `tracing::error!`).
>
> Rust-internal type organization (per-BC enums, use-case composites, serde tagging) is out of scope for this contract — it documents the BE↔FE frontier, not Rust internals.

---

## Commands

| Command            | Args | Return               | Errors                                                                                                                                                       |
| ------------------ | ---- | -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `check_for_update` | —    | `Option<UpdateInfo>` | _(none — network/server errors are silent per R21; command returns None)_                                                                                    |
| `download_update`  | —    | `()`                 | _(none — returns immediately (R6, R7); errors emitted as `update:error` event (R23); re-invoke to retry per R24; concurrent calls silently ignored per R10)_ |
| `install_update`   | —    | `()`                 | `NoUpdateReady` _(precondition guard — inferred from R13: install requires a completed download)_                                                            |

---

## Shared Types

```rust
struct UpdateInfo {
    version: String,  // semantic version of available update (e.g. "1.2.3")
}

struct UpdateProgress {
    percent: u64,  // download completion 0–100 (R8)
}
```

---

## Events

| Event              | Payload          | Rule    |
| ------------------ | ---------------- | ------- |
| `update:available` | `UpdateInfo`     | R1, R25 |
| `update:progress`  | `UpdateProgress` | R8      |
| `update:complete`  | —                | R11     |
| `update:error`     | error string     | R23, R9 |
