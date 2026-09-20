//! The intake of one sync run (SYN-033/034/035/037/063): every other device's manifest and
//! the segments past this device's sync cursor, decoded and shape-checked into the changes
//! the apply transaction (`run.rs`) will hand to the executor, plus the cursors to advance
//! and the roster the manifests name.

use crate::context::sync::domain::{
    segment_sequence_range, Change, FolderStore, MalformedChange, RosterEntry, Segment, SyncCursor,
    SyncDevice, SyncFailure, SyncStateRepository,
};
use crate::context::sync::error::SyncError;
use crate::context::sync::infrastructure::codec::{
    decode_manifest, decode_segment, DATA_FORMAT_VERSION,
};
use crate::context::sync::infrastructure::crypto::Key;
use crate::core::logger::BACKEND;

/// One change read from another device's segment, with where it came from (SYN-033).
pub(super) struct IncomingChange {
    pub origin_device_id: String,
    pub sequence: i64,
    pub change: Change,
}

/// What the other devices' areas yielded this run (SYN-033/034/037): the changes to apply,
/// the cursors to advance, and the roster every readable manifest names (SYN-063).
#[derive(Default)]
pub(super) struct Intake {
    pub changes: Vec<IncomingChange>,
    pub cursors: Vec<SyncCursor>,
    pub roster: Vec<RosterEntry>,
    pub unreadable_files: u32,
    pub failures: Vec<SyncFailure>,
}

/// A segment's changes past `after`, in the engine's shape; `Err` when one of them is
/// malformed — the whole segment is unreadable (SYN-034).
fn segment_changes(segment: Segment, after: i64) -> Result<Vec<IncomingChange>, MalformedChange> {
    let device_id = segment.device_id;
    segment
        .changes
        .into_iter()
        .filter(|change| change.sequence > after)
        .map(|change| {
            let sequence = change.sequence;
            Change::from_segment_change(&device_id, change).map(|change| IncomingChange {
                origin_device_id: device_id.clone(),
                sequence,
                change,
            })
        })
        .collect()
}

/// Reads every other device's manifest and the segments past this device's cursor for
/// it (SYN-033/037), naming each readable manifest's device in the roster with when its
/// changes were last applied here (SYN-063). A manifest or segment that cannot be
/// decoded is skipped and counted (SYN-034); a segment this device has not yet received
/// in full (a gap before it) stops the read of that area until the next run; a newer
/// data format is reported (SYN-035). A folder failure aborts the read (`Err`).
pub(super) async fn read_other_devices(
    folder_store: &dyn FolderStore,
    state_repo: &dyn SyncStateRepository,
    device: &SyncDevice,
    key: &Key,
) -> Result<Intake, SyncError> {
    let mut intake = Intake::default();
    for other in folder_store.list_device_ids().await? {
        if other == device.device_id {
            continue;
        }
        let Some(bytes) = folder_store.read_manifest_bytes(&other).await? else {
            continue;
        };
        let manifest = match decode_manifest(key, &bytes) {
            Ok(manifest) => manifest,
            Err(error) => {
                tracing::warn!(target: BACKEND, device_id = %other, err = %error, "run: manifest skipped");
                intake.unreadable_files += 1;
                continue;
            }
        };
        let cursor = state_repo.get_cursor(&other).await?;
        intake.roster.push(RosterEntry {
            device_id: other.clone(),
            device_name: manifest.device_name.clone(),
            data_format_version: manifest.data_format_version,
            app_version: manifest.app_version.clone(),
            last_applied_at: cursor
                .as_ref()
                .and_then(|cursor| cursor.last_applied_at.clone()),
        });
        if manifest.data_format_version > DATA_FORMAT_VERSION {
            intake.failures.push(SyncFailure::UpdateRequired {
                data_format_version: manifest.data_format_version,
            });
            continue;
        }
        let applied_through = cursor.map_or(0, |cursor| cursor.applied_through);
        if manifest.latest_sequence <= applied_through {
            continue;
        }
        let reached =
            read_segments(folder_store, &other, key, applied_through, &mut intake).await?;
        if reached > applied_through {
            let last_applied_at = Some(chrono::Utc::now().to_rfc3339());
            if let Some(entry) = intake.roster.last_mut() {
                entry.last_applied_at = last_applied_at.clone();
            }
            intake.cursors.push(SyncCursor {
                device_id: other,
                applied_through: reached,
                last_applied_at,
            });
        }
    }
    Ok(intake)
}

/// Reads `other`'s segments past `applied_through` in sequence order, returning the
/// last sequence taken in.
async fn read_segments(
    folder_store: &dyn FolderStore,
    other: &str,
    key: &Key,
    applied_through: i64,
    intake: &mut Intake,
) -> Result<i64, SyncError> {
    let mut names: Vec<(i64, i64, String)> = folder_store
        .list_segment_names(other)
        .await?
        .into_iter()
        .filter_map(|name| segment_sequence_range(&name).map(|(first, last)| (first, last, name)))
        .filter(|(_, last, _)| *last > applied_through)
        .collect();
    names.sort();
    let mut reached = applied_through;
    for (first, last, name) in names {
        if first > reached + 1 {
            break;
        }
        let Some(bytes) = folder_store.read_segment_bytes(other, &name).await? else {
            break;
        };
        let changes = decode_segment(key, &bytes)
            .map_err(|error| error.to_string())
            .and_then(|segment| {
                if segment.device_id != other {
                    return Err("segment belongs to another device".to_string());
                }
                if segment.data_format_version > DATA_FORMAT_VERSION {
                    intake.failures.push(SyncFailure::UpdateRequired {
                        data_format_version: segment.data_format_version,
                    });
                    return Ok(vec![]);
                }
                segment_changes(segment, reached).map_err(|problem| problem.to_string())
            });
        let changes = match changes {
            Ok(changes) => changes,
            Err(reason) => {
                tracing::warn!(target: BACKEND, device_id = %other, name = %name, reason = %reason, "run: segment skipped");
                intake.unreadable_files += 1;
                break;
            }
        };
        intake.changes.extend(changes);
        reached = reached.max(last);
    }
    Ok(reached)
}
