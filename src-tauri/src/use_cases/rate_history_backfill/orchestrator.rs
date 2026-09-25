//! Orchestrates the user-triggered historical exchange-rate backfill
//! (FXR-110–114): anchors the range at the earliest transaction date across
//! all accounts (FXR-111) and delegates the dated-series fetch/write to the
//! currency service.

use std::sync::Arc;

use crate::context::account::AccountServiceContract;
use crate::context::asset::AssetServiceContract;
use crate::context::currency::{CurrencyError, CurrencyService};
use crate::core::logger::BACKEND;
use crate::use_cases::shared::scope::holding_fx_pairs;

use super::error::RateHistoryBackfillError;

/// Orchestrates the historical rate backfill for the Currency Rates view.
pub struct RateHistoryBackfillUseCase {
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
    currency_service: Arc<CurrencyService>,
}

impl RateHistoryBackfillUseCase {
    /// Creates a new use case.
    pub fn new(
        account_service: Arc<dyn AccountServiceContract>,
        asset_service: Arc<dyn AssetServiceContract>,
        currency_service: Arc<CurrencyService>,
    ) -> Self {
        Self {
            account_service,
            asset_service,
            currency_service,
        }
    }

    /// Follows the pairs of active foreign holdings, then backfills dated daily rates
    /// for every persisted pair from the earliest transaction date across all
    /// accounts through today (FXR-111/112).
    /// Returns the number of rate rows written; zero when there is nothing to
    /// anchor the range on (FXR-111).
    pub async fn backfill(&self) -> Result<u32, RateHistoryBackfillError> {
        let accounts = self.account_service.get_all().await.map_err(|error| {
            tracing::error!(target: BACKEND, err = ?error, "rate backfill: account listing failed");
            RateHistoryBackfillError::DatabaseError
        })?;

        let mut earliest_date: Option<String> = None;
        for account in accounts {
            let transactions = self
                .account_service
                .get_all_transactions_for_account(&account.id)
                .await
                .map_err(|error| {
                    tracing::error!(target: BACKEND, err = ?error, "rate backfill: transaction listing failed");
                    RateHistoryBackfillError::DatabaseError
                })?;
            for transaction in transactions {
                // ISO dates order lexically, so a plain min works.
                if earliest_date
                    .as_deref()
                    .is_none_or(|current| transaction.date.as_str() < current)
                {
                    earliest_date = Some(transaction.date.clone());
                }
            }
        }

        let Some(from) = earliest_date else {
            return Ok(0);
        };
        let to = chrono::Local::now().date_naive().to_string();
        // FXR-112 — follow the pairs of active foreign holdings before fetching.
        let pairs = holding_fx_pairs(self.account_service.as_ref(), self.asset_service.as_ref())
            .await
            .map_err(|error| {
                tracing::error!(target: BACKEND, err = ?error, "rate backfill: holding lookup failed");
                RateHistoryBackfillError::DatabaseError
            })?;

        self.currency_service
            .backfill_rates_range(pairs, &from, &to)
            .await
            .map_err(|error| match error {
                CurrencyError::ProviderUnreachable => RateHistoryBackfillError::ProviderUnreachable,
                other => {
                    tracing::error!(target: BACKEND, err = ?other, "rate backfill: range write failed");
                    RateHistoryBackfillError::DatabaseError
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::{
        Account, AccountError, MockAccountServiceContract, Transaction, TransactionType,
        UpdateFrequency,
    };
    use crate::context::asset::{
        Asset, AssetCategory, AssetClass, MockAssetServiceContract, SYSTEM_CATEGORY_ID,
    };
    use crate::context::currency::domain::rate_provider::MockRateHistoryProvider;
    use crate::context::currency::domain::{
        MockCurrencyPairRepository, MockCurrencyRateRepository,
    };
    use crate::context::currency::RateHistoryProvider;

    fn make_account(id: &str) -> Account {
        Account::restore(
            id.to_string(),
            "Portfolio".to_string(),
            String::new(),
            "EUR".to_string(),
            UpdateFrequency::Automatic,
            false,
        )
    }

    fn make_transaction(account_id: &str, date: &str) -> Transaction {
        Transaction::restore(
            format!("tx-{account_id}-{date}"),
            account_id.to_string(),
            "asset-1".to_string(),
            TransactionType::Purchase,
            date.to_string(),
            1_000_000,
            50_000_000,
            1_000_000,
            0,
            50_000_000,
            None,
            None,
            format!("{date}T10:00:00"),
        )
    }

    /// An asset lookup never consulted: the accounts hold nothing.
    fn no_holding_lookup() -> MockAssetServiceContract {
        MockAssetServiceContract::new()
    }

    fn make_currency_service(
        pair_repo: MockCurrencyPairRepository,
        history_provider: MockRateHistoryProvider,
    ) -> Arc<CurrencyService> {
        Arc::new(
            CurrencyService::new(
                Box::new(pair_repo),
                Box::new(MockCurrencyRateRepository::new()),
            )
            .with_rate_history_provider(Arc::new(history_provider) as Arc<dyn RateHistoryProvider>),
        )
    }

    // FXR-111 — with no transaction anywhere there is nothing to anchor the
    // range on: quiet zero, the currency service is never consulted.
    #[tokio::test]
    async fn backfill_without_any_transaction_is_a_quiet_zero() {
        let mut account_service = MockAccountServiceContract::new();
        account_service
            .expect_get_all()
            .returning(|| Ok(vec![make_account("acc-1")]));
        account_service
            .expect_get_all_transactions_for_account()
            .returning(|_| Ok(vec![]));

        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo.expect_list_pairs_with_latest_rate().times(0);
        let mut history_provider = MockRateHistoryProvider::new();
        history_provider.expect_fetch_eur_range().times(0);

        let use_case = RateHistoryBackfillUseCase::new(
            Arc::new(account_service),
            Arc::new(no_holding_lookup()),
            make_currency_service(pair_repo, history_provider),
        );

        assert_eq!(use_case.backfill().await.unwrap(), 0);
    }

    // FXR-111 — the range anchors at the EARLIEST transaction date across ALL
    // accounts, through today.
    #[tokio::test]
    async fn backfill_anchors_at_the_earliest_transaction_across_accounts() {
        let mut account_service = MockAccountServiceContract::new();
        account_service
            .expect_get_all()
            .returning(|| Ok(vec![make_account("acc-1"), make_account("acc-2")]));
        account_service
            .expect_get_all_transactions_for_account()
            .returning(|account_id| {
                Ok(match account_id {
                    "acc-1" => vec![make_transaction("acc-1", "2021-06-15")],
                    _ => vec![
                        make_transaction("acc-2", "2019-03-05"),
                        make_transaction("acc-2", "2024-01-10"),
                    ],
                })
            });
        account_service
            .expect_get_holdings_for_account()
            .returning(|_| Ok(vec![]));

        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .times(1)
            .returning(|| {
                Ok(vec![crate::context::currency::CurrencyPairSummary {
                    from_currency: "USD".to_string(),
                    to_currency: "EUR".to_string(),
                    latest_rate: None,
                    latest_rate_date: None,
                    latest_rate_source: None,
                }])
            });
        let today = chrono::Local::now().date_naive().to_string();
        let mut history_provider = MockRateHistoryProvider::new();
        history_provider
            .expect_fetch_eur_range()
            .times(1)
            .withf(move |from, to| from == "2019-03-05" && to == today)
            .returning(|_, _| Ok(vec![]));

        let use_case = RateHistoryBackfillUseCase::new(
            Arc::new(account_service),
            Arc::new(no_holding_lookup()),
            make_currency_service(pair_repo, history_provider),
        );

        assert_eq!(use_case.backfill().await.unwrap(), 0);
    }

    // FXR-112 — "Update rates" first follows the pair of every active foreign holding,
    // so a USD asset bought since the last launch is covered at once.
    #[tokio::test]
    async fn backfill_follows_the_pairs_of_active_foreign_holdings_first() {
        let mut account_service = MockAccountServiceContract::new();
        account_service
            .expect_get_all()
            .returning(|| Ok(vec![make_account("acc-1")]));
        account_service
            .expect_get_all_transactions_for_account()
            .returning(|account_id| Ok(vec![make_transaction(account_id, "2024-01-10")]));
        account_service
            .expect_get_holdings_for_account()
            .returning(|account_id| {
                Ok(vec![crate::context::account::Holding::restore(
                    "holding-1".to_string(),
                    account_id.to_string(),
                    "asset-usd".to_string(),
                    1_000_000,
                    100_000_000,
                    0,
                    None,
                )])
            });
        let mut asset_service = MockAssetServiceContract::new();
        asset_service
            .expect_get_asset_by_id()
            .returning(|asset_id| {
                Ok(Some(Asset::restore(
                    asset_id.to_string(),
                    "Apple".to_string(),
                    AssetClass::Stocks,
                    AssetCategory::from_storage(
                        SYSTEM_CATEGORY_ID.to_string(),
                        "generic.uncategorized".to_string(),
                    ),
                    "USD".to_string(),
                    4,
                    "AAPL".to_string(),
                    None,
                    false,
                    None,
                    false,
                    false,
                )))
            });

        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo
            .expect_upsert_pair()
            .times(1)
            .withf(|pair| pair.from_currency == "USD" && pair.to_currency == "EUR")
            .returning(Ok);
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .returning(|| Ok(vec![]));

        let use_case = RateHistoryBackfillUseCase::new(
            Arc::new(account_service),
            Arc::new(asset_service),
            make_currency_service(pair_repo, MockRateHistoryProvider::new()),
        );

        assert_eq!(use_case.backfill().await.unwrap(), 0);
    }

    // A hard account-listing failure surfaces as DatabaseError.
    #[tokio::test]
    async fn backfill_surfaces_account_listing_failure() {
        let mut account_service = MockAccountServiceContract::new();
        account_service
            .expect_get_all()
            .returning(|| Err(AccountError::DatabaseError));

        let use_case = RateHistoryBackfillUseCase::new(
            Arc::new(account_service),
            Arc::new(no_holding_lookup()),
            make_currency_service(
                MockCurrencyPairRepository::new(),
                MockRateHistoryProvider::new(),
            ),
        );

        let error = use_case.backfill().await.unwrap_err();
        assert!(
            matches!(error, RateHistoryBackfillError::DatabaseError),
            "got: {error:?}"
        );
    }
}
