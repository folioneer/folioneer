//! The `PortfolioSnapshot` port (SYN-013/026): every synced record this installation
//! currently holds, in the shape a `Created` change carries. The use case implements it
//! over the owning bounded contexts' services (ADR-004), so the first segment's content is
//! serialized exactly as the repositories' change capture serializes it.

use crate::context::sync::error::SyncError;
use crate::shared::domain::{RecordIdentity, RecordKind};

/// One existing synced record, ready to become a `Created` change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioRecord {
    /// What kind of record this is (SYN-021).
    pub record_kind: RecordKind,
    /// Its cross-device identity (CFR-012).
    pub record_identity: RecordIdentity,
    /// Its full state, JSON-encoded.
    pub content: String,
}

/// Reads the whole current portfolio as synced records (SYN-013), system-seeded records
/// excluded (SYN-027).
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait PortfolioSnapshot: Send + Sync {
    /// Every synced record this installation holds, parents before children.
    async fn records(&self) -> Result<Vec<PortfolioRecord>, SyncError>;
}
