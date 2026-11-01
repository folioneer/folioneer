//! Orchestrates the scheduled daily price download (SPF spec): configuring the
//! OS schedule (`configure`), reporting status (`status`), and running the
//! actual sweep (`run_scheduled_fetch`) — invisibly, once per day, with
//! catch-up and backfill.

use crate::context::account::AccountServiceContract;
use crate::context::asset::{AssetServiceContract, PriceProvider};
use crate::context::currency::CurrencyService;
use crate::shared::infrastructure::scheduler::DailyFetchScheduler;
use crate::use_cases::shared::price_fetch_failures::{
    PriceFetchFailureReason, PriceFetchFailureRepository,
};
use crate::use_cases::shared::price_fetch_log::PriceFetchLogRepository;
use chrono::{NaiveDate, NaiveDateTime};
use std::sync::Arc;

use super::error::ScheduledFetchError;
use super::repository::{
    ScheduledFetchConfiguration, ScheduledFetchOutcome, ScheduledFetchRepository,
    ScheduledFetchRun, ScheduledFetchStatus,
};

/// Injectable source of "now" so tests can fix the current wall-clock moment
/// deterministically (mirrors `use_cases::asset_price_fetch::dispatcher::Clock`,
/// extended to a full timestamp — SPF-021's guard needs to know whether
/// *today's* trigger time has already passed, not just the calendar date).
pub type Clock = Arc<dyn Fn() -> NaiveDateTime + Send + Sync>;

/// Maximum number of days the daily-close / rate backfill looks back (SPF-031/036).
pub const BACKFILL_CAP_DAYS: i64 = 30;

/// Number of provider-connectivity attempts before a run is recorded as
/// `Failed` (SPF-051).
pub const MAX_RETRY_ATTEMPTS: u32 = 3;

/// Orchestrates the scheduled-fetch configuration and run pipeline.
pub struct ScheduledFetchOrchestrator {
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
    price_provider: Arc<dyn PriceProvider>,
    currency_service: Arc<CurrencyService>,
    repository: Arc<dyn ScheduledFetchRepository>,
    scheduler: Arc<dyn DailyFetchScheduler>,
    clock: Clock,
    fetch_log: Option<Arc<dyn PriceFetchLogRepository>>,
    fetch_failures: Option<Arc<dyn PriceFetchFailureRepository>>,
}

impl ScheduledFetchOrchestrator {
    /// Creates a new orchestrator.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        account_service: Arc<dyn AccountServiceContract>,
        asset_service: Arc<dyn AssetServiceContract>,
        price_provider: Arc<dyn PriceProvider>,
        currency_service: Arc<CurrencyService>,
        repository: Arc<dyn ScheduledFetchRepository>,
        scheduler: Arc<dyn DailyFetchScheduler>,
        clock: Clock,
    ) -> Self {
        Self {
            account_service,
            asset_service,
            price_provider,
            currency_service,
            repository,
            scheduler,
            clock,
            fetch_log: None,
            fetch_failures: None,
        }
    }

    /// Records the moment of each run that updated at least one asset in this
    /// installation's fetch log (MKT-201).
    pub fn with_fetch_log(mut self, fetch_log: Arc<dyn PriceFetchLogRepository>) -> Self {
        self.fetch_log = Some(fetch_log);
        self
    }

    /// Records, per asset, why the run could not price it, and forgets the failure of each
    /// asset it priced (MKT-220).
    pub fn with_fetch_failures(
        mut self,
        fetch_failures: Arc<dyn PriceFetchFailureRepository>,
    ) -> Self {
        self.fetch_failures = Some(fetch_failures);
        self
    }

    /// MKT-220 — writes what the run's final attempt learned of each asset: `None` when it
    /// was priced, the reason otherwise. A failed write never fails the run.
    async fn note_outcomes(&self, outcomes: &[(String, Option<PriceFetchFailureReason>)]) {
        let Some(failures) = &self.fetch_failures else {
            return;
        };
        let now = (self.clock)();
        for (asset_id, failure) in outcomes {
            let written = match failure {
                None => failures.clear(asset_id).await,
                Some(reason) => failures.record(asset_id, *reason, now).await,
            };
            if let Err(error) = written {
                tracing::warn!(target: crate::core::logger::BACKEND, asset_id = %asset_id, err = ?error, "scheduled run: recording why an asset was not priced failed (MKT-220)");
            }
        }
    }

    /// Applies a configuration change (SPF-010–013, SPF-019): validates the
    /// trigger time, registers/re-registers the OS schedule when `enabled`
    /// (or removes it when not) **before** persisting — so the stored
    /// configuration never contradicts the OS schedule (SPF-013). On a
    /// registration/removal failure, the configuration is left unchanged.
    pub async fn configure(
        &self,
        enabled: bool,
        trigger_time: String,
    ) -> Result<(), ScheduledFetchError> {
        let configuration = ScheduledFetchConfiguration::new(enabled, trigger_time)?;
        if configuration.enabled {
            self.scheduler
                .register(&configuration.trigger_time)
                .await
                .map_err(|error| {
                    tracing::error!(target: crate::core::logger::BACKEND, err = ?error, "OS schedule registration failed");
                    ScheduledFetchError::ScheduleRegistrationFailed
                })?;
        } else {
            self.scheduler.remove().await.map_err(|error| {
                tracing::error!(target: crate::core::logger::BACKEND, err = ?error, "OS schedule removal failed");
                ScheduledFetchError::ScheduleRemovalFailed
            })?;
        }
        self.repository
            .save_configuration(configuration.enabled, &configuration.trigger_time)
            .await
            .map_err(|error| {
                tracing::error!(target: crate::core::logger::BACKEND, err = ?error, "saving scheduled-fetch configuration failed");
                ScheduledFetchError::DatabaseError
            })?;
        Ok(())
    }

    /// Returns the current configuration and the most recent run, or `None`
    /// for `last_run` on a fresh install (SPF-052).
    pub async fn status(&self) -> Result<ScheduledFetchStatus, ScheduledFetchError> {
        let configuration = self
            .repository
            .get_configuration()
            .await
            .map_err(|error| {
                tracing::error!(target: crate::core::logger::BACKEND, err = ?error, "reading scheduled-fetch configuration failed");
                ScheduledFetchError::DatabaseError
            })?;
        let last_run = self.repository.last_run().await.map_err(|error| {
            tracing::error!(target: crate::core::logger::BACKEND, err = ?error, "reading last scheduled-fetch run failed");
            ScheduledFetchError::DatabaseError
        })?;
        Ok(ScheduledFetchStatus {
            configuration,
            last_run,
        })
    }

    /// Verifies and silently repairs the OS schedule against the stored
    /// configuration on app start (SPF-015): re-registers a missing/stale
    /// entry when enabled, removes a leftover when disabled. Failures are
    /// logged, never surfaced — the next app start retries.
    pub async fn self_heal(&self) {
        let configuration = match self.repository.get_configuration().await {
            Ok(configuration) => configuration,
            Err(error) => {
                tracing::warn!(target: crate::core::logger::BACKEND, err = ?error, "self-heal: configuration read failed");
                return;
            }
        };
        let outcome = if configuration.enabled {
            self.scheduler.register(&configuration.trigger_time).await
        } else {
            match self.scheduler.is_registered().await {
                Ok(true) => self.scheduler.remove().await,
                Ok(false) => Ok(()),
                Err(error) => Err(error),
            }
        };
        if let Err(error) = outcome {
            tracing::warn!(target: crate::core::logger::BACKEND, err = ?error, "self-heal: OS schedule repair failed");
        }
    }

    /// SPF-015, SPF-070 — the schedule follows whether an External provider is chosen: back
    /// as configured with one, gone without. The stored configuration is never written.
    pub async fn follow_provider(&self, on: bool) {
        if on {
            self.self_heal().await;
        } else {
            drop_schedule_without_provider(self.scheduler.as_ref()).await;
        }
    }

    /// Runs the scheduled-fetch sweep (SPF-020–SPF-053): resolves the latest
    /// pending trigger, exits early under the once-per-day guard (SPF-021),
    /// builds the fetch scope (SPF-040), records daily closes and FX rates
    /// with independent outcomes (SPF-030–039), and always records the run
    /// (SPF-050).
    pub async fn run_scheduled_fetch(&self) -> Result<ScheduledFetchRun, ScheduledFetchError> {
        let configuration = self
            .repository
            .get_configuration()
            .await
            .map_err(|error| {
                tracing::error!(target: crate::core::logger::BACKEND, err = ?error, "scheduled run: configuration read failed");
                ScheduledFetchError::DatabaseError
            })?;
        let now = (self.clock)();
        let trigger_date = latest_pending_trigger(now, &configuration.trigger_time);
        let executed_at = now.format("%Y-%m-%dT%H:%M:%S").to_string();

        let last_success = self.repository.last_successful_run().await.map_err(|error| {
            tracing::error!(target: crate::core::logger::BACKEND, err = ?error, "scheduled run: last-successful-run read failed");
            ScheduledFetchError::DatabaseError
        })?;

        // SPF-021 — the once-per-day guard: exit before any external call when
        // a successful run already settled this trigger day.
        if last_success
            .as_ref()
            .is_some_and(|run| run.trigger_date >= trigger_date.to_string())
        {
            let run = ScheduledFetchRun::new(
                executed_at,
                trigger_date.to_string(),
                ScheduledFetchOutcome::SkippedAlreadyRun,
                0,
                0,
            );
            self.record(run.clone()).await?;
            return Ok(run);
        }

        let last_success_date = last_success
            .as_ref()
            .and_then(|run| run.trigger_date.parse::<NaiveDate>().ok());
        let (from, to) = backfill_window(last_success_date, trigger_date);

        // SPF-040 — scope: active holdings across all accounts, minus cash /
        // locked / non-derivable assets. A hard failure building the scope is
        // still a recorded run (SPF-050).
        let (scope, fx_pairs) = match self.gather_scope().await {
            Ok(scope_and_pairs) => scope_and_pairs,
            Err(()) => {
                let run = ScheduledFetchRun::new(
                    executed_at,
                    trigger_date.to_string(),
                    ScheduledFetchOutcome::Failed,
                    0,
                    0,
                );
                self.record(run.clone()).await?;
                return Ok(run);
            }
        };

        let run = if scope.is_empty() {
            // SPF-042 — an empty scope is a quiet success.
            ScheduledFetchRun::new(
                executed_at,
                trigger_date.to_string(),
                ScheduledFetchOutcome::Succeeded,
                0,
                0,
            )
        } else {
            let (outcome, updated_count, skipped_count) =
                self.sweep_prices_with_retry(&scope, from, to).await;
            // SPF-035–039 — the FX portion never fails the price portion; its
            // own failures are absorbed inside refresh_all_rates_range.
            if outcome != ScheduledFetchOutcome::Failed {
                if let Err(error) = self
                    .currency_service
                    .refresh_all_rates_range(fx_pairs, &from.to_string(), &to.to_string())
                    .await
                {
                    tracing::warn!(target: crate::core::logger::BACKEND, err = ?error, "scheduled run: FX range refresh failed (SPF-039 — price outcome unaffected)");
                }
            }
            ScheduledFetchRun::new(
                executed_at,
                trigger_date.to_string(),
                outcome,
                updated_count,
                skipped_count,
            )
        };

        self.record(run.clone()).await?;
        // MKT-201 — a run that updated at least one asset is a fetch on this device. A
        // failed write never fails the run, which is already recorded.
        if run.updated_count > 0 {
            if let Some(fetch_log) = &self.fetch_log {
                if let Err(error) = fetch_log.record_completed(now).await {
                    tracing::warn!(target: crate::core::logger::BACKEND, err = ?error, "scheduled run: recording the fetch moment failed (MKT-201)");
                }
            }
        }
        Ok(run)
    }

    /// Persists a run record (SPF-050 — every path records).
    async fn record(&self, run: ScheduledFetchRun) -> Result<(), ScheduledFetchError> {
        self.repository.record_run(run).await.map_err(|error| {
            tracing::error!(target: crate::core::logger::BACKEND, err = ?error, "scheduled run: recording the run failed");
            ScheduledFetchError::DatabaseError
        })
    }

    /// Collects the fetch scope and FX pairs across all accounts' active
    /// holdings (SPF-040, FXR-071). `Err(())` signals a hard lookup failure —
    /// the caller records a `Failed` run.
    async fn gather_scope(
        &self,
    ) -> Result<
        (
            Vec<(crate::context::asset::Asset, String)>,
            Vec<crate::context::currency::CurrencyPair>,
        ),
        (),
    > {
        let accounts = match self.account_service.get_all().await {
            Ok(accounts) => accounts,
            Err(error) => {
                tracing::error!(target: crate::core::logger::BACKEND, err = ?error, "scheduled run: account listing failed");
                return Err(());
            }
        };
        let reference = match self.currency_service.reference_currency().await {
            Ok(reference) => reference,
            Err(error) => {
                tracing::error!(target: crate::core::logger::BACKEND, err = ?error, "scheduled run: reference currency not read");
                return Err(());
            }
        };
        let account_currencies: Vec<String> = accounts
            .iter()
            .map(|account| account.currency.clone())
            .collect();
        let mut asset_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut fx_inputs: Vec<(String, String)> = Vec::new();
        for account in accounts {
            let holdings = match self
                .account_service
                .get_holdings_for_account(&account.id)
                .await
            {
                Ok(holdings) => holdings,
                Err(error) => {
                    tracing::error!(target: crate::core::logger::BACKEND, err = ?error, "scheduled run: holdings listing failed");
                    return Err(());
                }
            };
            for holding in holdings.into_iter().filter(|holding| holding.is_active()) {
                fx_inputs.push((account.currency.clone(), holding.asset_id.clone()));
                asset_ids.insert(holding.asset_id);
            }
        }
        let (scope, currency_by_asset) = match crate::use_cases::shared::scope::build_scope(
            self.asset_service.as_ref(),
            asset_ids,
        )
        .await
        {
            Ok(scope_and_currencies) => scope_and_currencies,
            Err(error) => {
                tracing::error!(target: crate::core::logger::BACKEND, err = ?error, "scheduled run: scope build failed");
                return Err(());
            }
        };
        let fx_pairs = crate::use_cases::shared::scope::with_reference_pairs(
            crate::use_cases::shared::scope::build_fx_pairs(fx_inputs, &currency_by_asset),
            account_currencies.iter().map(String::as_str),
            &reference,
        );
        Ok((scope, fx_pairs))
    }

    /// Runs the per-asset daily-close sweep (SPF-030–034, SPF-041). When every
    /// asset in an attempt fails with a provider error — the provider is
    /// unreachable, not missing data — the whole sweep retries up to
    /// [`MAX_RETRY_ATTEMPTS`] with increasing delay before the run is `Failed`
    /// (SPF-051). Returns `(outcome, updated_count, skipped_count)`.
    async fn sweep_prices_with_retry(
        &self,
        scope: &[(crate::context::asset::Asset, String)],
        from: NaiveDate,
        to: NaiveDate,
    ) -> (ScheduledFetchOutcome, u32, u32) {
        let from = from.to_string();
        let to = to.to_string();
        let mut attempt: u32 = 0;
        loop {
            attempt += 1;
            let mut updated_count: u32 = 0;
            let mut skipped_count: u32 = 0;
            let mut provider_errors: usize = 0;
            // MKT-220 — what this attempt learned of each asset.
            let mut outcomes: Vec<(String, Option<PriceFetchFailureReason>)> =
                Vec::with_capacity(scope.len());
            for (asset, symbol) in scope {
                match self
                    .price_provider
                    .fetch_daily_closes(symbol, &from, &to)
                    .await
                {
                    Ok(Some(closes)) => {
                        // MKT-230 — the window's reported splits are read beside its closes,
                        // an empty answer included; a failed reading skips the asset as a
                        // failed close request does (MKT-193B, SPF-041).
                        let splits = match self.price_provider.fetch_splits(symbol, &from).await {
                            Ok(splits) => splits,
                            Err(error) => {
                                outcomes.push((
                                    asset.id.clone(),
                                    Some(PriceFetchFailureReason::ProviderUnreachable),
                                ));
                                tracing::warn!(target: crate::core::logger::BACKEND, symbol = %symbol, err = ?error, "scheduled run: split fetch failed");
                                provider_errors += 1;
                                continue;
                            }
                        };
                        let no_close = closes.is_empty();
                        let recorded = match self
                            .asset_service
                            .record_reported_splits(&asset.id, &from, &splits)
                            .await
                        {
                            Ok(()) if no_close => Ok(0),
                            Ok(()) => {
                                self.asset_service
                                    .record_daily_closes(&asset.id, closes)
                                    .await
                            }
                            Err(error) => Err(error),
                        };
                        match recorded {
                            // SPF-032/033 — an empty answer is a window with no close to give
                            // (no trading day, or a session not closed yet): MKT-220 keeps
                            // what it had.
                            Ok(_) if no_close => skipped_count += 1,
                            Ok(_) => {
                                updated_count += 1;
                                outcomes.push((asset.id.clone(), None));
                            }
                            Err(error) => {
                                outcomes.push((
                                    asset.id.clone(),
                                    Some(PriceFetchFailureReason::NotSaved),
                                ));
                                // SPF-041 — a write failure is a counted silent skip.
                                tracing::warn!(target: crate::core::logger::BACKEND, asset_id = %asset.id, err = ?error, "scheduled run: recording daily closes failed, asset skipped");
                                skipped_count += 1;
                            }
                        }
                    }
                    // SPF-041 — the provider has no data for this asset.
                    Ok(None) => {
                        skipped_count += 1;
                        outcomes.push((asset.id.clone(), Some(PriceFetchFailureReason::NoData)));
                    }
                    Err(error) => {
                        outcomes.push((
                            asset.id.clone(),
                            Some(PriceFetchFailureReason::ProviderUnreachable),
                        ));
                        tracing::warn!(target: crate::core::logger::BACKEND, symbol = %symbol, err = ?error, "scheduled run: daily-close fetch failed");
                        provider_errors += 1;
                    }
                }
            }
            // Every asset erroring means the provider is unreachable — retry
            // the whole sweep (SPF-051); a mixed outcome is per-asset skips.
            if provider_errors == scope.len() {
                if attempt >= MAX_RETRY_ATTEMPTS {
                    self.note_outcomes(&outcomes).await;
                    return (ScheduledFetchOutcome::Failed, 0, skipped_count);
                }
                tokio::time::sleep(std::time::Duration::from_millis(500 * u64::from(attempt)))
                    .await;
                continue;
            }
            skipped_count += provider_errors as u32;
            self.note_outcomes(&outcomes).await;
            return (
                ScheduledFetchOutcome::Succeeded,
                updated_count,
                skipped_count,
            );
        }
    }
}

/// Resolves the latest pending trigger day (SPF-021): today when the
/// configured trigger time has already passed at `now`, otherwise yesterday.
/// A malformed stored trigger time (impossible after SPF-019 validation)
/// falls back to treating the trigger as already passed.
fn latest_pending_trigger(now: NaiveDateTime, trigger_time: &str) -> NaiveDate {
    let trigger =
        chrono::NaiveTime::parse_from_str(trigger_time, "%H:%M").unwrap_or(chrono::NaiveTime::MIN);
    if now.time() >= trigger {
        now.date()
    } else {
        now.date() - chrono::Duration::days(1)
    }
}

/// Computes the `[from, to]` backfill window for the daily-close / rate fetch
/// (SPF-031/036): the day after `last_success` through `today`, capped at
/// [`BACKFILL_CAP_DAYS`] days back when `last_success` is absent or older than
/// the cap.
pub fn backfill_window(
    last_success: Option<NaiveDate>,
    today: NaiveDate,
) -> (NaiveDate, NaiveDate) {
    let capped_from = today - chrono::Duration::days(BACKFILL_CAP_DAYS);
    let from = match last_success {
        Some(date) => (date + chrono::Duration::days(1)).max(capped_from),
        None => capped_from,
    };
    (from, today)
}

/// SPF-070 — with no External provider chosen, a computer owns no scheduled fetch: it
/// registers none, and removes one left with the operating system, so nothing wakes up to
/// fetch what cannot be fetched. The stored configuration is untouched, so choosing a
/// provider again honours it (SPF-015).
///
/// Failures are logged, never surfaced: this runs at start-up, and a schedule that
/// could not be removed is retried at the next one.
async fn drop_schedule_without_provider(scheduler: &dyn DailyFetchScheduler) {
    match scheduler.is_registered().await {
        Ok(false) => {}
        Ok(true) => {
            if let Err(error) = scheduler.remove().await {
                tracing::warn!(target: crate::core::logger::BACKEND, err = ?error, "no External provider: leftover schedule not removed (SPF-070)");
            } else {
                tracing::info!(target: crate::core::logger::BACKEND, "no External provider: leftover schedule removed (SPF-070)");
            }
        }
        Err(error) => {
            tracing::warn!(target: crate::core::logger::BACKEND, err = ?error, "no External provider: schedule state unreadable (SPF-070)");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::repository::MockScheduledFetchRepository;
    use super::*;
    use crate::context::account::MockAccountServiceContract;
    use crate::context::asset::{MockAssetServiceContract, MockPriceProvider};
    use crate::context::currency::{MockCurrencyPairRepository, MockCurrencyRateRepository};
    use crate::shared::infrastructure::scheduler::MockDailyFetchScheduler;
    use crate::use_cases::shared::price_fetch_log::MockPriceFetchLogRepository;

    // SPF-070 — a build without a provider removes the daily download a build that had
    // one left behind, so nothing wakes up to fetch what cannot be fetched.
    #[tokio::test]
    async fn a_leftover_schedule_is_removed_when_there_is_no_provider() {
        let mut scheduler = MockDailyFetchScheduler::new();
        scheduler
            .expect_is_registered()
            .times(1)
            .returning(|| Ok(true));
        scheduler.expect_remove().times(1).returning(|| Ok(()));

        drop_schedule_without_provider(&scheduler).await;
    }

    // SPF-070 — nothing registered, nothing to remove: the start-up path stays quiet.
    #[tokio::test]
    async fn nothing_is_removed_when_no_schedule_is_registered() {
        let mut scheduler = MockDailyFetchScheduler::new();
        scheduler
            .expect_is_registered()
            .times(1)
            .returning(|| Ok(false));
        scheduler.expect_remove().never();

        drop_schedule_without_provider(&scheduler).await;
    }

    // SPF-070 — a scheduler that cannot be read or cannot remove is logged, never
    // surfaced: the application starts, and the next start tries again.
    #[tokio::test]
    async fn a_scheduler_that_fails_does_not_break_the_start_up() {
        let mut unreadable = MockDailyFetchScheduler::new();
        unreadable
            .expect_is_registered()
            .returning(|| Err(anyhow::anyhow!("scheduler unavailable")));
        unreadable.expect_remove().never();
        drop_schedule_without_provider(&unreadable).await;

        let mut stubborn = MockDailyFetchScheduler::new();
        stubborn.expect_is_registered().returning(|| Ok(true));
        stubborn
            .expect_remove()
            .returning(|| Err(anyhow::anyhow!("removal refused")));
        drop_schedule_without_provider(&stubborn).await;
    }
    use mockall::Sequence;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 6, 10).expect("valid date")
    }

    /// FXR-015 — an account in another currency than the reference has its pair declared,
    /// which these runs accept; no rate is fetched without a history provider.
    fn make_currency_service() -> Arc<CurrencyService> {
        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo.expect_upsert_pair().returning(Ok);
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .returning(|| Ok(vec![]));
        Arc::new(CurrencyService::new(
            Box::new(pair_repo),
            Box::new(MockCurrencyRateRepository::new()),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    /// Builds the orchestrator; a test that sets no expectation of its own on the splits
    /// gets a provider reporting none and a service keeping them (MKT-230).
    fn make_orchestrator(
        account_service: MockAccountServiceContract,
        mut asset_service: MockAssetServiceContract,
        mut price_provider: MockPriceProvider,
        repository: MockScheduledFetchRepository,
        scheduler: MockDailyFetchScheduler,
        now: NaiveDateTime,
    ) -> ScheduledFetchOrchestrator {
        price_provider
            .expect_fetch_splits()
            .returning(|_, _| Ok(Vec::new()));
        asset_service
            .expect_record_reported_splits()
            .returning(|_, _, _| Ok(()));
        ScheduledFetchOrchestrator::new(
            Arc::new(account_service),
            Arc::new(asset_service),
            Arc::new(price_provider),
            make_currency_service(),
            Arc::new(repository),
            Arc::new(scheduler),
            Arc::new(move || now),
        )
    }

    /// A fixed "now" after today's 22:15 trigger has already passed — the
    /// default fixture for tests exercising the guard/scope logic rather than
    /// the pending-trigger edge itself.
    fn now_after_trigger() -> NaiveDateTime {
        today().and_hms_opt(23, 0, 0).expect("valid time")
    }

    /// A fixed "now" BEFORE today's 22:15 trigger has passed — used by the
    /// catch-up test where the latest pending trigger must resolve to yesterday.
    fn now_before_trigger() -> NaiveDateTime {
        today().and_hms_opt(20, 0, 0).expect("valid time")
    }

    // -------------------------------------------------------------------------
    // backfill_window — SPF-031/036
    // -------------------------------------------------------------------------

    // SPF-031 — no prior successful run → the window starts 30 days back.
    #[test]
    fn backfill_window_defaults_to_30_days_back_when_no_prior_success() {
        let (from, to) = backfill_window(None, today());
        assert_eq!(from, today() - chrono::Duration::days(30));
        assert_eq!(to, today());
    }

    // SPF-031 — a recent prior success anchors the window to the day after it.
    #[test]
    fn backfill_window_starts_the_day_after_last_success() {
        let last_success = today() - chrono::Duration::days(5);
        let (from, to) = backfill_window(Some(last_success), today());
        assert_eq!(from, last_success + chrono::Duration::days(1));
        assert_eq!(to, today());
    }

    // SPF-031 — a prior success older than 30 days is capped, never reaching
    // further back than the 30-day window.
    #[test]
    fn backfill_window_caps_at_30_days_when_last_success_is_older() {
        let last_success = today() - chrono::Duration::days(40);
        let (from, _to) = backfill_window(Some(last_success), today());
        assert_eq!(from, today() - chrono::Duration::days(30));
    }

    // -------------------------------------------------------------------------
    // configure — SPF-011/012/013/019
    // -------------------------------------------------------------------------

    // SPF-019 — an invalid trigger time is rejected before any scheduler/repo call.
    #[tokio::test]
    async fn configure_rejects_invalid_trigger_time_without_side_effects() {
        let mut scheduler = MockDailyFetchScheduler::new();
        scheduler.expect_register().times(0);
        scheduler.expect_remove().times(0);
        let mut repository = MockScheduledFetchRepository::new();
        repository.expect_save_configuration().times(0);

        let orchestrator = make_orchestrator(
            MockAccountServiceContract::new(),
            MockAssetServiceContract::new(),
            MockPriceProvider::new(),
            repository,
            scheduler,
            now_after_trigger(),
        );

        let err = orchestrator
            .configure(true, "24:00".to_string())
            .await
            .unwrap_err();
        assert!(
            matches!(err, ScheduledFetchError::InvalidTriggerTime),
            "got: {err:?}"
        );
    }

    // SPF-012 — enabling registers the OS schedule BEFORE persisting the configuration.
    #[tokio::test]
    async fn configure_enabling_registers_before_persisting() {
        let mut sequence = Sequence::new();
        let mut scheduler = MockDailyFetchScheduler::new();
        scheduler
            .expect_register()
            .times(1)
            .withf(|t| t == "19:00")
            .in_sequence(&mut sequence)
            .returning(|_| Ok(()));
        let mut repository = MockScheduledFetchRepository::new();
        repository
            .expect_save_configuration()
            .times(1)
            .withf(|enabled, trigger_time| *enabled && trigger_time == "19:00")
            .in_sequence(&mut sequence)
            .returning(|_, _| Ok(()));

        let orchestrator = make_orchestrator(
            MockAccountServiceContract::new(),
            MockAssetServiceContract::new(),
            MockPriceProvider::new(),
            repository,
            scheduler,
            now_after_trigger(),
        );

        let result = orchestrator.configure(true, "19:00".to_string()).await;
        assert!(result.is_ok(), "got: {result:?}");
    }

    // SPF-012 — disabling removes the OS schedule before persisting.
    #[tokio::test]
    async fn configure_disabling_removes_schedule_before_persisting() {
        let mut sequence = Sequence::new();
        let mut scheduler = MockDailyFetchScheduler::new();
        scheduler
            .expect_remove()
            .times(1)
            .in_sequence(&mut sequence)
            .returning(|| Ok(()));
        let mut repository = MockScheduledFetchRepository::new();
        repository
            .expect_save_configuration()
            .times(1)
            .withf(|enabled, _| !*enabled)
            .in_sequence(&mut sequence)
            .returning(|_, _| Ok(()));

        let orchestrator = make_orchestrator(
            MockAccountServiceContract::new(),
            MockAssetServiceContract::new(),
            MockPriceProvider::new(),
            repository,
            scheduler,
            now_after_trigger(),
        );

        let result = orchestrator.configure(false, "22:15".to_string()).await;
        assert!(result.is_ok(), "got: {result:?}");
    }

    // SPF-012 — changing the trigger time while enabled re-registers (not just
    // persists) the OS schedule at the new time.
    #[tokio::test]
    async fn configure_changing_time_reregisters_schedule() {
        let mut scheduler = MockDailyFetchScheduler::new();
        scheduler
            .expect_register()
            .times(1)
            .withf(|t| t == "06:30")
            .returning(|_| Ok(()));
        let mut repository = MockScheduledFetchRepository::new();
        repository
            .expect_save_configuration()
            .times(1)
            .returning(|_, _| Ok(()));

        let orchestrator = make_orchestrator(
            MockAccountServiceContract::new(),
            MockAssetServiceContract::new(),
            MockPriceProvider::new(),
            repository,
            scheduler,
            now_after_trigger(),
        );

        let result = orchestrator.configure(true, "06:30".to_string()).await;
        assert!(result.is_ok(), "got: {result:?}");
    }

    // SPF-013 — a registration failure is surfaced and the configuration is
    // left unchanged (save_configuration is never called).
    #[tokio::test]
    async fn configure_registration_failure_leaves_configuration_unchanged() {
        let mut scheduler = MockDailyFetchScheduler::new();
        scheduler
            .expect_register()
            .times(1)
            .returning(|_| Err(anyhow::anyhow!("systemctl unavailable")));
        let mut repository = MockScheduledFetchRepository::new();
        repository.expect_save_configuration().times(0);

        let orchestrator = make_orchestrator(
            MockAccountServiceContract::new(),
            MockAssetServiceContract::new(),
            MockPriceProvider::new(),
            repository,
            scheduler,
            now_after_trigger(),
        );

        let err = orchestrator
            .configure(true, "19:00".to_string())
            .await
            .unwrap_err();
        assert!(
            matches!(err, ScheduledFetchError::ScheduleRegistrationFailed),
            "got: {err:?}"
        );
    }

    // SPF-013 — a removal failure is surfaced and the configuration is left unchanged.
    #[tokio::test]
    async fn configure_removal_failure_leaves_configuration_unchanged() {
        let mut scheduler = MockDailyFetchScheduler::new();
        scheduler
            .expect_remove()
            .times(1)
            .returning(|| Err(anyhow::anyhow!("systemctl unavailable")));
        let mut repository = MockScheduledFetchRepository::new();
        repository.expect_save_configuration().times(0);

        let orchestrator = make_orchestrator(
            MockAccountServiceContract::new(),
            MockAssetServiceContract::new(),
            MockPriceProvider::new(),
            repository,
            scheduler,
            now_after_trigger(),
        );

        let err = orchestrator
            .configure(false, "19:00".to_string())
            .await
            .unwrap_err();
        assert!(
            matches!(err, ScheduledFetchError::ScheduleRemovalFailed),
            "got: {err:?}"
        );
    }

    // -------------------------------------------------------------------------
    // self_heal — SPF-015
    // -------------------------------------------------------------------------

    // SPF-015 — an enabled configuration re-registers the OS schedule on app
    // start (repairs a missing/stale entry, e.g. after the app binary moved).
    #[tokio::test]
    async fn self_heal_reregisters_when_enabled() {
        let mut repository = MockScheduledFetchRepository::new();
        repository
            .expect_get_configuration()
            .times(1)
            .returning(|| {
                Ok(ScheduledFetchConfiguration::restore(
                    true,
                    "19:00".to_string(),
                ))
            });
        let mut scheduler = MockDailyFetchScheduler::new();
        scheduler
            .expect_register()
            .times(1)
            .withf(|trigger_time| trigger_time == "19:00")
            .returning(|_| Ok(()));

        let orchestrator = make_orchestrator(
            MockAccountServiceContract::new(),
            MockAssetServiceContract::new(),
            MockPriceProvider::new(),
            repository,
            scheduler,
            now_after_trigger(),
        );

        orchestrator.self_heal().await;
    }

    // SPF-070 — turned off, the schedule goes; the configuration is neither read nor written
    // (the repository mock has no expectation).
    #[tokio::test]
    async fn with_no_provider_the_schedule_goes_and_the_configuration_stays() {
        let mut scheduler = MockDailyFetchScheduler::new();
        scheduler
            .expect_is_registered()
            .times(1)
            .returning(|| Ok(true));
        scheduler.expect_remove().times(1).returning(|| Ok(()));
        let orchestrator = make_orchestrator(
            MockAccountServiceContract::new(),
            MockAssetServiceContract::new(),
            MockPriceProvider::new(),
            MockScheduledFetchRepository::new(),
            scheduler,
            now_after_trigger(),
        );

        orchestrator.follow_provider(false).await;
    }

    // SPF-015, SPF-070 — chosen again, the stored configuration comes back, unwritten.
    #[tokio::test]
    async fn with_a_provider_the_schedule_comes_back_as_configured() {
        let mut repository = MockScheduledFetchRepository::new();
        repository
            .expect_get_configuration()
            .times(1)
            .returning(|| {
                Ok(ScheduledFetchConfiguration::restore(
                    true,
                    "19:00".to_string(),
                ))
            });
        let mut scheduler = MockDailyFetchScheduler::new();
        scheduler
            .expect_register()
            .times(1)
            .withf(|trigger_time| trigger_time == "19:00")
            .returning(|_| Ok(()));
        let orchestrator = make_orchestrator(
            MockAccountServiceContract::new(),
            MockAssetServiceContract::new(),
            MockPriceProvider::new(),
            repository,
            scheduler,
            now_after_trigger(),
        );

        orchestrator.follow_provider(true).await;
    }

    // SPF-015 — a disabled configuration silently removes a leftover schedule.
    #[tokio::test]
    async fn self_heal_removes_leftover_schedule_when_disabled() {
        let mut repository = MockScheduledFetchRepository::new();
        repository
            .expect_get_configuration()
            .times(1)
            .returning(|| {
                Ok(ScheduledFetchConfiguration::restore(
                    false,
                    "22:15".to_string(),
                ))
            });
        let mut scheduler = MockDailyFetchScheduler::new();
        scheduler
            .expect_is_registered()
            .times(1)
            .returning(|| Ok(true));
        scheduler.expect_remove().times(1).returning(|| Ok(()));

        let orchestrator = make_orchestrator(
            MockAccountServiceContract::new(),
            MockAssetServiceContract::new(),
            MockPriceProvider::new(),
            repository,
            scheduler,
            now_after_trigger(),
        );

        orchestrator.self_heal().await;
    }

    // SPF-015 — disabled with nothing registered touches nothing.
    #[tokio::test]
    async fn self_heal_does_nothing_when_disabled_and_not_registered() {
        let mut repository = MockScheduledFetchRepository::new();
        repository
            .expect_get_configuration()
            .times(1)
            .returning(|| {
                Ok(ScheduledFetchConfiguration::restore(
                    false,
                    "22:15".to_string(),
                ))
            });
        let mut scheduler = MockDailyFetchScheduler::new();
        scheduler
            .expect_is_registered()
            .times(1)
            .returning(|| Ok(false));
        scheduler.expect_remove().times(0);

        let orchestrator = make_orchestrator(
            MockAccountServiceContract::new(),
            MockAssetServiceContract::new(),
            MockPriceProvider::new(),
            repository,
            scheduler,
            now_after_trigger(),
        );

        orchestrator.self_heal().await;
    }

    // -------------------------------------------------------------------------
    // status — SPF-052
    // -------------------------------------------------------------------------

    // SPF-052 — status returns the configuration and last_run = None on a fresh install.
    #[tokio::test]
    async fn status_returns_configuration_with_none_last_run_on_fresh_install() {
        let mut repository = MockScheduledFetchRepository::new();
        repository
            .expect_get_configuration()
            .times(1)
            .returning(|| {
                Ok(ScheduledFetchConfiguration::restore(
                    false,
                    "22:15".to_string(),
                ))
            });
        repository.expect_last_run().times(1).returning(|| Ok(None));

        let orchestrator = make_orchestrator(
            MockAccountServiceContract::new(),
            MockAssetServiceContract::new(),
            MockPriceProvider::new(),
            repository,
            MockDailyFetchScheduler::new(),
            now_after_trigger(),
        );

        let status = orchestrator.status().await.unwrap();
        assert!(!status.configuration.enabled);
        assert_eq!(status.configuration.trigger_time, "22:15");
        assert!(status.last_run.is_none());
    }

    // SPF-052 — status surfaces the most recent run when one exists.
    #[tokio::test]
    async fn status_returns_the_most_recent_run_when_one_exists() {
        let run = ScheduledFetchRun::new(
            "2026-06-09T22:15:00".to_string(),
            "2026-06-09".to_string(),
            ScheduledFetchOutcome::Succeeded,
            12,
            2,
        );
        let expected = run.clone();
        let mut repository = MockScheduledFetchRepository::new();
        repository
            .expect_get_configuration()
            .times(1)
            .returning(|| {
                Ok(ScheduledFetchConfiguration::restore(
                    true,
                    "22:15".to_string(),
                ))
            });
        repository
            .expect_last_run()
            .times(1)
            .returning(move || Ok(Some(run.clone())));

        let orchestrator = make_orchestrator(
            MockAccountServiceContract::new(),
            MockAssetServiceContract::new(),
            MockPriceProvider::new(),
            repository,
            MockDailyFetchScheduler::new(),
            now_after_trigger(),
        );

        let status = orchestrator.status().await.unwrap();
        assert_eq!(status.last_run, Some(expected));
    }

    // -------------------------------------------------------------------------
    // run_scheduled_fetch — SPF-021/022/040/041/042/050/051/039
    // -------------------------------------------------------------------------

    // SPF-021 — a trigger already settled today exits via the once-per-day
    // guard: no account/asset lookups, no provider calls; a SkippedAlreadyRun
    // run is recorded.
    #[tokio::test]
    async fn run_scheduled_fetch_records_skip_when_trigger_already_settled() {
        let mut repository = MockScheduledFetchRepository::new();
        repository.expect_get_configuration().returning(|| {
            Ok(ScheduledFetchConfiguration::restore(
                true,
                "22:15".to_string(),
            ))
        });
        repository.expect_last_successful_run().returning(|| {
            Ok(Some(ScheduledFetchRun::new(
                "2026-06-10T22:15:00".to_string(),
                "2026-06-10".to_string(),
                ScheduledFetchOutcome::Succeeded,
                3,
                0,
            )))
        });
        repository
            .expect_record_run()
            .times(1)
            .withf(|run| run.outcome == ScheduledFetchOutcome::SkippedAlreadyRun)
            .returning(|_| Ok(()));

        let mut account_service = MockAccountServiceContract::new();
        account_service.expect_get_all().times(0);
        let mut price_provider = MockPriceProvider::new();
        price_provider.expect_fetch_daily_closes().times(0);

        let orchestrator = make_orchestrator(
            account_service,
            MockAssetServiceContract::new(),
            price_provider,
            repository,
            MockDailyFetchScheduler::new(),
            now_after_trigger(),
        );

        let run = orchestrator.run_scheduled_fetch().await.unwrap();
        assert_eq!(run.outcome, ScheduledFetchOutcome::SkippedAlreadyRun);
    }

    // SPF-022 — multiple missed trigger days coalesce into a single run that
    // settles only the latest pending trigger.
    #[tokio::test]
    async fn run_scheduled_fetch_catch_up_settles_only_the_latest_pending_trigger() {
        let mut repository = MockScheduledFetchRepository::new();
        repository.expect_get_configuration().returning(|| {
            Ok(ScheduledFetchConfiguration::restore(
                true,
                "22:15".to_string(),
            ))
        });
        repository.expect_last_successful_run().returning(|| {
            Ok(Some(ScheduledFetchRun::new(
                "2026-06-05T22:15:00".to_string(),
                "2026-06-05".to_string(),
                ScheduledFetchOutcome::Succeeded,
                3,
                0,
            )))
        });
        repository
            .expect_record_run()
            .times(1)
            .withf(|run| run.trigger_date == "2026-06-09")
            .returning(|_| Ok(()));

        let mut account_service = MockAccountServiceContract::new();
        account_service.expect_get_all().returning(|| Ok(vec![]));

        let orchestrator = make_orchestrator(
            account_service,
            MockAssetServiceContract::new(),
            MockPriceProvider::new(),
            repository,
            MockDailyFetchScheduler::new(),
            now_before_trigger(),
        );

        let run = orchestrator.run_scheduled_fetch().await.unwrap();
        assert_eq!(
            run.trigger_date, "2026-06-09",
            "a catch-up run settles only the latest pending trigger day (today's 22:15 has not passed yet at the fixed 'now')"
        );
    }

    // SPF-042 — an empty scope is a quiet success: Succeeded with 0 updates, 0 skips.
    #[tokio::test]
    async fn run_scheduled_fetch_empty_scope_is_a_quiet_success() {
        let mut repository = MockScheduledFetchRepository::new();
        repository.expect_get_configuration().returning(|| {
            Ok(ScheduledFetchConfiguration::restore(
                true,
                "22:15".to_string(),
            ))
        });
        repository
            .expect_last_successful_run()
            .returning(|| Ok(None));
        repository
            .expect_record_run()
            .times(1)
            .withf(|run| {
                run.outcome == ScheduledFetchOutcome::Succeeded
                    && run.updated_count == 0
                    && run.skipped_count == 0
            })
            .returning(|_| Ok(()));

        let mut account_service = MockAccountServiceContract::new();
        account_service.expect_get_all().returning(|| Ok(vec![]));

        let orchestrator = make_orchestrator(
            account_service,
            MockAssetServiceContract::new(),
            MockPriceProvider::new(),
            repository,
            MockDailyFetchScheduler::new(),
            now_after_trigger(),
        );

        let run = orchestrator.run_scheduled_fetch().await.unwrap();
        assert_eq!(run.outcome, ScheduledFetchOutcome::Succeeded);
        assert_eq!(run.updated_count, 0);
        assert_eq!(run.skipped_count, 0);
    }

    // SPF-050 — every path records a run, including a provider-outage run.
    #[tokio::test]
    async fn run_scheduled_fetch_always_records_a_run_even_on_total_failure() {
        let mut repository = MockScheduledFetchRepository::new();
        repository.expect_get_configuration().returning(|| {
            Ok(ScheduledFetchConfiguration::restore(
                true,
                "22:15".to_string(),
            ))
        });
        repository
            .expect_last_successful_run()
            .returning(|| Ok(None));
        repository
            .expect_record_run()
            .times(1)
            .returning(|_| Ok(()));

        let mut account_service = MockAccountServiceContract::new();
        account_service
            .expect_get_all()
            .returning(|| Err(crate::context::account::AccountError::DatabaseError));

        let orchestrator = make_orchestrator(
            account_service,
            MockAssetServiceContract::new(),
            MockPriceProvider::new(),
            repository,
            MockDailyFetchScheduler::new(),
            now_after_trigger(),
        );

        // Even a hard failure building the scope must still result in a
        // recorded run (SPF-050) — never an early return that skips recording.
        let _ = orchestrator.run_scheduled_fetch().await;
    }

    // SPF-051 — the provider being totally unreachable retries up to 3 attempts
    // before the run is recorded as Failed. start_paused auto-advances the
    // retry backoff sleeps so the test runs instantly.
    #[tokio::test(start_paused = true)]
    async fn run_scheduled_fetch_retries_three_times_then_records_failed() {
        let mut repository = MockScheduledFetchRepository::new();
        repository.expect_get_configuration().returning(|| {
            Ok(ScheduledFetchConfiguration::restore(
                true,
                "22:15".to_string(),
            ))
        });
        repository
            .expect_last_successful_run()
            .returning(|| Ok(None));
        repository
            .expect_record_run()
            .times(1)
            .withf(|run| run.outcome == ScheduledFetchOutcome::Failed)
            .returning(|_| Ok(()));

        let mut account_service = MockAccountServiceContract::new();
        account_service.expect_get_all().returning(|| {
            Ok(vec![crate::context::account::Account::restore(
                "acc-1".to_string(),
                "Portfolio".to_string(),
                String::new(),
                "USD".to_string(),
                false,
            )])
        });
        account_service
            .expect_get_holdings_for_account()
            .returning(|_| {
                Ok(vec![crate::context::account::Holding::restore(
                    "holding-1".to_string(),
                    "acc-1".to_string(),
                    "asset-1".to_string(),
                    crate::shared::domain::quantity::from_surface(10_000_000),
                    50_000_000,
                    0,
                    None,
                )])
            });

        // The scope build resolves the held asset before any provider call
        // (SPF-040) — without this the holding could never yield a symbol.
        let mut asset_service = MockAssetServiceContract::new();
        asset_service.expect_get_asset_by_id().returning(|_| {
            Ok(Some(crate::context::asset::Asset::restore(
                "asset-1".to_string(),
                crate::context::asset::AssetKind::Custom,
                "Test Asset".to_string(),
                crate::context::asset::AssetClass::Stocks,
                crate::context::asset::AssetCategory::from_storage(
                    crate::context::asset::SYSTEM_CATEGORY_ID.to_string(),
                    "generic.uncategorized".to_string(),
                ),
                "USD".to_string(),
                1,
                "AAPL".to_string(),
                None,
                false,
                None,
                false,
                false,
            )))
        });

        let mut price_provider = MockPriceProvider::new();
        price_provider
            .expect_fetch_daily_closes()
            .times(MAX_RETRY_ATTEMPTS as usize)
            .returning(|_, _, _| Err(anyhow::anyhow!("network unreachable")));

        let orchestrator = make_orchestrator(
            account_service,
            asset_service,
            price_provider,
            repository,
            MockDailyFetchScheduler::new(),
            now_after_trigger(),
        );

        let run = orchestrator.run_scheduled_fetch().await.unwrap();
        assert_eq!(run.outcome, ScheduledFetchOutcome::Failed);
    }

    // -------------------------------------------------------------------------
    // fetch log — MKT-201
    // -------------------------------------------------------------------------

    fn configured_repository(
        last_success: Option<ScheduledFetchRun>,
    ) -> MockScheduledFetchRepository {
        let mut repository = MockScheduledFetchRepository::new();
        repository.expect_get_configuration().returning(|| {
            Ok(ScheduledFetchConfiguration::restore(
                true,
                "22:15".to_string(),
            ))
        });
        repository
            .expect_last_successful_run()
            .returning(move || Ok(last_success.clone()));
        repository.expect_record_run().returning(|_| Ok(()));
        repository
    }

    // MKT-201 — a run that updated an asset is a fetch on this device, recorded at the
    // run's moment.
    #[tokio::test]
    async fn mkt_201_a_run_that_updated_an_asset_records_its_moment_in_the_fetch_log() {
        let mut account_service = MockAccountServiceContract::new();
        account_service.expect_get_all().returning(|| {
            Ok(vec![crate::context::account::Account::restore(
                "acc-1".to_string(),
                "Portfolio".to_string(),
                String::new(),
                "USD".to_string(),
                false,
            )])
        });
        account_service
            .expect_get_holdings_for_account()
            .returning(|_| {
                Ok(vec![crate::context::account::Holding::restore(
                    "holding-1".to_string(),
                    "acc-1".to_string(),
                    "asset-1".to_string(),
                    crate::shared::domain::quantity::from_surface(10_000_000),
                    50_000_000,
                    0,
                    None,
                )])
            });
        let mut asset_service = MockAssetServiceContract::new();
        asset_service.expect_get_asset_by_id().returning(|_| {
            Ok(Some(crate::context::asset::Asset::restore(
                "asset-1".to_string(),
                crate::context::asset::AssetKind::Custom,
                "Test Asset".to_string(),
                crate::context::asset::AssetClass::Stocks,
                crate::context::asset::AssetCategory::from_storage(
                    crate::context::asset::SYSTEM_CATEGORY_ID.to_string(),
                    "generic.uncategorized".to_string(),
                ),
                "USD".to_string(),
                1,
                "AAPL".to_string(),
                None,
                false,
                None,
                false,
                false,
            )))
        });
        asset_service
            .expect_record_daily_closes()
            .times(1)
            .returning(|_, closes| Ok(closes.len() as u32));
        let mut price_provider = MockPriceProvider::new();
        price_provider
            .expect_fetch_daily_closes()
            .returning(|_, _, _| {
                Ok(Some(vec![crate::context::asset::DatedClose {
                    date: "2026-06-10".to_string(),
                    price: 100_000_000,
                }]))
            });
        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo.expect_upsert_pair().returning(Ok);
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .returning(|| Ok(vec![]));
        let mut fetch_log = MockPriceFetchLogRepository::new();
        fetch_log
            .expect_record_completed()
            .times(1)
            .withf(|moment| *moment == now_after_trigger())
            .returning(|_| Ok(()));

        price_provider
            .expect_fetch_splits()
            .returning(|_, _| Ok(Vec::new()));
        asset_service
            .expect_record_reported_splits()
            .returning(|_, _, _| Ok(()));
        let orchestrator = ScheduledFetchOrchestrator::new(
            Arc::new(account_service),
            Arc::new(asset_service),
            Arc::new(price_provider),
            Arc::new(CurrencyService::new(
                Box::new(pair_repo),
                Box::new(MockCurrencyRateRepository::new()),
            )),
            Arc::new(configured_repository(None)),
            Arc::new(MockDailyFetchScheduler::new()),
            Arc::new(now_after_trigger),
        )
        .with_fetch_log(Arc::new(fetch_log));

        let run = orchestrator.run_scheduled_fetch().await.unwrap();
        assert_eq!(run.outcome, ScheduledFetchOutcome::Succeeded);
        assert_eq!(run.updated_count, 1);
    }

    /// MKT-230 — how often the splits of the asset EMPTYSPLIT were kept.
    static EMPTY_ANSWER_SPLITS_KEPT: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(0);

    /// An orchestrator over one account holding 10 units of each of `references` — and
    /// nothing any more of a `CLOSED` asset — whose provider answers a symbol by `answer`
    /// and whose price writes fail for the asset `asset-writefail`.
    fn sweeping_orchestrator(
        references: &'static [&'static str],
        answer: fn(&str) -> anyhow::Result<Option<Vec<crate::context::asset::DatedClose>>>,
    ) -> ScheduledFetchOrchestrator {
        let mut account_service = MockAccountServiceContract::new();
        account_service.expect_get_all().returning(|| {
            Ok(vec![crate::context::account::Account::restore(
                "acc-1".to_string(),
                "Portfolio".to_string(),
                String::new(),
                "USD".to_string(),
                false,
            )])
        });
        account_service
            .expect_get_holdings_for_account()
            .returning(move |_| {
                let holding = |reference: &str, quantity: i64| {
                    crate::context::account::Holding::restore(
                        format!("holding-{}", reference.to_lowercase()),
                        "acc-1".to_string(),
                        format!("asset-{}", reference.to_lowercase()),
                        crate::shared::domain::quantity::from_surface(quantity),
                        50_000_000,
                        0,
                        None,
                    )
                };
                let mut holdings: Vec<_> = references
                    .iter()
                    .map(|reference| holding(reference, 10_000_000))
                    .collect();
                holdings.push(holding("CLOSED", 0));
                Ok(holdings)
            });
        let mut asset_service = MockAssetServiceContract::new();
        asset_service
            .expect_get_asset_by_id()
            .returning(|asset_id| {
                Ok(Some(crate::context::asset::Asset::restore(
                    asset_id.to_string(),
                    crate::context::asset::AssetKind::Custom,
                    "Test Asset".to_string(),
                    crate::context::asset::AssetClass::Stocks,
                    crate::context::asset::AssetCategory::from_storage(
                        crate::context::asset::SYSTEM_CATEGORY_ID.to_string(),
                        "generic.uncategorized".to_string(),
                    ),
                    "USD".to_string(),
                    1,
                    asset_id.trim_start_matches("asset-").to_uppercase(),
                    None,
                    false,
                    None,
                    false,
                    false,
                )))
            });
        asset_service
            .expect_record_daily_closes()
            .returning(|asset_id, closes| {
                if asset_id == "asset-writefail" {
                    Err(crate::context::asset::AssetError::DatabaseError)
                } else {
                    Ok(closes.len() as u32)
                }
            });
        let mut price_provider = MockPriceProvider::new();
        price_provider
            .expect_fetch_daily_closes()
            .returning(move |symbol, _, _| answer(symbol));
        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo.expect_upsert_pair().returning(Ok);
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .returning(|| Ok(vec![]));
        // MKT-230 — the asset SPLITDOWN's split reading fails; the others report none.
        price_provider
            .expect_fetch_splits()
            .returning(|symbol, _| match symbol {
                "SPLITDOWN" => Err(anyhow::anyhow!("split request failed")),
                _ => Ok(Vec::new()),
            });
        // MKT-232 — keeping KEEPFAIL's splits fails. MKT-230 — EMPTYSPLIT's are counted.
        asset_service
            .expect_record_reported_splits()
            .returning(|asset_id, _, _| match asset_id {
                "asset-keepfail" => Err(crate::context::asset::AssetError::DatabaseError),
                "asset-emptysplit" => {
                    EMPTY_ANSWER_SPLITS_KEPT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                }
                _ => Ok(()),
            });
        ScheduledFetchOrchestrator::new(
            Arc::new(account_service),
            Arc::new(asset_service),
            Arc::new(price_provider),
            Arc::new(CurrencyService::new(
                Box::new(pair_repo),
                Box::new(MockCurrencyRateRepository::new()),
            )),
            Arc::new(configured_repository(None)),
            Arc::new(MockDailyFetchScheduler::new()),
            Arc::new(now_after_trigger),
        )
    }

    fn one_close() -> Vec<crate::context::asset::DatedClose> {
        vec![crate::context::asset::DatedClose {
            date: "2026-06-10".to_string(),
            price: 100_000_000,
        }]
    }

    // SPF-041 — a sweep counts one updated asset per asset whose closes were recorded,
    // and one skipped asset per asset with no data, an empty answer, a failed write or a
    // provider error (four of the rule's skip cases; a symbol that cannot be derived is
    // not exercised here). SPF-040 — a position no longer held is not asked for.
    #[tokio::test]
    async fn spf_041_a_sweep_counts_its_updated_and_its_skipped_assets() {
        let orchestrator = sweeping_orchestrator(
            &["UPDATED", "WRITEFAIL", "NODATA", "EMPTY", "DOWN"],
            |symbol| match symbol {
                "UPDATED" | "WRITEFAIL" => Ok(Some(one_close())),
                "NODATA" => Ok(None),
                "EMPTY" => Ok(Some(vec![])),
                "DOWN" => Err(anyhow::anyhow!("provider error")),
                other => panic!("{other} is not held and must not be asked for"),
            },
        );
        let run = orchestrator.run_scheduled_fetch().await.unwrap();
        assert_eq!(run.outcome, ScheduledFetchOutcome::Succeeded);
        assert_eq!(run.updated_count, 1);
        assert_eq!(run.skipped_count, 4);
    }

    // MKT-230 / MKT-193B (#529) — a failed split reading skips the asset as a failed close
    // request does: its closes are not recorded, the others' are.
    #[tokio::test]
    async fn mkt_230_a_failed_split_reading_skips_the_asset() {
        let orchestrator =
            sweeping_orchestrator(&["UPDATED", "SPLITDOWN"], |_| Ok(Some(one_close())));
        let run = orchestrator.run_scheduled_fetch().await.unwrap();
        assert_eq!(run.updated_count, 1);
        assert_eq!(run.skipped_count, 1);
    }

    // MKT-230 (#609) — an empty answer still reads and keeps the window's splits, and the
    // asset is skipped as an empty answer is.
    #[tokio::test]
    async fn mkt_230_an_empty_answer_still_keeps_the_splits() {
        let orchestrator = sweeping_orchestrator(&["EMPTYSPLIT"], |_| Ok(Some(vec![])));
        let run = orchestrator.run_scheduled_fetch().await.unwrap();
        assert_eq!(run.updated_count, 0);
        assert_eq!(run.skipped_count, 1);
        assert_eq!(
            EMPTY_ANSWER_SPLITS_KEPT.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }

    // MKT-230 (#609) — a failed split reading on an empty answer skips the asset as a failed
    // close request does; the others are priced.
    #[tokio::test]
    async fn mkt_230_a_failed_split_reading_on_an_empty_answer_skips_the_asset() {
        let orchestrator =
            sweeping_orchestrator(&["UPDATED", "SPLITDOWN"], |symbol| match symbol {
                "SPLITDOWN" => Ok(Some(vec![])),
                _ => Ok(Some(one_close())),
            });
        let run = orchestrator.run_scheduled_fetch().await.unwrap();
        assert_eq!(run.outcome, ScheduledFetchOutcome::Succeeded);
        assert_eq!(run.updated_count, 1);
        assert_eq!(run.skipped_count, 1);
    }

    // MKT-232 (#529) — a failure to keep the splits skips the asset as a failure to keep a
    // close does.
    #[tokio::test]
    async fn mkt_232_a_failure_to_keep_the_splits_skips_the_asset() {
        let orchestrator =
            sweeping_orchestrator(&["UPDATED", "KEEPFAIL"], |_| Ok(Some(one_close())));
        let run = orchestrator.run_scheduled_fetch().await.unwrap();
        assert_eq!(run.updated_count, 1);
        assert_eq!(run.skipped_count, 1);
    }

    // MKT-220 — a run records why each asset it could not price was left, and clears the
    // failure of each asset it priced; an empty answer (SPF-032/033) leaves the asset's
    // reason as it was; a run that fails records the provider as unreachable for every
    // asset, once, after its last attempt.
    #[tokio::test(start_paused = true)]
    async fn mkt_220_a_run_records_why_each_asset_was_not_priced() {
        use crate::use_cases::shared::price_fetch_failures::{
            MockPriceFetchFailureRepository, PriceFetchFailureReason,
        };
        use std::sync::Mutex;

        type Noted = Arc<Mutex<Vec<(String, Option<PriceFetchFailureReason>)>>>;
        fn noting(noted: &Noted) -> MockPriceFetchFailureRepository {
            let mut failures = MockPriceFetchFailureRepository::new();
            let into = Arc::clone(noted);
            failures
                .expect_record()
                .returning(move |asset_id, reason, _| {
                    into.lock()
                        .unwrap()
                        .push((asset_id.to_string(), Some(reason)));
                    Ok(())
                });
            let into = Arc::clone(noted);
            failures.expect_clear().returning(move |asset_id| {
                into.lock().unwrap().push((asset_id.to_string(), None));
                Ok(())
            });
            failures
        }

        let noted: Noted = Arc::default();
        sweeping_orchestrator(
            &["UPDATED", "WRITEFAIL", "NODATA", "EMPTY", "DOWN"],
            |symbol| match symbol {
                "UPDATED" | "WRITEFAIL" => Ok(Some(one_close())),
                "NODATA" => Ok(None),
                "EMPTY" => Ok(Some(vec![])),
                _ => Err(anyhow::anyhow!("provider error")),
            },
        )
        .with_fetch_failures(Arc::new(noting(&noted)))
        .run_scheduled_fetch()
        .await
        .unwrap();
        let mut seen = noted.lock().unwrap().clone();
        seen.sort_by_key(|(asset_id, _)| asset_id.clone());
        assert_eq!(
            seen,
            vec![
                (
                    "asset-down".to_string(),
                    Some(PriceFetchFailureReason::ProviderUnreachable)
                ),
                (
                    "asset-nodata".to_string(),
                    Some(PriceFetchFailureReason::NoData)
                ),
                ("asset-updated".to_string(), None),
                (
                    "asset-writefail".to_string(),
                    Some(PriceFetchFailureReason::NotSaved)
                ),
            ]
        );

        let noted: Noted = Arc::default();
        sweeping_orchestrator(&["DOWN"], |_| Err(anyhow::anyhow!("provider error")))
            .with_fetch_failures(Arc::new(noting(&noted)))
            .run_scheduled_fetch()
            .await
            .unwrap();
        assert_eq!(
            *noted.lock().unwrap(),
            vec![(
                "asset-down".to_string(),
                Some(PriceFetchFailureReason::ProviderUnreachable)
            )]
        );
    }

    // SPF-051 — when every asset fails, the sweep waits longer before each retry and the
    // run fails after the third attempt. The rule gives no figures: half a second, then
    // a second, are the code's.
    #[tokio::test(start_paused = true)]
    async fn spf_051_a_failing_sweep_waits_longer_before_each_retry() {
        let orchestrator =
            sweeping_orchestrator(&["DOWN"], |_| Err(anyhow::anyhow!("provider error")));
        let started = tokio::time::Instant::now();
        let run = orchestrator.run_scheduled_fetch().await.unwrap();
        assert_eq!(run.outcome, ScheduledFetchOutcome::Failed);
        assert_eq!(started.elapsed(), std::time::Duration::from_millis(1_500));
    }

    // MKT-201 — a run that updated nothing (nothing held, failed, or stopped by the
    // once-per-day guard) leaves the fetch log alone.
    #[tokio::test]
    async fn mkt_201_a_run_that_updated_nothing_leaves_the_fetch_log_alone() {
        let mut no_accounts = MockAccountServiceContract::new();
        no_accounts.expect_get_all().returning(|| Ok(vec![]));
        let mut untouched_log = MockPriceFetchLogRepository::new();
        untouched_log.expect_record_completed().times(0);
        let nothing_held = make_orchestrator(
            no_accounts,
            MockAssetServiceContract::new(),
            MockPriceProvider::new(),
            configured_repository(None),
            MockDailyFetchScheduler::new(),
            now_after_trigger(),
        )
        .with_fetch_log(Arc::new(untouched_log));
        assert_eq!(
            nothing_held.run_scheduled_fetch().await.unwrap().outcome,
            ScheduledFetchOutcome::Succeeded
        );

        let mut failing_accounts = MockAccountServiceContract::new();
        failing_accounts
            .expect_get_all()
            .returning(|| Err(crate::context::account::AccountError::DatabaseError));
        let mut untouched_log = MockPriceFetchLogRepository::new();
        untouched_log.expect_record_completed().times(0);
        let failed = make_orchestrator(
            failing_accounts,
            MockAssetServiceContract::new(),
            MockPriceProvider::new(),
            configured_repository(None),
            MockDailyFetchScheduler::new(),
            now_after_trigger(),
        )
        .with_fetch_log(Arc::new(untouched_log));
        assert_eq!(
            failed.run_scheduled_fetch().await.unwrap().outcome,
            ScheduledFetchOutcome::Failed
        );

        let already_run = ScheduledFetchRun::restore(
            "2026-06-10T22:15:04".to_string(),
            today().to_string(),
            ScheduledFetchOutcome::Succeeded,
            3,
            0,
        );
        let mut untouched_log = MockPriceFetchLogRepository::new();
        untouched_log.expect_record_completed().times(0);
        let skipped = make_orchestrator(
            MockAccountServiceContract::new(),
            MockAssetServiceContract::new(),
            MockPriceProvider::new(),
            configured_repository(Some(already_run)),
            MockDailyFetchScheduler::new(),
            now_after_trigger(),
        )
        .with_fetch_log(Arc::new(untouched_log));
        assert_eq!(
            skipped.run_scheduled_fetch().await.unwrap().outcome,
            ScheduledFetchOutcome::SkippedAlreadyRun
        );
    }
}
