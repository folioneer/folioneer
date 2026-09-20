//! Shared kernel — cross-BC domain vocabulary (B1, ddd-reference.md § Shared Kernel).

/// The multi-device sync record-change vocabulary (`RecordKind`, `Origin`, `Operation`,
/// `LogicalTimestamp`, `RecordIdentity`, `Rank`, `ChangeDraft`) — SYN-021, CFR-010/012/014/016/020.
pub mod record_change;

pub use record_change::*;
