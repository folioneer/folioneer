//! Price Movement baseline capture (PMV spec) — the **stateful** half of the
//! feature: the only piece that talks to a repository. Lives with its only
//! caller (`Dispatcher`), not in `use_cases::shared` — `shared/` is stateless
//! free functions (B18 forbids importing another use case; it does not
//! mandate `shared/`).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::NaiveDate;

use crate::context::account::{AccountServiceContract, Holding};
use crate::context::asset::{AssetClass, AssetPrice, AssetServiceContract};
use crate::context::currency::CurrencyService;
use crate::core::logger::BACKEND;
use crate::use_cases::shared::global_value::{
    account_global_value, AssetValuationFacts, ValuationSnapshot, REFERENCE_CURRENCY,
};
use crate::use_cases::shared::price_movement::PriceMovementBaseline;

/// Captures the "before" snapshot at refresh start (PMV-020/021): every
/// account's holdings, the prices and rates they carry right now, and each
/// account's Global Value computed over that frozen snapshot.
pub struct PriceMovementCapture {
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
    currency_service: Arc<CurrencyService>,
}

impl PriceMovementCapture {
    /// Creates a new capture instance.
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

    /// PMV-020/021/032/050 — builds the "before" snapshot for every account,
    /// over `scope_asset_ids` (the assets this fetch is about to attempt), as
    /// of `today`. Returns `None` on any lookup failure — including a holding
    /// referencing a missing asset — after logging via `tracing::warn!`
    /// (PMV-014: the fetch itself is never aborted or altered by reporting).
    /// This is where PMV-020's frozen rates are threaded: every rate the two
    /// readings will ever need is resolved once here, before the per-asset
    /// fetch loop, so the FX refresh piggybacked after it (FXR-075) can never
    /// leak into either reading.
    pub async fn capture(
        &self,
        scope_asset_ids: &HashSet<String>,
        today: NaiveDate,
    ) -> Option<PriceMovementBaseline> {
        let today_str = today.format("%Y-%m-%d").to_string();
        let accounts = match self.account_service.get_all().await {
            Ok(accounts) => accounts,
            Err(error) => {
                tracing::warn!(target: BACKEND, err = ?error, "price_movement: account load failed; no report (PMV-014)");
                return None;
            }
        };

        let mut holdings_by_account: HashMap<String, Vec<Holding>> = HashMap::new();
        let mut assets: HashMap<String, AssetValuationFacts> = HashMap::new();
        let mut prices: HashMap<String, AssetPrice> = HashMap::new();
        let mut rates: HashMap<(String, String), i64> = HashMap::new();
        let mut rate_missing_accounts: HashSet<String> = HashSet::new();
        let mut observed_from: Option<String> = None;

        for account in &accounts {
            let holdings = match self
                .account_service
                .get_holdings_for_account(&account.id)
                .await
            {
                Ok(holdings) => holdings,
                Err(error) => {
                    tracing::warn!(target: BACKEND, account_id = %account.id, err = ?error, "price_movement: holdings load failed; no report (PMV-014)");
                    return None;
                }
            };

            // PMV-040/042 — the account -> reference-currency leg, resolved here so
            // the FX refresh piggybacked after the fetch loop (FXR-075) cannot move it.
            let reference_key = (account.currency.clone(), REFERENCE_CURRENCY.to_string());
            if let std::collections::hash_map::Entry::Vacant(e) = rates.entry(reference_key) {
                match self
                    .currency_service
                    .resolve_rate_micros(&account.currency, REFERENCE_CURRENCY, &today_str)
                    .await
                {
                    Ok(Some(rate)) => {
                        e.insert(rate);
                    }
                    Ok(None) => {
                        rate_missing_accounts.insert(account.id.clone());
                    }
                    Err(error) => {
                        tracing::warn!(target: BACKEND, account_id = %account.id, err = ?error, "price_movement: reference rate lookup failed; no report (PMV-014)");
                        return None;
                    }
                }
            }

            for holding in holdings.iter().filter(|holding| holding.quantity > 0) {
                let asset = match self.asset_service.get_asset_by_id(&holding.asset_id).await {
                    Ok(Some(asset)) => asset,
                    Ok(None) => {
                        tracing::warn!(target: BACKEND, asset_id = %holding.asset_id, "price_movement: holding references missing asset; no report (PMV-014)");
                        return None;
                    }
                    Err(error) => {
                        tracing::warn!(target: BACKEND, asset_id = %holding.asset_id, err = ?error, "price_movement: asset load failed; no report (PMV-014)");
                        return None;
                    }
                };

                if asset.class != AssetClass::Cash {
                    // PMV-020 — the holding -> account-currency leg, frozen alongside it.
                    let holding_key = (asset.currency.clone(), account.currency.clone());
                    if let std::collections::hash_map::Entry::Vacant(e) = rates.entry(holding_key) {
                        match self
                            .currency_service
                            .resolve_rate_micros(&asset.currency, &account.currency, &today_str)
                            .await
                        {
                            Ok(Some(rate)) => {
                                e.insert(rate);
                            }
                            Ok(None) => {
                                rate_missing_accounts.insert(account.id.clone());
                            }
                            Err(error) => {
                                tracing::warn!(target: BACKEND, asset_id = %holding.asset_id, err = ?error, "price_movement: rate lookup failed; no report (PMV-014)");
                                return None;
                            }
                        }
                    }

                    if let Ok(Some(latest)) =
                        self.asset_service.get_latest_price(&holding.asset_id).await
                    {
                        // PMV-050 — the earlier observation date spans the assets this
                        // fetch is about to attempt, not every holding.
                        if scope_asset_ids.contains(&holding.asset_id)
                            && observed_from
                                .as_deref()
                                .is_none_or(|from| latest.date.as_str() > from)
                        {
                            observed_from = Some(latest.date.clone());
                        }
                        prices.insert(holding.asset_id.clone(), latest);
                    }
                }

                assets
                    .entry(holding.asset_id.clone())
                    .or_insert_with(|| AssetValuationFacts {
                        currency: asset.currency.clone(),
                        class: asset.class,
                    });
            }

            holdings_by_account.insert(account.id.clone(), holdings);
        }

        let snapshot = ValuationSnapshot {
            assets,
            prices,
            rates,
        };
        // PMV-021 — each account's Global Value over the frozen snapshot.
        let before_by_account = accounts
            .iter()
            .map(|account| {
                let holdings = holdings_by_account
                    .get(&account.id)
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                (
                    account.id.clone(),
                    account_global_value(&account.currency, holdings, &snapshot),
                )
            })
            .collect();

        Some(PriceMovementBaseline {
            accounts,
            holdings_by_account,
            snapshot,
            before_by_account,
            observed_from,
            rate_missing_accounts,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::MockAccountServiceContract;
    use crate::context::asset::MockAssetServiceContract;
    use crate::context::currency::{MockCurrencyPairRepository, MockCurrencyRateRepository};

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 9, 11).expect("valid date")
    }

    fn make_currency_service() -> Arc<CurrencyService> {
        Arc::new(CurrencyService::new(
            Box::new(MockCurrencyPairRepository::new()),
            Box::new(MockCurrencyRateRepository::new()),
        ))
    }

    // PMV-014 — a failure loading accounts degrades to None; the fetch itself
    // is never aborted or altered by reporting.
    #[tokio::test]
    async fn capture_returns_none_when_the_account_load_fails() {
        let mut account_service = MockAccountServiceContract::new();
        account_service
            .expect_get_all()
            .returning(|| Err(crate::context::account::AccountError::DatabaseError));
        let asset_service = MockAssetServiceContract::new();

        let capture = PriceMovementCapture::new(
            Arc::new(account_service),
            Arc::new(asset_service),
            make_currency_service(),
        );

        let baseline = capture.capture(&HashSet::new(), today()).await;

        assert!(
            baseline.is_none(),
            "an account-load failure must degrade to None (PMV-014)"
        );
    }

    // PMV-014 — a holding referencing a missing asset degrades to None rather
    // than aborting the fetch (mirrors AccountSummaryUseCase::compute_global_value's
    // DatabaseError case, translated to a silent degradation here).
    #[tokio::test]
    async fn capture_returns_none_when_a_holding_references_a_missing_asset() {
        let mut account_service = MockAccountServiceContract::new();
        account_service.expect_get_all().returning(|| {
            Ok(vec![crate::context::account::Account::restore(
                "acc-1".to_string(),
                "Alpha".to_string(),
                String::new(),
                "EUR".to_string(),
                crate::context::account::UpdateFrequency::ManualMonth,
                false,
            )])
        });
        account_service
            .expect_get_holdings_for_account()
            .returning(|_| {
                Ok(vec![crate::context::account::Holding::restore(
                    "holding-1".to_string(),
                    "acc-1".to_string(),
                    "ghost-asset".to_string(),
                    1_000_000,
                    0,
                    0,
                    None,
                )])
            });
        let mut asset_service = MockAssetServiceContract::new();
        asset_service
            .expect_get_asset_by_id()
            .returning(|_| Ok(None));

        let capture = PriceMovementCapture::new(
            Arc::new(account_service),
            Arc::new(asset_service),
            make_currency_service(),
        );

        let baseline = capture.capture(&HashSet::new(), today()).await;

        assert!(baseline.is_none());
    }
    // PMV-014 — a failure loading an account's holdings degrades to None. Without
    // this the fetch would still run, but the report would silently describe a
    // portfolio missing an account's positions.
    #[tokio::test]
    async fn capture_returns_none_when_the_holdings_load_fails() {
        let mut account_service = MockAccountServiceContract::new();
        account_service.expect_get_all().returning(|| {
            Ok(vec![crate::context::account::Account::restore(
                "acc-1".to_string(),
                "Alpha".to_string(),
                String::new(),
                "EUR".to_string(),
                crate::context::account::UpdateFrequency::ManualMonth,
                false,
            )])
        });
        account_service
            .expect_get_holdings_for_account()
            .returning(|_| Err(crate::context::account::AccountError::DatabaseError));
        let asset_service = MockAssetServiceContract::new();

        let capture = PriceMovementCapture::new(
            Arc::new(account_service),
            Arc::new(asset_service),
            make_currency_service(),
        );

        let baseline = capture.capture(&HashSet::new(), today()).await;

        assert!(
            baseline.is_none(),
            "a holdings-load failure must degrade to None (PMV-014)"
        );
    }

    // PMV-014 — a failure loading an asset degrades to None rather than valuing the
    // holding at zero, which would report a movement that never happened.
    #[tokio::test]
    async fn capture_returns_none_when_the_asset_load_errors() {
        let mut account_service = MockAccountServiceContract::new();
        account_service.expect_get_all().returning(|| {
            Ok(vec![crate::context::account::Account::restore(
                "acc-1".to_string(),
                "Alpha".to_string(),
                String::new(),
                "EUR".to_string(),
                crate::context::account::UpdateFrequency::ManualMonth,
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
                    1_000_000,
                    0,
                    0,
                    None,
                )])
            });
        let mut asset_service = MockAssetServiceContract::new();
        asset_service
            .expect_get_asset_by_id()
            .returning(|_| Err(crate::context::asset::AssetError::DatabaseError));

        let capture = PriceMovementCapture::new(
            Arc::new(account_service),
            Arc::new(asset_service),
            make_currency_service(),
        );

        let baseline = capture.capture(&HashSet::new(), today()).await;

        assert!(
            baseline.is_none(),
            "an asset-load failure must degrade to None (PMV-014)"
        );
    }
    // PMV-014 — a currency-service failure on the ACCOUNT -> reference-currency
    // leg degrades to None. The account is deliberately non-EUR: an EUR account
    // short-circuits the identity pair (FXR-011) without touching the repository,
    // which is why the other tests in this module never reach this arm.
    #[tokio::test]
    async fn capture_returns_none_when_the_reference_rate_lookup_errors() {
        let mut account_service = MockAccountServiceContract::new();
        account_service.expect_get_all().returning(|| {
            Ok(vec![crate::context::account::Account::restore(
                "acc-1".to_string(),
                "Alpha".to_string(),
                String::new(),
                "USD".to_string(),
                crate::context::account::UpdateFrequency::ManualMonth,
                false,
            )])
        });
        account_service
            .expect_get_holdings_for_account()
            .returning(|_| Ok(vec![]));
        let asset_service = MockAssetServiceContract::new();

        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_latest_rate_on_or_before()
            .returning(|_, _, _| Err(anyhow::anyhow!("rate lookup failed")));
        let currency_service = Arc::new(CurrencyService::new(
            Box::new(MockCurrencyPairRepository::new()),
            Box::new(rate_repo),
        ));

        let capture = PriceMovementCapture::new(
            Arc::new(account_service),
            Arc::new(asset_service),
            currency_service,
        );

        let baseline = capture.capture(&HashSet::new(), today()).await;

        assert!(
            baseline.is_none(),
            "a reference-rate lookup failure must degrade to None (PMV-014)"
        );
    }

    // PMV-014 — the same on the HOLDING -> account-currency leg. A missing rate
    // is not an error (FXR-034 degrades it to 0 and marks the row incomplete);
    // a failing lookup is, and it must not produce a half-built report.
    #[tokio::test]
    async fn capture_returns_none_when_the_holding_rate_lookup_errors() {
        let mut account_service = MockAccountServiceContract::new();
        account_service.expect_get_all().returning(|| {
            Ok(vec![crate::context::account::Account::restore(
                "acc-1".to_string(),
                "Alpha".to_string(),
                String::new(),
                "EUR".to_string(),
                crate::context::account::UpdateFrequency::ManualMonth,
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
                    1_000_000,
                    0,
                    0,
                    None,
                )])
            });
        let mut asset_service = MockAssetServiceContract::new();
        asset_service.expect_get_asset_by_id().returning(|_| {
            Ok(Some(crate::context::asset::Asset::restore(
                "asset-1".to_string(),
                "Foreign Stock".to_string(),
                crate::context::asset::AssetClass::Stocks,
                crate::context::asset::AssetCategory::from_storage(
                    "cat-1".to_string(),
                    "Equities".to_string(),
                ),
                "USD".to_string(),
                1,
                "FGN".to_string(),
                None,
                false,
                None,
                false,
                false,
            )))
        });

        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_latest_rate_on_or_before()
            .returning(|_, _, _| Err(anyhow::anyhow!("rate lookup failed")));
        let currency_service = Arc::new(CurrencyService::new(
            Box::new(MockCurrencyPairRepository::new()),
            Box::new(rate_repo),
        ));

        let capture = PriceMovementCapture::new(
            Arc::new(account_service),
            Arc::new(asset_service),
            currency_service,
        );

        let baseline = capture.capture(&HashSet::new(), today()).await;

        assert!(
            baseline.is_none(),
            "a holding-rate lookup failure must degrade to None (PMV-014)"
        );
    }
}
