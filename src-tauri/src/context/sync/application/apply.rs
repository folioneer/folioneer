//! The apply executor (D4, ADR-019 constraint 2): takes one incoming change, asks the
//! resolution engine (`domain::resolution`) what to do with it, and carries the decision out
//! through the owning contexts (`ChangeApplier`), the tombstones (`ChangeLogRepository`),
//! and the notices it returns to its caller. It compares no rank, timestamp, or origin of
//! its own — every decision below is a call into the engine. Every read and write rides the
//! apply transaction's connection (SYN-065).

use std::collections::HashSet;

use sqlx::SqliteConnection;

use crate::context::sync::domain::{
    account_parent, cascade_child_tombstones, decide, display_name, duplicate_name_notice,
    parent_references, reference_outcome, removed_child_notice, upgraded_content, Change,
    ChangeApplier, ChangeLogRepository, NoticeDraft, Outcome, RecordState, Tombstone, WaitingFor,
};
use crate::context::sync::error::SyncError;
use crate::shared::domain::{Operation, RecordKind};

/// What applying one change amounted to (SYN-062's counts).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applied {
    /// The change became the record's state (or merged into it, CFR-044).
    Applied,
    /// The record's current state stands.
    Ignored,
    /// The change was dropped — its account stands removed (CFR-032).
    Dropped,
    /// The change waits for something this device has not received (SYN-041).
    HeldBack(WaitingFor),
}

/// The outcome of applying one change, plus the notices raised on this device by it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyResult {
    /// What happened to the change.
    pub applied: Applied,
    /// The notices CFR-060 raises on this device for it (never another device's).
    pub notices: Vec<NoticeDraft>,
}

/// The record's current state on this device and, when live, its content (CFR-014/015).
async fn state_of(
    conn: &mut SqliteConnection,
    applier: &dyn ChangeApplier,
    change_log: &dyn ChangeLogRepository,
    kind: RecordKind,
    identity: &str,
) -> Result<Option<(RecordState, Option<String>)>, SyncError> {
    if let Some(record) = applier.live_record(conn, kind, identity).await? {
        return Ok(Some((RecordState::live(record.rank), Some(record.content))));
    }
    Ok(change_log
        .tombstone(conn, kind, identity)
        .await?
        .map(|tombstone| (RecordState::Tombstone(tombstone.rank()), None)))
}

/// Applies `change` on `own_device_id`: holds it back when it refers to a record this
/// device has not received (CFR-031/033), drops it when its account stands removed
/// (CFR-032), and otherwise writes whatever the engine decides (CFR-020 and the rest).
pub async fn apply_change(
    conn: &mut SqliteConnection,
    applier: &dyn ChangeApplier,
    change_log: &dyn ChangeLogRepository,
    own_device_id: &str,
    change: &Change,
) -> Result<ApplyResult, SyncError> {
    let mut known: HashSet<String> = HashSet::new();
    for (kind, identity) in parent_references(change) {
        if state_of(conn, applier, change_log, kind, &identity)
            .await?
            .is_some()
        {
            known.insert(identity);
        }
    }
    let account_state = match account_parent(change) {
        Some(account_id) => state_of(conn, applier, change_log, RecordKind::Account, &account_id)
            .await?
            .map(|(state, _)| state),
        None => None,
    };
    match reference_outcome(change, &known, account_state.as_ref()) {
        Some(Outcome::Drop { notice }) => {
            return Ok(ApplyResult {
                applied: Applied::Dropped,
                notices: own_notices(vec![notice], own_device_id),
            });
        }
        Some(Outcome::HoldBack { waiting_for }) => {
            return Ok(ApplyResult {
                applied: Applied::HeldBack(waiting_for),
                notices: vec![],
            });
        }
        _ => {}
    }

    let current = state_of(
        conn,
        applier,
        change_log,
        change.record_kind,
        &change.record_identity,
    )
    .await?;
    let (state, content) = match &current {
        Some((state, content)) => (Some(state), content.as_deref()),
        None => (None, None),
    };
    let decision = decide(change, state, content);
    let mut notices = decision.notices;
    let applied = match decision.outcome {
        Outcome::Apply { .. } | Outcome::MergeMax => {
            write(conn, applier, change_log, change, content, &mut notices).await?;
            Applied::Applied
        }
        Outcome::Ignore { .. } => Applied::Ignored,
        Outcome::Drop { notice } => {
            notices.push(notice);
            Applied::Dropped
        }
        Outcome::HoldBack { waiting_for } => Applied::HeldBack(waiting_for),
    };
    Ok(ApplyResult {
        applied,
        notices: own_notices(notices, own_device_id),
    })
}

/// Only the notices CFR-060 raises on this device are persisted here; the other device
/// raises its own when it applies the same changes.
fn own_notices(notices: Vec<NoticeDraft>, own_device_id: &str) -> Vec<NoticeDraft> {
    notices
        .into_iter()
        .filter(|notice| notice.raised_on_device_id == own_device_id)
        .collect()
}

/// Writes a prevailing change: a removal leaves its tombstone and, for an account, removes
/// every child this device holds with a tombstone at the account tombstone's rank (CFR-030);
/// a creation or update writes the (format-upgraded, SYN-035) content, clears any tombstone,
/// and reports a display name it now shares with another record (CFR-035).
async fn write(
    conn: &mut SqliteConnection,
    applier: &dyn ChangeApplier,
    change_log: &dyn ChangeLogRepository,
    change: &Change,
    current_content: Option<&str>,
    notices: &mut Vec<NoticeDraft>,
) -> Result<(), SyncError> {
    match change.operation {
        Operation::Removed => {
            let children = if change.record_kind == RecordKind::Account {
                applier
                    .children_of_account(conn, &change.record_identity)
                    .await?
            } else {
                vec![]
            };
            applier.write(conn, change).await?;
            change_log
                .upsert_tombstone(conn, &tombstone_of(change))
                .await?;
            let account_rank = change.rank();
            let child_states: Vec<RecordState> = children
                .iter()
                .map(|child| RecordState::live(child.rank.clone()))
                .collect();
            let ranks = cascade_child_tombstones(&account_rank, &child_states);
            for (child, rank) in children.iter().zip(ranks) {
                change_log
                    .upsert_tombstone(
                        conn,
                        &Tombstone {
                            record_kind: child.record_kind,
                            record_identity: child.record_identity.clone(),
                            logical_timestamp: rank.logical_timestamp,
                            origin: rank.origin,
                            removed_by: change.device_id.clone(),
                        },
                    )
                    .await?;
                if let Some(child_rank) = &child.rank {
                    notices.push(removed_child_notice(
                        child.record_kind,
                        &child.record_identity,
                        child_rank,
                        &change.device_id,
                    ));
                }
            }
        }
        Operation::Created | Operation::Updated => {
            let upgraded = Change {
                content: upgraded_content(change, current_content),
                ..change.clone()
            };
            applier.write(conn, &upgraded).await?;
            change_log
                .clear_tombstone(conn, change.record_kind, &change.record_identity)
                .await?;
            if let Some(name) = display_name(change) {
                if let Some(clashing) = applier
                    .clashing_name(conn, change.record_kind, &change.record_identity, &name)
                    .await?
                {
                    if let Some((for_applied, for_clashing)) =
                        duplicate_name_notice(change, &clashing.device_id)
                    {
                        notices.push(for_applied);
                        notices.push(for_clashing);
                    }
                }
            }
        }
    }
    Ok(())
}

fn tombstone_of(change: &Change) -> Tombstone {
    Tombstone {
        record_kind: change.record_kind,
        record_identity: change.record_identity.clone(),
        logical_timestamp: change.logical_timestamp.clone(),
        origin: change.origin,
        removed_by: change.device_id.clone(),
    }
}
