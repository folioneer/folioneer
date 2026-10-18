use crate::shared::domain::{Rank, RecordKind, SyncedRecord};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;

/// GPF-011 — the setting naming the currency the portfolio is reported in.
pub const REFERENCE_CURRENCY_SETTING: &str = "reference_currency";

/// GPF-011 — the reference currency of a portfolio that never chose one.
pub const DEFAULT_REFERENCE_CURRENCY: &str = "EUR";

/// A portfolio-wide setting (SYN-021): the same on every device, synced, one per name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortfolioSetting {
    /// The setting's name, its identity (CFR-012).
    pub name: String,
    /// Its value, as text.
    pub value: String,
}

/// Persistence of the portfolio settings, with their sync entry points (CFR-014, CFR-017).
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait PortfolioSettingRepository: Send + Sync {
    /// The value of the setting `name`, when one is stored.
    async fn get(&self, name: &str) -> Result<Option<String>>;

    /// Stores `setting`, recording the change the user made (SYN-020); a value equal to the
    /// stored one records nothing.
    async fn set(&self, setting: &PortfolioSetting) -> Result<()>;

    /// Stamps `rank` on every setting never ranked (CFR-014, D6); how many were stamped.
    async fn stamp_sync_rank(&self, conn: &mut SqliteConnection, rank: &Rank) -> Result<u64>;

    /// The synced setting `identity` (its name) with its rank, when held (CFR-014).
    async fn synced_record(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
    ) -> Result<Option<SyncedRecord>>;

    /// Writes an incoming setting verbatim with its rank (CFR-017).
    async fn apply(
        &self,
        conn: &mut SqliteConnection,
        setting: &PortfolioSetting,
        rank: &Rank,
    ) -> Result<()>;

    /// Removes the setting `identity` (its name) on an incoming removal (CFR-017).
    async fn remove_synced(&self, conn: &mut SqliteConnection, identity: &str) -> Result<()>;

    /// Every stored setting, for the first publish's snapshot (SYN-013).
    async fn list(&self) -> Result<Vec<PortfolioSetting>>;
}
