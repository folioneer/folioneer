use crate::context::account::error::AccountError;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Recurrence frequency for a management fee schedule (FEE-030).
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Type)]
pub enum FeeFrequency {
    /// Deduction applied monthly (12 periods per year).
    Monthly,
    /// Deduction applied quarterly (4 periods per year).
    Quarterly,
    /// Deduction applied annually (1 period per year).
    Annually,
}

impl FeeFrequency {
    /// Number of deduction periods per calendar year (FEE-034).
    pub fn periods_per_year(self) -> i64 {
        match self {
            FeeFrequency::Monthly => 12,
            FeeFrequency::Quarterly => 4,
            FeeFrequency::Annually => 1,
        }
    }
}

impl std::fmt::Display for FeeFrequency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            FeeFrequency::Monthly => "Monthly",
            FeeFrequency::Quarterly => "Quarterly",
            FeeFrequency::Annually => "Annually",
        })
    }
}

impl std::str::FromStr for FeeFrequency {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "Monthly" => Ok(FeeFrequency::Monthly),
            "Quarterly" => Ok(FeeFrequency::Quarterly),
            "Annually" => Ok(FeeFrequency::Annually),
            _ => Err(()),
        }
    }
}

/// A recurring management fee schedule for an (account, asset) pair (FEE-030).
///
/// `annual_rate_percent_micros` is in micro-percent: 1% = 1_000_000,
/// 100% = 100_000_000. Must be strictly positive and ≤ 100_000_000.
#[derive(Debug, Serialize, Deserialize, Clone, Type)]
pub struct FeeSchedule {
    /// Unique identifier.
    pub id: String,
    /// The account this schedule applies to.
    pub account_id: String,
    /// The asset being charged the management fee.
    pub asset_id: String,
    /// Annual management fee rate in micro-percent (1% = 1_000_000, FEE-032).
    pub annual_rate_percent_micros: i64,
    /// How often the deduction is applied within a year.
    pub frequency: FeeFrequency,
    /// ISO date when the schedule becomes effective (YYYY-MM-DD).
    pub start_date: String,
    /// Optional ISO date when the schedule ends (YYYY-MM-DD). None = open-ended.
    pub end_date: Option<String>,
    /// Whether the schedule is currently active (FEE-061).
    pub active: bool,
    /// The last completed period boundary that was applied, as ISO date (FEE-043).
    /// None when no periods have been applied yet. A derived read of the schedule's
    /// `FeeCatchUpPosition` (CFR-044); never written through the schedule itself.
    pub last_applied_period: Option<String>,
}

impl FeeSchedule {
    /// Creates a new FeeSchedule with a generated ID.
    ///
    /// FEE-032 — validates: rate > 0 (`RateNotPositive`),
    /// rate ≤ 100_000_000 (`RateAboveHundred`), end_date > start_date (`EndBeforeStart`).
    pub fn new(
        account_id: String,
        asset_id: String,
        annual_rate_percent_micros: i64,
        frequency: FeeFrequency,
        start_date: String,
        end_date: Option<String>,
    ) -> Result<Self, AccountError> {
        if annual_rate_percent_micros <= 0 {
            return Err(AccountError::RateNotPositive);
        }
        if annual_rate_percent_micros > 100_000_000 {
            return Err(AccountError::RateAboveHundred);
        }
        if let Some(ref end) = end_date {
            if end.as_str() <= start_date.as_str() {
                return Err(AccountError::EndBeforeStart);
            }
        }
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            account_id,
            asset_id,
            annual_rate_percent_micros,
            frequency,
            start_date,
            end_date,
            active: true,
            last_applied_period: None,
        })
    }

    /// Applies an edit to the editable fields (FEE-060/061) and returns the updated
    /// aggregate to persist. `frequency` and `start_date` are immutable after creation.
    ///
    /// FEE-032 — validates: rate > 0 (`RateNotPositive`),
    /// rate ≤ 100_000_000 (`RateAboveHundred`), end_date > start_date (`EndBeforeStart`).
    pub fn update_from(
        mut self,
        annual_rate_percent_micros: i64,
        end_date: Option<String>,
        active: bool,
    ) -> Result<Self, AccountError> {
        if annual_rate_percent_micros <= 0 {
            return Err(AccountError::RateNotPositive);
        }
        if annual_rate_percent_micros > 100_000_000 {
            return Err(AccountError::RateAboveHundred);
        }
        if let Some(ref end) = end_date {
            if end.as_str() <= self.start_date.as_str() {
                return Err(AccountError::EndBeforeStart);
            }
        }
        self.annual_rate_percent_micros = annual_rate_percent_micros;
        self.end_date = end_date;
        self.active = active;
        Ok(self)
    }

    /// Reconstructs a FeeSchedule from storage without validation.
    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: String,
        account_id: String,
        asset_id: String,
        annual_rate_percent_micros: i64,
        frequency: FeeFrequency,
        start_date: String,
        end_date: Option<String>,
        active: bool,
        last_applied_period: Option<String>,
    ) -> Self {
        Self {
            id,
            account_id,
            asset_id,
            annual_rate_percent_micros,
            frequency,
            start_date,
            end_date,
            active,
            last_applied_period,
        }
    }
}

/// Interface for fee schedule persistence.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait FeeScheduleRepository: Send + Sync {
    /// Fetches the fee schedule for a given (account, asset) pair.
    async fn get_by_account_asset(
        &self,
        account_id: &str,
        asset_id: &str,
    ) -> Result<Option<FeeSchedule>>;
    /// Fetches all active fee schedules.
    async fn get_all_active(&self) -> Result<Vec<FeeSchedule>>;
    /// Fetches the active fee schedules of one account.
    async fn get_active_by_account(&self, account_id: &str) -> Result<Vec<FeeSchedule>>;
    /// Fetches every fee schedule of one account, active or not.
    async fn get_by_account(&self, account_id: &str) -> Result<Vec<FeeSchedule>>;
    /// Inserts a new fee schedule.
    async fn insert(&self, schedule: &FeeSchedule) -> Result<()>;
    /// Updates an existing fee schedule in place.
    async fn update(&self, schedule: &FeeSchedule) -> Result<()>;
    /// Deletes a fee schedule by (account_id, asset_id). No-op if not found (FEE-062).
    async fn delete_by_account_asset(&self, account_id: &str, asset_id: &str) -> Result<()>;
}

/// The fee catch-up cursor (FEE-043) as its own synced record (CFR-044, D5): identified by
/// the schedule's `(account_id, asset_id)` — not a field of `FeeSchedule` — so it can merge
/// by maximum between devices independently of the schedule's own rank (CFR-016).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FeeCatchUpPosition {
    /// The account of the schedule this position belongs to.
    pub account_id: String,
    /// The charged asset of the schedule this position belongs to.
    pub asset_id: String,
    /// The boundary date (ISO `YYYY-MM-DD`) of the most recently generated or skipped
    /// period (FEE-043).
    pub last_applied_period: String,
}

/// Interface for fee catch-up position persistence (D5, CFR-044).
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait FeeCatchUpRepository: Send + Sync {
    /// Fetches the catch-up position for a given (account, asset) pair, if generation has
    /// ever run for that holding.
    async fn get_by_account_asset(
        &self,
        account_id: &str,
        asset_id: &str,
    ) -> Result<Option<FeeCatchUpPosition>>;
    /// Fetches every catch-up position of one account.
    async fn get_by_account(&self, account_id: &str) -> Result<Vec<FeeCatchUpPosition>>;
    /// Merges `incoming` into the stored position by **maximum** (CFR-044): the stored
    /// `last_applied_period` becomes the later of the stored and incoming values, never
    /// moving backwards, whatever order changes arrive in.
    async fn upsert(&self, incoming: FeeCatchUpPosition) -> Result<FeeCatchUpPosition>;
    /// Deletes the catch-up position of one (account, asset) pair. No-op if absent.
    async fn delete_by_account_asset(&self, account_id: &str, asset_id: &str) -> Result<()>;
}
