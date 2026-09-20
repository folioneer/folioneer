//! Orchestrates the price history backfill of one holding (MKT-190–199): resolves
//! the held period from the account's transactions, fetches the provider's daily
//! closes in consecutive windows, and hands them to the asset service, which
//! records them on the dates that carry no price.

use std::sync::Arc;

use chrono::{Duration, NaiveDate};

use crate::context::account::{AccountError, AccountServiceContract};
use crate::context::asset::{
    derive_yahoo_symbol_with_exchange, AssetError, AssetServiceContract, DatedClose,
    PriceHistoryBackfillOutcome, PriceProvider,
};
use crate::core::cash::is_cash_asset;
use crate::core::logger::BACKEND;

use super::error::{PriceHistoryBackfillError, PriceHistoryBackfillTask};

/// Longest window requested at once (MKT-193).
const WINDOW_DAYS: i64 = 365;

/// Supplies today's local date; injected so the held period's end is testable.
pub type Today = Arc<dyn Fn() -> NaiveDate + Send + Sync>;

/// Orchestrates the price history backfill behind the holding-row action.
pub struct PriceHistoryBackfillUseCase {
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
    price_provider: Arc<dyn PriceProvider>,
    today: Today,
}

impl PriceHistoryBackfillUseCase {
    /// Creates a new use case.
    pub fn new(
        account_service: Arc<dyn AccountServiceContract>,
        asset_service: Arc<dyn AssetServiceContract>,
        price_provider: Arc<dyn PriceProvider>,
        today: Today,
    ) -> Self {
        Self {
            account_service,
            asset_service,
            price_provider,
            today,
        }
    }

    /// Fills the price history of `asset_id` in `account_id` over the held period
    /// (MKT-191) on the dates that carry no price (MKT-192). Every window is fetched
    /// before anything is recorded (MKT-195).
    pub async fn backfill(
        &self,
        account_id: &str,
        asset_id: &str,
    ) -> Result<PriceHistoryBackfillOutcome, PriceHistoryBackfillError> {
        self.account_service
            .get_by_id(account_id)
            .await?
            .ok_or_else(|| AccountError::AccountNotFound {
                account_id: account_id.to_string(),
            })?;
        if is_cash_asset(asset_id) {
            return Err(AssetError::CashAssetNotEditable.into());
        }
        let asset = self
            .asset_service
            .get_asset_by_id(asset_id)
            .await?
            .ok_or_else(|| AssetError::AssetNotFound {
                id: asset_id.to_string(),
            })?;
        if asset.is_archived {
            return Err(AssetError::Archived.into());
        }
        if asset.price_refresh_blocked {
            return Err(PriceHistoryBackfillTask::PriceRefreshBlocked.into());
        }

        let Some((from, to)) = self.held_period(account_id, asset_id).await? else {
            return Ok(PriceHistoryBackfillOutcome {
                written: 0,
                already_priced: 0,
            });
        };
        let symbol = derive_yahoo_symbol_with_exchange(&asset.reference, asset.exchange.as_ref())
            .ok_or(PriceHistoryBackfillTask::TickerNotResolved)?;
        let closes = self.fetch_closes(&symbol, from, to).await?;
        if closes.is_empty() {
            // MKT-196 — a period of a week or more always holds a trading day, so a
            // known symbol without any close there has no history with the provider.
            const TRADING_WEEK_DAYS: i64 = 7;
            if (to - from).num_days() + 1 >= TRADING_WEEK_DAYS {
                return Err(PriceHistoryBackfillTask::TickerNotResolved.into());
            }
            return Ok(PriceHistoryBackfillOutcome {
                written: 0,
                already_priced: 0,
            });
        }

        Ok(self
            .asset_service
            .record_missing_daily_closes(asset_id, closes)
            .await?)
    }

    /// The held period (MKT-191): from the account's earliest transaction on the
    /// asset through yesterday while the holding is active, otherwise through its
    /// latest transaction — never past yesterday, since today's price belongs to the
    /// fetch tasks. `None` when the period would start after yesterday (a holding
    /// opened today), or when today has no previous date.
    async fn held_period(
        &self,
        account_id: &str,
        asset_id: &str,
    ) -> Result<Option<(NaiveDate, NaiveDate)>, PriceHistoryBackfillError> {
        let transactions = self
            .account_service
            .get_all_transactions_for_account(account_id)
            .await?;
        let mut dates = transactions
            .iter()
            .filter(|transaction| transaction.asset_id == asset_id)
            .filter_map(|transaction| {
                NaiveDate::parse_from_str(&transaction.date, "%Y-%m-%d").ok()
            });
        let first = dates
            .next()
            .ok_or(PriceHistoryBackfillTask::AssetNeverHeld)?;
        let (earliest, latest) = dates.fold((first, first), |(earliest, latest), date| {
            (earliest.min(date), latest.max(date))
        });

        let active = self
            .account_service
            .get_holding_by_account_asset(account_id, asset_id)
            .await?
            .is_some_and(|holding| holding.quantity > 0);
        let Some(yesterday) = (self.today)().pred_opt() else {
            return Ok(None);
        };
        let end = if active {
            yesterday
        } else {
            latest.min(yesterday)
        };
        Ok((earliest <= end).then_some((earliest, end)))
    }

    /// Requests the daily closes of `[from, to]` window by window (MKT-193) and
    /// returns them in date order, one per date inside the period. Any failed
    /// request rejects the whole backfill (MKT-195); a symbol the provider reports
    /// unknown rejects it as an unresolvable ticker (MKT-196).
    async fn fetch_closes(
        &self,
        symbol: &str,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<DatedClose>, PriceHistoryBackfillError> {
        let mut closes: Vec<DatedClose> = Vec::new();
        for (window_from, window_to) in held_period_windows(from, to) {
            let served = self
                .price_provider
                .fetch_daily_closes(symbol, &window_from.to_string(), &window_to.to_string())
                .await
                .map_err(|error| {
                    tracing::warn!(target: BACKEND, symbol = %symbol, from = %window_from, to = %window_to, err = ?error, "price history backfill: daily-close request failed");
                    PriceHistoryBackfillTask::ProviderUnreachable
                })?;
            // MKT-196 — an unknown symbol is unknown for every window: stop here.
            let Some(served) = served else {
                return Err(PriceHistoryBackfillTask::TickerNotResolved.into());
            };
            closes.extend(served);
        }
        let (from, to) = (from.to_string(), to.to_string());
        closes.retain(|close| close.date >= from && close.date <= to);
        closes.sort_by(|left, right| left.date.cmp(&right.date));
        closes.dedup_by(|left, right| left.date == right.date);
        Ok(closes)
    }
}

/// Splits `[from, to]` into consecutive windows of at most [`WINDOW_DAYS`] days
/// (MKT-193).
fn held_period_windows(from: NaiveDate, to: NaiveDate) -> Vec<(NaiveDate, NaiveDate)> {
    let mut windows = Vec::new();
    let mut start = from;
    while start <= to {
        let end = (start + Duration::days(WINDOW_DAYS - 1)).min(to);
        windows.push((start, end));
        let Some(next) = end.succ_opt() else {
            break;
        };
        start = next;
    }
    windows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::{
        Account, Holding, MockAccountServiceContract, Transaction, TransactionType, UpdateFrequency,
    };
    use crate::context::asset::{
        Asset, AssetCategory, AssetClass, MockAssetServiceContract, MockPriceProvider,
        SYSTEM_CATEGORY_ID,
    };
    use crate::core::cash::system_cash_asset_id;

    const ACCOUNT: &str = "acc-1";
    const ASSET: &str = "asset-1";

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2024, 6, 30).expect("valid date")
    }

    fn date(iso: &str) -> NaiveDate {
        NaiveDate::parse_from_str(iso, "%Y-%m-%d").expect("valid ISO date")
    }

    fn make_account() -> Account {
        Account::restore(
            ACCOUNT.to_string(),
            "Portfolio".to_string(),
            String::new(),
            "EUR".to_string(),
            UpdateFrequency::Automatic,
            false,
        )
    }

    fn make_asset(reference: &str, archived: bool, locked: bool) -> Asset {
        Asset::restore(
            ASSET.to_string(),
            "Air Liquide".to_string(),
            AssetClass::Stocks,
            AssetCategory::from_storage(
                SYSTEM_CATEGORY_ID.to_string(),
                "generic.uncategorized".to_string(),
            ),
            "EUR".to_string(),
            1,
            reference.to_string(),
            None,
            archived,
            None,
            locked,
            false,
        )
    }

    fn make_transaction(asset_id: &str, date: &str) -> Transaction {
        Transaction::restore(
            format!("tx-{asset_id}-{date}"),
            ACCOUNT.to_string(),
            asset_id.to_string(),
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

    fn close(date: &str) -> DatedClose {
        DatedClose {
            date: date.to_string(),
            price: 10_000_000,
        }
    }

    /// An account whose transactions on `ASSET` are `transactions`, holding
    /// `quantity` of it today.
    fn account_holding(
        transactions: Vec<Transaction>,
        quantity: i64,
    ) -> MockAccountServiceContract {
        let mut account_service = MockAccountServiceContract::new();
        account_service
            .expect_get_by_id()
            .returning(|_| Ok(Some(make_account())));
        account_service
            .expect_get_all_transactions_for_account()
            .returning(move |_| Ok(transactions.clone()));
        account_service
            .expect_get_holding_by_account_asset()
            .returning(move |_, _| {
                Ok(Some(Holding::restore(
                    "holding-1".to_string(),
                    ACCOUNT.to_string(),
                    ASSET.to_string(),
                    quantity,
                    50_000_000,
                    0,
                    None,
                )))
            });
        account_service
    }

    fn asset_service_serving(asset: Asset) -> MockAssetServiceContract {
        let mut asset_service = MockAssetServiceContract::new();
        asset_service
            .expect_get_asset_by_id()
            .returning(move |_| Ok(Some(asset.clone())));
        asset_service
    }

    fn make_use_case(
        account_service: MockAccountServiceContract,
        asset_service: MockAssetServiceContract,
        price_provider: MockPriceProvider,
    ) -> PriceHistoryBackfillUseCase {
        PriceHistoryBackfillUseCase::new(
            Arc::new(account_service),
            Arc::new(asset_service),
            Arc::new(price_provider),
            Arc::new(today),
        )
    }

    fn filled(written: u32) -> PriceHistoryBackfillOutcome {
        PriceHistoryBackfillOutcome {
            written,
            already_priced: 0,
        }
    }

    // MKT-196 — an unknown account is rejected before the asset is looked up.
    #[tokio::test]
    async fn backfill_rejects_an_unknown_account() {
        let mut account_service = MockAccountServiceContract::new();
        account_service.expect_get_by_id().returning(|_| Ok(None));
        let mut asset_service = MockAssetServiceContract::new();
        asset_service.expect_get_asset_by_id().times(0);

        let error = make_use_case(account_service, asset_service, MockPriceProvider::new())
            .backfill(ACCOUNT, ASSET)
            .await
            .unwrap_err();

        assert!(
            matches!(
                &error,
                PriceHistoryBackfillError::Account(AccountError::AccountNotFound { account_id })
                    if account_id == ACCOUNT
            ),
            "got: {error:?}"
        );
    }

    // MKT-196 — the system Cash Asset is rejected without an asset lookup.
    #[tokio::test]
    async fn backfill_rejects_the_system_cash_asset_without_a_lookup() {
        let mut asset_service = MockAssetServiceContract::new();
        asset_service.expect_get_asset_by_id().times(0);

        let error = make_use_case(
            account_holding(vec![], 0),
            asset_service,
            MockPriceProvider::new(),
        )
        .backfill(ACCOUNT, &system_cash_asset_id("EUR"))
        .await
        .unwrap_err();

        assert!(
            matches!(
                error,
                PriceHistoryBackfillError::Asset(AssetError::CashAssetNotEditable)
            ),
            "got: {error:?}"
        );
    }

    // MKT-196 — an unknown asset is rejected with its id.
    #[tokio::test]
    async fn backfill_rejects_an_unknown_asset() {
        let mut asset_service = MockAssetServiceContract::new();
        asset_service
            .expect_get_asset_by_id()
            .returning(|_| Ok(None));

        let error = make_use_case(
            account_holding(vec![], 0),
            asset_service,
            MockPriceProvider::new(),
        )
        .backfill(ACCOUNT, ASSET)
        .await
        .unwrap_err();

        assert!(
            matches!(
                &error,
                PriceHistoryBackfillError::Asset(AssetError::AssetNotFound { id }) if id == ASSET
            ),
            "got: {error:?}"
        );
    }

    // MKT-196 / AST-006 — an archived asset is rejected before any request.
    #[tokio::test]
    async fn backfill_rejects_an_archived_asset_without_a_request() {
        let mut price_provider = MockPriceProvider::new();
        price_provider.expect_fetch_daily_closes().times(0);

        let error = make_use_case(
            account_holding(vec![make_transaction(ASSET, "2024-01-10")], 1_000_000),
            asset_service_serving(make_asset("AI", true, false)),
            price_provider,
        )
        .backfill(ACCOUNT, ASSET)
        .await
        .unwrap_err();

        assert!(
            matches!(
                error,
                PriceHistoryBackfillError::Asset(AssetError::Archived)
            ),
            "got: {error:?}"
        );
    }

    // MKT-196 / MKT-151 — a refresh-locked asset is rejected before any request.
    #[tokio::test]
    async fn backfill_rejects_a_locked_asset_without_a_request() {
        let mut price_provider = MockPriceProvider::new();
        price_provider.expect_fetch_daily_closes().times(0);

        let error = make_use_case(
            account_holding(vec![make_transaction(ASSET, "2024-01-10")], 1_000_000),
            asset_service_serving(make_asset("AI", false, true)),
            price_provider,
        )
        .backfill(ACCOUNT, ASSET)
        .await
        .unwrap_err();

        assert!(
            matches!(
                error,
                PriceHistoryBackfillError::Task(PriceHistoryBackfillTask::PriceRefreshBlocked)
            ),
            "got: {error:?}"
        );
    }

    // MKT-196 — an account with no transaction on the asset never held it; a
    // transaction on another asset does not count.
    #[tokio::test]
    async fn backfill_rejects_an_asset_the_account_never_held() {
        let mut price_provider = MockPriceProvider::new();
        price_provider.expect_fetch_daily_closes().times(0);

        let error = make_use_case(
            account_holding(vec![make_transaction("other-asset", "2024-01-10")], 0),
            asset_service_serving(make_asset("AI", false, false)),
            price_provider,
        )
        .backfill(ACCOUNT, ASSET)
        .await
        .unwrap_err();

        assert!(
            matches!(
                error,
                PriceHistoryBackfillError::Task(PriceHistoryBackfillTask::AssetNeverHeld)
            ),
            "got: {error:?}"
        );
    }

    // MKT-196 / MKT-110 — a reference no provider symbol derives from is rejected
    // as an unresolvable ticker before any request.
    #[tokio::test]
    async fn backfill_rejects_a_reference_with_no_provider_symbol() {
        let mut price_provider = MockPriceProvider::new();
        price_provider.expect_fetch_daily_closes().times(0);

        let error = make_use_case(
            account_holding(vec![make_transaction(ASSET, "2024-01-10")], 1_000_000),
            asset_service_serving(make_asset("ZZ NOPE", false, false)),
            price_provider,
        )
        .backfill(ACCOUNT, ASSET)
        .await
        .unwrap_err();

        assert!(
            matches!(
                error,
                PriceHistoryBackfillError::Task(PriceHistoryBackfillTask::TickerNotResolved)
            ),
            "got: {error:?}"
        );
    }

    // MKT-196 — a symbol the provider reports unknown is rejected as an
    // unresolvable ticker, and nothing is recorded.
    #[tokio::test]
    async fn backfill_rejects_a_symbol_the_provider_reports_unknown() {
        let mut price_provider = MockPriceProvider::new();
        price_provider
            .expect_fetch_daily_closes()
            .times(1)
            .returning(|_, _, _| Ok(None));
        let mut asset_service = asset_service_serving(make_asset("AI", false, false));
        asset_service.expect_record_missing_daily_closes().times(0);

        let error = make_use_case(
            account_holding(vec![make_transaction(ASSET, "2024-05-01")], 1_000_000),
            asset_service,
            price_provider,
        )
        .backfill(ACCOUNT, ASSET)
        .await
        .unwrap_err();

        assert!(
            matches!(
                error,
                PriceHistoryBackfillError::Task(PriceHistoryBackfillTask::TickerNotResolved)
            ),
            "got: {error:?}"
        );
    }

    // MKT-196 — a known symbol served no close over a held period of a week or more
    // has no history with the provider: rejected as an unresolvable ticker.
    #[tokio::test]
    async fn backfill_rejects_a_known_symbol_without_any_close_over_a_week() {
        let mut price_provider = MockPriceProvider::new();
        price_provider
            .expect_fetch_daily_closes()
            .times(1)
            .returning(|_, _, _| Ok(Some(vec![])));
        let mut asset_service = asset_service_serving(make_asset("AI", false, false));
        asset_service.expect_record_missing_daily_closes().times(0);

        let error = make_use_case(
            account_holding(vec![make_transaction(ASSET, "2024-06-20")], 1_000_000),
            asset_service,
            price_provider,
        )
        .backfill(ACCOUNT, ASSET)
        .await
        .unwrap_err();

        assert!(
            matches!(
                error,
                PriceHistoryBackfillError::Task(PriceHistoryBackfillTask::TickerNotResolved)
            ),
            "got: {error:?}"
        );
    }

    // MKT-196 — a known symbol served no close over a held period shorter than a
    // week (a weekend) is not a rejection: nothing is recorded and both counts are
    // zero.
    #[tokio::test]
    async fn backfill_of_a_period_without_a_close_succeeds_with_nothing_to_fill() {
        let mut price_provider = MockPriceProvider::new();
        price_provider
            .expect_fetch_daily_closes()
            .times(1)
            .returning(|_, _, _| Ok(Some(vec![])));
        let mut asset_service = asset_service_serving(make_asset("AI", false, false));
        asset_service.expect_record_missing_daily_closes().times(0);

        let outcome = make_use_case(
            account_holding(vec![make_transaction(ASSET, "2024-06-29")], 1_000_000),
            asset_service,
            price_provider,
        )
        .backfill(ACCOUNT, ASSET)
        .await
        .unwrap();

        assert_eq!(
            outcome,
            PriceHistoryBackfillOutcome {
                written: 0,
                already_priced: 0
            }
        );
    }

    // MKT-191 — an active holding is backfilled from its earliest transaction
    // through yesterday, never today.
    #[tokio::test]
    async fn backfill_of_an_active_holding_runs_from_its_first_transaction_through_yesterday() {
        let mut price_provider = MockPriceProvider::new();
        price_provider
            .expect_fetch_daily_closes()
            .withf(|symbol, from, to| symbol == "AI" && from == "2024-02-01" && to == "2024-06-29")
            .times(1)
            .returning(|_, _, _| Ok(Some(vec![close("2024-02-02")])));
        let mut asset_service = asset_service_serving(make_asset("AI", false, false));
        asset_service
            .expect_record_missing_daily_closes()
            .times(1)
            .returning(|_, _| Ok(filled(1)));

        let outcome = make_use_case(
            account_holding(
                vec![
                    make_transaction(ASSET, "2024-05-01"),
                    make_transaction(ASSET, "2024-02-01"),
                ],
                1_000_000,
            ),
            asset_service,
            price_provider,
        )
        .backfill(ACCOUNT, ASSET)
        .await
        .unwrap();

        assert_eq!(outcome, filled(1));
    }

    // MKT-191 — a closed holding is backfilled through its latest transaction,
    // not through today.
    #[tokio::test]
    async fn backfill_of_a_closed_holding_ends_on_its_latest_transaction() {
        let mut price_provider = MockPriceProvider::new();
        price_provider
            .expect_fetch_daily_closes()
            .withf(|_, from, to| from == "2024-01-10" && to == "2024-03-05")
            .times(1)
            .returning(|_, _, _| Ok(Some(vec![close("2024-01-11")])));
        let mut asset_service = asset_service_serving(make_asset("AI", false, false));
        asset_service
            .expect_record_missing_daily_closes()
            .returning(|_, _| Ok(filled(1)));

        let outcome = make_use_case(
            account_holding(
                vec![
                    make_transaction(ASSET, "2024-01-10"),
                    make_transaction(ASSET, "2024-03-05"),
                ],
                0,
            ),
            asset_service,
            price_provider,
        )
        .backfill(ACCOUNT, ASSET)
        .await
        .unwrap();

        assert_eq!(outcome, filled(1));
    }

    // MKT-191 — a closed holding whose latest transaction is today still ends
    // yesterday.
    #[tokio::test]
    async fn backfill_of_a_holding_closed_today_ends_yesterday() {
        let mut price_provider = MockPriceProvider::new();
        price_provider
            .expect_fetch_daily_closes()
            .withf(|_, from, to| from == "2024-06-10" && to == "2024-06-29")
            .times(1)
            .returning(|_, _, _| Ok(Some(vec![close("2024-06-11")])));
        let mut asset_service = asset_service_serving(make_asset("AI", false, false));
        asset_service
            .expect_record_missing_daily_closes()
            .returning(|_, _| Ok(filled(1)));

        let outcome = make_use_case(
            account_holding(
                vec![
                    make_transaction(ASSET, "2024-06-10"),
                    make_transaction(ASSET, "2024-06-30"),
                ],
                0,
            ),
            asset_service,
            price_provider,
        )
        .backfill(ACCOUNT, ASSET)
        .await
        .unwrap();

        assert_eq!(outcome, filled(1));
    }

    // MKT-191/194 — a holding opened today has an empty held period: nothing is
    // requested or recorded, and both counts are zero.
    #[tokio::test]
    async fn backfill_of_a_holding_opened_today_requests_nothing() {
        let mut price_provider = MockPriceProvider::new();
        price_provider.expect_fetch_daily_closes().times(0);
        let mut asset_service = asset_service_serving(make_asset("AI", false, false));
        asset_service.expect_record_missing_daily_closes().times(0);

        let outcome = make_use_case(
            account_holding(vec![make_transaction(ASSET, "2024-06-30")], 1_000_000),
            asset_service,
            price_provider,
        )
        .backfill(ACCOUNT, ASSET)
        .await
        .unwrap();

        assert_eq!(
            outcome,
            PriceHistoryBackfillOutcome {
                written: 0,
                already_priced: 0
            }
        );
    }

    // MKT-192/193 — the asset service receives only the closes inside the held
    // period, one per date, in date order.
    #[tokio::test]
    async fn backfill_hands_only_the_period_closes_to_the_asset_service_in_date_order() {
        let mut price_provider = MockPriceProvider::new();
        price_provider
            .expect_fetch_daily_closes()
            .returning(|_, _, _| {
                Ok(Some(vec![
                    close("2024-06-01"),
                    close("2024-04-30"),
                    close("2024-05-02"),
                    close("2024-05-02"),
                ]))
            });
        let mut asset_service = asset_service_serving(make_asset("AI", false, false));
        asset_service
            .expect_record_missing_daily_closes()
            .withf(|asset_id, closes| {
                let dates: Vec<&str> = closes.iter().map(|close| close.date.as_str()).collect();
                asset_id == ASSET && dates == ["2024-05-02", "2024-06-01"]
            })
            .times(1)
            .returning(|_, _| Ok(filled(2)));

        let outcome = make_use_case(
            account_holding(vec![make_transaction(ASSET, "2024-05-01")], 1_000_000),
            asset_service,
            price_provider,
        )
        .backfill(ACCOUNT, ASSET)
        .await
        .unwrap();

        assert_eq!(outcome, filled(2));
    }

    // MKT-195 — a failed request rejects the backfill and nothing is recorded.
    #[tokio::test]
    async fn backfill_rejects_a_failed_request_and_records_nothing() {
        let mut price_provider = MockPriceProvider::new();
        price_provider
            .expect_fetch_daily_closes()
            .returning(|_, _, _| Err(anyhow::anyhow!("connection reset")));
        let mut asset_service = asset_service_serving(make_asset("AI", false, false));
        asset_service.expect_record_missing_daily_closes().times(0);

        let error = make_use_case(
            account_holding(vec![make_transaction(ASSET, "2024-05-01")], 1_000_000),
            asset_service,
            price_provider,
        )
        .backfill(ACCOUNT, ASSET)
        .await
        .unwrap_err();

        assert!(
            matches!(
                error,
                PriceHistoryBackfillError::Task(PriceHistoryBackfillTask::ProviderUnreachable)
            ),
            "got: {error:?}"
        );
    }

    // An account lookup failure propagates the account context's DatabaseError.
    #[tokio::test]
    async fn backfill_propagates_an_account_lookup_failure() {
        let mut account_service = MockAccountServiceContract::new();
        account_service
            .expect_get_by_id()
            .returning(|_| Err(AccountError::DatabaseError));

        let error = make_use_case(
            account_service,
            MockAssetServiceContract::new(),
            MockPriceProvider::new(),
        )
        .backfill(ACCOUNT, ASSET)
        .await
        .unwrap_err();

        assert!(
            matches!(
                error,
                PriceHistoryBackfillError::Account(AccountError::DatabaseError)
            ),
            "got: {error:?}"
        );
    }

    // MKT-195 — a recording failure propagates the asset context's DatabaseError.
    #[tokio::test]
    async fn backfill_propagates_a_recording_failure() {
        let mut price_provider = MockPriceProvider::new();
        price_provider
            .expect_fetch_daily_closes()
            .returning(|_, _, _| Ok(Some(vec![close("2024-05-02")])));
        let mut asset_service = asset_service_serving(make_asset("AI", false, false));
        asset_service
            .expect_record_missing_daily_closes()
            .returning(|_, _| Err(AssetError::DatabaseError));

        let error = make_use_case(
            account_holding(vec![make_transaction(ASSET, "2024-05-01")], 1_000_000),
            asset_service,
            price_provider,
        )
        .backfill(ACCOUNT, ASSET)
        .await
        .unwrap_err();

        assert!(
            matches!(
                error,
                PriceHistoryBackfillError::Asset(AssetError::DatabaseError)
            ),
            "got: {error:?}"
        );
    }

    // MKT-193 — a period shorter than a year is one window.
    #[test]
    fn held_period_windows_keeps_a_short_period_in_one_window() {
        assert_eq!(
            held_period_windows(date("2024-01-10"), date("2024-03-05")),
            vec![(date("2024-01-10"), date("2024-03-05"))]
        );
    }

    // MKT-193 — a longer period is split into consecutive windows of 365 days,
    // the last one ending on the period's last day.
    #[test]
    fn held_period_windows_splits_a_long_period_into_consecutive_year_windows() {
        assert_eq!(
            held_period_windows(date("2022-01-01"), date("2024-06-30")),
            vec![
                (date("2022-01-01"), date("2022-12-31")),
                (date("2023-01-01"), date("2023-12-31")),
                (date("2024-01-01"), date("2024-06-30")),
            ]
        );
    }

    // MKT-193 — a period of a single day is one window of that day.
    #[test]
    fn held_period_windows_of_a_single_day_is_that_day() {
        assert_eq!(
            held_period_windows(date("2024-06-30"), date("2024-06-30")),
            vec![(date("2024-06-30"), date("2024-06-30"))]
        );
    }
}
