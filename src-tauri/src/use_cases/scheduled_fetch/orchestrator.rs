//! Orchestrates the scheduled daily price download (SPF spec): configuring the
//! OS schedule (`configure`), reporting status (`status`), and running the
//! actual sweep (`run_scheduled_fetch`) — invisibly, once per day, with
//! catch-up and backfill.

use crate::context::account::AccountServiceContract;
use crate::context::asset::{AssetServiceContract, PriceProvider};
use crate::context::currency::CurrencyService;
use crate::shared::infrastructure::scheduler::DailyFetchScheduler;
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
        }
    }

    /// Records the moment of each run that updated at least one asset in this
    /// installation's fetch log (MKT-201).
    pub fn with_fetch_log(mut self, fetch_log: Arc<dyn PriceFetchLogRepository>) -> Self {
        self.fetch_log = Some(fetch_log);
        self
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
            for holding in holdings.into_iter().filter(|holding| holding.quantity > 0) {
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
        let fx_pairs =
            crate::use_cases::shared::scope::build_fx_pairs(fx_inputs, &currency_by_asset);
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
            for (asset, symbol) in scope {
                match self
                    .price_provider
                    .fetch_daily_closes(symbol, &from, &to)
                    .await
                {
                    Ok(Some(closes)) if !closes.is_empty() => {
                        match self
                            .asset_service
                            .record_daily_closes(&asset.id, closes)
                            .await
                        {
                            Ok(_) => updated_count += 1,
                            Err(error) => {
                                // SPF-041 — a write failure is a counted silent skip.
                                tracing::warn!(target: crate::core::logger::BACKEND, asset_id = %asset.id, err = ?error, "scheduled run: recording daily closes failed, asset skipped");
                                skipped_count += 1;
                            }
                        }
                    }
                    // SPF-041 — the provider has no data for this asset.
                    Ok(_) => skipped_count += 1,
                    Err(error) => {
                        tracing::warn!(target: crate::core::logger::BACKEND, symbol = %symbol, err = ?error, "scheduled run: daily-close fetch failed");
                        provider_errors += 1;
                    }
                }
            }
            // Every asset erroring means the provider is unreachable — retry
            // the whole sweep (SPF-051); a mixed outcome is per-asset skips.
            if provider_errors == scope.len() {
                if attempt >= MAX_RETRY_ATTEMPTS {
                    return (ScheduledFetchOutcome::Failed, 0, skipped_count);
                }
                tokio::time::sleep(std::time::Duration::from_millis(500 * u64::from(attempt)))
                    .await;
                continue;
            }
            skipped_count += provider_errors as u32;
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
        Some(date) => {
            let day_after = date + chrono::Duration::days(1);
            if day_after < capped_from {
                capped_from
            } else {
                day_after
            }
        }
        None => capped_from,
    };
    (from, today)
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
    use mockall::Sequence;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 6, 10).expect("valid date")
    }

    fn make_currency_service() -> Arc<CurrencyService> {
        Arc::new(CurrencyService::new(
            Box::new(MockCurrencyPairRepository::new()),
            Box::new(MockCurrencyRateRepository::new()),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn make_orchestrator(
        account_service: MockAccountServiceContract,
        asset_service: MockAssetServiceContract,
        price_provider: MockPriceProvider,
        repository: MockScheduledFetchRepository,
        scheduler: MockDailyFetchScheduler,
        now: NaiveDateTime,
    ) -> ScheduledFetchOrchestrator {
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
                crate::context::account::UpdateFrequency::Automatic,
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
                    10_000_000,
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
                crate::context::account::UpdateFrequency::Automatic,
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
                    10_000_000,
                    50_000_000,
                    0,
                    None,
                )])
            });
        let mut asset_service = MockAssetServiceContract::new();
        asset_service.expect_get_asset_by_id().returning(|_| {
            Ok(Some(crate::context::asset::Asset::restore(
                "asset-1".to_string(),
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
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .returning(|| Ok(vec![]));
        let mut fetch_log = MockPriceFetchLogRepository::new();
        fetch_log
            .expect_record_completed()
            .times(1)
            .withf(|moment| *moment == now_after_trigger())
            .returning(|_| Ok(()));

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
