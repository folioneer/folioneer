use std::sync::Arc;

use chrono::{Datelike, Local};
use serde::Serialize;
use specta::Type;

use crate::context::account::{AccountError, AccountServiceContract, Profile, ProfileService};
use crate::context::asset::AssetServiceContract;
use crate::context::currency::CurrencyService;
use crate::use_cases::shared::account_summaries::SummaryReader;
use crate::use_cases::shared::global_performance::PerformanceAggregator;

/// PFL-055 — one profile's own figures.
#[derive(Debug, Clone, Serialize, Type)]
pub struct ProfileOverviewRow {
    /// The profile.
    pub profile: Profile,
    /// How many accounts it holds (PFL-023).
    pub account_count: u32,
    /// Its total value in the reference currency, reference-currency micros (ACC-027, ADR-001).
    pub value: i64,
    /// An account of it could not be converted, so the value leaves it out (ACC-028).
    pub value_incomplete: bool,
    /// Its year-to-date percentage as of the read date, micro-percent: the current month row's
    /// (PRF-034, GPF-031); none per PFL-055A.
    pub ytd_performance_pct: Option<i64>,
}

/// PFL-055 — every profile with its own figures, in the order of PFL-023.
#[derive(Debug, Clone, Serialize, Type)]
pub struct ProfileOverview {
    /// One row per profile; no row adds profiles together.
    pub rows: Vec<ProfileOverviewRow>,
    /// The reference currency every value is stated in (GPF-011).
    pub currency: String,
    /// The identifier of the profile in use (PFL-012).
    pub in_use: String,
}

/// PFL-055 — reads each profile's total value and performance within that profile alone.
pub struct ProfileOverviewUseCase {
    profiles: Arc<ProfileService>,
    account_service: Arc<dyn AccountServiceContract>,
    summaries: SummaryReader,
    performance: PerformanceAggregator,
}

impl ProfileOverviewUseCase {
    /// The use case over the profiles and the account, asset and currency services; the
    /// summaries and the performance are the shared computations (B18).
    pub fn new(
        profiles: Arc<ProfileService>,
        account_service: Arc<dyn AccountServiceContract>,
        asset_service: Arc<dyn AssetServiceContract>,
        currency_service: Arc<CurrencyService>,
    ) -> Self {
        Self {
            profiles,
            summaries: SummaryReader::new(
                account_service.clone(),
                asset_service.clone(),
                Arc::clone(&currency_service),
            ),
            performance: PerformanceAggregator::new(
                account_service.clone(),
                asset_service,
                currency_service,
            ),
            account_service,
        }
    }

    /// PFL-055 — every profile with its own figures; nothing is summed across them.
    pub async fn read(&self) -> Result<ProfileOverview, AccountError> {
        let list = self.profiles.list().await?;
        let year = Local::now().date_naive().year();
        let mut rows = Vec::with_capacity(list.profiles.len());
        let mut currency = String::new();
        for summary in list.profiles {
            let id = summary.profile.id.clone();
            let accounts = self.account_service.accounts_in_profile(&id).await?;
            let totals = self.summaries.summaries_of(accounts.clone()).await?.total;
            currency = totals.currency;
            let ytd_performance_pct = if accounts.is_empty() {
                None
            } else {
                // PRF-034 / GPF-031 — the current month row's year-to-date percentage, as an
                // account's (ACC-024): present in a profile's first year too.
                self.performance
                    .aggregate_across_accounts(accounts, None)
                    .await?
                    .monthly
                    .into_iter()
                    .filter(|period| period.year == year)
                    .max_by_key(|period| period.month)
                    .and_then(|period| period.year_to_date)
                    .and_then(|metric| metric.pct)
            };
            rows.push(ProfileOverviewRow {
                profile: summary.profile,
                account_count: summary.account_count,
                value: totals.total_global_value,
                value_incomplete: totals.incomplete,
                ytd_performance_pct,
            });
        }
        Ok(ProfileOverview {
            rows,
            currency,
            in_use: list.in_use,
        })
    }
}
