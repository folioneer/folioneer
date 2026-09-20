//! `HeldBackChange` (SYN-041): a received change waiting for a record this device has not
//! received yet. PR-B only persists the shape; nothing holds a change back until PR-C's
//! apply path exists.

use crate::shared::domain::RecordKind;

/// A change received from another device that cannot be applied yet (SYN-041).
#[derive(Debug, Clone, PartialEq)]
pub struct HeldBackChange {
    /// Identifies this held-back entry.
    pub id: String,
    /// The device that originally recorded the change.
    pub origin_device_id: String,
    /// That device's sequence number for the change.
    pub sequence: i64,
    /// The change exactly as received, JSON-encoded.
    pub payload: String,
    /// The kind of the record it is waiting for.
    pub waiting_kind: RecordKind,
    /// The identity of the record it is waiting for.
    pub waiting_identity: String,
    /// When it was first held back.
    pub held_since: String,
}
