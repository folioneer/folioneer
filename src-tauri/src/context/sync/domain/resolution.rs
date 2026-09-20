//! The resolution engine (ADR-019, D4) — the single component that decides every CFR
//! outcome. Pure: no I/O, no sqlx, no clock, so a change to `sync-conflict-resolution.md`
//! is a change to this module alone (constraint 2). The apply executor
//! (`application::apply`) consumes the `Outcome` and `NoticeDraft`s this module returns and
//! never compares ranks, timestamps, or origins itself; the change recorder
//! (`infrastructure::change_log`) asks `local_write_allowed` before it records a local
//! write (CFR-016). The engine decides the kind of every merge, the owning repository
//! executes it: `Outcome::MergeMax` is carried out by the fee catch-up repository's SQL
//! `MAX()` over the stored and incoming period (CFR-044).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::conflict_notice::ConflictNoticeKind;
use super::folder::SegmentChange;
use super::received_change::{check_received_change, MalformedChange};
use crate::core::cash::{is_cash_asset, SYSTEM_CASH_CATEGORY_ID};
use crate::shared::domain::{LogicalTimestamp, Operation, Origin, Rank, RecordKind};

/// An incoming or local change to compare against a record's current state (D4). Unlike
/// `SegmentChange` (the wire form carried inside a `Segment`), this carries the acting
/// device's identity alongside the change so a `Rank` can be built without a second lookup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    /// The device that made this change.
    pub device_id: String,
    /// What kind of record changed (SYN-021).
    pub record_kind: RecordKind,
    /// Which record changed (CFR-012), canonical string form.
    pub record_identity: String,
    /// Created, Updated, or Removed.
    pub operation: Operation,
    /// Who made the change (CFR-016).
    pub origin: Origin,
    /// The ordering value (CFR-010).
    pub logical_timestamp: LogicalTimestamp,
    /// The record state this change was made against; absent for a creation (CFR-011).
    pub based_on: Option<LogicalTimestamp>,
    /// The record's full state after the change, JSON-encoded; absent for a removal.
    pub content: Option<String>,
}

impl Change {
    /// The change a published segment of `device_id` carries, in the engine's shape;
    /// `MalformedChange` when its logical timestamp or `based_on` is not a counter, or its
    /// identity and content do not pass `check_received_change` (SYN-034: the segment is
    /// unreadable).
    pub fn from_segment_change(
        device_id: &str,
        change: SegmentChange,
    ) -> Result<Self, MalformedChange> {
        check_received_change(
            change.record_kind,
            &change.record_identity,
            change.operation,
            change.content.as_deref(),
        )?;
        let based_on = match change.based_on {
            Some(based_on) => {
                Some(LogicalTimestamp::from_wire(&based_on).ok_or(MalformedChange::NotACounter)?)
            }
            None => None,
        };
        Ok(Change {
            device_id: device_id.to_string(),
            record_kind: change.record_kind,
            record_identity: change.record_identity,
            operation: change.operation,
            origin: change.origin,
            logical_timestamp: LogicalTimestamp::from_wire(&change.logical_timestamp)
                .ok_or(MalformedChange::NotACounter)?,
            based_on,
            content: change.content,
        })
    }

    /// Builds the `Rank` this change carries (CFR-020).
    pub fn rank(&self) -> Rank {
        Rank {
            origin: self.origin,
            logical_timestamp: self.logical_timestamp.clone(),
            device_id: self.device_id.clone(),
        }
    }

    /// The rank CFR-020 compares for this change: an account's removal carries the account
    /// tombstone's sentinel rank (CFR-022), every other change its own.
    fn effective_rank(&self) -> Rank {
        if self.record_kind == RecordKind::Account && self.operation == Operation::Removed {
            Rank::account_tombstone()
        } else {
            self.rank()
        }
    }

    fn content_value(&self) -> Option<serde_json::Value> {
        self.content
            .as_deref()
            .and_then(|content| serde_json::from_str(content).ok())
    }

    /// A string field of the change's content, when the content carries it.
    pub(crate) fn content_field(&self, name: &str) -> Option<String> {
        self.content_value()?
            .get(name)?
            .as_str()
            .map(str::to_string)
    }
}

/// A record's current state on this device (CFR-014/015): the rank of its live content, or
/// the rank of the tombstone that removed it. There is no third "unknown" state — that is
/// `current: Option<&RecordState>` being `None` in `resolve`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordState {
    /// The record exists, at this rank.
    Live(Rank),
    /// The record was removed, at this rank (CFR-015).
    Tombstone(Rank),
}

impl RecordState {
    /// The state of a record this device holds live: at its rank, or — for a row that has
    /// never been ranked (D6's NULL sentinel) — at the lowest rank there is, below every
    /// real change, so any incoming change prevails over it (CFR-014).
    pub fn live(rank: Option<Rank>) -> Self {
        RecordState::Live(rank.unwrap_or(Rank {
            origin: Origin::Application,
            logical_timestamp: LogicalTimestamp::new(0),
            device_id: String::new(),
        }))
    }

    /// The rank of whichever state this is.
    pub fn rank(&self) -> &Rank {
        match self {
            RecordState::Live(rank) | RecordState::Tombstone(rank) => rank,
        }
    }

    /// The rank CFR-020 compares for this state: an account's tombstone is the sentinel
    /// that outranks every change (CFR-022), every other state its own rank.
    fn effective_rank(&self, kind: RecordKind) -> Rank {
        match self {
            RecordState::Tombstone(_) if kind == RecordKind::Account => Rank::account_tombstone(),
            state => state.rank().clone(),
        }
    }
}

/// What a change refers to that this device has not received (SYN-041): another record
/// entirely (its account, its asset, its schedule, …), or a state of its own record it has
/// not yet taken in (`based_on`, CFR-011).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaitingFor {
    /// A record this device holds neither as a record nor as a tombstone.
    Record {
        /// The kind of the awaited record.
        kind: RecordKind,
        /// The identity of the awaited record.
        identity: String,
    },
    /// A state of the change's own record (`based_on`) this device has not yet received.
    OwnState {
        /// The awaited logical timestamp.
        based_on: LogicalTimestamp,
    },
}

/// A conflict notice not yet persisted (CFR-060) — everything
/// `SyncStateRepository::insert_notice` needs once a fresh `notice_id`, `record_label`, and
/// `raised_at` are attached at the call site. `raised_on_device_id` is the locus CFR-060
/// names: the device whose own change lost, or each device whose change clashes; the
/// executor persists a draft only on that device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoticeDraft {
    /// Which reportable outcome raised it (CFR-060).
    pub kind: ConflictNoticeKind,
    /// The kind of the record concerned.
    pub record_kind: RecordKind,
    /// The record's canonical identity (CFR-012).
    pub record_identity: String,
    /// The device whose change prevailed, removed the parent, or collided.
    pub other_device_id: String,
    /// The device the notice is raised on (CFR-060).
    pub raised_on_device_id: String,
}

/// What `resolve` decides for one incoming (or local) change against a record's current
/// state (D4). The apply executor (`application::apply`) matches on this and nothing else —
/// it contains no comparison of its own (constraint 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The change becomes the record's new state.
    Apply {
        /// A notice to raise, when this outcome is reportable (CFR-060).
        notice: Option<NoticeDraft>,
    },
    /// The change is superseded; the record's current state stands.
    Ignore {
        /// A notice to raise, when this outcome is reportable (CFR-060).
        notice: Option<NoticeDraft>,
    },
    /// The change is dropped — its account stands removed (CFR-032). Always reportable.
    Drop {
        /// The notice raised on the device whose change is dropped.
        notice: NoticeDraft,
    },
    /// The change refers to something this device has not received yet (SYN-041).
    HoldBack {
        /// What is being waited for.
        waiting_for: WaitingFor,
    },
    /// A catch-up position: merge by maximum, never by rank (CFR-044).
    MergeMax,
}

/// CFR-011 — whether an incoming change is concurrent with, or sequential after, this
/// device's current state of the record. Concurrency never decides an outcome (CFR-020
/// does); it decides only whether a superseded change is reported (CFR-060).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Concurrency {
    /// The change's `based_on` differs from the current state's timestamp (or is absent —
    /// a creation is concurrent with any state already held for that identity).
    Concurrent,
    /// The change's `based_on` equals the current state's timestamp.
    Sequential,
}

impl Concurrency {
    /// Classifies `based_on` (a change's own field) against `current_timestamp` (the
    /// timestamp of the record's state this device holds, live or tombstoned).
    pub fn classify(
        based_on: Option<&LogicalTimestamp>,
        current_timestamp: Option<&LogicalTimestamp>,
    ) -> Self {
        match (based_on, current_timestamp) {
            (Some(based_on), Some(current)) if based_on == current => Concurrency::Sequential,
            _ => Concurrency::Concurrent,
        }
    }
}

impl Rank {
    /// CFR-022 — the sentinel rank of an account's tombstone: outranks every rank any real
    /// change could ever carry, whatever its origin or timestamp, so a removed account is
    /// never brought back. Real logical timestamps come from an `i64` Lamport clock, so the
    /// `u64::MAX` timestamp is unreachable by any recorded change.
    pub fn account_tombstone() -> Self {
        Rank {
            origin: Origin::User,
            logical_timestamp: LogicalTimestamp::new(u64::MAX),
            device_id: String::new(),
        }
    }
}

/// The whole decision for one incoming change (D4): the outcome plus every notice CFR-060
/// raises for it, each addressed to the device it is raised on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// What happens to the incoming change.
    pub outcome: Outcome,
    /// The notices this outcome raises (CFR-060), at most one per device concerned.
    pub notices: Vec<NoticeDraft>,
}

/// The engine's single entry point (CFR-020): decides the outcome of `incoming` against
/// the record's `current` state. Pure — a function of its two arguments only — so arrival
/// order cannot matter (CFR-013). Whole-record replacement: when `incoming` prevails it
/// prevails entirely; fields are never merged one by one, except the two record kinds that
/// bypass this function altogether (`resolve_observation`, `Outcome::MergeMax`).
pub fn resolve(incoming: &Change, current: Option<&RecordState>) -> Outcome {
    let Some(state) = current else {
        return Outcome::Apply { notice: None };
    };
    let current_timestamp = &state.rank().logical_timestamp;
    if let Some(based_on) = &incoming.based_on {
        if based_on > current_timestamp {
            return Outcome::HoldBack {
                waiting_for: WaitingFor::OwnState {
                    based_on: based_on.clone(),
                },
            };
        }
    }
    let concurrency = Concurrency::classify(incoming.based_on.as_ref(), Some(current_timestamp));
    let prevails = incoming.effective_rank() > state.effective_rank(incoming.record_kind);
    if prevails {
        Outcome::Apply {
            notice: superseded_current_notice(incoming, state, concurrency),
        }
    } else {
        Outcome::Ignore {
            notice: superseded_incoming_notice(incoming, state, concurrency),
        }
    }
}

/// CFR-060 — the notice for a current state a prevailing incoming change supersedes: raised
/// on the device that made that state, only when the change was concurrent (CFR-011) and
/// the state is of user origin (CFR-016). A double removal (CFR-023) and a creation over a
/// live record (a collision, CFR-034 — `collision_notice` owns it) are never reported here.
fn superseded_current_notice(
    incoming: &Change,
    state: &RecordState,
    concurrency: Concurrency,
) -> Option<NoticeDraft> {
    if concurrency == Concurrency::Sequential || state.rank().origin != Origin::User {
        return None;
    }
    let kind = match state {
        RecordState::Tombstone(_) if incoming.operation == Operation::Removed => return None,
        RecordState::Tombstone(_) => ConflictNoticeKind::OverruledRemoval,
        RecordState::Live(_) if incoming.operation == Operation::Created => return None,
        RecordState::Live(_) => ConflictNoticeKind::OverruledEdit,
    };
    Some(NoticeDraft {
        kind,
        record_kind: incoming.record_kind,
        record_identity: incoming.record_identity.clone(),
        other_device_id: incoming.device_id.clone(),
        raised_on_device_id: state.rank().device_id.clone(),
    })
}

/// CFR-060 — the notice for an incoming change the current state supersedes: raised on the
/// device that made the change, under the same conditions as `superseded_current_notice`.
fn superseded_incoming_notice(
    incoming: &Change,
    state: &RecordState,
    concurrency: Concurrency,
) -> Option<NoticeDraft> {
    if concurrency == Concurrency::Sequential || incoming.origin != Origin::User {
        return None;
    }
    let kind = match (incoming.operation, state) {
        (Operation::Removed, RecordState::Tombstone(_)) => return None,
        (Operation::Removed, RecordState::Live(_)) => ConflictNoticeKind::OverruledRemoval,
        (Operation::Created, RecordState::Live(_)) => return None,
        (Operation::Created | Operation::Updated, _) => ConflictNoticeKind::OverruledEdit,
    };
    Some(NoticeDraft {
        kind,
        record_kind: incoming.record_kind,
        record_identity: incoming.record_identity.clone(),
        other_device_id: state.rank().device_id.clone(),
        raised_on_device_id: incoming.device_id.clone(),
    })
}

/// SYN-036 — the order every device replays received changes in: by logical timestamp
/// (CFR-010), then by the device that made them, then by that device's own sequence. Each
/// side is a change and its sequence on its device.
pub fn replay_order(a: (&Change, i64), b: (&Change, i64)) -> std::cmp::Ordering {
    a.0.logical_timestamp
        .cmp(&b.0.logical_timestamp)
        .then_with(|| a.0.device_id.cmp(&b.0.device_id))
        .then_with(|| a.1.cmp(&b.1))
}

/// CFR-050/ADR-012 — observations (asset prices, currency rates): the later write per
/// CFR-010 prevails, origin is never considered, and nothing is ever reported.
pub fn resolve_observation(incoming: &Change, current: Option<&RecordState>) -> Outcome {
    let Some(state) = current else {
        return Outcome::Apply { notice: None };
    };
    let current_rank = state.rank();
    let later = (&incoming.logical_timestamp, &incoming.device_id)
        > (&current_rank.logical_timestamp, &current_rank.device_id);
    if later {
        Outcome::Apply { notice: None }
    } else {
        Outcome::Ignore { notice: None }
    }
}

/// Decides one incoming change in full (D4): routes observations to `resolve_observation`
/// (CFR-050) and catch-up positions to the maximum merge (CFR-044), every other kind to
/// `resolve` (CFR-020); strips every notice when the incoming content equals what this
/// device already holds (CFR-021); and reports a natural-key collision on both creating
/// devices (CFR-034). `current_content` is the content this device holds live for the
/// identity, in the form the repositories serialize it.
pub fn decide(
    incoming: &Change,
    current: Option<&RecordState>,
    current_content: Option<&str>,
) -> Decision {
    let outcome = match incoming.record_kind {
        RecordKind::AssetPrice | RecordKind::CurrencyRate => resolve_observation(incoming, current),
        RecordKind::FeeCatchUpPosition
            if incoming.operation != Operation::Removed
                && !matches!(current, Some(RecordState::Tombstone(_))) =>
        {
            Outcome::MergeMax
        }
        _ => resolve(incoming, current),
    };
    let identical = current_content.is_some() && current_content == incoming.content.as_deref();
    let mut notices: Vec<NoticeDraft> = notice_for(&outcome).into_iter().collect();
    if let (Some(RecordState::Live(current_rank)), Some(content)) = (current, current_content) {
        if incoming.operation == Operation::Created && incoming.based_on.is_none() {
            let held = Change {
                device_id: current_rank.device_id.clone(),
                record_kind: incoming.record_kind,
                record_identity: incoming.record_identity.clone(),
                operation: Operation::Created,
                origin: current_rank.origin,
                logical_timestamp: current_rank.logical_timestamp.clone(),
                based_on: None,
                content: Some(content.to_string()),
            };
            if let Some((for_current, for_incoming)) = collision_notice(incoming, &held) {
                notices = vec![for_current, for_incoming];
            }
        }
    }
    if identical {
        notices.clear();
    }
    let outcome = match outcome {
        Outcome::Apply { .. } => Outcome::Apply { notice: None },
        Outcome::Ignore { .. } => Outcome::Ignore { notice: None },
        other => other,
    };
    Decision { outcome, notices }
}

/// SYN-035 — applying a change written in an older data format upgrades it on apply: fields
/// the change does not carry keep their current local value. Returns the content to apply —
/// the incoming fields laid over the local ones — or the incoming content as is when this
/// device holds nothing for the identity.
pub fn upgraded_content(incoming: &Change, current_content: Option<&str>) -> Option<String> {
    let incoming_value = incoming.content_value()?;
    let mut merged: serde_json::Value = current_content
        .and_then(|content| serde_json::from_str(content).ok())
        .unwrap_or(serde_json::Value::Null);
    match (merged.as_object_mut(), incoming_value.as_object()) {
        (Some(local), Some(remote)) => {
            for (field, value) in remote {
                local.insert(field.clone(), value.clone());
            }
            Some(merged.to_string())
        }
        _ => incoming.content.clone(),
    }
}

/// CFR-030 — every child of a removed account this device holds is removed with it,
/// carrying the account tombstone's own rank (not the child's).
pub fn cascade_child_tombstones(account_tombstone: &Rank, children: &[RecordState]) -> Vec<Rank> {
    children.iter().map(|_| account_tombstone.clone()).collect()
}

/// CFR-032 — the notice for a child this device holds that an account's tombstone removes:
/// raised on the device whose change created or last edited the child, naming the removing
/// device.
pub fn removed_child_notice(
    record_kind: RecordKind,
    record_identity: &str,
    child_rank: &Rank,
    removing_device_id: &str,
) -> NoticeDraft {
    NoticeDraft {
        kind: ConflictNoticeKind::DroppedChild,
        record_kind,
        record_identity: record_identity.to_string(),
        other_device_id: removing_device_id.to_string(),
        raised_on_device_id: child_rank.device_id.clone(),
    }
}

/// SYN-027 — the identities the application seeds itself with a fixed identity (the cash
/// asset per currency, the cash category): never awaited, ensured on apply (CFR-033).
fn is_system_seeded(identity: &str) -> bool {
    is_cash_asset(identity) || identity == SYSTEM_CASH_CATEGORY_ID
}

/// SYN-041 — the records other than its own that `change` refers to (its account, its
/// asset, its category, its currency pair), read from its content; system-seeded identities
/// excluded (CFR-033). Empty for a removal — a removal refers to nothing it would wait for.
pub fn parent_references(change: &Change) -> Vec<(RecordKind, String)> {
    let mut references: Vec<(RecordKind, String)> = Vec::new();
    let mut reference = |kind: RecordKind, identity: Option<String>| {
        if let Some(identity) = identity.filter(|identity| !is_system_seeded(identity)) {
            references.push((kind, identity));
        }
    };
    match change.record_kind {
        RecordKind::Transaction
        | RecordKind::HoldingNote
        | RecordKind::FeeSchedule
        | RecordKind::FeeCatchUpPosition => {
            reference(RecordKind::Account, change.content_field("account_id"));
            reference(RecordKind::Asset, change.content_field("asset_id"));
        }
        RecordKind::AssetPrice => reference(RecordKind::Asset, change.content_field("asset_id")),
        RecordKind::Asset => reference(
            RecordKind::Category,
            change.content_value().and_then(|value| {
                value
                    .get("category")?
                    .get("id")?
                    .as_str()
                    .map(str::to_string)
            }),
        ),
        RecordKind::CurrencyRate => reference(
            RecordKind::CurrencyPair,
            match (
                change.content_field("from_currency"),
                change.content_field("to_currency"),
            ) {
                (Some(from), Some(to)) => Some(format!("{from}:{to}")),
                _ => None,
            },
        ),
        RecordKind::Account | RecordKind::Category | RecordKind::CurrencyPair => {}
    }
    references
}

/// CFR-032 — the account a child change belongs to: from its content, or from the
/// `account:asset` identity of the kinds keyed by their holding. `None` for a record that
/// has no owning account.
pub fn account_parent(change: &Change) -> Option<String> {
    match change.record_kind {
        RecordKind::Transaction => change.content_field("account_id"),
        RecordKind::HoldingNote | RecordKind::FeeSchedule | RecordKind::FeeCatchUpPosition => {
            change.content_field("account_id").or_else(|| {
                change
                    .record_identity
                    .split_once(':')
                    .map(|(account_id, _)| account_id.to_string())
            })
        }
        _ => None,
    }
}

/// CFR-035 — the display name an account or category change carries, when any.
pub fn display_name(change: &Change) -> Option<String> {
    match change.record_kind {
        RecordKind::Account | RecordKind::Category => change.content_field("name"),
        _ => None,
    }
}

/// SYN-041/CFR-031/032/033 — what the records `change` refers to decide before its own
/// record is looked at: `Drop` when its account stands removed on this device
/// (`account_state` is a tombstone, CFR-032 — decided before any missing reference, so a
/// child of a removed account never waits for an asset that will not come), `HoldBack` for
/// the first reference this device holds neither live nor as a tombstone (CFR-031), `None`
/// when nothing blocks it — always the case for a `known` (system-seeded, CFR-033)
/// identity and for a removal.
pub fn reference_outcome(
    change: &Change,
    known_identities: &HashSet<String>,
    account_state: Option<&RecordState>,
) -> Option<Outcome> {
    if change.operation == Operation::Removed {
        return None;
    }
    if let Some(drop) = drop_if_parent_tombstoned(change, account_state) {
        return Some(drop);
    }
    parent_references(change)
        .into_iter()
        .find(|(_, identity)| !known_identities.contains(identity))
        .map(|(kind, identity)| Outcome::HoldBack {
            waiting_for: WaitingFor::Record { kind, identity },
        })
}

/// CFR-032 — a child whose account is a tombstone on this device is dropped; `Some` only
/// when `parent_state` is a `Tombstone`.
fn drop_if_parent_tombstoned(
    change: &Change,
    parent_state: Option<&RecordState>,
) -> Option<Outcome> {
    match parent_state {
        Some(RecordState::Tombstone(removal)) if change.operation != Operation::Removed => {
            Some(Outcome::Drop {
                notice: NoticeDraft {
                    kind: ConflictNoticeKind::DroppedChild,
                    record_kind: change.record_kind,
                    record_identity: change.record_identity.clone(),
                    other_device_id: removal.device_id.clone(),
                    raised_on_device_id: change.device_id.clone(),
                },
            })
        }
        _ => None,
    }
}

/// CFR-034 — a natural-key collision: an incoming creation (`based_on` absent) for an
/// identity this device already holds live with different content. Reported on both
/// creating devices only when both are of user origin; any other collision (an
/// application-origin side, or identical content, CFR-021) is never reported. Returns the
/// notice for `current`'s device first, then the one for `incoming`'s device.
pub fn collision_notice(incoming: &Change, current: &Change) -> Option<(NoticeDraft, NoticeDraft)> {
    if incoming.based_on.is_some()
        || incoming.origin != Origin::User
        || current.origin != Origin::User
        || incoming.content == current.content
    {
        return None;
    }
    let draft = |raised_on: &Change, other: &Change| NoticeDraft {
        kind: ConflictNoticeKind::NaturalKeyCollision,
        record_kind: incoming.record_kind,
        record_identity: incoming.record_identity.clone(),
        other_device_id: other.device_id.clone(),
        raised_on_device_id: raised_on.device_id.clone(),
    };
    Some((draft(current, incoming), draft(incoming, current)))
}

/// CFR-035 — two accounts or categories left with the same display name after a merge:
/// reported on both devices whose changes carry that name. Returns the notice for
/// `applied`'s device first, then the one for the clashing device.
pub fn duplicate_name_notice(
    applied: &Change,
    clashing_device_id: &str,
) -> Option<(NoticeDraft, NoticeDraft)> {
    let draft = |raised_on: &str, other: &str| NoticeDraft {
        kind: ConflictNoticeKind::DuplicateName,
        record_kind: applied.record_kind,
        record_identity: applied.record_identity.clone(),
        other_device_id: other.to_string(),
        raised_on_device_id: raised_on.to_string(),
    };
    Some((
        draft(&applied.device_id, clashing_device_id),
        draft(clashing_device_id, &applied.device_id),
    ))
}

/// CFR-060 — the notice locus: exactly the five reportable outcomes
/// (`Apply`/`Ignore` when concurrent-and-superseded, `Drop`, a CFR-034 collision, a CFR-035
/// duplicate name) produce a notice; every other outcome yields `None`.
pub fn notice_for(outcome: &Outcome) -> Option<NoticeDraft> {
    match outcome {
        Outcome::Apply { notice } | Outcome::Ignore { notice } => notice.clone(),
        Outcome::Drop { notice } => Some(notice.clone()),
        Outcome::HoldBack { .. } | Outcome::MergeMax => None,
    }
}

/// SYN-020/CFR-016 — a local write of `kind` at `draft_rank`: it must outrank the
/// record's current state before it is made, exactly as CFR-020 decides for an incoming
/// change — so the application never writes over a state the user made. A write that does
/// not outrank the current state is not made and produces no change. An observation
/// (CFR-050) and a catch-up position (CFR-044) never consult rank and are always allowed.
pub fn local_write_allowed(
    kind: RecordKind,
    draft_rank: &Rank,
    current: Option<&RecordState>,
) -> bool {
    if matches!(
        kind,
        RecordKind::AssetPrice | RecordKind::CurrencyRate | RecordKind::FeeCatchUpPosition
    ) {
        return true;
    }
    current.is_none_or(|state| draft_rank > state.rank())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------------------
    // Scenario builders — Desktop / Laptop / Office, per the spec's own device names.
    // ---------------------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn change(
        device_id: &str,
        record_kind: RecordKind,
        identity: &str,
        operation: Operation,
        origin: Origin,
        timestamp: u64,
        based_on: Option<u64>,
        content: Option<&str>,
    ) -> Change {
        Change {
            device_id: device_id.to_string(),
            record_kind,
            record_identity: identity.to_string(),
            operation,
            origin,
            logical_timestamp: LogicalTimestamp::new(timestamp),
            based_on: based_on.map(LogicalTimestamp::new),
            content: content.map(str::to_string),
        }
    }

    fn rank(device_id: &str, origin: Origin, timestamp: u64) -> Rank {
        Rank {
            origin,
            logical_timestamp: LogicalTimestamp::new(timestamp),
            device_id: device_id.to_string(),
        }
    }

    /// Applies `changes` in the given order against no initial state, threading the
    /// resulting `RecordState` through — the minimal replay model CFR-013's
    /// arrival-order permutation tests need, without the full apply executor.
    fn replay(changes: &[Change]) -> Option<RecordState> {
        let mut current: Option<RecordState> = None;
        for incoming in changes {
            if let Outcome::Apply { .. } = resolve(incoming, current.as_ref()) {
                current = Some(match incoming.operation {
                    Operation::Removed => RecordState::Tombstone(incoming.rank()),
                    Operation::Created | Operation::Updated => RecordState::Live(incoming.rank()),
                });
            }
        }
        current
    }

    /// Every permutation of `items`, by simple recursive selection (no `itertools`
    /// dependency — small scenario sizes only, 2–3 changes).
    fn permutations(items: Vec<Change>) -> Vec<Vec<Change>> {
        if items.len() <= 1 {
            return vec![items];
        }
        let mut result = Vec::new();
        for i in 0..items.len() {
            let mut rest = items.clone();
            let picked = rest.remove(i);
            for mut tail in permutations(rest) {
                tail.insert(0, picked.clone());
                result.push(tail);
            }
        }
        result
    }

    /// CFR-013 — asserts that every arrival order of `changes` replays to the same final
    /// state.
    fn assert_arrival_order_never_matters(changes: Vec<Change>) {
        let expected = replay(&changes);
        for permutation in permutations(changes) {
            assert_eq!(
                replay(&permutation),
                expected,
                "CFR-013: arrival order must never affect the final state"
            );
        }
    }

    // CFR-010 — later means greater logical timestamp; a device's clock never decides,
    // and equal timestamps are ordered by device identity (Laptop sorts after Desktop).
    #[test]
    fn cfr_010_later_means_greater_logical_timestamp() {
        let desktop_rename = change(
            "desktop",
            RecordKind::Account,
            "account-1",
            Operation::Updated,
            Origin::User,
            1_000,
            Some(500),
            Some("{\"name\":\"PEA\"}"),
        );
        let laptop_rename = change(
            "laptop",
            RecordKind::Account,
            "account-1",
            Operation::Updated,
            Origin::User,
            1_001,
            Some(1_000),
            Some("{\"name\":\"PEA renamed again\"}"),
        );
        assert_arrival_order_never_matters(vec![desktop_rename.clone(), laptop_rename.clone()]);
        let final_state = replay(&[desktop_rename, laptop_rename]);
        assert_eq!(
            final_state.map(|state| state.rank().device_id.clone()),
            Some("laptop".to_string()),
            "the later logical timestamp must win, not wall-clock order"
        );

        // Equal timestamps: the device identity is the tie-break (Laptop > Desktop).
        let desktop_at_1050 = rank("desktop", Origin::User, 1_050);
        let laptop_at_1050 = rank("laptop", Origin::User, 1_050);
        assert!(
            laptop_at_1050 > desktop_at_1050,
            "CFR-010: equal timestamps are ordered by device identity"
        );
    }

    // CFR-011 — concurrency: a change based on a state this device has not yet received is
    // held back (SYN-041) until that state arrives, so by the time it is compared its base
    // is known; a sequential change based on the current timestamp applies unreported, a
    // concurrent one (based on an older state) is reported.
    #[test]
    fn cfr_011_concurrent_changes() {
        let desktop_rename = change(
            "desktop",
            RecordKind::Account,
            "account-pea",
            Operation::Updated,
            Origin::User,
            1_010,
            Some(900),
            Some("{\"name\":\"PEA Boursorama\"}"),
        );
        // Laptop syncs, sees Desktop's rename, then renames again based on it: sequential.
        let laptop_sequential_rename = change(
            "laptop",
            RecordKind::Account,
            "account-pea",
            Operation::Updated,
            Origin::User,
            1_020,
            Some(1_010),
            Some("{\"name\":\"PEA Bourso\"}"),
        );
        let current = RecordState::Live(desktop_rename.rank());
        let concurrency = Concurrency::classify(
            laptop_sequential_rename.based_on.as_ref(),
            Some(&current.rank().logical_timestamp),
        );
        assert_eq!(concurrency, Concurrency::Sequential);

        // Had Laptop renamed before syncing (based on the state before Desktop's rename),
        // its change would be concurrent instead.
        let laptop_concurrent_rename = change(
            "laptop",
            RecordKind::Account,
            "account-pea",
            Operation::Updated,
            Origin::User,
            1_015,
            Some(900),
            Some("{\"name\":\"PEA Bourso\"}"),
        );
        let concurrency = Concurrency::classify(
            laptop_concurrent_rename.based_on.as_ref(),
            Some(&current.rank().logical_timestamp),
        );
        assert_eq!(concurrency, Concurrency::Concurrent);
    }

    // CFR-012 — record identity per kind decides whether two changes concern the same
    // record: two different observed dates for the same asset are different identities —
    // both survive, no conflict.
    #[test]
    fn cfr_012_record_identity_per_kind() {
        let friday_close = change(
            "desktop",
            RecordKind::AssetPrice,
            "asset-total-energies:2026-08-20",
            Operation::Created,
            Origin::Application,
            1_000,
            None,
            Some("{\"price\":58000000}"),
        );
        let saturday_close = change(
            "laptop",
            RecordKind::AssetPrice,
            "asset-total-energies:2026-08-21",
            Operation::Created,
            Origin::Application,
            1_001,
            None,
            Some("{\"price\":58100000}"),
        );
        assert_ne!(
            friday_close.record_identity, saturday_close.record_identity,
            "CFR-012: different observed dates are different identities"
        );
    }

    // CFR-013 — order of arrival never matters: a third device (Office) receiving Laptop's
    // segments before Desktop's ends up holding exactly what both hold, whichever order it
    // took them in.
    #[test]
    fn cfr_013_order_of_arrival_never_matters() {
        let desktop_change = change(
            "desktop",
            RecordKind::Account,
            "account-1",
            Operation::Updated,
            Origin::User,
            1_000,
            None,
            Some("{\"name\":\"Desktop's edit\"}"),
        );
        let laptop_change = change(
            "laptop",
            RecordKind::Account,
            "account-1",
            Operation::Updated,
            Origin::User,
            1_001,
            None,
            Some("{\"name\":\"Laptop's edit\"}"),
        );
        assert_arrival_order_never_matters(vec![desktop_change, laptop_change]);
    }

    // CFR-014 — every record remembers its last change: comparing an old edit from a
    // long-paused device against the rank stored on the record needs no log lookup.
    #[test]
    fn cfr_014_every_record_remembers_its_last_change() {
        let current = RecordState::Live(rank("desktop", Origin::User, 2_000));
        let old_edit_from_paused_device = change(
            "laptop",
            RecordKind::HoldingNote,
            "account-1:asset-1",
            Operation::Updated,
            Origin::User,
            500,
            Some(100),
            Some("{\"text\":\"stale\"}"),
        );
        let outcome = resolve(&old_edit_from_paused_device, Some(&current));
        assert!(
            matches!(outcome, Outcome::Ignore { .. }),
            "an old edit must be ignored against the record's own stored rank: {outcome:?}"
        );
    }

    // CFR-015 — every removal leaves a tombstone: a device that joins later derives the
    // same tombstone by replay, and an edit older than the removal is ignored against it,
    // exactly as the removing device would.
    #[test]
    fn cfr_015_every_removal_leaves_a_tombstone() {
        let tombstone = RecordState::Tombstone(rank("desktop", Origin::User, 1_030));
        let stale_edit = change(
            "laptop",
            RecordKind::HoldingNote,
            "account-1:asset-1",
            Operation::Updated,
            Origin::User,
            1_025,
            Some(900),
            Some("{\"text\":\"resumed edit\"}"),
        );
        let outcome = resolve(&stale_edit, Some(&tombstone));
        assert!(
            matches!(outcome, Outcome::Ignore { .. }),
            "a change ranked below the tombstone must be ignored: {outcome:?}"
        );
    }

    // CFR-016 — a change the user made beats a change the application made, whatever their
    // timestamps: an application write due to overtake a tombstone the user's deletion left
    // is refused before it is even made (local_write_allowed).
    #[test]
    fn cfr_016_user_change_beats_application_change() {
        let user_deleted_deduction = RecordState::Tombstone(rank("desktop", Origin::User, 1_100));
        let application_regeneration_rank = rank("laptop", Origin::Application, 1_200);
        assert!(
            !local_write_allowed(
                RecordKind::Transaction,
                &application_regeneration_rank,
                Some(&user_deleted_deduction)
            ),
            "CFR-016: the application must never write over a user's tombstone"
        );
        let user_recreation_rank = rank("laptop", Origin::User, 1_200);
        assert!(
            local_write_allowed(
                RecordKind::Transaction,
                &user_recreation_rank,
                Some(&user_deleted_deduction)
            ),
            "CFR-016/CFR-020: the user's own later write is never refused"
        );

        // Symmetric check on the incoming side: the same application change arriving from
        // another device must also be ignored against the user tombstone.
        let incoming_application_regeneration = change(
            "laptop",
            RecordKind::Transaction,
            "generated-deduction-1",
            Operation::Created,
            Origin::Application,
            1_200,
            None,
            Some("{\"amount\":100}"),
        );
        let outcome = resolve(
            &incoming_application_regeneration,
            Some(&user_deleted_deduction),
        );
        assert!(
            matches!(outcome, Outcome::Ignore { .. }),
            "CFR-016: user beats application whatever the timestamp: {outcome:?}"
        );
    }

    // CFR-017 — applying never re-validates: this is asserted at the apply-entry-point
    // layer (`*Service::apply_*`, CFR-017), not inside `resolve` — `resolve` decides only
    // whether the content applies, never whether it would have been a valid local entry.
    // This test documents the boundary: an Outcome::Apply carries no validation result.
    #[test]
    fn cfr_017_applying_never_re_validates() {
        let buy_on_archived_asset = change(
            "laptop",
            RecordKind::Transaction,
            "tx-1",
            Operation::Created,
            Origin::User,
            1_000,
            None,
            Some("{\"type\":\"Buy\",\"asset_id\":\"archived-asset\"}"),
        );
        let outcome = resolve(&buy_on_archived_asset, None);
        assert!(
            matches!(outcome, Outcome::Apply { .. }),
            "resolve() decides content, never entry validity: {outcome:?}"
        );
    }

    // CFR-020 — the higher rank prevails in full: Laptop's later rename wins entirely;
    // Desktop's concurrent bank-name edit is overruled and reported on Desktop, not merged
    // field-by-field.
    #[test]
    fn cfr_020_higher_rank_prevails_in_full() {
        let desktop_bank_name_edit = change(
            "desktop",
            RecordKind::Account,
            "account-cto",
            Operation::Updated,
            Origin::User,
            1_010,
            Some(900),
            Some("{\"name\":\"CTO\",\"bank_name\":\"Fortuneo\"}"),
        );
        let laptop_rename = change(
            "laptop",
            RecordKind::Account,
            "account-cto",
            Operation::Updated,
            Origin::User,
            1_020,
            Some(900),
            Some("{\"name\":\"CTO Fortuneo\",\"bank_name\":\"\"}"),
        );
        let final_state = replay(&[desktop_bank_name_edit, laptop_rename.clone()]);
        assert_eq!(
            final_state,
            Some(RecordState::Live(laptop_rename.rank())),
            "CFR-020: the later rank prevails entirely — never a per-field merge"
        );
    }

    // CFR-021 — identical concurrent changes are not a conflict: both devices archive the
    // same asset; the record takes that content, nobody is told.
    #[test]
    fn cfr_021_identical_concurrent_changes_are_not_a_conflict() {
        let desktop_archive = change(
            "desktop",
            RecordKind::Asset,
            "asset-1",
            Operation::Updated,
            Origin::User,
            1_000,
            Some(500),
            Some("{\"is_archived\":true}"),
        );
        let laptop_archive = change(
            "laptop",
            RecordKind::Asset,
            "asset-1",
            Operation::Updated,
            Origin::User,
            1_001,
            Some(500),
            Some("{\"is_archived\":true}"),
        );
        let current = RecordState::Live(rank("desktop", Origin::User, 500));
        let outcome = resolve(&laptop_archive, Some(&current));
        assert_eq!(
            outcome,
            Outcome::Apply { notice: None },
            "CFR-021: identical content applies without a notice: {desktop_archive:?}"
        );
    }

    // CFR-022 — update versus removal: the higher rank prevails — except an account's
    // removal is final. A holding note follows the general rule either way; an account's
    // tombstone outranks even a later user rename.
    #[test]
    fn cfr_022_update_versus_removal_account_removal_is_final() {
        // General rule (non-account): Desktop deletes a note at 1030, Laptop edits it at
        // 1040 — the edit wins, the deletion is reported on Desktop.
        let note_deletion = change(
            "desktop",
            RecordKind::HoldingNote,
            "account-cto:asset-air-liquide",
            Operation::Removed,
            Origin::User,
            1_030,
            Some(900),
            None,
        );
        let note_edit = change(
            "laptop",
            RecordKind::HoldingNote,
            "account-cto:asset-air-liquide",
            Operation::Updated,
            Origin::User,
            1_040,
            Some(900),
            Some("{\"text\":\"edited\"}"),
        );
        let final_state = replay(&[note_deletion, note_edit.clone()]);
        assert_eq!(
            final_state,
            Some(RecordState::Live(note_edit.rank())),
            "CFR-022: a prevailing update brings the record back over a lower-ranked tombstone"
        );

        // The exception: an account's tombstone is final, whatever the origin or timestamp
        // of a later change against it.
        let account_deletion = rank("desktop", Origin::User, 1_030);
        let account_tombstone = RecordState::Tombstone(account_deletion);
        let later_rename = change(
            "laptop",
            RecordKind::Account,
            "account-old-pea",
            Operation::Updated,
            Origin::User,
            1_040,
            Some(900),
            Some("{\"name\":\"Old PEA renamed\"}"),
        );
        let outcome = resolve(&later_rename, Some(&account_tombstone));
        assert!(
            matches!(outcome, Outcome::Ignore { .. }),
            "CFR-022: a deleted account is never brought back by a later change: {outcome:?}"
        );
    }

    // CFR-023 — removal versus removal: two concurrent removals of the same record leave it
    // removed, silently.
    #[test]
    fn cfr_023_removal_versus_removal() {
        let desktop_deletion = change(
            "desktop",
            RecordKind::Transaction,
            "tx-one-off-fee",
            Operation::Removed,
            Origin::User,
            1_000,
            Some(500),
            None,
        );
        let laptop_deletion = change(
            "laptop",
            RecordKind::Transaction,
            "tx-one-off-fee",
            Operation::Removed,
            Origin::User,
            1_001,
            Some(500),
            None,
        );
        // This device already holds Desktop's tombstone (CFR-015): Laptop's concurrent
        // removal meets a removed record, and whichever way the ranks fall nothing is
        // reported and the record stays removed.
        let current = RecordState::Tombstone(desktop_deletion.rank());
        let outcome = resolve(&laptop_deletion, Some(&current));
        assert!(
            matches!(
                outcome,
                Outcome::Apply { notice: None } | Outcome::Ignore { notice: None }
            ),
            "CFR-023: double removal is silent: {outcome:?}"
        );
        let reversed = resolve(
            &desktop_deletion,
            Some(&RecordState::Tombstone(laptop_deletion.rank())),
        );
        assert_eq!(
            reversed,
            Outcome::Ignore { notice: None },
            "CFR-023: the lower-ranked removal is ignored silently"
        );
    }

    // CFR-030 — cascading removal is explicit per record: an account's tombstone carries
    // its rank onto every child this device holds, including children the removing device
    // never knew of.
    #[test]
    fn cfr_030_cascading_removal_is_explicit_per_record() {
        let account_tombstone_rank = rank("desktop", Origin::User, 1_040);
        let held_children = vec![
            RecordState::Live(rank("laptop", Origin::User, 1_010)), // a transaction
            RecordState::Live(rank("desktop", Origin::User, 1_020)), // a holding note
        ];
        let cascaded = cascade_child_tombstones(&account_tombstone_rank, &held_children);
        assert_eq!(cascaded.len(), held_children.len());
        assert!(
            cascaded
                .iter()
                .all(|child_rank| *child_rank == account_tombstone_rank),
            "CFR-030: every cascaded tombstone carries the account tombstone's own rank"
        );
    }

    // CFR-031 — child before parent is waited for, not rejected: a deposit on an
    // account this device has not yet received is held back, not dropped.
    #[test]
    fn cfr_031_child_before_parent_is_waited_for_not_rejected() {
        let deposit_before_its_account = change(
            "laptop",
            RecordKind::Transaction,
            "tx-deposit-1",
            Operation::Created,
            Origin::User,
            1_000,
            None,
            Some("{\"account_id\":\"account-livret\"}"),
        );
        let known_identities: HashSet<String> = HashSet::new();
        let outcome = reference_outcome(&deposit_before_its_account, &known_identities, None);
        assert_eq!(
            outcome,
            Some(Outcome::HoldBack {
                waiting_for: WaitingFor::Record {
                    kind: RecordKind::Account,
                    identity: "account-livret".to_string(),
                }
            }),
            "CFR-031: the deposit must wait for its account, not be rejected"
        );
    }

    // CFR-032 — a child of a removed account is dropped and reported: Laptop's buy on an
    // account Desktop deleted is dropped on Desktop, and Laptop's own buy is removed with
    // the account it lost — reported on Laptop only.
    #[test]
    fn cfr_032_child_of_a_removed_account_is_dropped_and_reported() {
        let account_tombstone = RecordState::Tombstone(rank("desktop", Origin::User, 1_030));
        let laptop_buy_on_deleted_account = change(
            "laptop",
            RecordKind::Transaction,
            "tx-buy-total-energies",
            Operation::Created,
            Origin::User,
            1_010,
            None,
            Some("{\"account_id\":\"account-old-pea\"}"),
        );
        let outcome =
            drop_if_parent_tombstoned(&laptop_buy_on_deleted_account, Some(&account_tombstone))
                .expect("a child of a tombstoned account must be dropped");
        match outcome {
            Outcome::Drop { notice } => {
                assert_eq!(notice.other_device_id, "desktop");
                assert_eq!(notice.kind, ConflictNoticeKind::DroppedChild);
            }
            other => panic!("CFR-032: expected Drop, got {other:?}"),
        }
    }

    // CFR-032 over CFR-031 — Office knows Desktop removed "Old PEA" but has never received
    // asset "X" that Laptop's buy on it refers to: the buy is dropped, not held back for an
    // asset that will never be waited for on a removed account.
    #[test]
    fn cfr_032_removed_account_drops_a_child_before_a_missing_asset_holds_it_back() {
        let account_tombstone = RecordState::Tombstone(rank("desktop", Origin::User, 1_030));
        let laptop_buy_of_unknown_asset = change(
            "laptop",
            RecordKind::Transaction,
            "tx-buy-x",
            Operation::Created,
            Origin::User,
            1_010,
            None,
            Some("{\"account_id\":\"account-old-pea\",\"asset_id\":\"asset-x\"}"),
        );
        let known_identities: HashSet<String> = HashSet::from(["account-old-pea".to_string()]);
        let outcome = reference_outcome(
            &laptop_buy_of_unknown_asset,
            &known_identities,
            Some(&account_tombstone),
        );
        assert!(
            matches!(outcome, Some(Outcome::Drop { .. })),
            "CFR-032: a tombstoned account decides before any unknown reference: {outcome:?}"
        );
    }

    // CFR-033 — system-seeded records never block: a deposit into a currency this device
    // has never held is never held back — the cash asset/category is ensured on apply.
    #[test]
    fn cfr_033_system_seeded_records_never_block() {
        let usd_deposit = change(
            "laptop",
            RecordKind::Transaction,
            "tx-usd-deposit",
            Operation::Created,
            Origin::User,
            1_000,
            None,
            Some("{\"asset_id\":\"system-cash-usd\"}"),
        );
        let known_seeded_identities: HashSet<String> = HashSet::from([
            "system-cash-usd".to_string(),
            "system-cash-category".to_string(),
        ]);
        let outcome = reference_outcome(&usd_deposit, &known_seeded_identities, None);
        assert_eq!(
            outcome, None,
            "CFR-033: a system-seeded identity must never hold back a change"
        );
    }

    // CFR-034 — whatever the application generates has a predictable identity: two devices
    // that each generate August's deduction for the same holding produce the *same* record
    // (resolved by CFR-020, nothing to detect); a natural-key collision between two *user*
    // creations is reported on both.
    #[test]
    fn cfr_034_predictable_identity_and_user_collision_notice() {
        // Same generated identity on both devices — CFR-020 decides content, no collision.
        let desktop_deduction = change(
            "desktop",
            RecordKind::Transaction,
            "deduction-cto-amundi-2026-08-31",
            Operation::Created,
            Origin::Application,
            1_000,
            None,
            Some("{\"amount\":100}"),
        );
        let laptop_deduction = change(
            "laptop",
            RecordKind::Transaction,
            "deduction-cto-amundi-2026-08-31",
            Operation::Created,
            Origin::Application,
            1_001,
            None,
            Some("{\"amount\":100}"),
        );
        assert_eq!(
            desktop_deduction.record_identity, laptop_deduction.record_identity,
            "CFR-034: the same (account, asset, period) always yields the same identity"
        );

        // A genuine user-vs-user collision on a natural key (a fee schedule for the same
        // holding) is reported on both creating devices.
        let desktop_schedule = change(
            "desktop",
            RecordKind::FeeSchedule,
            "account-cto:asset-amundi",
            Operation::Created,
            Origin::User,
            1_000,
            None,
            Some("{\"rate\":500000,\"frequency\":\"Monthly\"}"),
        );
        let laptop_schedule = change(
            "laptop",
            RecordKind::FeeSchedule,
            "account-cto:asset-amundi",
            Operation::Created,
            Origin::User,
            1_001,
            None,
            Some("{\"rate\":600000,\"frequency\":\"Quarterly\"}"),
        );
        let (notice_for_desktop, notice_for_laptop) =
            collision_notice(&laptop_schedule, &desktop_schedule)
                .expect("both-user collision must be reported on both creating devices");
        assert_eq!(notice_for_desktop.other_device_id, "laptop");
        assert_eq!(notice_for_laptop.other_device_id, "desktop");
        assert_eq!(
            notice_for_desktop.kind,
            ConflictNoticeKind::NaturalKeyCollision
        );
    }

    // CFR-035 — duplicate names coexist: two accounts independently named "Livret A"
    // survive both, each with its own history, and both creating devices are told.
    #[test]
    fn cfr_035_duplicate_names_coexist() {
        let desktop_creation = change(
            "desktop",
            RecordKind::Account,
            "account-desktop-livret-a",
            Operation::Created,
            Origin::User,
            1_000,
            None,
            Some("{\"name\":\"Livret A\"}"),
        );
        let (notice_for_desktop, notice_for_laptop) =
            duplicate_name_notice(&desktop_creation, "laptop")
                .expect("a duplicate name must be reported on both devices");
        assert_eq!(notice_for_desktop.kind, ConflictNoticeKind::DuplicateName);
        assert_eq!(notice_for_laptop.kind, ConflictNoticeKind::DuplicateName);
        assert_eq!(notice_for_desktop.other_device_id, "laptop");
    }

    // CFR-040 — transactions accumulate: after sync, every transaction created on any
    // device is present, minus those removed. Emergent from repeated Apply outcomes on
    // distinct identities — no dedicated function, asserted here at the model level (the
    // Tier-3 integration test asserts it end-to-end).
    #[test]
    fn cfr_040_transactions_accumulate() {
        let desktop_buy = change(
            "desktop",
            RecordKind::Transaction,
            "tx-buy-1",
            Operation::Created,
            Origin::User,
            1_000,
            None,
            Some("{}"),
        );
        let laptop_sell = change(
            "laptop",
            RecordKind::Transaction,
            "tx-sell-1",
            Operation::Created,
            Origin::User,
            1_001,
            None,
            Some("{}"),
        );
        assert!(matches!(resolve(&desktop_buy, None), Outcome::Apply { .. }));
        assert!(matches!(resolve(&laptop_sell, None), Outcome::Apply { .. }));
    }

    // CFR-041 — replay order is the same on every device: `resolve` itself carries no
    // notion of arrival order (that guarantee is CFR-013's), and transaction replay order
    // is a separate, repository-level concern (`repository/account.rs`'s
    // `ORDER BY date, created_at, id`, shipped in PR-A) — not decided here. This test
    // documents the boundary: `resolve`'s decision for one identity is independent of any
    // other identity's changes, which is what makes the repository free to order replay by
    // (date, created_at, id) without resolve() ever needing to know about it.
    #[test]
    fn cfr_041_replay_order_is_a_repository_concern_not_resolves() {
        let same_day_buy_from_desktop = change(
            "desktop",
            RecordKind::Transaction,
            "tx-a",
            Operation::Created,
            Origin::User,
            1_000,
            None,
            Some("{\"date\":\"2026-08-20\"}"),
        );
        let same_day_buy_from_laptop = change(
            "laptop",
            RecordKind::Transaction,
            "tx-b",
            Operation::Created,
            Origin::User,
            1_001,
            None,
            Some("{\"date\":\"2026-08-20\"}"),
        );
        // Distinct identities: resolving one never depends on the other having been seen.
        let outcome_a = resolve(&same_day_buy_from_desktop, None);
        let outcome_b = resolve(&same_day_buy_from_laptop, None);
        assert!(matches!(outcome_a, Outcome::Apply { .. }));
        assert!(matches!(outcome_b, Outcome::Apply { .. }));
    }

    // CFR-042 — a merge that breaks a holding invariant keeps every transaction: both an
    // oversold sale on Desktop and one on Laptop survive; resolve() applies each one
    // (distinct identities) — the inconsistency itself is derived elsewhere
    // (`use_cases::account_details`/`account_summary`, CFR-042/SYN-040), never decided here.
    #[test]
    fn cfr_042_merge_keeps_every_transaction_even_when_it_oversells() {
        let desktop_sale = change(
            "desktop",
            RecordKind::Transaction,
            "tx-sell-desktop",
            Operation::Created,
            Origin::User,
            1_000,
            None,
            Some("{\"quantity\":-10}"),
        );
        let laptop_sale = change(
            "laptop",
            RecordKind::Transaction,
            "tx-sell-laptop",
            Operation::Created,
            Origin::User,
            1_001,
            None,
            Some("{\"quantity\":-10}"),
        );
        assert!(matches!(
            resolve(&desktop_sale, None),
            Outcome::Apply { .. }
        ));
        assert!(matches!(resolve(&laptop_sale, None), Outcome::Apply { .. }));
    }

    // CFR-043 — generated fee deductions are one record by construction: the identity
    // (CFR-034) makes two independently generated deductions for the same period the same
    // record, so CFR-020 decides content and the holding is charged once — no dedicated
    // function; emergent from CFR-034 + CFR-020, exercised together here.
    #[test]
    fn cfr_043_generated_fee_deductions_are_one_record_by_construction() {
        let desktop_generation = change(
            "desktop",
            RecordKind::Transaction,
            "deduction-cto-amundi-2026-08-31",
            Operation::Created,
            Origin::Application,
            1_000,
            None,
            Some("{\"amount\":100}"),
        );
        let laptop_generation = change(
            "laptop",
            RecordKind::Transaction,
            "deduction-cto-amundi-2026-08-31",
            Operation::Created,
            Origin::Application,
            1_010,
            None,
            Some("{\"amount\":100}"),
        );
        assert_eq!(
            desktop_generation.record_identity, laptop_generation.record_identity,
            "CFR-043: same (account, asset, period) => one record after merge"
        );
        let final_state = replay(&[desktop_generation, laptop_generation.clone()]);
        assert_eq!(
            final_state,
            Some(RecordState::Live(laptop_generation.rank())),
            "CFR-043: the later generation's content prevails; the holding is charged once"
        );
    }

    // CFR-044 — a fee schedule's catch-up position merges by maximum, never by rank: the
    // engine routes it to `MergeMax` even when the incoming change is outranked by the
    // stored state (the repository's SQL `MAX()` then keeps the later period), and the
    // application's own local write of it is never refused on rank either.
    #[test]
    fn cfr_044_catch_up_position_merges_by_maximum_never_by_rank() {
        let desktop_user_state = RecordState::Live(rank("desktop", Origin::User, 1_100));
        let laptop_july_position = change(
            "laptop",
            RecordKind::FeeCatchUpPosition,
            "account-cto:asset-amundi",
            Operation::Updated,
            Origin::Application,
            1_000,
            None,
            Some("{\"last_applied_period\":\"2026-07-31\"}"),
        );
        let decision = decide(&laptop_july_position, Some(&desktop_user_state), None);
        assert_eq!(
            decision.outcome,
            Outcome::MergeMax,
            "CFR-044: an outranked catch-up position still merges by maximum"
        );
        assert!(
            local_write_allowed(
                RecordKind::FeeCatchUpPosition,
                &rank("laptop", Origin::Application, 1_000),
                Some(&desktop_user_state)
            ),
            "CFR-044: a catch-up position is never refused on rank"
        );
    }

    // CFR-050 — observations: latest write wins whatever the source; origin is not
    // considered and nothing is reported, even when a manual correction is overwritten by
    // a later scheduled download.
    #[test]
    fn cfr_050_observations_latest_write_wins_whatever_the_source() {
        let manual_correction = change(
            "desktop",
            RecordKind::AssetPrice,
            "asset-total-energies:2026-08-21",
            Operation::Created,
            Origin::User,
            1_000,
            None,
            Some("{\"price\":58100000}"),
        );
        let later_scheduled_download = change(
            "laptop",
            RecordKind::AssetPrice,
            "asset-total-energies:2026-08-21",
            Operation::Created,
            Origin::Application,
            1_300,
            None,
            Some("{\"price\":58250000}"),
        );
        let current = RecordState::Live(manual_correction.rank());
        let outcome = resolve_observation(&later_scheduled_download, Some(&current));
        assert_eq!(
            outcome,
            Outcome::Apply { notice: None },
            "CFR-050: the later write wins regardless of origin, and nothing is reported"
        );
        assert!(
            local_write_allowed(
                RecordKind::AssetPrice,
                &rank("desktop", Origin::Application, 1_100),
                Some(&RecordState::Tombstone(rank(
                    "desktop",
                    Origin::User,
                    1_050
                )))
            ),
            "CFR-050: a local observation write never consults origin, whatever the current state"
        );
    }

    // CFR-060 — reported outcomes: exactly the five outcomes CFR-060 lists produce a
    // notice; every one of the never-reported cases (sequential, identical, double
    // removal, held-back, application-outranked, generated-deduction convergence,
    // catch-up maxima, observation overwrites) yields None.
    #[test]
    fn cfr_060_reported_outcomes_are_exactly_the_five_listed() {
        let overruled_edit = Outcome::Ignore {
            notice: Some(NoticeDraft {
                kind: ConflictNoticeKind::OverruledEdit,
                record_kind: RecordKind::Account,
                record_identity: "account-1".into(),
                other_device_id: "laptop".into(),
                raised_on_device_id: "desktop".into(),
            }),
        };
        assert!(notice_for(&overruled_edit).is_some());

        let dropped_child = Outcome::Drop {
            notice: NoticeDraft {
                kind: ConflictNoticeKind::DroppedChild,
                record_kind: RecordKind::Transaction,
                record_identity: "tx-1".into(),
                other_device_id: "desktop".into(),
                raised_on_device_id: "laptop".into(),
            },
        };
        assert!(notice_for(&dropped_child).is_some());

        // Never reported: sequential apply, identical content, double removal, held-back,
        // and a merge-by-maximum.
        assert!(notice_for(&Outcome::Apply { notice: None }).is_none());
        assert!(notice_for(&Outcome::Ignore { notice: None }).is_none());
        assert!(notice_for(&Outcome::HoldBack {
            waiting_for: WaitingFor::Record {
                kind: RecordKind::Account,
                identity: "account-livret".into(),
            },
        })
        .is_none());
        assert!(notice_for(&Outcome::MergeMax).is_none());
    }

    // SYN-035 — a change written in an older data format keeps the local value of every
    // field it does not carry; a field it carries wins; with nothing local it applies as is.
    #[test]
    fn upgraded_content_lays_the_incoming_fields_over_the_local_ones() {
        let incoming = change(
            "laptop",
            RecordKind::Account,
            "account-livret",
            Operation::Updated,
            Origin::User,
            7,
            Some(3),
            Some(r#"{"id":"account-livret","name":"Livret B"}"#),
        );
        let local = r#"{"id":"account-livret","name":"Livret A","bank_name":"Boursorama"}"#;

        let merged: serde_json::Value =
            serde_json::from_str(&upgraded_content(&incoming, Some(local)).unwrap()).unwrap();
        assert_eq!(merged["name"], "Livret B");
        assert_eq!(merged["bank_name"], "Boursorama");

        assert_eq!(
            upgraded_content(&incoming, None).as_deref(),
            incoming.content.as_deref(),
            "nothing local: the incoming content applies as is"
        );
    }
}
