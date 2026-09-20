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
        JoinError::HistoryIncomplete
    })?;
    if manifest.data_format_version > DATA_FORMAT_VERSION {
        return Err(SyncError::UpdateRequired {
            data_format_version: manifest.data_format_version,
        }
        .into());
    }
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
            JoinError::HistoryIncomplete
        })?;
        if segment.data_format_version > DATA_FORMAT_VERSION {
            return Err(SyncError::UpdateRequired {
                data_format_version: segment.data_format_version,
            }
            .into());
        }
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
