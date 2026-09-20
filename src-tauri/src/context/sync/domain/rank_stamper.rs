//! The `RankStamper` port (CFR-014, D6): stamps every synced row that has never been ranked
//! with the first segment's rank, on the enrolment transaction's connection (SYN-013). The use
//! case implements it over the owning bounded contexts' services (ADR-004), the way
//! `PortfolioSnapshot` reads them — each context stamps its own tables.

use sqlx::SqliteConnection;

use crate::context::sync::error::SyncError;
use crate::shared::domain::Rank;

/// Ranks the rows that existed before sync did.
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait RankStamper: Send + Sync {
    /// Stamps `rank` on every synced row whose rank columns are still NULL; returns how many
    /// rows were stamped.
    async fn stamp_unranked_rows(
        &self,
        conn: &mut SqliteConnection,
        rank: &Rank,
    ) -> Result<u64, SyncError>;
}
