# Business Rules — Multi-Device Sync (SYN)

## Context

The user keeps one portfolio but works on more than one computer. Today each installation holds its own isolated copy, so whatever is recorded on one machine is missing on the other. Multi-Device Sync lets every installation converge on the same portfolio: each device records what it changes, publishes those changes into a folder the user already keeps synchronised across machines (Dropbox, Google Drive, OneDrive, Syncthing, …), and applies the changes published by the other devices.

The application itself never talks to a network. It reads and writes files in a local folder; the user's existing cloud client moves those files between machines. Everything written into that folder is encrypted on the device before it is written, with a key derived from a **sync passphrase** the user chooses. The cloud provider holds bytes it cannot read. The local database on each device stays unencrypted, exactly as today — a device in the user's hands is always readable; the encryption protects only the copy that leaves the machine.

This is a **feature spec**: it adds a cross-cutting capability on top of every bounded context that owns persisted user data (`account`, `asset`, `currency`) and introduces a sync surface of its own. It is deliberately scoped to **desktop installations used one at a time**; concurrent editing on two devices is tolerated and merged, but is not the design centre. A possible later extension to a phone is out of scope, and the rules below are written so that it would extend this design rather than replace it. The structural choice — a per-device append-only change log rather than whole-database snapshots — is recorded in ADR-019. The outcome of every concurrent-change situation is specified separately in `sync-conflict-resolution.md` (CFR).

**Vocabulary.** Throughout this spec, _transaction_ keeps its portfolio meaning — a recorded purchase, sale, dividend, split, fee deduction, and so on. It never refers to the database mechanism. The sync vocabulary is **change** (one recorded modification of one record), **segment** (a file carrying a batch of changes from one device), **manifest** (a device's published identity card), **area** (the part of the folder one device publishes into), **roster** (the set of manifests — every device sharing the portfolio), **sync cursor** (how far a device has applied another device's changes — distinct from the fee schedule's catch-up cursor, FEE-043), and **logical timestamp** (the per-change ordering value that decides which of two independent edits is the later one). Rates between currencies are **currency rates**, never "exchange rates" — the latter is reserved for a transaction's trade-time rate.

---

## Entity Definition

### SyncDevice

One installation participating in sync. Each device is known to the others by a stable identity it chose for itself when it joined.

| Field         | Business meaning                                                                                                 |
| ------------- | ---------------------------------------------------------------------------------------------------------------- |
| `device_id`   | The stable identity of this installation, unique among all devices sharing the folder.                           |
| `device_name` | A user-friendly label ("Laptop", "Office desktop") shown in sync status.                                         |
| `folder`      | The location of the synchronised folder on this device.                                                          |
| `joined_at`   | When this device joined the shared portfolio.                                                                    |
| `paused`      | Whether sync is currently paused on this device (SYN-070). A paused device keeps its identity.                   |
| `portfolio`   | Which folder header this device follows — its creation mark (`created_at`) — so a reset (SYN-084) is recognised. |

### Change

One recorded modification of one record on one device. Changes are the unit of exchange between devices.

| Field               | Business meaning                                                                                                                                                                         |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `device_id`         | The device on which the change was made.                                                                                                                                                 |
| `sequence`          | The position of this change in that device's own history; strictly increasing, never reused.                                                                                             |
| `logical_timestamp` | The ordering value that decides which of two independent changes to the same record is the later one (CFR-010). Advances past every change the device has seen, so it is never "behind". |
| `based_on`          | The logical timestamp of the record state this change was made against; absent for a creation. Decides whether the change is concurrent with what another device holds (CFR-011).        |
| `record_kind`       | What kind of record changed (account, category, asset, transaction, fee schedule, fee catch-up position, asset price, currency pair, currency rate, holding note).                       |
| `record_identity`   | Which record changed — the record's own identity, as defined per kind in CFR-012.                                                                                                        |
| `operation`         | Whether the record was created, updated, or removed.                                                                                                                                     |
| `origin`            | Whether the user made the change or the application generated it on its own (CFR-016).                                                                                                   |
| `content`           | The full state of the record after the change; absent for a removal.                                                                                                                     |

### Tombstone

What a removal leaves behind (CFR-015): stands in for the removed record when a later or earlier change to it arrives. Kept permanently; re-derived by replaying the history on a rebuild.

| Field               | Business meaning                                          |
| ------------------- | --------------------------------------------------------- |
| `record_kind`       | The kind of the removed record.                           |
| `record_identity`   | Its identity (CFR-012).                                   |
| `logical_timestamp` | The removal's logical timestamp.                          |
| `origin`            | Whether the user or the application removed it (CFR-016). |
| `removed_by`        | The device that removed it, named in conflict notices.    |

### ConflictNotice

A persisted notice of an outcome CFR-060 lists as reportable, shown in sync status until the user dismisses it.

| Field          | Business meaning                                                                                      |
| -------------- | ----------------------------------------------------------------------------------------------------- |
| `notice_id`    | Identifies the notice, so it can be dismissed individually.                                           |
| `kind`         | Overruled edit, overruled removal, dropped child, natural-key collision, or duplicate name (CFR-060). |
| `record`       | The record concerned — kind, identity, and a human-readable label captured at the time.               |
| `other_device` | The device whose change prevailed or removed the parent.                                              |
| `raised_at`    | When the notice was raised.                                                                           |
| `dismissed`    | Whether the user has dismissed it.                                                                    |

### SyncCursor

How far this device has applied another device's history.

| Field             | Business meaning                                                                                                       |
| ----------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `device_id`       | The other device.                                                                                                      |
| `applied_through` | The last of that device's changes this device has taken in — applied, or held back pending a missing record (SYN-041). |

### HeldBackChange

A change received from another device that cannot be applied yet because it refers to a record this device has not received (SYN-041). Kept on the device until it can be applied or is discarded.

| Field         | Business meaning                                                                                                                  |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `change`      | The change itself, exactly as received.                                                                                           |
| `waiting_for` | What would unblock it: a record (kind and identity) not yet received, or a state of its own record (`based_on`) not yet received. |
| `held_since`  | When it was first held back, shown in sync status.                                                                                |

### Manifest

A device's published identity card, the only published file that is rewritten in place (SYN-037).

| Field                 | Business meaning                                                                                                                                                |
| --------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `device_id`           | The publishing device.                                                                                                                                          |
| `device_name`         | Its current name.                                                                                                                                               |
| `data_format_version` | The data format of the application that last published from this device.                                                                                        |
| `app_version`         | The released version of the application that last synced from this device; absent from a manifest written before the field existed. Information only (SYN-037). |
| `latest_sequence`     | The last change this device has published.                                                                                                                      |

### Segment

A published file carrying a consecutive batch of one device's changes.

| Field                 | Business meaning                                        |
| --------------------- | ------------------------------------------------------- |
| `device_id`           | The publishing device.                                  |
| `first_sequence`      | The first change in the batch.                          |
| `last_sequence`       | The last change in the batch.                           |
| `data_format_version` | The data format the changes are expressed in (SYN-035). |

### FolderHeader

The one readable file in the folder: what every device needs to derive the encryption key from the passphrase, and to check the passphrase before anything is rebuilt. Contains no secret.

| Field                   | Business meaning                                                                                         |
| ----------------------- | -------------------------------------------------------------------------------------------------------- |
| `derivation_parameters` | The public inputs (salt and cost settings) every device combines with the passphrase (SYN-051).          |
| `passphrase_check`      | An encrypted marker that decrypts correctly only with the right passphrase (SYN-055).                    |
| `data_format_version`   | The data format of the device that created the portfolio.                                                |
| `created_at`            | The creating device's logical timestamp and identity at creation; decides between two headers (SYN-081). |

---

## Business Rules

### Setup and Joining (010–019)

**SYN-010 — Opt-in (frontend + backend)**: Sync is off by default. An installation behaves exactly as today until the user enables sync on it. Enabling on one device never affects another.

**SYN-011 — Enabling requires a folder and a passphrase (frontend + backend)**: To enable sync the user designates the synchronised folder on this device and supplies the sync passphrase. The passphrase is entered twice when it is being chosen for the first time (the folder holds no portfolio yet) and once when joining an existing portfolio.

**SYN-012 — Passphrase minimum length (frontend + backend)**: The passphrase must be at least 12 characters. No composition requirement is imposed. A strength indication is shown while typing and is advisory only.

**SYN-013 — First device publishes the whole portfolio (backend)**: When the folder holds no portfolio, enabling sync creates the folder header and publishes this device's complete current portfolio as its first segment — one creation change per record, all stamped with that segment's logical timestamp — then its manifest. Nothing the user already recorded is left behind. If publishing fails partway (folder unwritable, out of space), the action is rejected with a specific error, what was written is removed, and sync stays disabled.

**SYN-014 — Joining device must hold no user data (frontend + backend)**: When the folder already holds a portfolio, the joining device must have **no user-entered records** — no accounts, no user-created assets or categories, no transactions, no manually recorded prices or currency rates; system-seeded records (SYN-027) and automatically fetched observations do not count. It rebuilds its local portfolio by replaying the shared history (SYN-035). An installation holding user data cannot join; the action is rejected with a specific error stating that a fresh installation is required, and nothing is changed. (A device that previously synced this same portfolio and paused does not join — it resumes, SYN-073.)

**SYN-015 — Wrong passphrase is detected (frontend + backend)**: Joining with a passphrase that does not match the one the portfolio was encrypted with is rejected with a specific error before anything is rebuilt (SYN-055). The error states only that the passphrase does not match.

**SYN-016 — Device identity (backend)**: On enabling, the device generates a stable random identity (a version-4 UUID, whose collision chance is negligible) and keeps it for its lifetime; two devices sharing a folder never present the same identity.

**SYN-017 — Honest positioning (frontend)**: Every surface that enables or explains sync states that the shared copy is encrypted and that the local copy on each device is not. The wording never implies that the data on this machine is protected by the passphrase.

**SYN-018 — Device name (frontend + backend)**: On enabling, the user gives the device a name shown in every device's sync status. The name is required and non-blank.

**SYN-019 — Folder validation (frontend + backend)**: The designated folder must exist and be writable; otherwise enabling is rejected with a specific error. A folder that holds a portfolio created under a data-format version newer than this device can read is rejected with the update-required error (SYN-035).

### Recording Changes (020–029)

**SYN-020 — Every write is recorded (backend)**: On a device with sync enabled — paused or not (SYN-070) — every creation, update, and removal of a synced record **made on this device** produces exactly one change, recorded together with the write it describes so that neither can exist without the other. A write the application makes on its own is first compared as CFR-020 compares an incoming change (CFR-016); if it does not outrank the record's current state it is not made and no change is recorded. Applying another device's change never records a change. Every synced record keeps the logical timestamp, origin, and device of the change that produced its current state (CFR-014).

**SYN-021 — Synced records (backend)**: The following kinds of records are synced: accounts, categories, assets, transactions (every type, including generated fee deductions), fee schedules, fee catch-up positions (CFR-044), asset prices, currency pairs, currency rates, holding notes. A change on any of these is recorded and published. Record identity per kind is defined in CFR-012; fee schedules, catch-up positions, and currency pairs take their identity from their natural key (CFR-034).

**SYN-022 — Derived data is never synced (backend)**: Holdings, performance figures, and every other value the application computes from synced records are not synced. Each device recomputes them. Two devices holding the same records therefore show the same figures.

**SYN-023 — Device-local data is never synced (backend)**: Data that describes this installation rather than the portfolio — scheduled download configuration and run history, the moment this device last fetched prices (MKT-201), window layout, the release whose notes the user last saw, and the sync configuration itself (device identity, folder, cursors) — stays on the device. The device name and the application version travel via the manifest (SYN-037), not as synced records.

**SYN-024 — Removal leaves a trace (backend)**: Removing a synced record produces a change that records the removal and leaves a permanent tombstone (CFR-015). A removal that cascades — an account taking its transactions, holding notes, and fee schedules with it — produces one removal change and one tombstone per removed record, so every device removes the same set. Without these traces, another device would have no way to learn the records are gone and would bring them back.

**SYN-025 — Changes are ordered per device (backend)**: Every device numbers its own changes strictly increasingly. Numbers are never reused, even after the change is published or the application restarts.

**SYN-026 — History recorded while not yet publishing is never lost (backend)**: Records created, updated, or removed before sync was first enabled are included in the first segment (SYN-013). Changes recorded while sync is paused (SYN-070) are published on resume (SYN-073). There is no period of a device's history that sync cannot see.

**SYN-027 — System-seeded records are never synced (backend)**: Records the application creates on its own with a fixed, deterministic identity — the cash asset per currency and the cash category (CSH-011, CSH-017) — are not synced. Applying a change that refers to such an identity ensures the record exists locally, through the same idempotent seeding the application already performs at account creation (CSH-010); these identities are therefore never held back (SYN-041).

### Exchanging Changes (030–039)

**SYN-030 — Each device writes only in its own area (backend)**: Inside the synchronised folder, every device publishes into an area named by its identity. A device never writes into another device's area, with exactly two exceptions, each stated where it applies: the folder header is written once by the first device (SYN-013, SYN-081), and start-over clears the whole folder (SYN-071).

**SYN-031 — Published segments are never modified (backend)**: A device publishes changes by adding new segment files; it never rewrites or truncates a segment it has already published. This is what lets ordinary cloud clients move the files safely: there is never a file that two machines write, and a file is complete the moment it exists.

**SYN-032 — Files appear whole or not at all (backend)**: A segment or manifest becomes visible to other devices only once it is complete. A file that is still being written, or that arrived partially through the cloud client, is never mistaken for a finished one.

**SYN-033 — Sync cursors make steady-state sync incremental (backend)**: For every other device, this device remembers the last change it has taken in — applied or held back (SYN-041) — and reads only what comes after. Re-reading a device's whole history is never required once it has been taken in.

**SYN-034 — Unreadable segments and manifests are skipped and reported, never applied (backend)**: A segment or manifest that cannot be decrypted or whose content fails validation is ignored, the failure is surfaced in sync status (SYN-063), and the file is retried at the next sync. A device never applies part of a file. The folder header is not covered here: a header whose passphrase check fails is the reset case (SYN-084).

**SYN-035 — Data format version gate (frontend + backend)**: Every published file carries the data format version of the application that wrote it. A device running an older data format than a file it encounters stops applying changes, keeps recording and publishing its own, and tells the user that the application must be updated before sync can resume. It never applies records it does not understand, and never publishes in a format newer than its own. Publishing continues deliberately: the gated device's edits are real user intent and apply on the others once it is updated; where they collide with edits it could not see, CFR-020 and SYN-066 apply as usual. In the other direction, a device applying a change written in an **older** format upgrades it on apply: fields introduced after that format keep their current local value rather than being erased — the one field-level exception CFR-020 names.

**SYN-036 — Published history is complete and never removed (backend)**: Every segment ever published stays in the folder; no device removes segments, its own or another's, except under start-over (SYN-071). A joining device (SYN-014) rebuilds its portfolio by replaying every area from its first segment, in logical-timestamp order across devices (CFR-010); if any area is incomplete the rebuild is rejected with a specific error and nothing is changed. A portfolio's history is small — a few kilobytes per month — so nothing cleverer is needed.

**SYN-037 — Manifest and roster (backend)**: Every device publishes a manifest in its own area, rewritten in place whenever its name, data format version, or latest published change advances. The manifest also states the released version of the application that wrote it — the version the rest of the interface shows. A device rewrites its manifest at every sync it runs, so the roster tells which version each device last synced with. That version is information for the user and gates nothing — the data format version is the only gate (SYN-035). A device ignores a manifest field it does not know, and reads a manifest without the application version as a device whose version is unknown; before a field is added to the manifest within a data format version, the previous release is shown to read the result. The set of manifests is the roster: how every device learns which devices exist, what they are called, and how far each has published. A manifest is rewritten only by its own device. A device that will never return simply stays in the roster; it costs nothing.

**SYN-038 — A data format version pins one written form (backend)**: A data format version stands for one written form of everything a device publishes: the fields and their types of each kind of synced record, every value of every enumeration those records and their envelope carry, and the shape of the folder header, the manifest and the segment. A build writes only the form pinned for its own data format version, and the form pinned for a version that has shipped never changes: a change to any of it arrives as a new data format version, which is what makes the gate of SYN-035 hold for devices that have not been updated. The form pinned for version 1 is the one written from the release that introduced this rule; it differs from earlier releases only by the manifest's application version, which they ignore (SYN-037).

### Merge Outcomes in the Application (040–049)

> The outcome of every concurrent situation is defined in the Sync Conflict Resolution spec (CFR). The two rules below cover only what the application does around those outcomes.

**SYN-040 — Inconsistent holding: what the user sees and can do (frontend + backend)**: An inconsistent holding (CFR-042) is marked as such, with the reason, on the account-details holding row (cross-amends ACD) and in the sync status; the accounts list marks an account that contains one (cross-amends ACC). Its figures are computed from the merged ledger as they stand. The user resolves it by correcting or removing one of the transactions through the existing journal flows; new sales on an inconsistent position, and new purchases against inconsistent cash, are rejected by the existing oversell and insufficient-cash guards until it is resolved.

**SYN-041 — Changes that reference what this device has not received are held back (backend)**: A change is held back, not rejected, when it refers to a record other than the one it changes (its account, its asset, its schedule, …) that this device holds neither as a record nor as a tombstone, or when it is based on a state of its own record (`based_on`) this device has not yet received (CFR-011) — because the other device's area arrived first. It is kept on the device (HeldBackChange), the sync cursor advances past it, and it is applied at the first sync after what it waits for arrives (CFR-031). If what arrives is the tombstone of its account, it is dropped under CFR-032. Held-back changes survive restarts and are counted in sync status (SYN-063).

### Encryption (050–059)

**SYN-050 — Everything published is encrypted on the device (backend)**: No segment or manifest is written into the synchronised folder in readable form. The only readable file is the folder header (SYN-051, SYN-055), which contains no secret and no portfolio content.

**SYN-051 — The key is derived from the passphrase (backend)**: The encryption key is derived from the sync passphrase and the folder header's public derivation parameters through a deliberately slow derivation, so that the passphrase cannot be recovered from the published files by trying candidates quickly. Every device sharing the portfolio derives the same key from the same passphrase.

**SYN-052 — The passphrase is never stored in readable form (backend)**: The passphrase is held only long enough to derive the key. The derived key is kept on the device so that automatic sync (SYN-060) does not prompt for the passphrase on every launch. The passphrase itself is never written to storage, logs, or diagnostics.

**SYN-053 — No recovery (frontend + backend)**: If the passphrase is forgotten, the published copy cannot be decrypted and there is no recovery path. The setup surface states this plainly. Because every device's local copy remains readable, a forgotten passphrase is survivable from any device that still holds the portfolio: the user starts over with a new passphrase from that device (SYN-071).

**SYN-054 — What the folder host can still see (frontend)**: The setup surface states that whoever hosts the folder can observe file names, sizes, and times — and therefore when and roughly how much the portfolio is used, and how many devices share it — plus the folder header's public derivation parameters; but not accounts, holdings, amounts, or any other content.

**SYN-055 — Passphrase check (backend)**: The folder header carries a marker encrypted under the derived key. A device verifies the passphrase by decrypting the marker before reading or publishing anything else; failure is SYN-015.

### Running Sync (060–069)

**SYN-060 — Automatic sync (backend)**: On a device with sync enabled and not paused, a sync runs on application launch and after recorded changes (SYN-067). A sync publishes this device's unpublished changes, reads other devices' manifests, and applies their unapplied changes. On launch, the sync runs **before** the startup fee catch-up (FEE-040), so generation sees the merged ledger.

**SYN-061 — Manual sync (frontend + backend)**: A visible "Sync now" action runs a sync immediately.

**SYN-062 — Offline is not an error (backend)**: The application never contacts a network. When the cloud client is offline, publishing still succeeds locally and the files travel once connectivity returns. The application reports no error for this condition; it reports only what it can observe — the last successful sync, unreadable files, held-back changes, and an unavailable folder (SYN-069).

**SYN-063 — Sync status (frontend + backend)**: The user can see whether sync is enabled or paused on this device, this device's name and the version of the application it runs (the status states that version whether or not sync is enabled; the interface shows it with the enabled status), when the last sync completed, the roster with each device's name, the application version its manifest states (or that it is unknown, SYN-037) and the time its changes were last applied here, the count and age of held-back changes (SYN-041), the conflict notices (SYN-066), inconsistent holdings (SYN-040), and any failure from SYN-034, SYN-035, SYN-069, or SYN-084. Conflict notices persist until the user dismisses them; the inconsistent-holding marker is derived on every recomputation and clears by itself when the ledger is valid again. The compact indicator in the header is absent while sync is not enabled (SYN-010); it names its subject in words — the user's devices, "computers" in the interface — and carries its own icon, so it cannot be read as a price refresh (MKT-202 sits beside it); its attention badge carries a tooltip saying that sync needs attention.

**SYN-064 — Sync never blocks the user and announces itself once (frontend + backend)**: Publishing and applying happen without locking the interface. The user may keep working; a local write and an in-progress apply never interleave — one completes before the other starts (SYN-020, SYN-065). Applying a change raises no event of its own — the domain events other rules define (`AccountUpdated`, `TransactionUpdated`, `AssetUpdated`, `CategoryUpdated`, `AssetPriceUpdated`, `FeeScheduleUpdated`, `CurrencyRateUpdated`, `CurrencyPairUpdated`, `HoldingNoteUpdated`) are raised by writes made on this device only. A sync that applied changes — and a join that rebuilt the portfolio — is announced once, by the `SyncCompleted` event, once the changes it applied have become visible together (SYN-065); a sync that applied nothing still raises it when its failures or its paused state differ from the previous sync's, so the sync indicator follows a reset, a format gate or an unavailable folder without polling. The number of events therefore never grows with the number of records applied, and no event describes a change that did not become visible. On `SyncCompleted`, every view whose content can change through an apply reloads once, and the sync status (SYN-063) is read again; after a sync that raised no event, what else the status reports — the last sync's time, the held-back count, the notices — follows at its next read.

**SYN-065 — Applying changes is atomic per sync (backend)**: The changes applied during one sync become visible together. A sync interrupted midway leaves the local portfolio as it was before the sync started, and the next sync picks up from the same sync cursors.

**SYN-066 — Conflict notices (frontend + backend)**: Exactly the outcomes CFR-060 lists produce a persisted conflict notice (ConflictNotice) in sync status, on the device CFR-060 names, naming the record and the other device. Notices survive restarts and persist until the user dismisses them individually; dismissing a notice that no longer exists is rejected with a specific error and changes nothing. The user is never left to discover a lost edit by chance.

**SYN-067 — Changes are published in batches (backend)**: After a recorded change, publishing waits a settling interval of 5 seconds — restarted by each further change, capped at 30 seconds from the first — and then publishes every change recorded since the last segment as one segment. A burst of changes — a price download recording dozens of prices — produces one segment, not one per change.

**SYN-068 — Headless scheduled run (backend)**: The scheduled close-of-day download (SPF) that runs without a window records its price and rate changes (SYN-020) and publishes them (SYN-067) like any other change, after verifying the passphrase check (SYN-055); on mismatch it publishes nothing and leaves the reset for the next launch (SYN-084). It never applies other devices' changes — applying, merging, and recomputing happen at the next application launch (SYN-060).

**SYN-069 — Folder unavailable (frontend + backend)**: When the designated folder cannot be read or written — renamed, unmounted, permission withdrawn — the device keeps recording changes, the sync is reported as failed in sync status with the reason, and the next sync retries. A folder that is reachable but carries no header — the mount point a detached removable volume leaves behind — is reported as the volume not being mounted; the device neither pauses nor publishes anything there, so plugging the volume back in and syncing again picks up where it left off. Nothing is lost: unpublished changes wait until the folder is back, or until the user designates a new folder (SYN-074).

### Managing Sync (070–079)

**SYN-070 — Pause on this device (frontend + backend)**: The user can pause sync on a device. The device stops publishing and applying but keeps recording changes (SYN-020, SYN-026), its identity, its sync cursors, and its derived key. Its local portfolio and the folder are untouched; other devices continue unaffected.

**SYN-071 — Start over with a new passphrase (frontend + backend)**: A device can enable sync into a folder that already holds a portfolio _as a new origin_: after an explicit confirmation stating that every published file will be discarded and every other device will have to rejoin from a fresh installation, the device clears the folder, then proceeds as the first device (SYN-013) under the new passphrase. This is both the passphrase-change path and the forgotten-passphrase path (SYN-053), and the only case in which published history is removed (SYN-036). If interrupted after clearing, the folder holds no portfolio and the device may retry as a first device.

**SYN-072 — Rename device (frontend + backend)**: The device name can be changed at any time; it must remain non-blank (SYN-018). The manifest is republished (SYN-037) and the new name appears in other devices' status after their next sync.

**SYN-073 — Resume (frontend + backend)**: A paused device resumes into the same portfolio without re-entering the passphrase: it publishes the changes recorded while paused, then syncs normally. Resuming is rejected with a specific error while the folder is unavailable (SYN-069). Resuming into a folder whose passphrase check no longer matches the kept key is the reset case (SYN-084).

**SYN-074 — Change folder (frontend + backend)**: On a device that is syncing or paused, the user can designate a different folder. The new folder must hold the same portfolio (its passphrase check matches the kept key) or be empty — in which case the device proceeds as a first device (SYN-013) with its current portfolio. Any other folder is rejected with a specific error.

### Setup, Leaving, and Recovery (080–084)

**SYN-080 — Joining is atomic (backend)**: If the rebuild performed when joining (SYN-014, SYN-036) is interrupted, the device is returned to its prior state — still holding no user data, not yet a member — and may retry. A half-rebuilt device is never left behind.

**SYN-081 — Two devices enabling into the same empty folder (frontend + backend)**: Immediately before publishing the folder header (SYN-013), a device re-checks the folder. If a header now exists, it does not publish its own: the action is rejected with a specific error stating that another device created the portfolio first, and the user is offered the join path (SYN-014) instead.

**SYN-082 — Leave sync on this device (frontend + backend)**: The user can turn sync off for good on a device. The device publishes its unpublished changes, removes its own manifest — it is no longer in the roster — and stops recording, publishing, and applying; its identity, sync cursors, held-back changes, and kept key are discarded; its local portfolio is untouched. Its area stays in the folder (SYN-037). Leaving is rejected with a specific error while the folder is unavailable (SYN-069), so unpublished work is never abandoned. A device that has left holds user data and can only rejoin from a fresh installation (SYN-014).

**SYN-083 — Joiner's pre-existing non-user records are replaced (backend)**: Asset prices, currency pairs, and currency rates that a fresh installation fetched before joining are discarded by the rebuild (SYN-036) and replaced by the shared portfolio's, so that every device holds the same observations (SYN-022). A rebuild is a replacement, not a merge: CFR does not apply to the discarded local observations.

**SYN-084 — Portfolio reset detected (frontend + backend)**: A device follows one folder header (SyncDevice `portfolio`). Whenever a sync — automatic, manual, or resume — finds a header file whose passphrase check (SYN-055) no longer matches the kept key, the portfolio was started over elsewhere (SYN-071). A folder that carries no header at all is not a reset: for an enrolled device it means the volume is not mounted (SYN-069). When several header files are present (two devices enabled while offline), the device follows the one it knows and ignores the others. On reset, the device pauses itself, stops publishing, and tells the user that the portfolio was reset and that this device must rejoin from a fresh installation — or leave sync (SYN-082) and keep its local data unsynced. It never publishes under the old key into a reset folder.

---

## Workflow

```
Device A (has data)                    Folder                   Device B (fresh install)
      │                                  │                            │
  enable: folder + passphrase ×2         │                            │
      ├── header + first segment ────────►│ header · A/segment · A/manifest
      │                                  │                            │
      │        ... user records 3 transactions on A ...               │
      ├── one segment (SYN-067) ────────►│ A/segment                  │
      │                                  │                            │
      │                                  │◄──── enable: folder + passphrase ×1 (SYN-014/015)
      │                                  │──── every segment, replayed in order ────►│
      │                                  │                 rebuild (SYN-036), B/manifest
      │                                  │                            │
      │        ... user records 1 account on B ...                    │
      │                                  │◄──── segment ──────────────┤
      │                                  │                            │
  launch / "Sync now" (SYN-060/061)      │                            │
      │◄── B's area from A's cursor ─────┤                            │
  apply atomically, recompute (SYN-065, CFR-041), SyncCompleted            │
```

---

## UX Draft

### Entry Point

Settings — a new "Multi-device sync" section. Disabled state offers "Enable sync"; enabled state shows the status block (SYN-063) with "Sync now", "Rename device", "Change folder", "Pause sync" / "Resume sync", "Leave sync", the roster, and "Start over". A compact sync indicator in the application shell shows the last-sync time and surfaces failures and attention items.

### Main Component

Enable-sync modal in two steps: folder picker (validated, SYN-019), then passphrase with the SYN-017/053/054 statements. If the folder already holds a portfolio the modal switches to the join wording (single passphrase entry, fresh-installation requirement). Confirmation dialog for start-over (SYN-071).

### States

- **Disabled**: single enable action plus the positioning note.
- **Enabling / joining**: progress while publishing or rebuilding; the application is usable once done.
- **Enabled, healthy**: last sync time, this device's application version after its name, roster with each device's application version ("version unknown" when its manifest does not state it) and last-applied time.
- **Paused**: status shows paused; "Resume sync" offered.
- **Attention needed**: unreadable file (SYN-034), update required (SYN-035), folder unavailable (SYN-069), held-back changes (SYN-041), conflict notices (SYN-066, dismissable one by one), inconsistent holdings (SYN-040), reset-by-another-device (SYN-084) — each with its reason.
- **Passphrase mismatch** (SYN-015), **installation holds user data** (SYN-014), **folder invalid** (SYN-019/074), **another device created the portfolio first** (SYN-081): inline errors in the modal.

### User Flow

1. On the first machine, the user enables sync: picks the shared folder, chooses a passphrase twice, names the device. The whole portfolio is published.
2. On the second machine (fresh install), the user enables sync: picks the same folder, enters the passphrase once, names the device. The portfolio is rebuilt locally.
3. The user records on either machine; at the next launch or "Sync now" the other machine shows the same portfolio.
4. If the application is updated on one machine and not the other, the older one stops applying and asks to be updated.
5. If the user pauses sync on the laptop for a trip and records transactions offline, resuming publishes them and the desktop picks them up.

---

## Open Questions

None — all questions have been resolved.
