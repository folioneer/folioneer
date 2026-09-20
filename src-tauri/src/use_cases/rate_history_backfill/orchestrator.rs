//! Orchestrates the user-triggered historical exchange-rate backfill
//! (FXR-110–114): anchors the range at the earliest transaction date across
//! all accounts (FXR-111) and delegates the dated-series fetch/write to the
//! currency service.

use std::sync::Arc;

use crate::context::account::AccountServiceContract;
use crate::context::currency::{CurrencyError, CurrencyService};
use crate::core::logger::BACKEND;

use super::error::RateHistoryBackfillError;

/// Orchestrates the historical rate backfill for the Currency Rates view.
pub struct RateHistoryBackfillUseCase {
    account_service: Arc<dyn AccountServiceContract>,
    currency_service: Arc<CurrencyService>,
}

impl RateHistoryBackfillUseCase {
    /// Creates a new use case.
    pub fn new(
        account_service: Arc<dyn AccountServiceContract>,
        currency_service: Arc<CurrencyService>,
    ) -> Self {
        Self {
            account_service,
            currency_service,
        }
    }

    /// Backfills dated daily rates for every persisted pair from the earliest
    /// transaction date across all accounts through today (FXR-111/112).
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

        self.currency_service
            .backfill_rates_range(&from, &to)
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
            make_currency_service(pair_repo, history_provider),
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
