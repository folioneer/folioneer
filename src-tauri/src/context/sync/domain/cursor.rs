//! `SyncCursor` (SYN-033): how far this device has taken in another device's changes.

/// How far this device has applied another device's history (SYN-033/037).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncCursor {
    /// The other device.
    pub device_id: String,
    /// The last of that device's changes this device has taken in — applied, or held back
    /// (SYN-041).
    pub applied_through: i64,
    /// When this cursor last advanced.
    pub last_applied_at: Option<String>,
}
