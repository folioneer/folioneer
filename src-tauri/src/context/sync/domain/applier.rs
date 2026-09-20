//! The `ChangeApplier` port (D4, ADR-004): what the apply executor needs from the owning
//! bounded contexts — the state each record currently has on this device, the children an
//! account owns, and the verbatim writes that run no entry guards (CFR-017). The use case
//! implements it over the account, asset, and currency services, the way `PortfolioSnapshot`
//! reads them; every call rides the apply transaction's connection (SYN-065).

use sqlx::SqliteConnection;

use crate::context::sync::domain::resolution::Change;
use crate::context::sync::error::SyncError;
use crate::shared::domain::{Rank, RecordKind, SyncedChild, SyncedRecord};

/// Reads and writes synced records through the owning bounded contexts' services.
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait ChangeApplier: Send + Sync {
    /// The record this device holds live for `identity`, or `None` when it holds none.
    async fn live_record(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
    ) -> Result<Option<SyncedRecord>, SyncError>;

    /// Every child record of `account_id` this device holds (CFR-030): its transactions,
    /// holding notes, fee schedules, and catch-up positions.
    async fn children_of_account(
        &self,
        conn: &mut SqliteConnection,
        account_id: &str,
    ) -> Result<Vec<SyncedChild>, SyncError>;

    /// The rank of another live record of `kind` that carries `name` (CFR-035), or `None`
    /// when no other record does (or the clashing record has never been ranked).
    async fn clashing_name(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
        name: &str,
    ) -> Result<Option<Rank>, SyncError>;

    /// Writes one prevailing change into its bounded context verbatim (CFR-017): a creation
    /// or update stamps `change`'s rank on the row (CFR-014); a removal removes the record
    /// and, for an account, every child it owns (CFR-030). Seeds the cash asset a change
    /// refers to first (SYN-027/CFR-033).
    async fn write(&self, conn: &mut SqliteConnection, change: &Change) -> Result<(), SyncError>;

    /// SYN-083 — discards every asset price, currency pair, and currency rate this
    /// installation fetched before joining, so the rebuild replaces them.
    async fn discard_observations(&self, conn: &mut SqliteConnection) -> Result<(), SyncError>;
}
