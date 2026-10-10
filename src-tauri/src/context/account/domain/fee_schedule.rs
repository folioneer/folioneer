use crate::context::account::error::AccountError;
use anyhow::Result;
use async_trait::async_trait;
use chrono::{Datelike, NaiveDate};
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

    /// The boundary of the period that contains `date`: the last day of its month, its
    /// quarter or its year (FEE-042). `None` only at the ceiling of representable dates.
    pub fn period_end_containing(self, date: NaiveDate) -> Option<NaiveDate> {
        match self {
            FeeFrequency::Monthly => last_day_of_month(date.year(), date.month()),
            FeeFrequency::Quarterly => {
                let quarter_end_month = ((date.month() - 1) / 3) * 3 + 3; // 3, 6, 9, or 12
                last_day_of_month(date.year(), quarter_end_month)
            }
            FeeFrequency::Annually => last_day_of_month(date.year(), 12),
        }
    }

    /// The boundary of the period immediately following the one ending at `boundary`.
    pub fn next_period_end(self, boundary: NaiveDate) -> Option<NaiveDate> {
        self.period_end_containing(boundary.succ_opt()?)
    }

    /// The boundary of the last period completed on `today` (FEE-040): today's own when
    /// today ends a period, otherwise the one before.
    pub fn last_completed_period_end(self, today: NaiveDate) -> Option<NaiveDate> {
        let current = self.period_end_containing(today)?;
        if current <= today {
            return Some(current);
        }
        let first_month = match self {
            FeeFrequency::Monthly => today.month(),
            FeeFrequency::Quarterly => ((today.month() - 1) / 3) * 3 + 1,
            FeeFrequency::Annually => 1,
        };
        NaiveDate::from_ymd_opt(today.year(), first_month, 1)?.pred_opt()
    }
}

/// Last calendar day of `year`/`month`. `None` only at the ceiling of representable dates.
fn last_day_of_month(year: i32, month: u32) -> Option<NaiveDate> {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)?.pred_opt()
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
/// 100% = 100_000_000. Must be strictly positive and strictly below 100_000_000.
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
    /// rate < 100_000_000 (`RateAboveHundred`: 100 % itself is refused), end_date > start_date (`EndBeforeStart`).
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
        if annual_rate_percent_micros >= 100_000_000 {
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
    /// rate < 100_000_000 (`RateAboveHundred`: 100 % itself is refused), end_date > start_date (`EndBeforeStart`).
    pub fn update_from(
        mut self,
        annual_rate_percent_micros: i64,
        end_date: Option<String>,
        active: bool,
    ) -> Result<Self, AccountError> {
        if annual_rate_percent_micros <= 0 {
            return Err(AccountError::RateNotPositive);
        }
        if annual_rate_percent_micros >= 100_000_000 {
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

#[cfg(test)]
mod rate_bound_tests {
    use super::*;

    // FEE-040 / FEE-061 — the last period completed on a day: its own when the day ends a
    // period, otherwise the one before, across a year's start.
    #[test]
    fn fee_040_the_last_completed_period_ends_on_or_before_the_day() {
        let day = |y, m, d| chrono::NaiveDate::from_ymd_opt(y, m, d).expect("date");
        for (frequency, today, expected) in [
            (FeeFrequency::Monthly, day(2025, 3, 15), day(2025, 2, 28)),
            (FeeFrequency::Monthly, day(2025, 3, 31), day(2025, 3, 31)),
            (FeeFrequency::Monthly, day(2025, 1, 1), day(2024, 12, 31)),
            (FeeFrequency::Quarterly, day(2025, 5, 20), day(2025, 3, 31)),
            (FeeFrequency::Quarterly, day(2025, 2, 1), day(2024, 12, 31)),
            (FeeFrequency::Quarterly, day(2025, 6, 30), day(2025, 6, 30)),
            (FeeFrequency::Annually, day(2025, 7, 4), day(2024, 12, 31)),
            (FeeFrequency::Annually, day(2025, 12, 31), day(2025, 12, 31)),
        ] {
            assert_eq!(
                frequency.last_completed_period_end(today),
                Some(expected),
                "{frequency} on {today}"
            );
        }
    }

    // FEE-032 — a rate is accepted strictly below 100 % a year: 100 % itself is refused,
    // like anything above, on creation and on edit.
    #[test]
    fn fee_032_a_rate_of_a_hundred_percent_is_refused_like_anything_above() {
        let schedule = |rate| {
            FeeSchedule::new(
                "acc".to_string(),
                "asset".to_string(),
                rate,
                FeeFrequency::Annually,
                "2024-01-01".to_string(),
                None,
            )
        };
        let existing = || schedule(1_000_000).expect("schedule");
        assert!(schedule(99_999_999).is_ok());
        assert!(existing().update_from(99_999_999, None, true).is_ok());
        for refused in [100_000_000, 100_000_001, 150_000_000] {
            assert!(matches!(
                schedule(refused),
                Err(AccountError::RateAboveHundred)
            ));
            assert!(matches!(
                existing().update_from(refused, None, true),
                Err(AccountError::RateAboveHundred)
            ));
        }
    }
}
