//! Joining a portfolio another device created (SYN-014/015/036/080/083): a fresh
//! installation derives the key from the folder header, checks the passphrase, reads every
//! device's whole published history, and rebuilds the portfolio by replaying it in logical
//! order — in one transaction, rolled back entirely on any failure. `SyncRun::join` is the
//! entry point; the orchestrator decides beforehand that the installation holds no user data
//! (`InstallationHoldsUserData`).

use zeroize::Zeroizing;

use crate::context::sync::application::apply::{apply_change, Applied};
use crate::context::sync::domain::{
    ensure_device_name, replay_order, segment_sequence_range, Change, ChangeApplier,
    ChangeLogRepository, FolderProblem, FolderStore, HeldBackChange, Manifest, SyncCursor,
    SyncDevice, SyncStateRepository, SyncStatus, WaitingFor, APP_VERSION,
};
use crate::context::sync::error::SyncError;
use crate::context::sync::infrastructure::codec::{
    decode_header, decode_manifest, decode_segment, encode_manifest, header_data_format_version,
    DATA_FORMAT_VERSION,
};
use crate::context::sync::infrastructure::crypto::{
    derive_key_blocking, ensure_derivation_parameters, ensure_passphrase_length, verify_check,
};
use crate::core::logger::BACKEND;
use crate::shared::infrastructure::change_recorder::ChangeRecorder;

/// Why a join did not complete. The two rebuild-specific outcomes are the use case's task
/// codes (`PortfolioSyncTask`); everything else is the sync BC's own rejection.
#[derive(Debug)]
pub enum JoinError {
    /// A sync-BC rejection: folder, passphrase, data format, device name, database.
    Sync(SyncError),
    /// A device's published history is missing a manifest or a segment (SYN-036).
    HistoryIncomplete,
    /// The rebuild failed partway and was rolled back; the installation is as before
    /// (SYN-080).
    RebuildInterrupted,
}

impl From<SyncError> for JoinError {
    fn from(error: SyncError) -> Self {
        JoinError::Sync(error)
    }
}

/// One change of the history, with where it came from.
struct HistoryChange {
    origin_device_id: String,
    sequence: i64,
    change: Change,
}

/// Everything the folder holds: every change of every device, the cursors to set once
/// they are applied, and the roster's names.
struct History {
    changes: Vec<HistoryChange>,
    cursors: Vec<SyncCursor>,
}

/// A manifest or a segment a join could not read: one written in a newer data format asks
/// for an update (SYN-035); anything else leaves the history incomplete (SYN-036).
fn unreadable(error: SyncError) -> JoinError {
    match error {
        SyncError::UpdateRequired { .. } => JoinError::Sync(error),
        _ => JoinError::HistoryIncomplete,
    }
}

/// Reads one device's complete history: its manifest, then its segments, which must cover
/// `1..=latest_sequence` without a gap (SYN-036).
async fn read_device_history(
    folder_store: &dyn FolderStore,
    key: &crate::context::sync::infrastructure::crypto::Key,
    device_id: &str,
    history: &mut History,
) -> Result<(), JoinError> {
    let manifest_bytes = folder_store
        .read_manifest_bytes(device_id)
        .await?
        .ok_or(JoinError::HistoryIncomplete)?;
    let manifest = decode_manifest(key, &manifest_bytes).map_err(|error| {
        tracing::warn!(target: BACKEND, device_id, err = %error, "join: manifest unreadable");
        unreadable(error)
    })?;
    let mut names: Vec<(i64, i64, String)> = folder_store
        .list_segment_names(device_id)
        .await?
        .into_iter()
        .filter_map(|name| segment_sequence_range(&name).map(|(first, last)| (first, last, name)))
        .collect();
    names.sort();
    let mut expected = 1;
    for (first, last, name) in names {
        if first != expected {
            return Err(JoinError::HistoryIncomplete);
        }
        let bytes = folder_store
            .read_segment_bytes(device_id, &name)
            .await?
            .ok_or(JoinError::HistoryIncomplete)?;
        let segment = decode_segment(key, &bytes).map_err(|error| {
            tracing::warn!(target: BACKEND, device_id, name = %name, err = %error, "join: segment unreadable");
            unreadable(error)
        })?;
        for segment_change in segment.changes {
            let sequence = segment_change.sequence;
            let change =
                Change::from_segment_change(device_id, segment_change).map_err(|problem| {
                    tracing::warn!(target: BACKEND, device_id, name = %name, err = %problem, "join: segment malformed");
                    JoinError::HistoryIncomplete
                })?;
            history.changes.push(HistoryChange {
                origin_device_id: device_id.to_string(),
                sequence,
                change,
            });
        }
        expected = last + 1;
    }
    if expected <= manifest.latest_sequence {
        return Err(JoinError::HistoryIncomplete);
    }
    history.cursors.push(SyncCursor {
        device_id: device_id.to_string(),
        applied_through: manifest.latest_sequence,
        last_applied_at: Some(chrono::Utc::now().to_rfc3339()),
    });
    Ok(())
}

/// The components a join reads and writes through (SYN-014/036/080).
#[derive(Clone, Copy)]
pub(super) struct JoinPorts<'a> {
    pub change_log: &'a dyn ChangeLogRepository,
    pub state_repo: &'a dyn SyncStateRepository,
    pub folder_store: &'a dyn FolderStore,
    pub change_recorder: &'a dyn ChangeRecorder,
    pub applier: &'a dyn ChangeApplier,
}

/// What the user supplied to join (SYN-011).
pub(super) struct JoinRequest {
    pub folder: String,
    pub passphrase: String,
    pub device_name: String,
}

/// Joins the portfolio `folder` holds as a new device named `device_name`. The change
/// recorder stays suspended for the whole rebuild — replaying the history records nothing
/// (SYN-020).
pub(super) async fn join(
    ports: &JoinPorts<'_>,
    request: JoinRequest,
) -> Result<SyncStatus, JoinError> {
    let JoinPorts {
        change_log,
        folder_store,
        change_recorder,
        ..
    } = *ports;
    let JoinRequest {
        folder,
        passphrase,
        device_name,
    } = request;
    ensure_passphrase_length(&passphrase)?;
    ensure_device_name(&device_name)?;
    folder_store.retarget(&folder);
    folder_store
        .check_available()
        .await
        .map_err(|problem| SyncError::FolderUnavailable { problem })?;
    let header_bytes =
        folder_store
            .read_header_bytes()
            .await?
            .ok_or(SyncError::FolderUnavailable {
                problem: FolderProblem::Missing,
            })?;
    if let Some(data_format_version) =
        header_data_format_version(&header_bytes).filter(|version| *version > DATA_FORMAT_VERSION)
    {
        return Err(SyncError::UpdateRequired {
            data_format_version,
        }
        .into());
    }
    let header = decode_header(&header_bytes)?;
    ensure_derivation_parameters(&header.derivation_parameters)?;
    let key = derive_key_blocking(
        Zeroizing::new(passphrase),
        header.derivation_parameters.clone(),
    )
    .await?;
    if !verify_check(&key, &header.passphrase_check) {
        return Err(SyncError::PassphraseMismatch.into());
    }

    let mut history = History {
        changes: vec![],
        cursors: vec![],
    };
    for device_id in folder_store.list_device_ids().await? {
        read_device_history(folder_store, &key, &device_id, &mut history).await?;
    }
    history
        .changes
        .sort_by(|a, b| replay_order((&a.change, a.sequence), (&b.change, b.sequence)));
    let logical_clock = history
        .changes
        .iter()
        .map(|entry| entry.change.logical_timestamp.value() as i64)
        .max()
        .unwrap_or(0)
        + 1;
    let device = SyncDevice::new(
        device_name,
        folder,
        header.created_at.clone(),
        DATA_FORMAT_VERSION,
    )?;

    let _recording_suspended = change_recorder.suspend();
    let mut transaction = change_log.begin().await?;
    let rebuilt = rebuild(
        &mut transaction,
        ports,
        &device,
        key.as_bytes(),
        logical_clock,
        &history,
    )
    .await;
    if let Err(error) = rebuilt {
        tracing::error!(target: BACKEND, err = %error, "join: rebuild interrupted, rolled back");
        return Err(JoinError::RebuildInterrupted);
    }
    let manifest = Manifest {
        device_id: device.device_id.clone(),
        device_name: device.device_name.clone(),
        data_format_version: DATA_FORMAT_VERSION,
        app_version: Some(APP_VERSION.to_string()),
        latest_sequence: 0,
    };
    if let Err(error) = folder_store
        .write_manifest(&device.device_id, encode_manifest(&key, &manifest)?)
        .await
    {
        remove_device_area(folder_store, &device.device_id).await;
        return Err(error.into());
    }
    if let Err(error) = transaction.commit().await {
        tracing::error!(target: BACKEND, err = ?error, "join: commit failed");
        remove_device_area(folder_store, &device.device_id).await;
        return Err(JoinError::RebuildInterrupted);
    }
    Ok(SyncStatus::for_device(&device, None, vec![]))
}

/// Takes the joining device's area back out of the folder when its enrolment did not
/// complete (SYN-080): the header stays — it is the portfolio's, not this device's.
async fn remove_device_area(folder_store: &dyn FolderStore, device_id: &str) {
    if let Err(cleanup) = folder_store.remove_device_area(device_id).await {
        tracing::warn!(target: BACKEND, err = %cleanup, "join: device area not removed");
    }
}

/// The rebuild itself, on the enrolment transaction: the device row with its kept key,
/// the discarded observations (SYN-083), every change of the history applied in order,
/// what is held back, and the cursors at each device's latest sequence.
async fn rebuild(
    transaction: &mut sqlx::Transaction<'static, sqlx::Sqlite>,
    ports: &JoinPorts<'_>,
    device: &SyncDevice,
    key_bytes: &[u8],
    logical_clock: i64,
    history: &History,
) -> Result<(), SyncError> {
    let conn: &mut sqlx::SqliteConnection = transaction;
    let (change_log, state_repo, applier) = (ports.change_log, ports.state_repo, ports.applier);
    change_log
        .save_enrolment(conn, device, key_bytes, logical_clock)
        .await?;
    applier.discard_observations(conn).await?;
    let now = chrono::Utc::now().to_rfc3339();
    for entry in &history.changes {
        let result =
            apply_change(conn, applier, change_log, &device.device_id, &entry.change).await?;
        if let Applied::HeldBack(waiting_for) = result.applied {
            let (waiting_kind, waiting_identity) = match waiting_for {
                WaitingFor::Record { kind, identity } => (kind, identity),
                WaitingFor::OwnState { .. } => (
                    entry.change.record_kind,
                    entry.change.record_identity.clone(),
                ),
            };
            let payload = serde_json::to_string(&entry.change).map_err(|error| {
                tracing::error!(target: BACKEND, err = %error, "join: held-back payload not serialized");
                SyncError::DatabaseError
            })?;
            state_repo
                .insert_held_back_on(
                    conn,
                    &HeldBackChange {
                        id: uuid::Uuid::new_v4().to_string(),
                        origin_device_id: entry.origin_device_id.clone(),
                        sequence: entry.sequence,
                        payload,
                        waiting_kind,
                        waiting_identity,
                        held_since: now.clone(),
                    },
                )
                .await?;
        }
    }
    for cursor in &history.cursors {
        state_repo.upsert_cursor_on(conn, cursor).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::sync::domain::{
        segment_file_name, DerivationParameters, MockFolderStore, Segment,
    };
    use crate::context::sync::infrastructure::codec::encode_segment;
    use crate::context::sync::infrastructure::crypto::{derive_key, Key};

    const DEVICE: &str = "desktop-device";

    fn key() -> Key {
        let params = DerivationParameters {
            salt: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            memory_cost_kib: 19_456,
            iterations: 2,
            parallelism: 1,
        };
        derive_key("correct horse battery staple", &params).expect("valid parameters")
    }

    fn manifest_written_in(data_format_version: u32) -> Vec<u8> {
        let manifest = Manifest {
            device_id: DEVICE.into(),
            device_name: "Desktop".into(),
            data_format_version,
            app_version: Some(APP_VERSION.to_string()),
            latest_sequence: 1,
        };
        encode_manifest(&key(), &manifest).expect("sealed manifest")
    }

    fn segment_written_in(data_format_version: u32) -> Vec<u8> {
        let segment = Segment {
            device_id: DEVICE.into(),
            first_sequence: 1,
            last_sequence: 1,
            data_format_version,
            changes: vec![],
        };
        encode_segment(&key(), &segment).expect("sealed segment")
    }

    async fn read(folder_store: MockFolderStore) -> Result<usize, JoinError> {
        let mut history = History {
            changes: vec![],
            cursors: vec![],
        };
        read_device_history(&folder_store, &key(), DEVICE, &mut history).await?;
        Ok(history.cursors.len())
    }

    fn update_required(outcome: Result<usize, JoinError>) -> Option<u32> {
        match outcome {
            Err(JoinError::Sync(SyncError::UpdateRequired {
                data_format_version,
            })) => Some(data_format_version),
            _ => None,
        }
    }

    // SYN-035 — a device whose manifest was written in a newer data format is not read as
    // an incomplete history: the joining device is told to update.
    #[tokio::test]
    async fn syn_035_a_manifest_in_a_newer_data_format_asks_for_an_update() {
        let mut folder_store = MockFolderStore::new();
        folder_store
            .expect_read_manifest_bytes()
            .returning(|_| Ok(Some(manifest_written_in(DATA_FORMAT_VERSION + 1))));

        assert_eq!(
            update_required(read(folder_store).await),
            Some(DATA_FORMAT_VERSION + 1)
        );
    }

    // SYN-035 — the same for one of its segments, its manifest being readable.
    #[tokio::test]
    async fn syn_035_a_segment_in_a_newer_data_format_asks_for_an_update() {
        let mut folder_store = MockFolderStore::new();
        folder_store
            .expect_read_manifest_bytes()
            .returning(|_| Ok(Some(manifest_written_in(DATA_FORMAT_VERSION))));
        folder_store
            .expect_list_segment_names()
            .returning(|_| Ok(vec![segment_file_name(1, 1)]));
        folder_store
            .expect_read_segment_bytes()
            .returning(|_, _| Ok(Some(segment_written_in(DATA_FORMAT_VERSION + 1))));

        assert_eq!(
            update_required(read(folder_store).await),
            Some(DATA_FORMAT_VERSION + 1)
        );
    }

    // SYN-036 — a history written in this build's format is read whole, and a file that
    // cannot be opened leaves it incomplete.
    #[tokio::test]
    async fn syn_036_a_history_in_this_format_is_read_and_a_damaged_file_is_not() {
        let mut readable = MockFolderStore::new();
        readable
            .expect_read_manifest_bytes()
            .returning(|_| Ok(Some(manifest_written_in(DATA_FORMAT_VERSION))));
        readable
            .expect_list_segment_names()
            .returning(|_| Ok(vec![segment_file_name(1, 1)]));
        readable
            .expect_read_segment_bytes()
            .returning(|_, _| Ok(Some(segment_written_in(DATA_FORMAT_VERSION))));
        assert!(matches!(read(readable).await, Ok(1)));

        let mut damaged = MockFolderStore::new();
        damaged.expect_read_manifest_bytes().returning(|_| {
            let mut bytes = manifest_written_in(DATA_FORMAT_VERSION);
            let last = bytes.len() - 1;
            bytes[last] ^= 0xff;
            Ok(Some(bytes))
        });
        assert!(matches!(
            read(damaged).await,
            Err(JoinError::HistoryIncomplete)
        ));
    }

    // SYN-035 — a folder with both a file in a newer format and a gap reports the first
    // one the read meets: the manifest, then the segments in order, a gap before the
    // segment that follows it.
    #[tokio::test]
    async fn syn_035_the_first_problem_met_is_the_one_reported() {
        let mut newer_then_short = MockFolderStore::new();
        newer_then_short
            .expect_read_manifest_bytes()
            .returning(|_| {
                let manifest = Manifest {
                    device_id: DEVICE.into(),
                    device_name: "Desktop".into(),
                    data_format_version: DATA_FORMAT_VERSION,
                    app_version: Some(APP_VERSION.to_string()),
                    latest_sequence: 5,
                };
                Ok(Some(encode_manifest(&key(), &manifest).expect("sealed")))
            });
        newer_then_short
            .expect_list_segment_names()
            .returning(|_| Ok(vec![segment_file_name(1, 1)]));
        newer_then_short
            .expect_read_segment_bytes()
            .returning(|_, _| Ok(Some(segment_written_in(DATA_FORMAT_VERSION + 1))));
        assert_eq!(
            update_required(read(newer_then_short).await),
            Some(DATA_FORMAT_VERSION + 1),
            "a newer segment is met before the history is found short of its manifest"
        );

        let mut gap_then_newer = MockFolderStore::new();
        gap_then_newer
            .expect_read_manifest_bytes()
            .returning(|_| Ok(Some(manifest_written_in(DATA_FORMAT_VERSION))));
        gap_then_newer
            .expect_list_segment_names()
            .returning(|_| Ok(vec![segment_file_name(2, 2)]));
        gap_then_newer
            .expect_read_segment_bytes()
            .times(0)
            .returning(|_, _| Ok(Some(segment_written_in(DATA_FORMAT_VERSION + 1))));
        assert!(
            matches!(
                read(gap_then_newer).await,
                Err(JoinError::HistoryIncomplete)
            ),
            "a missing first segment is met before the newer one that follows it"
        );
    }

    // SYN-080 — an enrolment that does not complete takes the device's area back out of
    // the folder; a folder that refuses is logged, never an error of its own.
    #[tokio::test]
    async fn syn_080_an_unfinished_enrolment_takes_its_area_out_of_the_folder() {
        let mut folder_store = MockFolderStore::new();
        folder_store
            .expect_remove_device_area()
            .withf(|device_id| device_id == DEVICE)
            .times(1)
            .returning(|_| Ok(()));
        remove_device_area(&folder_store, DEVICE).await;

        let mut refusing = MockFolderStore::new();
        refusing
            .expect_remove_device_area()
            .times(1)
            .returning(|_| {
                Err(SyncError::FolderUnavailable {
                    problem: FolderProblem::Missing,
                })
            });
        remove_device_area(&refusing, DEVICE).await;
    }
}
