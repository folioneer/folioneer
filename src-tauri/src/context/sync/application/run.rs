//! One sync run (SYN-060/061/065/067/069): `publish` seals this device's unpublished changes
//! into one segment and rewrites its manifest; `run` publishes, then reads every other
//! device's manifest and unapplied segments from its sync cursor, hands each change to the
//! apply executor (`apply.rs`, driven by the resolution engine), and commits the whole apply
//! as one SQLite transaction under the `SyncGate` with the change recorder suspended
//! (SYN-020); `join` rebuilds a fresh installation from the folder's entire history
//! (`join.rs`).

use std::sync::Arc;

use sqlx::SqliteConnection;

use crate::context::sync::application::apply::{apply_change, Applied};
use crate::context::sync::application::intake::{self, IncomingChange, Intake};
use crate::context::sync::application::join::{self, JoinError};
use crate::context::sync::domain::{
    display_name, replay_order, Change, ChangeApplier, ChangeLogRepository, ConflictNotice,
    FolderProblem, FolderStore, HeldBackChange, Manifest, NoticeDraft, RosterEntry, Segment,
    SyncDevice, SyncFailure, SyncReport, SyncStateRepository, SyncStatus, WaitingFor, APP_VERSION,
};
use crate::context::sync::error::SyncError;
use crate::context::sync::infrastructure::codec::{
    decode_header, encode_manifest, encode_segment, header_data_format_version, DATA_FORMAT_VERSION,
};
use crate::context::sync::infrastructure::crypto::{verify_check, Key};
use crate::core::logger::BACKEND;
use crate::shared::infrastructure::change_recorder::ChangeRecorder;

/// Serializes a local write against an in-progress apply (SYN-064/SYN-020): the mutex
/// `SyncRun::run`'s full apply transaction holds for its whole duration, so a local write
/// waits for it rather than interleaving. The change recorder's `suspend()` gate (SYN-020)
/// and this mutex are the same invariant's two halves — the run holds both.
pub struct SyncGate {
    mutex: tokio::sync::Mutex<()>,
}

impl Default for SyncGate {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncGate {
    /// Creates an ungated `SyncGate` — nothing is held yet.
    pub fn new() -> Self {
        Self {
            mutex: tokio::sync::Mutex::new(()),
        }
    }

    /// Runs `f` while holding the gate, so a concurrent local write started after this call
    /// begins waits until `f` completes (SYN-064).
    pub async fn run_exclusive<F, Fut, T>(&self, f: F) -> T
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        let _permit = self.mutex.lock().await;
        f().await
    }
}

/// Executes one sync run: the publish half, the full run, or a join.
pub struct SyncRun {
    change_log: Arc<dyn ChangeLogRepository>,
    state_repo: Arc<dyn SyncStateRepository>,
    folder_store: Arc<dyn FolderStore>,
    change_recorder: Arc<dyn ChangeRecorder>,
    sync_gate: Arc<SyncGate>,
}

/// What the apply transaction did (SYN-062).
#[derive(Default)]
struct ApplyCounts {
    applied: u32,
    held_back: u32,
    dropped: u32,
    notices: u32,
}

/// What the folder header says about continuing this run (SYN-035/084).
enum HeaderGate {
    /// The header is this portfolio's and readable: publish.
    Proceed,
    /// The header was written in a newer data format: publish anyway, and say so.
    UpdateRequired(u32),
    /// No header at all. An enrolled device follows a header (SYN-084), so a reachable
    /// folder without one is a mount point whose volume is gone, not a portfolio that was
    /// started over (SYN-069).
    Absent,
    /// A header whose passphrase check no longer matches the kept key: the portfolio was
    /// started over elsewhere.
    Reset,
}

fn header_gate(header_bytes: Option<&[u8]>, key: &Key) -> HeaderGate {
    let Some(bytes) = header_bytes else {
        return HeaderGate::Absent;
    };
    if let Some(version) =
        header_data_format_version(bytes).filter(|version| *version > DATA_FORMAT_VERSION)
    {
        return HeaderGate::UpdateRequired(version);
    }
    match decode_header(bytes) {
        Ok(header) if verify_check(key, &header.passphrase_check) => HeaderGate::Proceed,
        _ => HeaderGate::Reset,
    }
}

/// Loads this device's kept key (SYN-052). `Ok(None)` when the stored bytes cannot be a key —
/// the device can open nothing, which is the reset case.
pub(super) async fn kept_key(
    change_log: &dyn ChangeLogRepository,
) -> Result<Option<Key>, SyncError> {
    Ok(change_log
        .kept_key_bytes()
        .await?
        .and_then(|bytes| Key::from_bytes(bytes).ok()))
}

fn count(counts: &mut ApplyCounts, applied: &Applied) {
    match applied {
        Applied::Applied => counts.applied += 1,
        Applied::Ignored => {}
        Applied::Dropped => counts.dropped += 1,
        Applied::HeldBack(_) => counts.held_back += 1,
    }
}

fn report(device: &SyncDevice, published_changes: u32, failures: Vec<SyncFailure>) -> SyncReport {
    let completed_at = chrono::Utc::now().to_rfc3339();
    SyncReport {
        published_changes,
        applied_changes: 0,
        held_back_changes: 0,
        dropped_changes: 0,
        notices_raised: 0,
        failures: failures.clone(),
        completed_at: completed_at.clone(),
        status: SyncStatus::for_device(device, Some(completed_at), failures),
    }
}

/// The folder condition a failed read or write reveals, as a run failure (SYN-069).
fn folder_failure(error: SyncError) -> Result<SyncFailure, SyncError> {
    match error {
        SyncError::FolderUnavailable { problem } | SyncError::PublishFailed { problem } => {
            Ok(SyncFailure::FolderUnavailable { problem })
        }
        other => Err(other),
    }
}

impl SyncRun {
    /// Creates a run bound to the given change log, sync state, folder, and the change
    /// recorder it suspends while applying (SYN-020).
    pub fn new(
        change_log: Arc<dyn ChangeLogRepository>,
        state_repo: Arc<dyn SyncStateRepository>,
        folder_store: Arc<dyn FolderStore>,
        change_recorder: Arc<dyn ChangeRecorder>,
    ) -> Self {
        Self {
            change_log,
            state_repo,
            folder_store,
            change_recorder,
            sync_gate: Arc::new(SyncGate::new()),
        }
    }

    /// Shares the `SyncGate` a full run's apply transaction holds for its duration
    /// (SYN-064).
    pub fn with_sync_gate(mut self, sync_gate: Arc<SyncGate>) -> Self {
        self.sync_gate = sync_gate;
        self
    }

    /// Enables sync by joining the portfolio `folder` already holds (SYN-014): derives the
    /// key from the header, checks the passphrase (SYN-015), reads every device's whole
    /// history (`HistoryIncomplete` when any of it is missing, SYN-036), and rebuilds the
    /// portfolio from it in one transaction — rolled back entirely on any failure
    /// (`RebuildInterrupted`, SYN-080). Local prices, pairs, and rates are discarded first
    /// (SYN-083). `applier` is the owning contexts' write surface (CFR-017).
    pub async fn join(
        &self,
        applier: &dyn ChangeApplier,
        folder: String,
        passphrase: String,
        device_name: String,
    ) -> Result<SyncStatus, JoinError> {
        join::join(
            &join::JoinPorts {
                change_log: self.change_log.as_ref(),
                state_repo: self.state_repo.as_ref(),
                folder_store: self.folder_store.as_ref(),
                change_recorder: self.change_recorder.as_ref(),
                applier,
            },
            join::JoinRequest {
                folder,
                passphrase,
                device_name,
            },
        )
        .await
    }

    /// The full sync run (SYN-060/065): publishes this device's unpublished changes, reads
    /// every other device's manifest and unapplied segments from its own sync cursor,
    /// applies each change as the resolution engine decides, and commits the outcomes in
    /// one SQLite transaction under the `SyncGate` — so an interrupted run leaves the
    /// portfolio exactly as it was and the next run retries from the same cursors
    /// (SYN-065). Held-back changes persist and are retried each run (SYN-041); a file that
    /// cannot be read is skipped and counted (SYN-034); a newer data format anywhere keeps
    /// publishing but stops applying (SYN-035); applying writes through `applier` and
    /// records no change (CFR-017, SYN-020). Never rejects for the folder's state
    /// (SYN-062).
    pub async fn run(
        &self,
        device: &SyncDevice,
        applier: &dyn ChangeApplier,
    ) -> Result<SyncReport, SyncError> {
        let published = self.publish(device).await?;
        if published.failures.iter().any(|failure| {
            matches!(
                failure,
                SyncFailure::PortfolioReset
                    | SyncFailure::FolderUnavailable { .. }
                    | SyncFailure::UpdateRequired { .. }
            )
        }) {
            return Ok(published);
        }
        let Some(key) = kept_key(self.change_log.as_ref()).await? else {
            return Ok(published);
        };
        let mut intake = match intake::read_other_devices(
            self.folder_store.as_ref(),
            self.state_repo.as_ref(),
            device,
            &key,
        )
        .await
        {
            Ok(intake) => intake,
            Err(error) => {
                let mut failures = published.failures;
                failures.push(folder_failure(error)?);
                return Ok(report(device, published.published_changes, failures));
            }
        };
        let mut failures = published.failures;
        failures.append(&mut intake.failures);
        if intake.unreadable_files > 0 {
            failures.push(SyncFailure::UnreadableFiles {
                count: intake.unreadable_files,
            });
        }
        let roster = intake.roster.clone();
        let update_required = failures
            .iter()
            .any(|failure| matches!(failure, SyncFailure::UpdateRequired { .. }));
        let counts = if update_required {
            ApplyCounts::default()
        } else {
            let held_back = self.state_repo.list_held_back().await?;
            self.sync_gate
                .run_exclusive(|| async {
                    // SYN-020 — applying never records a change: the recorder stays
                    // suspended for the whole apply transaction.
                    let _recording_suspended = self.change_recorder.suspend();
                    self.apply_intake(device, applier, held_back, intake).await
                })
                .await?
        };
        let mut report = report(device, published.published_changes, failures);
        report.applied_changes = counts.applied;
        report.held_back_changes = counts.held_back;
        report.dropped_changes = counts.dropped;
        report.notices_raised = counts.notices;
        report.status.roster = roster;
        Ok(report)
    }

    /// The apply transaction (SYN-065): applies what was read in replay order, then retries
    /// every held-back change — the rows from earlier runs and the ones this run just held
    /// back — until a pass reunites nothing more, so a parent arriving in the same run
    /// reunites its child at once (SYN-041); persists what is still held back and the
    /// notices raised on this device, advances the cursors and the logical clock, and
    /// commits — or rolls everything back.
    async fn apply_intake(
        &self,
        device: &SyncDevice,
        applier: &dyn ChangeApplier,
        held_back: Vec<HeldBackChange>,
        mut intake: Intake,
    ) -> Result<ApplyCounts, SyncError> {
        intake
            .changes
            .sort_by(|a, b| replay_order((&a.change, a.sequence), (&b.change, b.sequence)));
        let mut transaction = self.change_log.begin().await?;
        let conn: &mut SqliteConnection = &mut transaction;
        let mut counts = ApplyCounts::default();
        let now = chrono::Utc::now().to_rfc3339();
        let mut highest_timestamp: i64 = 0;

        let mut pending: Vec<(String, Change)> = Vec::new();
        for held in held_back {
            match serde_json::from_str::<Change>(&held.payload) {
                Ok(change) => pending.push((held.id, change)),
                Err(_) => {
                    tracing::error!(target: BACKEND, id = %held.id, "apply: held-back payload unreadable, discarded");
                    self.state_repo.remove_held_back_on(conn, &held.id).await?;
                }
            }
        }

        for incoming in &intake.changes {
            let change = &incoming.change;
            highest_timestamp = highest_timestamp.max(change.logical_timestamp.value() as i64);
            let result = apply_change(
                conn,
                applier,
                self.change_log.as_ref(),
                &device.device_id,
                change,
            )
            .await?;
            if let Applied::HeldBack(waiting_for) = &result.applied {
                let id = self.hold_back(conn, incoming, waiting_for, &now).await?;
                pending.push((id, change.clone()));
                continue;
            }
            counts.notices += self
                .persist_notices(conn, change, result.notices, &intake.roster, &now)
                .await?;
            count(&mut counts, &result.applied);
        }

        loop {
            let mut still_waiting: Vec<(String, Change)> = Vec::new();
            let mut reunited = false;
            for (id, change) in pending {
                highest_timestamp = highest_timestamp.max(change.logical_timestamp.value() as i64);
                let result = apply_change(
                    conn,
                    applier,
                    self.change_log.as_ref(),
                    &device.device_id,
                    &change,
                )
                .await?;
                if matches!(result.applied, Applied::HeldBack(_)) {
                    still_waiting.push((id, change));
                    continue;
                }
                reunited = true;
                self.state_repo.remove_held_back_on(conn, &id).await?;
                counts.notices += self
                    .persist_notices(conn, &change, result.notices, &intake.roster, &now)
                    .await?;
                count(&mut counts, &result.applied);
            }
            pending = still_waiting;
            if !reunited || pending.is_empty() {
                break;
            }
        }
        counts.held_back = pending.len() as u32;

        for cursor in &intake.cursors {
            self.state_repo.upsert_cursor_on(conn, cursor).await?;
        }
        self.change_log
            .advance_logical_clock(conn, highest_timestamp)
            .await?;
        transaction.commit().await.map_err(|error| {
            tracing::error!(target: BACKEND, err = ?error, "apply: commit failed");
            SyncError::DatabaseError
        })?;
        Ok(counts)
    }

    /// Persists one change this run holds back (SYN-041), returning the row's id.
    async fn hold_back(
        &self,
        conn: &mut SqliteConnection,
        incoming: &IncomingChange,
        waiting_for: &WaitingFor,
        held_since: &str,
    ) -> Result<String, SyncError> {
        let change = &incoming.change;
        let (waiting_kind, waiting_identity) = match waiting_for {
            WaitingFor::Record { kind, identity } => (*kind, identity.clone()),
            WaitingFor::OwnState { .. } => (change.record_kind, change.record_identity.clone()),
        };
        let payload = serde_json::to_string(change).map_err(|error| {
            tracing::error!(target: BACKEND, err = %error, "apply: held-back payload not serialized");
            SyncError::DatabaseError
        })?;
        let id = uuid::Uuid::new_v4().to_string();
        self.state_repo
            .insert_held_back_on(
                conn,
                &HeldBackChange {
                    id: id.clone(),
                    origin_device_id: incoming.origin_device_id.clone(),
                    sequence: incoming.sequence,
                    payload,
                    waiting_kind,
                    waiting_identity,
                    held_since: held_since.to_string(),
                },
            )
            .await?;
        Ok(id)
    }

    /// Persists the notices one applied change raised on this device (SYN-066), naming the
    /// other device by the name its manifest carries; returns how many were raised.
    async fn persist_notices(
        &self,
        conn: &mut SqliteConnection,
        change: &Change,
        notices: Vec<NoticeDraft>,
        roster: &[RosterEntry],
        raised_at: &str,
    ) -> Result<u32, SyncError> {
        let raised = notices.len() as u32;
        for draft in notices {
            let other_device_name = roster
                .iter()
                .find(|entry| entry.device_id == draft.other_device_id)
                .map_or_else(
                    || draft.other_device_id.clone(),
                    |entry| entry.device_name.clone(),
                );
            let notice = ConflictNotice {
                notice_id: uuid::Uuid::new_v4().to_string(),
                kind: draft.kind,
                record_kind: draft.record_kind,
                record_identity: draft.record_identity.clone(),
                record_label: display_name(change).unwrap_or_else(|| draft.record_identity.clone()),
                other_device_id: draft.other_device_id,
                other_device_name,
                raised_at: raised_at.to_string(),
            };
            self.state_repo.insert_notice_on(conn, &notice).await?;
        }
        Ok(raised)
    }

    /// Publishes this device's unpublished changes as one segment and rewrites its manifest
    /// (SYN-060 publish half, SYN-061, SYN-067). Never rejects for the folder's state
    /// (SYN-062) — `FolderUnavailable`, `UpdateRequired`, and `PortfolioReset` surface in the
    /// returned report's `failures`, not as an `Err`.
    pub async fn publish(&self, device: &SyncDevice) -> Result<SyncReport, SyncError> {
        self.folder_store.retarget(&device.folder);
        if let Err(problem) = self.folder_store.check_available().await {
            return Ok(report(
                device,
                0,
                vec![SyncFailure::FolderUnavailable { problem }],
            ));
        }
        let header_bytes = match self.folder_store.read_header_bytes().await {
            Ok(bytes) => bytes,
            Err(error) => return Ok(report(device, 0, vec![folder_failure(error)?])),
        };
        let Some(key) = kept_key(self.change_log.as_ref()).await? else {
            return self.pause_for_reset(device).await;
        };
        let mut failures = Vec::new();
        match header_gate(header_bytes.as_deref(), &key) {
            HeaderGate::Proceed => {}
            HeaderGate::UpdateRequired(data_format_version) => {
                failures.push(SyncFailure::UpdateRequired {
                    data_format_version,
                });
            }
            HeaderGate::Absent => {
                return Ok(report(
                    device,
                    0,
                    vec![SyncFailure::FolderUnavailable {
                        problem: FolderProblem::Unmounted,
                    }],
                ));
            }
            HeaderGate::Reset => return self.pause_for_reset(device).await,
        }

        let changes = self.change_log.list_unpublished(&device.device_id).await?;
        let published_changes = changes.len() as u32;
        if let (Some(first), Some(last)) = (changes.first(), changes.last()) {
            let segment = Segment {
                device_id: device.device_id.clone(),
                first_sequence: first.sequence,
                last_sequence: last.sequence,
                data_format_version: DATA_FORMAT_VERSION,
                changes: changes.clone(),
            };
            let written = match encode_segment(&key, &segment) {
                Ok(bytes) => {
                    self.folder_store
                        .write_segment(
                            &device.device_id,
                            segment.first_sequence,
                            segment.last_sequence,
                            bytes,
                        )
                        .await
                }
                Err(error) => Err(error),
            };
            if let Err(error) = written {
                failures.push(folder_failure(error)?);
                return Ok(report(device, 0, failures));
            }
            self.change_log
                .mark_published(
                    &device.device_id,
                    segment.first_sequence,
                    segment.last_sequence,
                )
                .await?;
        }

        let manifest = Manifest {
            device_id: device.device_id.clone(),
            device_name: device.device_name.clone(),
            data_format_version: DATA_FORMAT_VERSION,
            app_version: Some(APP_VERSION.to_string()),
            latest_sequence: self
                .change_log
                .latest_published_sequence(&device.device_id)
                .await?,
        };
        let written = match encode_manifest(&key, &manifest) {
            Ok(bytes) => {
                self.folder_store
                    .write_manifest(&device.device_id, bytes)
                    .await
            }
            Err(error) => Err(error),
        };
        if let Err(error) = written {
            failures.push(folder_failure(error)?);
        }
        Ok(report(device, published_changes, failures))
    }

    /// SYN-084 — the portfolio was started over elsewhere: the device pauses itself and
    /// publishes nothing under the old key.
    async fn pause_for_reset(&self, device: &SyncDevice) -> Result<SyncReport, SyncError> {
        let paused = if device.paused {
            device.clone()
        } else {
            device.clone().pause()?
        };
        self.state_repo.save_device(&paused).await?;
        Ok(report(&paused, 0, vec![SyncFailure::PortfolioReset]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::sync::domain::{
        segment_file_name, DerivationParameters, FolderHeader, MockChangeApplier, MockFolderStore,
        MockSyncStateRepository, SegmentChange,
    };
    use crate::context::sync::infrastructure::codec::encode_header;
    use crate::context::sync::infrastructure::crypto::make_check;
    use crate::context::sync::infrastructure::{
        SqliteChangeLogRepository, SqliteChangeRecorder, SqliteSyncStateRepository,
    };
    use crate::shared::domain::{ChangeDraft, Operation, Origin, RecordIdentity, RecordKind};
    use crate::shared::infrastructure::change_recorder::NoopChangeRecorder;
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::{Pool, Sqlite};

    fn run_over(
        pool: &Pool<Sqlite>,
        state_repo: Arc<dyn SyncStateRepository>,
        folder_store: MockFolderStore,
    ) -> SyncRun {
        SyncRun::new(
            Arc::new(SqliteChangeLogRepository::new(pool.clone())),
            state_repo,
            Arc::new(folder_store),
            Arc::new(NoopChangeRecorder),
        )
    }

    async fn make_pool() -> Pool<Sqlite> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("test pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations");
        pool
    }

    /// The key the seeded `sync_device` row keeps: 32 zero bytes.
    fn kept_key_for_tests() -> Key {
        Key::from_bytes(vec![0; 32]).expect("32 bytes is a valid key")
    }

    async fn seed_sync_device(pool: &Pool<Sqlite>, device_id: &str) {
        let derived_key = kept_key_for_tests().as_bytes().to_vec();
        sqlx::query(
            r#"INSERT INTO sync_device
               (id, device_id, device_name, folder, joined_at, paused, portfolio_created_at,
                logical_clock, derived_key, data_format_version)
               VALUES (1, ?, 'Desktop', '/tmp/sync', '2026-08-22T00:00:00Z', 0,
                       '2026-08-22T00:00:00Z', 0, ?, 1)"#,
        )
        .bind(device_id)
        .bind(derived_key)
        .execute(pool)
        .await
        .expect("seed sync_device");
    }

    async fn seed_unpublished_change(pool: &Pool<Sqlite>, device_id: &str, sequence: i64) {
        sqlx::query(
            r#"INSERT INTO changes
               (device_id, sequence, logical_timestamp, based_on, record_kind, record_identity,
                operation, origin, content, published)
               VALUES (?, ?, ?, NULL, 'Account', ?, 'Created', 'User', ?, 0)"#,
        )
        .bind(device_id)
        .bind(sequence)
        .bind(format!("{sequence:020}"))
        .bind(format!("account-{sequence}"))
        .bind(format!("{{\"id\":\"account-{sequence}\"}}"))
        .execute(pool)
        .await
        .expect("seed unpublished change");
    }

    fn device() -> SyncDevice {
        SyncDevice::restore(crate::context::sync::StoredDevice {
            device_id: "desktop-device".into(),
            device_name: "Desktop".into(),
            folder: "/tmp/sync".into(),
            joined_at: "2026-08-22T00:00:00Z".into(),
            paused: false,
            portfolio_created_at: "2026-08-22T00:00:00Z".into(),
            data_format_version: 1,
        })
    }

    /// The header of the portfolio the seeded device follows: its passphrase check was made
    /// under the kept key.
    fn matching_header_bytes() -> Vec<u8> {
        encode_header(&FolderHeader {
            derivation_parameters: DerivationParameters {
                salt: vec![1; 16],
                memory_cost_kib: 19_456,
                iterations: 2,
                parallelism: 1,
            },
            passphrase_check: make_check(&kept_key_for_tests()),
            data_format_version: DATA_FORMAT_VERSION,
            created_at: "00000000000000000001".into(),
            created_by_device_id: "desktop-device".into(),
        })
        .expect("a valid header encodes")
    }

    // SYN-060/067 — unpublished changes are collected into one segment, and the rows are
    // marked published afterward.
    #[tokio::test]
    async fn publish_collects_unpublished_changes_and_marks_them_published() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        seed_unpublished_change(&pool, "desktop-device", 1).await;
        seed_unpublished_change(&pool, "desktop-device", 2).await;

        let state_repo = Arc::new(MockSyncStateRepository::new());
        let mut folder_store = MockFolderStore::new();
        folder_store.expect_retarget().return_const(());
        folder_store.expect_check_available().returning(|| Ok(()));
        folder_store
            .expect_read_header_bytes()
            .returning(|| Ok(Some(matching_header_bytes())));
        folder_store
            .expect_write_segment()
            .withf(|device_id, first, last, _| {
                device_id == "desktop-device" && *first == 1 && *last == 2
            })
            .times(1)
            .returning(|_, _, _, _| Ok(()));
        folder_store
            .expect_write_manifest()
            .times(1)
            .returning(|_, _| Ok(()));
        let run = run_over(&pool, state_repo, folder_store);

        let report = run
            .publish(&device())
            .await
            .expect("publish must not error");
        assert_eq!(
            report.published_changes, 2,
            "both unpublished rows must be counted"
        );
        assert!(report.failures.is_empty());

        let published_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM changes WHERE published = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            published_count, 2,
            "SYN-031/067: the rows must be marked published"
        );
    }

    // SYN-067 — a second run with nothing new writes no segment and reports 0 published; the
    // manifest is still rewritten (a rename travels through it, SYN-072).
    #[tokio::test]
    async fn second_run_with_nothing_new_publishes_nothing() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        seed_unpublished_change(&pool, "desktop-device", 1).await;

        let state_repo = Arc::new(MockSyncStateRepository::new());
        let mut folder_store = MockFolderStore::new();
        folder_store.expect_retarget().return_const(());
        folder_store.expect_check_available().returning(|| Ok(()));
        folder_store
            .expect_read_header_bytes()
            .returning(|| Ok(Some(matching_header_bytes())));
        folder_store
            .expect_write_segment()
            .times(1)
            .returning(|_, _, _, _| Ok(()));
        folder_store
            .expect_write_manifest()
            .times(2)
            .returning(|_, _| Ok(()));
        let run = run_over(&pool, state_repo, folder_store);

        run.publish(&device()).await.unwrap();
        let second = run.publish(&device()).await.unwrap();
        assert_eq!(second.published_changes, 0);
    }

    // SYN-069 — an unavailable folder is reported in `failures`, not thrown; unpublished
    // changes stay unpublished so the next run retries them.
    #[tokio::test]
    async fn folder_unavailable_reports_failure_and_leaves_changes_unpublished() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        seed_unpublished_change(&pool, "desktop-device", 1).await;

        let state_repo = Arc::new(MockSyncStateRepository::new());
        let mut folder_store = MockFolderStore::new();
        folder_store.expect_retarget().return_const(());
        folder_store
            .expect_check_available()
            .returning(|| Err(crate::context::sync::domain::FolderProblem::Missing));
        let run = run_over(&pool, state_repo, folder_store);

        let report = run
            .publish(&device())
            .await
            .expect("SYN-062: never rejects for folder state");
        assert!(
            report
                .failures
                .iter()
                .any(|f| matches!(f, SyncFailure::FolderUnavailable { .. })),
            "an unavailable folder must surface in SyncReport.failures: {:?}",
            report.failures
        );

        let published_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM changes WHERE published = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            published_count, 0,
            "unpublished rows must stay unpublished for the retry"
        );
    }

    // SYN-069 — a segment write that fails midway is reported, and the rows stay unpublished.
    #[tokio::test]
    async fn failed_segment_write_reports_failure_and_leaves_changes_unpublished() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        seed_unpublished_change(&pool, "desktop-device", 1).await;

        let state_repo = Arc::new(MockSyncStateRepository::new());
        let mut folder_store = MockFolderStore::new();
        folder_store.expect_retarget().return_const(());
        folder_store.expect_check_available().returning(|| Ok(()));
        folder_store
            .expect_read_header_bytes()
            .returning(|| Ok(Some(matching_header_bytes())));
        folder_store.expect_write_segment().returning(|_, _, _, _| {
            Err(SyncError::PublishFailed {
                problem: crate::context::sync::domain::FolderProblem::OutOfSpace,
            })
        });
        let run = run_over(&pool, state_repo, folder_store);

        let report = run
            .publish(&device())
            .await
            .expect("SYN-062: never rejects");
        assert!(report.failures.contains(&SyncFailure::FolderUnavailable {
            problem: crate::context::sync::domain::FolderProblem::OutOfSpace
        }));
        assert_eq!(report.published_changes, 0);

        let published_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM changes WHERE published = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(published_count, 0);
    }

    // SYN-035 — a too-new header/manifest already in the folder reports UpdateRequired, but
    // this device's own segment still publishes (publishing continues deliberately).
    #[tokio::test]
    async fn too_new_header_reports_update_required_but_this_device_still_publishes() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        seed_unpublished_change(&pool, "desktop-device", 1).await;

        let state_repo = Arc::new(MockSyncStateRepository::new());
        let mut folder_store = MockFolderStore::new();
        folder_store.expect_retarget().return_const(());
        folder_store.expect_check_available().returning(|| Ok(()));
        folder_store
            .expect_read_header_bytes()
            .returning(|| Ok(Some(b"{\"data_format_version\":99}".to_vec())));
        folder_store
            .expect_write_segment()
            .returning(|_, _, _, _| Ok(()));
        folder_store
            .expect_write_manifest()
            .returning(|_, _| Ok(()));
        let run = run_over(&pool, state_repo, folder_store);

        let report = run
            .publish(&device())
            .await
            .expect("SYN-062: never rejects");
        assert!(
            report
                .failures
                .iter()
                .any(|f| matches!(f, SyncFailure::UpdateRequired { data_format_version } if *data_format_version == 99)),
            "a too-new format must surface as UpdateRequired: {:?}",
            report.failures
        );
        assert_eq!(
            report.published_changes, 1,
            "SYN-035: the gated device still publishes its own real user intent"
        );
    }

    // SYN-084 — a passphrase-check mismatch against the kept key reports PortfolioReset and
    // pauses the device; nothing is published under the old key into the reset folder.
    #[tokio::test]
    async fn passphrase_mismatch_reports_portfolio_reset_and_pauses_the_device() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        seed_unpublished_change(&pool, "desktop-device", 1).await;

        let mut state_repo = MockSyncStateRepository::new();
        state_repo.expect_save_device().returning(|_| Ok(()));
        let mut folder_store = MockFolderStore::new();
        folder_store.expect_retarget().return_const(());
        folder_store.expect_check_available().returning(|| Ok(()));
        folder_store
            .expect_read_header_bytes()
            .returning(|| Ok(Some(b"{\"passphrase_check\":\"does-not-match\"}".to_vec())));
        let run = run_over(&pool, Arc::new(state_repo), folder_store);

        let report = run
            .publish(&device())
            .await
            .expect("SYN-062: never rejects");
        assert!(
            report.failures.contains(&SyncFailure::PortfolioReset),
            "a passphrase-check mismatch must surface as PortfolioReset: {:?}",
            report.failures
        );
        assert!(
            report.status.paused,
            "SYN-084: the device pauses itself on a detected reset"
        );

        let published_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM changes WHERE published = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            published_count, 0,
            "SYN-084: nothing is ever published under the old key into a reset folder"
        );
    }

    // SYN-069/084 — a reachable folder with no header is a detached volume, not a reset:
    // the run reports it unavailable, the device stays as it was, and nothing is published.
    // `state_repo` carries no `save_device` expectation on purpose — pausing would panic.
    #[tokio::test]
    async fn missing_header_reports_the_volume_unmounted_and_does_not_pause() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        seed_unpublished_change(&pool, "desktop-device", 1).await;

        let state_repo = MockSyncStateRepository::new();
        let mut folder_store = MockFolderStore::new();
        folder_store.expect_retarget().return_const(());
        folder_store.expect_check_available().returning(|| Ok(()));
        folder_store
            .expect_read_header_bytes()
            .returning(|| Ok(None));
        let run = run_over(&pool, Arc::new(state_repo), folder_store);

        let report = run
            .publish(&device())
            .await
            .expect("SYN-062: never rejects");
        assert!(
            report.failures.contains(&SyncFailure::FolderUnavailable {
                problem: FolderProblem::Unmounted
            }),
            "SYN-069: an empty folder is an unmounted volume: {:?}",
            report.failures
        );
        assert!(
            !report.failures.contains(&SyncFailure::PortfolioReset),
            "SYN-084: an absent header is never read as a reset"
        );

        let published_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM changes WHERE published = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            published_count, 0,
            "SYN-069: the change waits for the volume to come back"
        );
    }

    // -------------------------------------------------------------------------
    // SYN-060/065 — the full apply run
    // -------------------------------------------------------------------------

    // SYN-064 — SyncGate.run_exclusive holds the mutex for the whole apply; a local write
    // started after the run begins must wait until it completes.
    #[tokio::test]
    async fn sync_gate_run_exclusive_returns_the_closures_value() {
        let gate = SyncGate::new();
        let value = gate.run_exclusive(|| async { 42 }).await;
        assert_eq!(
            value, 42,
            "SYN-064: run_exclusive must return the wrapped work's value"
        );
    }

    /// A laptop area holding one segment `1..=1` with `change`, sealed under the kept key,
    /// and the manifest announcing it — served by the mock folder store.
    fn folder_with_laptop_segment(change: SegmentChange) -> MockFolderStore {
        let key = kept_key_for_tests();
        let manifest = encode_manifest(
            &key,
            &Manifest {
                device_id: "laptop-device".into(),
                device_name: "Laptop".into(),
                data_format_version: DATA_FORMAT_VERSION,
                app_version: Some("0.42.0".into()),
                latest_sequence: 1,
            },
        )
        .expect("manifest encodes");
        let segment = encode_segment(
            &key,
            &Segment {
                device_id: "laptop-device".into(),
                first_sequence: 1,
                last_sequence: 1,
                data_format_version: DATA_FORMAT_VERSION,
                changes: vec![change],
            },
        )
        .expect("segment encodes");
        let mut folder_store = MockFolderStore::new();
        folder_store.expect_retarget().return_const(());
        folder_store.expect_check_available().returning(|| Ok(()));
        folder_store
            .expect_read_header_bytes()
            .returning(|| Ok(Some(matching_header_bytes())));
        folder_store
            .expect_write_manifest()
            .returning(|_, _| Ok(()));
        folder_store
            .expect_list_device_ids()
            .returning(|| Ok(vec!["desktop-device".into(), "laptop-device".into()]));
        folder_store
            .expect_read_manifest_bytes()
            .returning(move |_| Ok(Some(manifest.clone())));
        folder_store
            .expect_list_segment_names()
            .returning(|_| Ok(vec![segment_file_name(1, 1)]));
        folder_store
            .expect_read_segment_bytes()
            .returning(move |_, _| Ok(Some(segment.clone())));
        folder_store
    }

    fn laptop_change(record_kind: RecordKind, identity: &str, content: &str) -> SegmentChange {
        SegmentChange {
            sequence: 1,
            logical_timestamp: format!("{:020}", 7),
            based_on: None,
            record_kind,
            record_identity: identity.into(),
            operation: Operation::Created,
            origin: Origin::User,
            content: Some(content.into()),
        }
    }

    // SYN-065 — a full run applies another device's segment through the applier, counts
    // it, and advances that device's cursor in the same transaction.
    #[tokio::test]
    async fn full_run_applies_another_devices_segment_atomically() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        let state_repo: Arc<dyn SyncStateRepository> =
            Arc::new(SqliteSyncStateRepository::new(pool.clone()));
        let folder_store = folder_with_laptop_segment(laptop_change(
            RecordKind::Account,
            "account-laptop",
            r#"{"id":"account-laptop","name":"Livret","currency":"EUR"}"#,
        ));
        let mut applier = MockChangeApplier::new();
        applier.expect_live_record().returning(|_, _, _| Ok(None));
        applier
            .expect_clashing_name()
            .returning(|_, _, _, _| Ok(None));
        applier
            .expect_write()
            .withf(|_, change| change.record_identity == "account-laptop")
            .times(1)
            .returning(|_, _| Ok(()));
        let run = run_over(&pool, Arc::clone(&state_repo), folder_store);

        let report = run
            .run(&device(), &applier)
            .await
            .expect("SYN-060/065: a full run must succeed and report what it did");
        assert_eq!(report.applied_changes, 1);
        assert_eq!(report.held_back_changes, 0);
        assert!(report.failures.is_empty());
        let cursor = state_repo
            .get_cursor("laptop-device")
            .await
            .unwrap()
            .expect("SYN-033: the cursor on Laptop must exist after the run");
        assert_eq!(cursor.applied_through, 1);
        assert_eq!(
            report.status.roster,
            vec![RosterEntry {
                device_id: "laptop-device".into(),
                device_name: "Laptop".into(),
                data_format_version: DATA_FORMAT_VERSION,
                app_version: Some("0.42.0".into()),
                last_applied_at: cursor.last_applied_at,
            }],
            "SYN-063: the roster names every other device the run read, with the version \
             its manifest states and when its changes were last applied here"
        );
    }

    // SYN-041 — a change referring to a record this device has not received (its account)
    // is held back, not rejected nor written; the sync cursor still advances past it, and
    // the held-back row waits for the next run.
    #[tokio::test]
    async fn full_run_holds_back_a_change_whose_parent_has_not_arrived() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        let state_repo: Arc<dyn SyncStateRepository> =
            Arc::new(SqliteSyncStateRepository::new(pool.clone()));
        let folder_store = folder_with_laptop_segment(laptop_change(
            RecordKind::Transaction,
            "tx-laptop",
            r#"{"id":"tx-laptop","account_id":"account-unknown","asset_id":"asset-unknown"}"#,
        ));
        let mut applier = MockChangeApplier::new();
        applier.expect_live_record().returning(|_, _, _| Ok(None));
        let run = run_over(&pool, Arc::clone(&state_repo), folder_store);

        let report = run
            .run(&device(), &applier)
            .await
            .expect("SYN-060/065: a full run must succeed and report what it did");
        assert_eq!(report.held_back_changes, 1);
        assert_eq!(report.applied_changes, 0);
        let held = state_repo.list_held_back().await.unwrap();
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].waiting_kind, RecordKind::Account);
        assert_eq!(held[0].waiting_identity, "account-unknown");
        assert_eq!(
            state_repo
                .get_cursor("laptop-device")
                .await
                .unwrap()
                .map(|cursor| cursor.applied_through),
            Some(1),
            "SYN-041: the cursor advances past a held-back change"
        );
    }
    // SYN-034/CFR-012 — a change that declares identity A but carries record B's content is
    // malformed: the whole segment is skipped and counted unreadable, nothing is written,
    // and the cursor does not advance past it.
    #[tokio::test]
    async fn full_run_skips_a_segment_whose_change_identity_does_not_match_its_content() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        let state_repo: Arc<dyn SyncStateRepository> =
            Arc::new(SqliteSyncStateRepository::new(pool.clone()));
        let folder_store = folder_with_laptop_segment(laptop_change(
            RecordKind::Account,
            "account-a",
            r#"{"id":"account-b","name":"Livret","currency":"EUR"}"#,
        ));
        // No expectations: any read or write through the applier fails the test.
        let applier = MockChangeApplier::new();
        let run = run_over(&pool, Arc::clone(&state_repo), folder_store);

        let report = run
            .run(&device(), &applier)
            .await
            .expect("SYN-062: never rejects for the folder's state");
        assert_eq!(report.applied_changes, 0);
        assert!(
            report
                .failures
                .contains(&SyncFailure::UnreadableFiles { count: 1 }),
            "SYN-034: the malformed segment must be counted unreadable: {:?}",
            report.failures
        );
        assert!(
            state_repo
                .get_cursor("laptop-device")
                .await
                .unwrap()
                .is_none(),
            "the cursor must not advance past a skipped segment"
        );
    }

    // SYN-020 — applying never records a change: a write that goes through the change
    // recorder during the apply transaction is met by the suspended recorder and leaves no
    // `changes` row for this device.
    #[tokio::test]
    async fn full_run_suspends_the_change_recorder_while_applying() {
        struct RecordingApplier {
            recorder: Arc<SqliteChangeRecorder>,
            recorded: std::sync::Mutex<Option<Option<crate::shared::domain::Rank>>>,
        }

        #[async_trait::async_trait]
        impl ChangeApplier for RecordingApplier {
            async fn live_record(
                &self,
                _conn: &mut SqliteConnection,
                _kind: RecordKind,
                _identity: &str,
            ) -> Result<Option<crate::shared::domain::SyncedRecord>, SyncError> {
                Ok(None)
            }
            async fn children_of_account(
                &self,
                _conn: &mut SqliteConnection,
                _account_id: &str,
            ) -> Result<Vec<crate::shared::domain::SyncedChild>, SyncError> {
                Ok(vec![])
            }
            async fn clashing_name(
                &self,
                _conn: &mut SqliteConnection,
                _kind: RecordKind,
                _identity: &str,
                _name: &str,
            ) -> Result<Option<crate::shared::domain::Rank>, SyncError> {
                Ok(None)
            }
            async fn write(
                &self,
                conn: &mut SqliteConnection,
                change: &Change,
            ) -> Result<(), SyncError> {
                let draft = ChangeDraft::new(
                    change.record_kind,
                    RecordIdentity::canonical(change.record_kind, &[&change.record_identity]),
                    Operation::Created,
                    Origin::User,
                    None,
                    change.content.clone(),
                );
                let rank = self
                    .recorder
                    .record(conn, draft)
                    .await
                    .map_err(|_| SyncError::DatabaseError)?;
                *self.recorded.lock().unwrap() = Some(rank);
                Ok(())
            }
            async fn discard_observations(
                &self,
                _conn: &mut SqliteConnection,
            ) -> Result<(), SyncError> {
                Ok(())
            }
        }

        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        let state_repo: Arc<dyn SyncStateRepository> =
            Arc::new(SqliteSyncStateRepository::new(pool.clone()));
        let folder_store = folder_with_laptop_segment(laptop_change(
            RecordKind::Account,
            "account-laptop",
            r#"{"id":"account-laptop","name":"Livret","currency":"EUR"}"#,
        ));
        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let applier = RecordingApplier {
            recorder: Arc::clone(&recorder),
            recorded: std::sync::Mutex::new(None),
        };
        let run = SyncRun::new(
            Arc::new(SqliteChangeLogRepository::new(pool.clone())),
            state_repo,
            Arc::new(folder_store),
            recorder,
        );

        let report = run.run(&device(), &applier).await.expect("run");
        assert_eq!(report.applied_changes, 1);
        assert_eq!(
            *applier.recorded.lock().unwrap(),
            Some(crate::shared::domain::Rank::NEVER),
            "SYN-020: the recorder must be suspended for the whole apply"
        );
        let recorded: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM changes WHERE device_id = 'desktop-device'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            recorded, 0,
            "SYN-020: applying leaves no changes row for this device"
        );
        assert!(
            run.change_recorder.is_recording().await,
            "the gate is released once the run completes"
        );
    }

    // SYN-041 — a change held back by an earlier run is applied in the very run that brings
    // what it waits for: the fresh intake is applied first, then the held-back rows are
    // retried, so the parent's arrival reunites the child at once.
    #[tokio::test]
    async fn full_run_reunites_a_held_back_child_with_the_parent_arriving_in_the_same_run() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        let state_repo: Arc<dyn SyncStateRepository> =
            Arc::new(SqliteSyncStateRepository::new(pool.clone()));
        let held_change = Change {
            device_id: "office-device".into(),
            record_kind: RecordKind::Transaction,
            record_identity: "tx-office".into(),
            operation: Operation::Created,
            origin: Origin::User,
            logical_timestamp: crate::shared::domain::LogicalTimestamp::new(3),
            based_on: None,
            content: Some(
                r#"{"id":"tx-office","account_id":"account-laptop","asset_id":"system-cash-eur"}"#
                    .into(),
            ),
        };
        state_repo
            .insert_held_back(&HeldBackChange {
                id: "held-1".into(),
                origin_device_id: "office-device".into(),
                sequence: 1,
                payload: serde_json::to_string(&held_change).unwrap(),
                waiting_kind: RecordKind::Account,
                waiting_identity: "account-laptop".into(),
                held_since: "2026-08-22T00:00:00Z".into(),
            })
            .await
            .unwrap();
        let folder_store = folder_with_laptop_segment(laptop_change(
            RecordKind::Account,
            "account-laptop",
            r#"{"id":"account-laptop","name":"Livret","currency":"EUR"}"#,
        ));
        // The applier remembers what it wrote, so a later lookup finds the parent.
        let written: Arc<std::sync::Mutex<std::collections::HashSet<String>>> =
            Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
        let mut applier = MockChangeApplier::new();
        let lookup = Arc::clone(&written);
        applier
            .expect_live_record()
            .returning(move |_, _, identity| {
                Ok(lookup.lock().unwrap().contains(identity).then(|| {
                    crate::shared::domain::SyncedRecord {
                        rank: None,
                        content: "{}".into(),
                    }
                }))
            });
        applier
            .expect_clashing_name()
            .returning(|_, _, _, _| Ok(None));
        let writes = Arc::clone(&written);
        applier.expect_write().times(2).returning(move |_, change| {
            writes
                .lock()
                .unwrap()
                .insert(change.record_identity.clone());
            Ok(())
        });
        let run = run_over(&pool, Arc::clone(&state_repo), folder_store);

        let report = run.run(&device(), &applier).await.expect("run");
        assert_eq!(
            report.applied_changes, 2,
            "SYN-041: the parent and the reunited child both apply this run"
        );
        assert_eq!(report.held_back_changes, 0);
        assert!(
            state_repo.list_held_back().await.unwrap().is_empty(),
            "the reunited change leaves the held-back table"
        );
    }
}
