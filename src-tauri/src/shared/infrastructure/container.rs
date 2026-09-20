//! Composition root for the service layer: boxes the SQLite repositories into
//! their owning application services so every process entry point (Tauri app,
//! headless scheduled fetch) shares one wiring path.

use std::sync::Arc;

use sqlx::{Pool, Sqlite};

use crate::context::account::{
    AccountService, SqliteAccountRepository, SqliteFeeCatchUpRepository,
    SqliteFeeScheduleRepository, SqliteHoldingNoteRepository, SqliteHoldingRepository,
    SqliteTransactionRepository,
};
use crate::context::asset::{
    AssetService, PriceProvider, SqliteAssetCategoryRepository, SqliteAssetPriceRepository,
    SqliteAssetRepository,
};
use crate::context::currency::{
    CurrencyService, RateHistoryProvider, RateProvider, SqliteCurrencyPairRepository,
    SqliteCurrencyRateRepository,
};
use crate::core::SideEffectEventBus;
use crate::shared::infrastructure::change_recorder::ChangeRecorder;

/// Fully wired service layer shared by every process entry point.
///
/// Owns construction only. Each entry point supplies its own pool, external
/// providers, and (optionally) the event bus, because those inputs differ per
/// entry point: the Tauri app attaches the bus and the rate-provider chain;
/// the headless scheduled fetch runs without a bus and attaches only the
/// rate-history provider. Use-case construction stays at the call sites,
/// consuming these fields.
pub struct AppContainer {
    /// Account management service (accounts, holdings, transactions, fee
    /// schedules, holding notes).
    pub account_service: Arc<AccountService>,
    /// Unified asset management service (assets, categories, prices).
    pub asset_service: Arc<AssetService>,
    /// Currency pair and rate service.
    pub currency_service: Arc<CurrencyService>,
    /// External price quote source supplied by the entry point.
    pub price_provider: Arc<dyn PriceProvider>,
}

impl AppContainer {
    /// Boxes the SQLite repositories into their application services.
    ///
    /// `event_bus` is attached to all three services when present; headless
    /// entry points pass `None` so no side-effect events are ever published.
    /// `rate_provider` and `rate_history_provider` are attached when present —
    /// the caller decides which external currency tiers its flow needs.
    /// `change_recorder` is wired into every repository that writes a synced
    /// record (SYN-020, D1).
    pub fn build(
        pool: Pool<Sqlite>,
        price_provider: Arc<dyn PriceProvider>,
        rate_provider: Option<Arc<dyn RateProvider>>,
        rate_history_provider: Option<Arc<dyn RateHistoryProvider>>,
        event_bus: Option<Arc<SideEffectEventBus>>,
        change_recorder: Arc<dyn ChangeRecorder>,
    ) -> Self {
        let mut asset_service = AssetService::new(
            Box::new(
                SqliteAssetRepository::new(pool.clone())
                    .with_change_recorder(Arc::clone(&change_recorder)),
            ),
            Box::new(
                SqliteAssetCategoryRepository::new(pool.clone())
                    .with_change_recorder(Arc::clone(&change_recorder)),
            ),
            Box::new(
                SqliteAssetPriceRepository::new(pool.clone())
                    .with_change_recorder(Arc::clone(&change_recorder)),
            ),
        );

        let mut account_service = AccountService::new(
            Box::new(
                SqliteAccountRepository::new(pool.clone())
                    .with_change_recorder(Arc::clone(&change_recorder)),
            ),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(
                SqliteTransactionRepository::new(pool.clone())
                    .with_change_recorder(Arc::clone(&change_recorder)),
            ),
        )
        .with_fee_schedule_repo(Box::new(
            SqliteFeeScheduleRepository::new(pool.clone())
                .with_change_recorder(Arc::clone(&change_recorder)),
        ))
        .with_fee_catch_up_repo(Box::new(
            SqliteFeeCatchUpRepository::new(pool.clone())
                .with_change_recorder(Arc::clone(&change_recorder)),
        ))
        .with_holding_note_repo(Box::new(
            SqliteHoldingNoteRepository::new(pool.clone())
                .with_change_recorder(Arc::clone(&change_recorder)),
        ));

        let mut currency_service = CurrencyService::new(
            Box::new(
                SqliteCurrencyPairRepository::new(pool.clone())
                    .with_change_recorder(Arc::clone(&change_recorder)),
            ),
            Box::new(SqliteCurrencyRateRepository::new(pool).with_change_recorder(change_recorder)),
        );

        if let Some(event_bus) = &event_bus {
            asset_service = asset_service.with_event_bus(Arc::clone(event_bus));
            account_service = account_service.with_event_bus(Arc::clone(event_bus));
            currency_service = currency_service.with_event_bus(Arc::clone(event_bus));
        }
        if let Some(rate_provider) = rate_provider {
            currency_service = currency_service.with_rate_provider(rate_provider);
        }
        if let Some(rate_history_provider) = rate_history_provider {
            currency_service = currency_service.with_rate_history_provider(rate_history_provider);
        }

        Self {
            account_service: Arc::new(account_service),
            asset_service: Arc::new(asset_service),
            currency_service: Arc::new(currency_service),
            price_provider,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::asset::MockPriceProvider;
    use crate::context::currency::domain::rate_provider::{
        MockRateHistoryProvider, MockRateProvider,
    };
    use crate::shared::infrastructure::change_recorder::NoopChangeRecorder;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn make_pool() -> Pool<Sqlite> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("test pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations");
        pool
    }

    // Headless shape: no event bus, no rate provider — the wired services must
    // still reach the database.
    #[tokio::test]
    async fn build_without_optional_dependencies_wires_functional_services() {
        let pool = make_pool().await;
        let container = AppContainer::build(
            pool,
            Arc::new(MockPriceProvider::new()) as Arc<dyn PriceProvider>,
            None,
            None,
            None,
            Arc::new(NoopChangeRecorder),
        );

        let accounts = container
            .account_service
            .get_all()
            .await
            .expect("account service reaches the database");
        assert!(accounts.is_empty());
    }

    // App shape: the event bus must be cloned into each of the three services.
    #[tokio::test]
    async fn build_attaches_event_bus_to_all_three_services() {
        let pool = make_pool().await;
        let event_bus = Arc::new(SideEffectEventBus::new());

        let _container = AppContainer::build(
            pool,
            Arc::new(MockPriceProvider::new()) as Arc<dyn PriceProvider>,
            Some(Arc::new(MockRateProvider::new()) as Arc<dyn RateProvider>),
            Some(Arc::new(MockRateHistoryProvider::new()) as Arc<dyn RateHistoryProvider>),
            Some(Arc::clone(&event_bus)),
            Arc::new(NoopChangeRecorder),
        );

        // One clone per service (asset, account, currency) plus this handle.
        assert_eq!(Arc::strong_count(&event_bus), 4);
    }

    // D1 — `AppContainer::build` must accept a `ChangeRecorder` and wire it into every
    // synced repository; a `NoopChangeRecorder` (SYN-010: sync never enabled) must be
    // accepted without requiring the sync bounded context to be otherwise configured.
    #[tokio::test]
    async fn build_accepts_a_noop_change_recorder() {
        use crate::shared::infrastructure::change_recorder::{ChangeRecorder, NoopChangeRecorder};

        let pool = make_pool().await;
        let _container = AppContainer::build(
            pool,
            Arc::new(MockPriceProvider::new()) as Arc<dyn PriceProvider>,
            None,
            None,
            None,
            Arc::new(NoopChangeRecorder) as Arc<dyn ChangeRecorder>,
        );
    }
}
