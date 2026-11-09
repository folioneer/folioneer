//! GPF-012–041 — the cross-account performance aggregation in the reference currency: the
//! computation the global performance and the profiles overview (PFL-055) share (B18).

use crate::context::account::{
    Account, AccountError, AccountServiceContract, Transaction, TransactionType,
};
use crate::context::asset::AssetServiceContract;
use crate::context::currency::CurrencyService;
use crate::core::logger::BACKEND;
use crate::use_cases::shared::performance::{
    annualized_yield_metric, lifetime_unavailable, residual_pnl, AccountPerformanceResponse,
    PerformancePeriod, PeriodBridge,
};
use crate::use_cases::shared::valuation::{
    at_rate, convert_at_rate, end_value_as_of, external_cash_flows, external_cash_flows_windowed,
    holding_close_date_as_of, holding_end_value_as_of, holding_performance_for_span_over_flows,
    load_priced_assets, load_rate_map_for_dates, market_valued_flow_dates,
    metric_for_span_over_flows, month_periods, opening_balance_flow_value, parse_date,
    position_flows, position_flows_windowed, position_held_as_of, year_periods, DatedFlow,
    MonthPeriod, PerformanceMetric, PricedAsset, RateMap, YearPeriod,
};
use chrono::{Datelike, Local, NaiveDate};
use std::collections::{BTreeSet, HashMap};
use std::result::Result as StdResult;
use std::sync::Arc;

/// Identity conversion rate in micros for reference-currency accounts.
const IDENTITY_RATE_MICROS: i64 = 1_000_000;

/// GPF-012–041 — aggregates the performance of a set of accounts over the account, asset and
/// currency services; held by each use case that needs it.
pub(crate) struct PerformanceAggregator {
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
    currency_service: Arc<CurrencyService>,
}

impl PerformanceAggregator {
    /// The aggregator over the three services.
    pub(crate) fn new(
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

    /// GPF-012–041 — the cross-account aggregation: every included account's
    /// own-currency figures are computed with the single-account machinery and
    /// converted to the reference currency before summation.
    pub(crate) async fn aggregate_across_accounts(
        &self,
        accounts: Vec<Account>,
        asset_scope: Option<&str>,
    ) -> StdResult<AccountPerformanceResponse, AccountError> {
        let today = Local::now().date_naive();
        let reference = self.currency_service.reference_currency().await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "aggregate_across_accounts: reference currency not read");
            AccountError::DatabaseError
        })?;

        // GPF-012 — an account participates only with at least one dated
        // in-scope transaction.
        let mut included: Vec<(Account, Vec<Transaction>)> = Vec::new();
        for account in accounts {
            let transactions = self
                .account_service
                .get_all_transactions_for_account(&account.id)
                .await?;
            let transactions: Vec<Transaction> = match asset_scope {
                None => transactions,
                Some(asset_id) => transactions
                    .into_iter()
                    .filter(|transaction| transaction.asset_id == asset_id)
                    .collect(),
            };
            if transactions
                .iter()
                .any(|transaction| parse_date(&transaction.date).is_some())
            {
                included.push((account, transactions));
            }
        }

        // GPF-015 — an empty portfolio (no included account) has no data span.
        let earliest_date = included
            .iter()
            .flat_map(|(_, transactions)| {
                transactions
                    .iter()
                    .filter_map(|transaction| parse_date(&transaction.date))
            })
            .min();
        let Some(earliest_date) = earliest_date else {
            return Ok(empty_response(&reference));
        };

        // Every date the aggregation values: yearly and monthly period ends, and the
        // monthly periods' prior-year-end YTD baselines (FXR-035/042 pre-resolution set).
        let mut valued_dates: BTreeSet<NaiveDate> = BTreeSet::new();
        for period in year_periods(earliest_date, today) {
            valued_dates.insert(period.period_end);
        }
        for period in month_periods(earliest_date, today) {
            valued_dates.insert(period.period_end);
            valued_dates.insert(period.year_start_baseline);
        }
        let valued_dates: Vec<NaiveDate> = valued_dates.into_iter().collect();

        let mut converted_accounts = Vec::with_capacity(included.len());
        for (account, transactions) in included {
            converted_accounts.push(
                self.prepare_account(
                    account,
                    transactions,
                    &valued_dates,
                    asset_scope,
                    &reference,
                )
                .await?,
            );
        }

        let flows = aggregated_flows(&converted_accounts);

        // PRF-085 — the union of the scoped transactions across accounts feeds
        // the aggregation-wide close-date probe (empty when unscoped).
        let scoped_transactions: Vec<Transaction> = match asset_scope {
            None => Vec::new(),
            Some(_) => converted_accounts
                .iter()
                .flat_map(|account| account.transactions.iter().cloned())
                .collect(),
        };

        let yearly = build_yearly_aggregated(
            &converted_accounts,
            &flows,
            &scoped_transactions,
            asset_scope,
            earliest_date,
            today,
        );
        let monthly = build_monthly_aggregated(
            &converted_accounts,
            &flows,
            &scoped_transactions,
            asset_scope,
            earliest_date,
            today,
        );

        // PRF-087 — the cause, across every included account's transactions in scope.
        let all_transactions: Vec<Transaction> = converted_accounts
            .iter()
            .flat_map(|account| account.transactions.iter().cloned())
            .collect();
        let lifetime_unavailable =
            lifetime_unavailable(yearly.iter().chain(&monthly), &all_transactions);
        Ok(AccountPerformanceResponse {
            account_name: String::new(),
            currency: reference,
            yearly,
            monthly,
            lifetime_unavailable,
        })
    }

    /// Preloads one included account's valuation inputs and converts its dated
    /// flows to the reference currency (GPF-020/030): the asset price
    /// histories, the asset-currency → account-currency rate map for the valued
    /// dates, the account-currency → reference-currency rates for the valued
    /// dates and the flow-relevant transaction dates, and the converted Simple
    /// Dietz flows.
    async fn prepare_account(
        &self,
        account: Account,
        transactions: Vec<Transaction>,
        valued_dates: &[NaiveDate],
        asset_scope: Option<&str>,
        reference: &str,
    ) -> StdResult<ConvertedAccount, AccountError> {
        let priced_assets = load_priced_assets(self.asset_service.as_ref(), &transactions).await?;
        // PRF-086 — opening balances and transfers are valued at their entry date,
        // so those dates join the valued-date set for this account's own rate map.
        let mut asset_rate_dates: BTreeSet<NaiveDate> = valued_dates.iter().copied().collect();
        asset_rate_dates.extend(market_valued_flow_dates(&transactions));
        let asset_rate_dates: Vec<NaiveDate> = asset_rate_dates.into_iter().collect();
        let rate_map = load_rate_map_for_dates(
            &self.currency_service,
            &priced_assets,
            &account.currency,
            &asset_rate_dates,
        )
        .await?;

        let mut reference_rate_by_date: HashMap<NaiveDate, i64> = HashMap::new();
        if account.currency != reference {
            let mut dates: BTreeSet<NaiveDate> = valued_dates.iter().copied().collect();
            dates.extend(reference_rate_dates(&transactions, asset_scope));
            for date in dates {
                let as_of = date.format("%Y-%m-%d").to_string();
                if let Some(rate) = self
                    .currency_service
                    .resolve_rate_micros(&account.currency, reference, &as_of)
                    .await
                    .map_err(|e| {
                        tracing::error!(target: BACKEND, currency = %account.currency, as_of = %as_of, err = ?e, "prepare_account: resolve_rate_micros failed");
                        AccountError::DatabaseError
                    })?
                {
                    reference_rate_by_date.insert(date, rate);
                }
            }
        }

        let mut converted_account = ConvertedAccount {
            currency: account.currency,
            reference_currency: reference.to_string(),
            transactions,
            priced_assets,
            rate_map,
            reference_rate_by_date,
            trade_flows: Vec::new(),
            windowed_trade_flows: Vec::new(),
            dividend_flows: Vec::new(),
        };

        // GPF-030 — each flow converts at the rate of its own transaction date;
        // a flow with no usable rate contributes 0 (dropped). The lifetime set
        // keeps opening balances at typed cost (PRF-035); the windowed set
        // values them at their entry-date market price (PRF-086).
        // TRF-028 — what a transfer takes out of one account reaches another: no flow of the
        // portfolio, so neither side enters its flows, whatever the two currencies.
        let portfolio_transactions: Vec<Transaction> = converted_account
            .transactions
            .iter()
            .filter(|t| {
                !matches!(
                    t.transaction_type,
                    TransactionType::TransferIn | TransactionType::TransferOut
                )
            })
            .cloned()
            .collect();
        let (raw_trades, raw_windowed_trades, raw_dividends) = match asset_scope {
            None => (
                external_cash_flows(&portfolio_transactions),
                external_cash_flows_windowed(
                    &portfolio_transactions,
                    &converted_account.priced_assets,
                    &converted_account.rate_map,
                    &converted_account.currency,
                ),
                Vec::new(),
            ),
            Some(_) => {
                let asset_transactions: Vec<&Transaction> = portfolio_transactions.iter().collect();
                let flows = position_flows(&asset_transactions);
                let windowed = position_flows_windowed(
                    &asset_transactions,
                    &converted_account.priced_assets,
                    &converted_account.rate_map,
                    &converted_account.currency,
                );
                (flows.trades, windowed.trades, flows.dividends)
            }
        };
        converted_account.trade_flows = converted_account.convert_flows(raw_trades);
        converted_account.windowed_trade_flows =
            converted_account.convert_flows(raw_windowed_trades);
        converted_account.dividend_flows = converted_account.convert_flows(raw_dividends);
        Ok(converted_account)
    }
}

/// GPF-030/040 — the transaction dates whose reference-currency rate the
/// conversion actually reads, per scope. Unscoped: Deposit/Withdrawal (flows +
/// bridge cash) — Purchase/Sell convert nothing. Scoped: Purchase/Sell trade
/// flows — Deposit/Withdrawal never touch a position. OpeningBalance, Dividend,
/// FreeShares and Interest convert at their own date in both scopes;
/// ManagementFee is never a flow (FEE-071). Resolving only these avoids the
/// wasted `resolve_rate_micros` round-trips of a resolve-every-date sweep.
pub(crate) fn reference_rate_dates(
    transactions: &[Transaction],
    asset_scope: Option<&str>,
) -> BTreeSet<NaiveDate> {
    transactions
        .iter()
        .filter(|transaction| match transaction.transaction_type {
            TransactionType::Deposit | TransactionType::Withdrawal => asset_scope.is_none(),
            TransactionType::Purchase | TransactionType::Sell => asset_scope.is_some(),
            TransactionType::OpeningBalance | TransactionType::Dividend => true,
            // PRF-076 — free shares and interest are earned, no flow.
            // SPL-050 — a split is not a flow and needs no rate.
            // TRF-028 — a transfer is no flow of the portfolio.
            TransactionType::FreeShares
            | TransactionType::Interest
            | TransactionType::ManagementFee
            | TransactionType::Split
            | TransactionType::WriteOff
            | TransactionType::TransferIn
            | TransactionType::TransferOut => false,
        })
        .filter_map(|transaction| parse_date(&transaction.date))
        .collect()
}

/// GPF-015 — the PRF-043-shaped empty result of an aggregation with no data span.
fn empty_response(reference: &str) -> AccountPerformanceResponse {
    AccountPerformanceResponse {
        account_name: String::new(),
        currency: reference.to_string(),
        yearly: Vec::new(),
        monthly: Vec::new(),
        lifetime_unavailable: None,
    }
}

/// One included account with its preloaded valuation inputs and its dated flows
/// pre-converted to the reference currency (GPF-020/030).
struct ConvertedAccount {
    currency: String,
    /// The currency the portfolio is reported in (GPF-011).
    reference_currency: String,
    transactions: Vec<Transaction>,
    priced_assets: HashMap<String, PricedAsset>,
    rate_map: RateMap,
    reference_rate_by_date: HashMap<NaiveDate, i64>,
    /// Lifetime Simple Dietz flows in reference-currency micros: the
    /// account-level external flows (PRF-030), or the position trade flows in
    /// asset scope (PRF-083), converted per GPF-030. Opening balances carry
    /// their typed cost (PRF-035).
    trade_flows: Vec<DatedFlow>,
    /// The windowed variant of `trade_flows` (PRF-086): opening balances at
    /// their entry-date market value.
    windowed_trade_flows: Vec<DatedFlow>,
    /// The scoped asset's dividend income in reference-currency micros (asset
    /// scope only; empty otherwise).
    dividend_flows: Vec<DatedFlow>,
}

impl ConvertedAccount {
    /// The account-currency → reference-currency rate as of `date`: identity
    /// for reference-currency accounts, the carry-forward resolution otherwise;
    /// `None` when no usable rate exists (FXR-034).
    fn reference_rate(&self, date: NaiveDate) -> Option<i64> {
        if self.currency == self.reference_currency {
            Some(IDENTITY_RATE_MICROS)
        } else {
            self.reference_rate_by_date.get(&date).copied()
        }
    }

    /// GPF-020 — the account's own-currency end value at `period_end` (Global
    /// Value, or the scoped position value) converted at the period-end rate;
    /// 0 when no usable rate exists.
    fn end_value_reference(&self, asset_scope: Option<&str>, period_end: NaiveDate) -> i64 {
        let own_currency_value = match asset_scope {
            None => end_value_as_of(
                &self.transactions,
                &self.priced_assets,
                &self.rate_map,
                &self.currency,
                period_end,
            ),
            Some(asset_id) => holding_end_value_as_of(
                &self.transactions,
                asset_id,
                &self.priced_assets,
                &self.rate_map,
                &self.currency,
                period_end,
            ),
        };
        match self.reference_rate(period_end) {
            Some(rate) => convert_at_rate(own_currency_value, rate),
            None => 0,
        }
    }

    /// GPF-040 — this account's bridge terms within `[period_start, period_end]`
    /// in reference-currency micros, mirroring the PRF-070–072 account bridge or
    /// the PRF-084 position bridge: every flow term converts at the rate of its
    /// own date — cash and dividend flows at their transaction date, an opening
    /// balance at its entry-date market value (PRF-086). Free shares and interest
    /// are earned, no flow (PRF-076). A term with no usable rate contributes 0.
    fn bridge_reference(
        &self,
        asset_scope: Option<&str>,
        period_start: NaiveDate,
        period_end: NaiveDate,
    ) -> PeriodBridge {
        let mut cash_flow: i128 = 0;
        let mut asset_flow: i128 = 0;
        let mut dividends: i128 = 0;

        for transaction in &self.transactions {
            let Some(date) = parse_date(&transaction.date) else {
                continue;
            };
            if date < period_start || date > period_end {
                continue;
            }
            let transaction_date_rate = self.reference_rate(date);
            match asset_scope {
                None => match transaction.transaction_type {
                    TransactionType::Deposit => {
                        cash_flow +=
                            convert_or_zero(transaction.total_amount, transaction_date_rate);
                    }
                    TransactionType::Withdrawal => {
                        cash_flow -=
                            convert_or_zero(transaction.total_amount, transaction_date_rate);
                    }
                    // PRF-086 — entry-date market value (fallback typed cost),
                    // converted at the entry-date rate.
                    TransactionType::OpeningBalance => {
                        asset_flow += convert_or_zero(
                            opening_balance_flow_value(
                                transaction,
                                &self.priced_assets,
                                &self.rate_map,
                                &self.currency,
                            ),
                            transaction_date_rate,
                        );
                    }
                    TransactionType::Dividend => {
                        dividends +=
                            convert_or_zero(transaction.total_amount, transaction_date_rate);
                    }
                    // PRF-076 — free shares and interest are earned, no flow.
                    // SPL-050 — a split is not a flow.
                    TransactionType::Purchase
                    | TransactionType::Sell
                    | TransactionType::FreeShares
                    | TransactionType::Interest
                    | TransactionType::ManagementFee
                    | TransactionType::Split
                    | TransactionType::WriteOff
                    // TRF-028 — what leaves one account reaches the other.
                    | TransactionType::TransferIn
                    | TransactionType::TransferOut => {}
                },
                Some(_) => match transaction.transaction_type {
                    TransactionType::Purchase => {
                        cash_flow +=
                            convert_or_zero(transaction.total_amount, transaction_date_rate);
                    }
                    // PRF-086 — no cash leg: an in-kind contribution at its
                    // entry-date market value, converted at the entry-date rate.
                    TransactionType::OpeningBalance => {
                        asset_flow += convert_or_zero(
                            opening_balance_flow_value(
                                transaction,
                                &self.priced_assets,
                                &self.rate_map,
                                &self.currency,
                            ),
                            transaction_date_rate,
                        );
                    }
                    TransactionType::Sell => {
                        cash_flow -=
                            convert_or_zero(transaction.total_amount, transaction_date_rate);
                    }
                    TransactionType::Dividend => {
                        dividends +=
                            convert_or_zero(transaction.total_amount, transaction_date_rate);
                    }
                    // PRF-084 — free shares and interest are earned, no flow.
                    // SPL-050 — a split is not a flow.
                    TransactionType::Deposit
                    | TransactionType::Withdrawal
                    | TransactionType::FreeShares
                    | TransactionType::Interest
                    | TransactionType::ManagementFee
                    | TransactionType::Split
                    | TransactionType::WriteOff
                    // TRF-028 — what leaves one account reaches the other.
                    | TransactionType::TransferIn
                    | TransactionType::TransferOut => {}
                },
            }
        }

        debug_assert!(
            [cash_flow, asset_flow, dividends]
                .iter()
                .all(|v| *v <= i64::MAX as i128 && *v >= i64::MIN as i128),
            "bridge_reference i64 overflow: cash_flow={cash_flow} asset_flow={asset_flow} dividends={dividends}"
        );
        PeriodBridge {
            cash_flow: cash_flow as i64,
            asset_flow: asset_flow as i64,
            dividends: dividends as i64,
        }
    }

    /// Converts a batch of own-currency flows at their per-date rates (GPF-030);
    /// flows with no usable rate are dropped (contribute 0).
    fn convert_flows(&self, flows: Vec<DatedFlow>) -> Vec<DatedFlow> {
        flows
            .into_iter()
            .filter_map(|flow| {
                self.reference_rate(flow.date).map(|rate| DatedFlow {
                    date: flow.date,
                    amount: convert_at_rate(flow.amount, rate),
                })
            })
            .collect()
    }
}

/// Applies a per-date conversion when a rate exists; contributes 0 otherwise
/// (GPF-030/040 missing-rate degradation).
fn convert_or_zero(amount: i64, rate_micros: Option<i64>) -> i128 {
    match rate_micros {
        Some(rate) => at_rate(amount as i128, rate),
        None => 0,
    }
}

/// The converted Simple Dietz flow set of the whole aggregation (GPF-030):
/// every included account's trade flows — lifetime and windowed variants — and,
/// in asset scope, dividend flows, concatenated.
struct AggregatedFlows {
    /// Lifetime trades: opening balances at typed cost (PRF-035).
    trades: Vec<DatedFlow>,
    /// Windowed trades: opening balances at entry-date market value (PRF-086).
    windowed_trades: Vec<DatedFlow>,
    dividends: Vec<DatedFlow>,
}

fn aggregated_flows(accounts: &[ConvertedAccount]) -> AggregatedFlows {
    let mut trades = Vec::new();
    let mut windowed_trades = Vec::new();
    let mut dividends = Vec::new();
    for account in accounts {
        trades.extend(account.trade_flows.iter().copied());
        windowed_trades.extend(account.windowed_trade_flows.iter().copied());
        dividends.extend(account.dividend_flows.iter().copied());
    }
    AggregatedFlows {
        trades,
        windowed_trades,
        dividends,
    }
}

/// GPF-031 — the span metric of the aggregation, over the given converted trade
/// set (`flows.windowed_trades` for calendar windows per PRF-086,
/// `flows.trades` for lifetime metrics per PRF-035): the PRF-031/032 account
/// Dietz, or the PRF-083 position Dietz (dividends added to gain) in asset
/// scope.
fn aggregated_metric(
    trades: &[DatedFlow],
    dividends: &[DatedFlow],
    asset_scope: Option<&str>,
    start_value: i64,
    end_value: i64,
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> PerformanceMetric {
    match asset_scope {
        None => {
            metric_for_span_over_flows(trades, start_value, end_value, period_start, period_end)
        }
        Some(_) => holding_performance_for_span_over_flows(
            trades,
            dividends,
            start_value,
            end_value,
            period_start,
            period_end,
        ),
    }
}

/// PRF-083, ACD-057 — a calendar-window metric of the aggregation: in asset scope, a
/// window whose baseline day (the end of the day before `period_start`) finds the
/// position holding nothing across the included accounts has no figure.
#[allow(clippy::too_many_arguments)]
fn windowed_aggregated_metric(
    scoped_transactions: &[Transaction],
    trades: &[DatedFlow],
    dividends: &[DatedFlow],
    asset_scope: Option<&str>,
    start_value: i64,
    end_value: i64,
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> Option<PerformanceMetric> {
    if let Some(asset_id) = asset_scope {
        let baseline = period_start.pred_opt()?;
        if !position_held_as_of(scoped_transactions, asset_id, baseline) {
            return None;
        }
    }
    Some(aggregated_metric(
        trades,
        dividends,
        asset_scope,
        start_value,
        end_value,
        period_start,
        period_end,
    ))
}

/// GPF-020 — Σ over included accounts of the converted end value at `period_end`.
fn summed_end_value(
    accounts: &[ConvertedAccount],
    asset_scope: Option<&str>,
    period_end: NaiveDate,
) -> i64 {
    let total: i128 = accounts
        .iter()
        .map(|account| account.end_value_reference(asset_scope, period_end) as i128)
        .sum();
    debug_assert!(
        total <= i64::MAX as i128 && total >= i64::MIN as i128,
        "summed_end_value i64 overflow: {total}"
    );
    total as i64
}

/// GPF-040 — Σ over included accounts of the converted bridge terms.
fn summed_bridge(
    accounts: &[ConvertedAccount],
    asset_scope: Option<&str>,
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> PeriodBridge {
    let mut cash_flow: i128 = 0;
    let mut asset_flow: i128 = 0;
    let mut dividends: i128 = 0;
    for account in accounts {
        let bridge = account.bridge_reference(asset_scope, period_start, period_end);
        cash_flow += bridge.cash_flow as i128;
        asset_flow += bridge.asset_flow as i128;
        dividends += bridge.dividends as i128;
    }
    debug_assert!(
        [cash_flow, asset_flow, dividends]
            .iter()
            .all(|v| *v <= i64::MAX as i128 && *v >= i64::MIN as i128),
        "summed_bridge i64 overflow: cash_flow={cash_flow} asset_flow={asset_flow} dividends={dividends}"
    );
    PeriodBridge {
        cash_flow: cash_flow as i64,
        asset_flow: asset_flow as i64,
        dividends: dividends as i64,
    }
}

/// Builds the aggregated yearly series, most-recent first (PRF-012/040/041
/// applied to the global span, GPF-013).
fn build_yearly_aggregated(
    accounts: &[ConvertedAccount],
    flows: &AggregatedFlows,
    scoped_transactions: &[Transaction],
    asset_scope: Option<&str>,
    earliest_date: NaiveDate,
    today: NaiveDate,
) -> Vec<PerformancePeriod> {
    let periods = year_periods(earliest_date, today);
    let first_year = earliest_date.year();
    let mut rows = Vec::with_capacity(periods.len());
    let mut previous_end_value: i64 = 0;
    for YearPeriod {
        year,
        period_start,
        period_end,
    } in periods
    {
        let end_value = summed_end_value(accounts, asset_scope, period_end);

        let period_over_period = if year == first_year {
            None
        } else {
            windowed_aggregated_metric(
                scoped_transactions,
                &flows.windowed_trades,
                &flows.dividends,
                asset_scope,
                previous_end_value,
                end_value,
                period_start,
                period_end,
            )
        };
        // PRF-085 — a scoped position closed across every included account
        // freezes its cumulative metrics at the aggregation-wide close date.
        let cumulative_end = match asset_scope {
            None => period_end,
            Some(_) => {
                holding_close_date_as_of(scoped_transactions, period_end).unwrap_or(period_end)
            }
        };
        let since_inception = Some(aggregated_metric(
            &flows.trades,
            &flows.dividends,
            asset_scope,
            0,
            end_value,
            earliest_date,
            cumulative_end,
        ));
        let annualized_yield = since_inception
            .as_ref()
            .and_then(|metric| annualized_yield_metric(metric, earliest_date, cumulative_end));
        let bridge = summed_bridge(accounts, asset_scope, period_start, period_end);
        let pnl = residual_pnl(end_value, previous_end_value, &bridge, asset_scope);

        rows.push(PerformancePeriod {
            year,
            month: None,
            end_value,
            previous_value: previous_end_value,
            cash_flow: bridge.cash_flow,
            asset_flow: bridge.asset_flow,
            dividends: bridge.dividends,
            pnl,
            period_over_period,
            // PRF-037 — year_to_date is omitted on year rows.
            year_to_date: None,
            since_inception,
            annualized_yield,
        });
        previous_end_value = end_value;
    }
    rows.reverse();
    rows
}

/// Builds the aggregated monthly series over the full span, most-recent first
/// (PRF-040/041 applied to the global span, GPF-013/014).
fn build_monthly_aggregated(
    accounts: &[ConvertedAccount],
    flows: &AggregatedFlows,
    scoped_transactions: &[Transaction],
    asset_scope: Option<&str>,
    earliest_date: NaiveDate,
    today: NaiveDate,
) -> Vec<PerformancePeriod> {
    let periods = month_periods(earliest_date, today);
    let (first_year, first_month) = (earliest_date.year(), earliest_date.month());
    let mut rows = Vec::with_capacity(periods.len());
    let mut previous_end_value: i64 = 0;
    for MonthPeriod {
        year,
        month,
        period_start,
        period_end,
        year_start,
        year_start_baseline,
    } in periods
    {
        let end_value = summed_end_value(accounts, asset_scope, period_end);

        let is_first_period = year == first_year && month == first_month;
        let period_over_period = if is_first_period {
            None
        } else {
            windowed_aggregated_metric(
                scoped_transactions,
                &flows.windowed_trades,
                &flows.dividends,
                asset_scope,
                previous_end_value,
                end_value,
                period_start,
                period_end,
            )
        };

        // PRF-085 — a scoped position closed across every included account
        // freezes its cumulative metrics at the aggregation-wide close date.
        let cumulative_end = match asset_scope {
            None => period_end,
            Some(_) => {
                holding_close_date_as_of(scoped_transactions, period_end).unwrap_or(period_end)
            }
        };
        // PRF-034 — year-to-date baseline is the prior 31 December end value.
        let year_start_baseline_value =
            summed_end_value(accounts, asset_scope, year_start_baseline);
        let year_to_date = windowed_aggregated_metric(
            scoped_transactions,
            &flows.windowed_trades,
            &flows.dividends,
            asset_scope,
            year_start_baseline_value,
            end_value,
            year_start,
            cumulative_end,
        );

        let since_inception = Some(aggregated_metric(
            &flows.trades,
            &flows.dividends,
            asset_scope,
            0,
            end_value,
            earliest_date,
            cumulative_end,
        ));
        let bridge = summed_bridge(accounts, asset_scope, period_start, period_end);
        let pnl = residual_pnl(end_value, previous_end_value, &bridge, asset_scope);

        rows.push(PerformancePeriod {
            year,
            month: Some(month as u8),
            end_value,
            previous_value: previous_end_value,
            cash_flow: bridge.cash_flow,
            asset_flow: bridge.asset_flow,
            dividends: bridge.dividends,
            pnl,
            period_over_period,
            year_to_date,
            since_inception,
            // Annualized yield is a year-row concept only.
            annualized_yield: None,
        });
        previous_end_value = end_value;
    }
    rows.reverse();
    rows
}

#[cfg(test)]
mod bridge_reference_tests {
    use super::*;
    use crate::use_cases::shared::valuation::test_fixtures::{
        a_year_of_transactions, day, euro_rate_on_the_grant_date, priced_assets, transaction_on,
    };

    /// The dollar account of `transactions`, half a euro a dollar on every date.
    fn dollar_account(transactions: Vec<Transaction>) -> ConvertedAccount {
        let reference_rate_by_date = transactions
            .iter()
            .filter_map(|transaction| parse_date(&transaction.date))
            .map(|date| (date, 500_000))
            .collect();
        ConvertedAccount {
            currency: "USD".to_string(),
            reference_currency: "EUR".to_string(),
            transactions,
            priced_assets: priced_assets(),
            rate_map: euro_rate_on_the_grant_date(),
            reference_rate_by_date,
            trade_flows: Vec::new(),
            windowed_trade_flows: Vec::new(),
            dividend_flows: Vec::new(),
        }
    }

    // GPF-030 — the bridge of a period in the reference currency, each term converted at
    // the rate of its date (half a euro a dollar): cash (100 − 20) / 2; in kind the
    // opening balance 30 / 2; dividends 6 / 2. Interest and free shares are earned, no
    // flow. The first and the last day of the period count, the days around it do not.
    #[test]
    fn the_account_bridge_converts_each_term_at_the_rate_of_its_date() {
        let bridge = dollar_account(a_year_of_transactions()).bridge_reference(
            None,
            day("2024-01-01"),
            day("2024-12-31"),
        );
        assert_eq!(bridge.cash_flow, 40_000_000);
        assert_eq!(bridge.asset_flow, 15_000_000);
        assert_eq!(bridge.dividends, 3_000_000);
    }

    // PRF-084 — the bridge of one position: its purchases less its sales are its cash
    // flow, (40 − 10) / 2; its opening balance its in-kind flow, 30 / 2; its dividends
    // 6 / 2. Free shares and interest are earned, no flow; deposits are not the position's.
    #[test]
    fn the_position_bridge_takes_trades_as_cash_and_opening_balances_as_in_kind() {
        use TransactionType::*;
        let position = vec![
            transaction_on("stock-usd", Purchase, "2024-01-01", 40_000_000),
            transaction_on("stock-usd", Sell, "2024-02-01", 10_000_000),
            transaction_on("stock-usd", Dividend, "2024-04-01", 6_000_000),
            transaction_on("stock-usd", OpeningBalance, "2024-05-01", 3_000_000),
            transaction_on("stock-usd", FreeShares, "2024-06-01", 2_000_000),
            transaction_on("stock-usd", Interest, "2024-12-31", 1_000_000),
            transaction_on("system-cash-usd", Deposit, "2024-07-01", 900_000_000),
        ];
        let bridge = dollar_account(position).bridge_reference(
            Some("stock-usd"),
            day("2024-01-01"),
            day("2024-12-31"),
        );
        assert_eq!(bridge.cash_flow, 15_000_000);
        assert_eq!(bridge.asset_flow, 15_000_000);
        assert_eq!(bridge.dividends, 3_000_000);
    }
}
