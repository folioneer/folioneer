//! `ConflictNotice` (SYN-066): a persisted notice of an outcome CFR-060 lists as reportable.
//! PR-B only persists/dismisses the shape; nothing raises a notice until PR-C's resolution
//! engine exists.

use crate::shared::domain::RecordKind;
use serde::{Deserialize, Serialize};
use specta::Type;

/// Exactly CFR-060's reportable outcomes.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Type,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum ConflictNoticeKind {
    /// A losing edit was overruled by a higher-ranked one.
    OverruledEdit,
    /// A losing removal was overruled by a higher-ranked edit.
    OverruledRemoval,
    /// A child record was dropped because its parent was removed.
    DroppedChild,
    /// Two independently created records collided on the same natural key.
    NaturalKeyCollision,
    /// Two records ended up sharing the same display name.
    DuplicateName,
}

/// One persisted, dismissible notice (SYN-066, CFR-060).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ConflictNotice {
    /// Identifies the notice, so it can be dismissed individually.
    pub notice_id: String,
    /// Which reportable outcome raised it.
    pub kind: ConflictNoticeKind,
    /// The kind of the record concerned.
    pub record_kind: RecordKind,
    /// The record's canonical identity (CFR-012).
    pub record_identity: String,
    /// A human-readable label, captured when raised.
    pub record_label: String,
    /// The device whose change prevailed or removed the parent.
    pub other_device_id: String,
    /// That device's name at the time the notice was raised.
    pub other_device_name: String,
    /// When the notice was raised.
    pub raised_at: String,
}
