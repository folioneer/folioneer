# Contract — Sync

> Domain: `sync`
> Last updated by: `multi-device-sync` + `sync-conflict-resolution` specs

> **Error model on the wire**: each command's error serializes as a flat `{ code: "VariantName", ...payload }` object. The FE matches on `code`. Per-command reachable codes are listed in the "Errors" column. Infrastructure failures surface as `{ code: "DatabaseError" }` (no payload; diagnostic chain preserved server-side via `tracing::error!`), consistent with the other contracts. Precondition guards not stated as rejections in the spec are marked _(precondition guard — inferred)_.
>
> **Runs return a report, never throw for the folder's state.** A sync run that is update-gated (SYN-035), finds the folder unavailable (SYN-069), or detects a reset (SYN-084) is a partial success — it still recorded and, where possible, published — so `sync_now` / `resume_sync` return a `SyncReport` whose `failures` carry the condition (SYN-062: "the application reports no error for this condition"). Errors are reserved for calls that cannot start or cannot complete their own action.
>
> **What is not a command.** Recording changes (SYN-020), the change log's ordering and identity (SYN-021..027), publishing and applying (SYN-030..033, 036, 037, 060, 065, 067), encryption (SYN-050..052, 055), the headless scheduled run's publish step (SYN-068), the joiner's observation replacement (SYN-083), and every merge outcome (CFR-010..060) are backend-internal: they run inside existing writes, inside `sync_now` / automatic syncs, or inside the scheduled-fetch binary path. They have no command of their own. Four of them surface through `SyncStatus` — unreadable files (SYN-034), the format gate (SYN-035), inconsistent holdings (SYN-040), held-back changes (SYN-041). Every other backend-scoped SYN rule is an internal mechanism with no interface. All merge outcomes are decided by the single backend resolution component (ADR-019); the frontend only reads `SyncStatus`.
>
> **Passphrase confirmation** ("entered twice", SYN-011) is a frontend check; the backend receives one passphrase. Destructive confirmations (SYN-071) are frontend dialogs; the backend trusts the call.
>
> **Launch ordering (SYN-060).** The launch sync runs in the backend _inside_ `apply_due_fee_deductions` (account contract), before any generation, when sync is enabled and not paused — the frontend keeps calling that one command on startup; the launch run's completion reaches it through `SyncCompleted` when the run applied changes or changed the device state (see Events) — a bare marker after which it re-reads `get_sync_status` — never through that command's return value.
>
> **One condition, two wire shapes.** `UpdateRequired` and `FolderUnavailable` exist both as error codes (on calls that cannot start) and as `SyncFailure` variants (on runs that completed partially). They describe the same user-facing condition; the frontend presenter maps both shapes to one message.
>
> **Wire conventions**: identifiers and logical timestamps are strings; instants are ISO-8601 strings; the folder is an absolute path string; money and quantities are `i64` micros (ADR-001).

---

## Commands

### Setup and membership

> `inspect_sync_folder` is the pre-flight read behind the two-step enable modal (SYN-011, SYN-014, SYN-019): it tells the frontend whether the folder is usable, whether it already holds a portfolio (join wording, passphrase once) or not (first device, passphrase twice), whether this installation may join, and whether the portfolio's format is readable. It changes nothing.

| Command               | Args                                                      | Return            | Errors                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| --------------------- | --------------------------------------------------------- | ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `inspect_sync_folder` | `folder: String`                                          | `SyncFolderState` | `DatabaseError` (SYN-011/014/019 — never rejects: every condition is reported in the returned state)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `enable_sync`         | `folder: String, passphrase: String, device_name: String` | `SyncStatus`      | `AlreadyEnabled` _(precondition guard — inferred from SYN-010)_, `PassphraseTooShort { minimum: u32 }` (SYN-012), `DeviceNameBlank` (SYN-018), `FolderUnavailable { problem: FolderProblem }` (SYN-019/069), `UpdateRequired { data_format_version: u32 }` (SYN-019/035), `PortfolioCreatedElsewhere` (SYN-081), `InstallationHoldsUserData` (SYN-014), `HeaderRejected` (SYN-051 — the folder header's key-derivation parameters are outside the range this build derives with; the header is never derived against), `PassphraseMismatch` (SYN-015/055), `HistoryIncomplete` (SYN-036 — also raised when a segment of the replay set is unreadable: a join never skips, SYN-034 applies to steady-state syncs only), `PublishFailed { problem: FolderProblem }` (SYN-013 — rolled back), `RebuildInterrupted` (SYN-080 — rolled back), `DatabaseError` |
| `start_sync_over`     | `folder: String, passphrase: String, device_name: String` | `SyncStatus`      | `PassphraseTooShort { minimum: u32 }` (SYN-012), `DeviceNameBlank` (SYN-018), `FolderUnavailable { problem: FolderProblem }` (SYN-069), `PublishFailed { problem: FolderProblem }` (SYN-013/071 — covers a failure while clearing as well as while publishing; the folder is left empty and the device may retry as a first device), `DatabaseError` _(callable whether or not sync is currently enabled on this device, SYN-071; an enabled device is re-enrolled as the new origin)_                                                                                                                                                                                                                                                                                                                                                                   |
| `leave_sync`          | —                                                         | `()`              | `SyncDisabled` _(precondition guard — inferred from SYN-010)_, `FolderUnavailable { problem: FolderProblem }` (SYN-082/069), `PublishFailed { problem: FolderProblem }` (SYN-082 — unpublished work is never abandoned: the device stays enabled), `DatabaseError` _(from a device that has detected a reset — `SyncStatus.failures` contains `PortfolioReset` — leaving skips the publish step entirely, SYN-084: nothing is ever written under the old key)_                                                                                                                                                                                                                                                                                                                                                                                           |

### Running

| Command           | Args | Return       | Errors                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| ----------------- | ---- | ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `sync_now`        | —    | `SyncReport` | `SyncDisabled` _(precondition guard — inferred from SYN-010)_, `SyncPaused` (SYN-070), `DatabaseError` _(SYN-061; unreadable files, the format gate, an unavailable folder, and a reset are reported in `SyncReport.failure`, not thrown — SYN-034/035/069/084)_                                                                                                                                                                                                    |
| `pause_sync`      | —    | `SyncStatus` | `SyncDisabled` _(precondition guard — inferred from SYN-010)_, `AlreadyPaused` _(precondition guard — inferred from SYN-070)_, `DatabaseError`                                                                                                                                                                                                                                                                                                                      |
| `resume_sync`     | —    | `SyncReport` | `SyncDisabled` _(precondition guard — inferred from SYN-010)_, `NotPaused` _(precondition guard — inferred from SYN-073)_, `FolderUnavailable { problem: FolderProblem }` (SYN-073/069 — the device stays paused), `PublishFailed { problem: FolderProblem }` (SYN-073 — the device stays paused), `DatabaseError` _(once the paused changes are published the run continues as `sync_now`: the format gate and a reset land in `SyncReport.failure`, SYN-035/084)_ |
| `get_sync_status` | —    | `SyncStatus` | `DatabaseError` (SYN-063 — when sync is disabled, `enabled = false`, the `Option` fields are `None`, the collections are empty, and `app_version` is still stated)                                                                                                                                                                                                                                                                                                  |

### Managing this device

| Command                   | Args                  | Return       | Errors                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ------------------------- | --------------------- | ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `rename_sync_device`      | `device_name: String` | `SyncStatus` | `SyncDisabled` _(precondition guard — inferred from SYN-010)_, `DeviceNameBlank` (SYN-018/072), `DatabaseError` _(the manifest is republished at the next sync; an unavailable folder does not fail the rename — SYN-069)_                                                                                                                                                                                                                                                                                                                                                                                                     |
| `change_sync_folder`      | `folder: String`      | `SyncStatus` | `SyncDisabled` _(precondition guard — inferred from SYN-010)_, `FolderUnavailable { problem: FolderProblem }` (SYN-074/069), `FolderHoldsOtherPortfolio` (SYN-074 — passphrase check does not match the kept key and the folder is not empty), `UpdateRequired { data_format_version: u32 }` (SYN-035 — the same portfolio republished in a newer format), `HeaderRejected` (SYN-051 — empty-folder path: the current folder's header carries key-derivation parameters outside this build's bounds, so they are never republished), `PublishFailed { problem: FolderProblem }` (SYN-013 — empty-folder path), `DatabaseError` |
| `dismiss_conflict_notice` | `notice_id: String`   | `()`         | `SyncDisabled` _(precondition guard — inferred from SYN-010)_, `NoticeNotFound { notice_id: String }` (SYN-066), `DatabaseError`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |

---

## Shared Types

```rust
// Pre-flight read of a candidate folder (SYN-011/014/019). Never rejects.
struct SyncFolderState {
    problem: Option<FolderProblem>,         // None when the folder exists and is writable; the problem otherwise
    holds_portfolio: bool,                  // a folder header is present
    data_format_version: Option<u32>,       // Some when holds_portfolio; compare with this build's data format (SYN-035)
    format_readable: bool,                  // false → joining would raise UpdateRequired
    installation_holds_user_data: bool,     // true → joining would raise InstallationHoldsUserData (SYN-014)
}

// Why a folder cannot be used (SYN-019/069). Structured so the frontend can translate it.
enum FolderProblem {
    Missing,
    NotADirectory,
    PermissionDenied,
    Unmounted,
    OutOfSpace,
    IoFailure,          // catch-all alongside the named causes: a mid-write failure, a failed whole-file rename (SYN-032), or an encryption failure during publish
}

// What the Settings section and the shell indicator read (SYN-063).
struct SyncStatus {
    enabled: bool,
    paused: bool,
    device_id: Option<String>,                       // None while disabled
    device_name: Option<String>,                     // None while disabled
    folder: Option<String>,                          // None while disabled
    app_version: String,                             // the application running on this device (SYN-063); stated whether or not sync is enabled
    last_sync_completed_at: Option<String>,          // None when never synced
    roster: Vec<RosterEntry>,                        // every other device (SYN-037)
    held_back_count: u32,                            // SYN-041
    oldest_held_back_since: Option<String>,          // None when held_back_count == 0
    notices: Vec<ConflictNotice>,                    // undismissed only (SYN-066)
    inconsistent_holdings: Vec<InconsistentHolding>, // derived on read from the account BC's replayed ledger (CFR-042 / SYN-040) — a cross-BC read, like account_details
    failures: Vec<SyncFailure>,                      // empty when the last run was healthy; several may hold at once (SYN-034 + SYN-035, SYN-034 + SYN-069)
}

struct RosterEntry {
    device_id: String,
    device_name: String,
    data_format_version: u32,
    app_version: Option<String>,            // what its manifest states; None when it does not say (SYN-037) — information only
    last_applied_at: Option<String>,        // when its changes were last applied here; None if never
}

// One persisted, undismissed notice (SYN-066, CFR-060).
struct ConflictNotice {
    notice_id: String,
    kind: ConflictNoticeKind,
    record_kind: RecordKind,
    record_identity: String,                // canonical string of the CFR-012 key
    record_label: String,                   // human-readable, captured when raised
    other_device_id: String,
    other_device_name: String,
    raised_at: String,
}

// Exactly CFR-060's reportable outcomes.
enum ConflictNoticeKind {
    OverruledEdit,
    OverruledRemoval,
    DroppedChild,
    NaturalKeyCollision,
    DuplicateName,
}

// Exactly SYN-021's synced kinds.
enum RecordKind {
    Account,
    Category,
    Asset,
    Transaction,
    FeeSchedule,
    FeeCatchUpPosition,
    AssetPrice,
    CurrencyPair,
    CurrencyRate,
    HoldingNote,
}

// A holding whose merged ledger breaks an invariant (CFR-042). Derived on read, never stored.
struct InconsistentHolding {
    account_id: String,
    account_name: String,
    asset_id: String,
    asset_name: String,
    reason: HoldingInconsistency,
}

// Shared with account-contract.md (HoldingDetail.inconsistency).
enum HoldingInconsistency {
    Oversold { quantity: i64 },             // micros; the replayed quantity, negative by construction
    CashOverdrawn { amount: i64 },          // micros, account currency; the replayed cash balance, negative
}

// Why the last run needs attention (SYN-063). Any number may apply at once.
enum SyncFailure {
    UnreadableFiles { count: u32 },         // SYN-034
    UpdateRequired { data_format_version: u32 }, // SYN-035
    FolderUnavailable { problem: FolderProblem }, // SYN-069
    PortfolioReset,                         // SYN-084 — the device has paused itself
}

// Outcome of one run — the return value of sync_now / resume_sync. An automatic run's outcome
// reaches the frontend through get_sync_status after SyncCompleted, never as an event payload.
struct SyncReport {
    published_changes: u32,
    applied_changes: u32,
    held_back_changes: u32,
    dropped_changes: u32,                   // CFR-032
    notices_raised: u32,
    failures: Vec<SyncFailure>,             // empty when the run completed cleanly
    completed_at: String,
    status: SyncStatus,                     // the device state after the run (SYN-084 may have paused it) — no follow-up get_sync_status needed
}
```

Record identity per kind follows CFR-012: accounts, categories, assets, transactions by id; fee schedules and catch-up positions by `account_id + asset_id`; currency pairs by `from + to`; prices by `asset_id + date`; rates by `pair + date`; holding notes by `account_id + asset_id`. On the wire `record_identity` is the canonical string form of that key.

---

## Events

| Event                                                                                                                                                                                    | Payload                                                      | Rule                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `SyncCompleted`                                                                                                                                                                          | — (none); the frontend re-reads `get_sync_status` on receipt | SYN-064/063 — raised once, after the apply transaction commits, at the end of every run (automatic, launch inside `apply_due_fee_deductions`, `sync_now`, `resume_sync`) that applied at least one change, and at the end of a join, **or** whose `failures` or `paused` state differ from the previous run's — so a background reset, format gate, or unavailable folder reaches the shell indicator without polling. The event carries nothing: the frontend treats it as a global refresh — every view that shows synced data reloads once — and re-reads `get_sync_status` for the device state the run left behind (`sync_now` / `resume_sync` still return their `SyncReport` directly) |
| `AccountUpdated`, `TransactionUpdated`, `AssetUpdated`, `CategoryUpdated`, `AssetPriceUpdated`, `FeeScheduleUpdated`, `CurrencyRateUpdated`, `HoldingNoteUpdated`, `CurrencyPairUpdated` | as today                                                     | SYN-064 — raised by local writes only; applying another device's change raises none of them (the run's single `SyncCompleted` covers everything it applied)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |

---

## Cross-contract amendments (applied in their own contracts)

- `account-contract.md` — `HoldingDetail` gains `inconsistency: Option<HoldingInconsistency>` (SYN-040, CFR-042); `AccountSummary` gains `has_inconsistent_holding: bool` (SYN-040); both derived on read. `FeeSchedule.last_applied_period` is the derived read of the synced catch-up record (CFR-044, FEE-043). `apply_due_fee_deductions` runs the launch sync first (SYN-060). `Account.name` uniqueness and `update_account`'s `NameAlreadyExists` bind the name being set (CFR-035).
- `account-contract.md`, `asset-contract.md`, `currency-contract.md` — applied changes raise no event of their own; their views subscribe to `SyncCompleted` (SYN-064).
- `asset-contract.md` — `Asset.category` resolves to the default category when the stored category stands removed (CFR-030); `update_category`'s `DuplicateName` binds the name being set (CFR-035).

---

## Changelog

- 2026-09-19 — SYN-037/063 amended: `SyncStatus.app_version` and `RosterEntry.app_version` added. No command, error or event changes; the manifest's new field is ignored by older builds, so the data format version stays 1 (SYN-038 pins the written form).
- 2026-09-19 — SYN-064 amended (reverses the 2026-09-12 entry): applying a change raises no event of its own; one `SyncCompleted` per run that applied changes or whose failures or paused state changed, raised after the apply commits, and one per join; every view that shows synced data reloads on it
- 2026-09-12 — Events row amended: applied holding notes and currency pairs now raise `HoldingNoteUpdated` / `CurrencyPairUpdated` (SYN-064), so `SyncCompleted` no longer stands in for them
- 2026-08-22 — `SyncCompleted` becomes a bare marker (no `SyncReport` payload; the frontend re-reads `get_sync_status`); `HeaderRejected` added to `enable_sync` and `change_sync_folder` (SYN-051 key-derivation parameter bounds).
- 2026-08-22 — Added by `multi-device-sync` + `sync-conflict-resolution` specs: `inspect_sync_folder`, `enable_sync`, `start_sync_over`, `leave_sync`, `sync_now`, `pause_sync`, `resume_sync`, `get_sync_status`, `rename_sync_device`, `change_sync_folder`, `dismiss_conflict_notice`; types `SyncFolderState`, `FolderProblem`, `SyncStatus`, `RosterEntry`, `ConflictNotice`, `ConflictNoticeKind`, `RecordKind`, `InconsistentHolding`, `HoldingInconsistency`, `SyncFailure`, `SyncReport`; event `SyncCompleted`.
