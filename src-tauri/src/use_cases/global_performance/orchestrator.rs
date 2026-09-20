use crate::context::account::{
    Account, AccountError, AccountServiceContract, Transaction, TransactionType, UpdateFrequency,
};
use crate::context::asset::AssetServiceContract;
use crate::context::currency::CurrencyService;
use crate::core::logger::BACKEND;
use crate::use_cases::shared::global_value::REFERENCE_CURRENCY;
use crate::use_cases::shared::performance::{
    account_performance_series, annualized_yield_metric, residual_pnl, zero_cost_credit_value,
    AccountPerformanceResponse, PerformancePeriod, PeriodBridge,
};
use crate::use_cases::shared::valuation::{
    end_value_as_of, external_cash_flows, external_cash_flows_windowed, holding_close_date_as_of,
    holding_end_value_as_of, holding_performance_for_span_over_flows, load_priced_assets,
    load_rate_map_for_dates, market_valued_flow_dates, metric_for_span_over_flows, month_periods,
    opening_balance_flow_value, parse_date, position_flows, position_flows_windowed, year_periods,
    DatedFlow, MonthPeriod, PerformanceMetric, PricedAsset, RateMap, YearPeriod, MICRO,
};
use chrono::{Datelike, Local, NaiveDate};
use std::collections::{BTreeSet, HashMap};
use std::result::Result as StdResult;
use std::sync::Arc;

/// Identity conversion rate in micros for reference-currency accounts.
const IDENTITY_RATE_MICROS: i64 = 1_000_000;

/// Orchestrates the portfolio-wide performance read (GPF spec): all accounts —
/// or one asset's positions across all accounts — aggregated in the reference
/// currency, with the single-account scopes served by the shared performance
/// series engine (ADR-003, ADR-013).
pub struct GlobalPerformanceUseCase {
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
    currency_service: Arc<CurrencyService>,
}

impl GlobalPerformanceUseCase {
    /// Creates a new use case instance. The currency service resolves both the
    /// per-holding valuation rates (FXR-042/035) and the account-currency →
    /// reference-currency conversion (GPF-020/030).
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

    /// Computes per-period performance for the requested scope (GPF-010): with
    /// an `account_id` the read is the single-account series of
    /// `get_account_performance` (PRF-010–084) for that account and optional
    /// asset; without one, every account — or every position of the scoped
    /// asset — is aggregated in the reference currency (GPF-011–041).
    pub async fn get_global_performance(
        &self,
        account_id: Option<&str>,
        asset_id: Option<&str>,
    ) -> StdResult<AccountPerformanceResponse, AccountError> {
        match account_id {
            Some(account_id) => {
                account_performance_series(
                    self.account_service.as_ref(),
                    self.asset_service.as_ref(),
                    &self.currency_service,
                    account_id,
                    asset_id,
                )
                .await
            }
            None => self.aggregate_across_accounts(asset_id).await,
        }
    }

    /// GPF-012–041 — the cross-account aggregation: every included account's
    /// own-currency figures are computed with the single-account machinery and
    /// converted to the reference currency before summation.
    async fn aggregate_across_accounts(
        &self,
        asset_scope: Option<&str>,
    ) -> StdResult<AccountPerformanceResponse, AccountError> {
        let accounts = self.account_service.get_all().await?;
        let today = Local::now().date_naive();

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
            return Ok(empty_response());
        };

        // GPF-014 — the monthly series exists only when EVERY included account
        // is month-eligible (PRF-013).
        let month_view_available = included.iter().all(|(account, _)| {
            matches!(
                account.update_frequency,
                UpdateFrequency::Automatic
                    | UpdateFrequency::ManualDay
                    | UpdateFrequency::ManualWeek
            )
        });

        // Every date the aggregation values: yearly period ends, plus — when the
        // monthly series exists — monthly period ends and their prior-year-end
        // YTD baselines (FXR-035/042 pre-resolution set).
        let mut valued_dates: BTreeSet<NaiveDate> = BTreeSet::new();
        for period in year_periods(earliest_date, today) {
            valued_dates.insert(period.period_end);
        }
        if month_view_available {
            for period in month_periods(earliest_date, today) {
                valued_dates.insert(period.period_end);
                valued_dates.insert(period.year_start_baseline);
            }
        }
        let valued_dates: Vec<NaiveDate> = valued_dates.into_iter().collect();

        let mut converted_accounts = Vec::with_capacity(included.len());
        for (account, transactions) in included {
            converted_accounts.push(
                self.prepare_account(account, transactions, &valued_dates, asset_scope)
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
        let monthly = if month_view_available {
            build_monthly_aggregated(
                &converted_accounts,
                &flows,
                &scoped_transactions,
                asset_scope,
                earliest_date,
                today,
            )
        } else {
            Vec::new()
        };

        Ok(AccountPerformanceResponse {
            account_name: String::new(),
            currency: REFERENCE_CURRENCY.to_string(),
            month_view_available,
            yearly,
            monthly,
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
    ) -> StdResult<ConvertedAccount, AccountError> {
        let priced_assets = load_priced_assets(self.asset_service.as_ref(), &transactions).await?;
        // PRF-071 — zero-cost credits are valued at their grant date, so this
        // account's grant dates join the valued-date set for its own rate map.
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
        if account.currency != REFERENCE_CURRENCY {
            let mut dates: BTreeSet<NaiveDate> = valued_dates.iter().copied().collect();
            dates.extend(reference_rate_dates(&transactions, asset_scope));
            for date in dates {
                let as_of = date.format("%Y-%m-%d").to_string();
                if let Some(rate) = self
                    .currency_service
                    .resolve_rate_micros(&account.currency, REFERENCE_CURRENCY, &as_of)
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
        let (raw_trades, raw_windowed_trades, raw_dividends) = match asset_scope {
            None => (
                external_cash_flows(&converted_account.transactions),
                external_cash_flows_windowed(
                    &converted_account.transactions,
                    &converted_account.priced_assets,
                    &converted_account.rate_map,
                    &converted_account.currency,
                ),
                Vec::new(),
            ),
            Some(_) => {
                let asset_transactions: Vec<&Transaction> =
                    converted_account.transactions.iter().collect();
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
fn reference_rate_dates(
    transactions: &[Transaction],
    asset_scope: Option<&str>,
) -> BTreeSet<NaiveDate> {
    transactions
        .iter()
        .filter(|transaction| match transaction.transaction_type {
            TransactionType::Deposit | TransactionType::Withdrawal => asset_scope.is_none(),
            TransactionType::Purchase | TransactionType::Sell => asset_scope.is_some(),
            TransactionType::OpeningBalance
            | TransactionType::Dividend
            | TransactionType::FreeShares
            | TransactionType::Interest => true,
            // SPL-050 — a split is not a flow and needs no rate.
            TransactionType::ManagementFee | TransactionType::Split => false,
        })
        .filter_map(|transaction| parse_date(&transaction.date))
        .collect()
}

/// GPF-015 — the PRF-043-shaped empty result of an aggregation with no data span.
fn empty_response() -> AccountPerformanceResponse {
    AccountPerformanceResponse {
        account_name: String::new(),
        currency: REFERENCE_CURRENCY.to_string(),
        month_view_available: false,
        yearly: Vec::new(),
        monthly: Vec::new(),
    }
}

/// One included account with its preloaded valuation inputs and its dated flows
/// pre-converted to the reference currency (GPF-020/030).
struct ConvertedAccount {
    currency: String,
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
        if self.currency == REFERENCE_CURRENCY {
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
            Some(rate) => convert_amount(own_currency_value, rate),
            None => 0,
        }
    }

    /// GPF-040 — this account's bridge terms within `[period_start, period_end]`
    /// in reference-currency micros, mirroring the PRF-070–072 account bridge or
    /// the PRF-084 position bridge: every flow term converts at the rate of its
    /// own date — cash and dividend flows at their transaction date, an opening
    /// balance at its entry-date market value (PRF-086), and zero-cost in-kind
    /// credits at their grant date (PRF-071). A term with no usable rate
    /// contributes 0.
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
                    TransactionType::FreeShares => {
                        asset_flow += self.in_kind_credit_reference(transaction);
                    }
                    // INT-023 — interest on the cash line is a cash credit of
                    // `quantity`; on a non-cash asset it is an in-kind credit (INT-024).
                    TransactionType::Interest => {
                        if crate::core::cash::is_cash_asset(&transaction.asset_id) {
                            cash_flow +=
                                convert_or_zero(transaction.quantity, transaction_date_rate);
                        } else {
                            asset_flow += self.in_kind_credit_reference(transaction);
                        }
                    }
                    TransactionType::Dividend => {
                        dividends +=
                            convert_or_zero(transaction.total_amount, transaction_date_rate);
                    }
                    // SPL-050 — a split is not a flow.
                    TransactionType::Purchase
                    | TransactionType::Sell
                    | TransactionType::ManagementFee
                    | TransactionType::Split => {}
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
                    TransactionType::FreeShares | TransactionType::Interest => {
                        asset_flow += self.in_kind_credit_reference(transaction);
                    }
                    // SPL-050 — a split is not a flow.
                    TransactionType::Deposit
                    | TransactionType::Withdrawal
                    | TransactionType::ManagementFee
                    | TransactionType::Split => {}
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

    /// The reference-currency market value of a zero-cost in-kind credit as of
    /// its grant date (PRF-071 valuation converted at the grant-date rate,
    /// consistent with GPF-030's flow-at-its-own-date conversion).
    fn in_kind_credit_reference(&self, transaction: &Transaction) -> i128 {
        let Some(grant_date) = parse_date(&transaction.date) else {
            return 0;
        };
        let own_currency_value = zero_cost_credit_value(
            transaction,
            &self.priced_assets,
            &self.rate_map,
            &self.currency,
        );
        match self.reference_rate(grant_date) {
            Some(rate) => own_currency_value * rate as i128 / MICRO,
            None => 0,
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
                    amount: convert_amount(flow.amount, rate),
                })
            })
            .collect()
    }
}

/// Applies a micro-scaled conversion rate to an amount (ADR-001 i128 intermediate).
fn convert_amount(amount: i64, rate_micros: i64) -> i64 {
    let converted = amount as i128 * rate_micros as i128 / MICRO;
    debug_assert!(
        converted <= i64::MAX as i128 && converted >= i64::MIN as i128,
        "convert_amount i64 overflow: {converted}"
    );
    converted as i64
}

/// Applies a per-date conversion when a rate exists; contributes 0 otherwise
/// (GPF-030/040 missing-rate degradation).
fn convert_or_zero(amount: i64, rate_micros: Option<i64>) -> i128 {
    match rate_micros {
        Some(rate) => amount as i128 * rate as i128 / MICRO,
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
            Some(aggregated_metric(
                &flows.windowed_trades,
                &flows.dividends,
                asset_scope,
                previous_end_value,
                end_value,
                period_start,
                period_end,
            ))
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
            Some(aggregated_metric(
                &flows.windowed_trades,
                &flows.dividends,
                asset_scope,
                previous_end_value,
                end_value,
                period_start,
                period_end,
            ))
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
        let year_to_date = Some(aggregated_metric(
            &flows.windowed_trades,
            &flows.dividends,
            asset_scope,
            year_start_baseline_value,
            end_value,
            year_start,
            cumulative_end,
        ));

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
mod tests {
    use super::*;
    use crate::context::account::{
        AccountService, SqliteAccountRepository, SqliteHoldingRepository,
        SqliteTransactionRepository,
    };
    use crate::context::asset::{
        AssetService, CreateAssetDTO, SqliteAssetCategoryRepository, SqliteAssetPriceRepository,
        SqliteAssetRepository, SYSTEM_CATEGORY_ID,
    };
    use crate::context::currency::domain::{
        CurrencyRate, CurrencyRateSource, MockCurrencyPairRepository, MockCurrencyRateRepository,
    };
    use crate::use_cases::account_performance::AccountPerformanceUseCase;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn make_pool() -> sqlx::Pool<sqlx::Sqlite> {
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

    async fn setup(pool: &sqlx::Pool<sqlx::Sqlite>) -> (Arc<AccountService>, Arc<AssetService>) {
        let account_svc = Arc::new(AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        ));
        let asset_svc = Arc::new(AssetService::new(
            Box::new(SqliteAssetRepository::new(pool.clone())),
            Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
            Box::new(SqliteAssetPriceRepository::new(pool.clone())),
        ));
        (account_svc, asset_svc)
    }

    fn make_currency_service_with_no_rate() -> Arc<CurrencyService> {
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_latest_rate_on_or_before()
            .times(0..)
            .returning(|_, _, _| Ok(None));
        Arc::new(CurrencyService::new(
            Box::new(MockCurrencyPairRepository::new()),
            Box::new(rate_repo),
        ))
    }

    // USD→EUR observations: 0.80 before June 2024, 0.90 from June 2024 onward —
    // lets a test distinguish the transaction-date conversion of flows (GPF-030)
    // from the period-end conversion of end values (GPF-020).
    fn make_currency_service_with_usd_rate_by_date() -> Arc<CurrencyService> {
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_latest_rate_on_or_before()
            .times(0..)
            .returning(|from_currency, to_currency, as_of| {
                let rate_micros = if as_of < "2024-06-01" {
                    800_000
                } else {
                    900_000
                };
                Ok(Some(CurrencyRate::from_storage(
                    from_currency.to_string(),
                    to_currency.to_string(),
                    as_of.to_string(),
                    rate_micros,
                    CurrencyRateSource::Manual,
                )))
            });
        Arc::new(CurrencyService::new(
            Box::new(MockCurrencyPairRepository::new()),
            Box::new(rate_repo),
        ))
    }

    async fn create_account(
        account_svc: &AccountService,
        name: &str,
        currency: &str,
        update_frequency: UpdateFrequency,
    ) -> crate::context::account::Account {
        account_svc
            .create(
                name.to_string(),
                String::new(),
                currency.to_string(),
                update_frequency,
                false,
            )
            .await
            .expect("account created")
    }

    async fn create_stock(asset_svc: &AssetService, name: &str, reference: &str) -> String {
        asset_svc
            .create_asset(CreateAssetDTO {
                name: name.to_string(),
                reference: reference.to_string(),
                isin: None,
                class: crate::context::asset::AssetClass::Stocks,
                currency: "EUR".to_string(),
                risk_level: 1,
                category_id: SYSTEM_CATEGORY_ID.to_string(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .expect("stock created")
            .id
    }

    fn year_row(response: &AccountPerformanceResponse, year: i32) -> &PerformancePeriod {
        response
            .yearly
            .iter()
            .find(|row| row.year == year)
            .expect("year row present")
    }

    // GPF-020 — two reference-currency accounts sum exactly, with no conversion.
    #[tokio::test]
    async fn two_reference_currency_accounts_sum_into_one_series() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        let first = create_account(&account_svc, "First", "EUR", UpdateFrequency::Automatic).await;
        let second =
            create_account(&account_svc, "Second", "EUR", UpdateFrequency::Automatic).await;
        account_svc
            .record_deposit(&first.id, "2024-03-01".to_string(), 1_000_000_000, None)
            .await
            .unwrap();
        account_svc
            .record_deposit(&second.id, "2024-05-01".to_string(), 500_000_000, None)
            .await
            .unwrap();

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let response = uc.get_global_performance(None, None).await.unwrap();

        assert_eq!(response.account_name, "", "GPF-011: no backend label");
        assert_eq!(response.currency, "EUR", "GPF-011: reference currency");
        let row = year_row(&response, 2024);
        assert_eq!(
            row.end_value, 1_500_000_000,
            "end values of both accounts sum exactly"
        );
        assert_eq!(row.cash_flow, 1_500_000_000, "both deposits in the period");
        let since_inception = row.since_inception.as_ref().expect("since_inception");
        assert_eq!(since_inception.gain, 0, "deposits only → no gain");
    }

    // GPF-020/030 — a foreign account's end value converts at the period-end
    // rate (0.90) while its deposit converts at the transaction-date rate
    // (0.80); the difference is currency movement and lands in the pnl residual.
    #[tokio::test]
    async fn foreign_account_converts_end_value_at_period_end_and_flows_at_transaction_date() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("USD").await.unwrap();
        let account =
            create_account(&account_svc, "Foreign", "USD", UpdateFrequency::Automatic).await;
        account_svc
            .record_deposit(&account.id, "2024-03-15".to_string(), 1_000_000_000, None)
            .await
            .unwrap();

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_usd_rate_by_date(),
        );
        let response = uc.get_global_performance(None, None).await.unwrap();

        let row = year_row(&response, 2024);
        assert_eq!(
            row.end_value, 900_000_000,
            "1000 USD cash × 0.90 (rate as of 2024-12-31)"
        );
        assert_eq!(
            row.cash_flow, 800_000_000,
            "1000 USD deposit × 0.80 (rate as of 2024-03-15)"
        );
        assert_eq!(row.asset_flow, 0);
        assert_eq!(row.dividends, 0);
        assert_eq!(
            row.pnl, 100_000_000,
            "currency movement is the bridge residual"
        );
        assert_eq!(
            row.end_value,
            row.previous_value + row.cash_flow + row.asset_flow + row.dividends + row.pnl,
            "GPF-041 bridge identity balances"
        );
        let since_inception = row.since_inception.as_ref().expect("since_inception");
        assert_eq!(
            since_inception.gain, 100_000_000,
            "gain vs converted net invested"
        );
    }

    // GPF-020/030 — a foreign account with no usable rate contributes 0 to every
    // period and its flows are dropped (FXR-034 spirit).
    #[tokio::test]
    async fn foreign_account_without_usable_rate_contributes_zero() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        asset_svc.seed_cash_asset("USD").await.unwrap();
        let domestic =
            create_account(&account_svc, "Domestic", "EUR", UpdateFrequency::Automatic).await;
        let foreign =
            create_account(&account_svc, "Foreign", "USD", UpdateFrequency::Automatic).await;
        account_svc
            .record_deposit(&domestic.id, "2024-02-10".to_string(), 500_000_000, None)
            .await
            .unwrap();
        account_svc
            .record_deposit(&foreign.id, "2024-03-15".to_string(), 1_000_000_000, None)
            .await
            .unwrap();

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let response = uc.get_global_performance(None, None).await.unwrap();

        let row = year_row(&response, 2024);
        assert_eq!(
            row.end_value, 500_000_000,
            "only the EUR account contributes"
        );
        assert_eq!(
            row.cash_flow, 500_000_000,
            "the unconvertible USD deposit contributes 0"
        );
        assert_eq!(row.pnl, 0, "no residual from the excluded contribution");
        let since_inception = row.since_inception.as_ref().expect("since_inception");
        assert_eq!(since_inception.gain, 0);
    }

    // GPF-014 — one month-ineligible included account disables the monthly series.
    #[tokio::test]
    async fn month_view_unavailable_when_any_included_account_is_month_ineligible() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        let eligible =
            create_account(&account_svc, "Eligible", "EUR", UpdateFrequency::Automatic).await;
        let ineligible = create_account(
            &account_svc,
            "Ineligible",
            "EUR",
            UpdateFrequency::ManualMonth,
        )
        .await;
        account_svc
            .record_deposit(&eligible.id, "2024-03-01".to_string(), 100_000_000, None)
            .await
            .unwrap();
        account_svc
            .record_deposit(&ineligible.id, "2024-04-01".to_string(), 100_000_000, None)
            .await
            .unwrap();

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let response = uc.get_global_performance(None, None).await.unwrap();

        assert!(
            !response.month_view_available,
            "AND over included accounts: ManualMonth disables the month view"
        );
        assert!(response.monthly.is_empty(), "no monthly series");
        assert!(!response.yearly.is_empty(), "yearly series always present");
    }

    // GPF-014 — the monthly series exists when every included account is eligible.
    #[tokio::test]
    async fn month_view_available_when_all_included_accounts_are_month_eligible() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        let first = create_account(&account_svc, "First", "EUR", UpdateFrequency::Automatic).await;
        let second =
            create_account(&account_svc, "Second", "EUR", UpdateFrequency::ManualWeek).await;
        account_svc
            .record_deposit(&first.id, "2024-03-01".to_string(), 100_000_000, None)
            .await
            .unwrap();
        account_svc
            .record_deposit(&second.id, "2024-04-01".to_string(), 100_000_000, None)
            .await
            .unwrap();

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let response = uc.get_global_performance(None, None).await.unwrap();

        assert!(response.month_view_available);
        assert!(!response.monthly.is_empty());
    }

    // GPF-012 — an account with no in-scope transactions is excluded from the
    // aggregation AND from the month-view eligibility.
    #[tokio::test]
    async fn accounts_without_in_scope_transactions_are_excluded() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        let active =
            create_account(&account_svc, "Active", "EUR", UpdateFrequency::Automatic).await;
        create_account(&account_svc, "Dormant", "EUR", UpdateFrequency::ManualMonth).await;
        account_svc
            .record_deposit(&active.id, "2024-03-01".to_string(), 100_000_000, None)
            .await
            .unwrap();

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let response = uc.get_global_performance(None, None).await.unwrap();

        assert!(
            response.month_view_available,
            "the transaction-less ManualMonth account does not drag eligibility"
        );
    }

    // GPF-010 — (Some, None) returns the exact single-account read.
    #[tokio::test]
    async fn account_scoped_read_matches_account_performance() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        let account =
            create_account(&account_svc, "Delegated", "EUR", UpdateFrequency::Automatic).await;
        account_svc
            .record_deposit(&account.id, "2024-03-01".to_string(), 12_500_000_000, None)
            .await
            .unwrap();
        let stock = create_stock(&asset_svc, "Delegated Stock", "DLG").await;
        account_svc
            .buy_holding(
                &account.id,
                stock.clone(),
                "2024-03-01".to_string(),
                10_000_000,
                1_000_000_000,
                1_000_000,
                0,
                None,
                None,
            )
            .await
            .unwrap();
        asset_svc
            .record_asset_price(&stock, "2024-03-31", 1350.0)
            .await
            .unwrap();
        account_svc
            .record_dividend(
                &account.id,
                stock.clone(),
                "2024-06-01".to_string(),
                200_000_000,
                1_000_000,
                None,
            )
            .await
            .unwrap();

        let account_uc = AccountPerformanceUseCase::new(
            account_svc.clone(),
            asset_svc.clone(),
            make_currency_service_with_no_rate(),
        );
        let global_uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );

        let expected = account_uc
            .get_account_performance(&account.id, None)
            .await
            .unwrap();
        let actual = global_uc
            .get_global_performance(Some(&account.id), None)
            .await
            .unwrap();
        assert_eq!(
            serde_json::to_value(&actual).unwrap(),
            serde_json::to_value(&expected).unwrap(),
            "account-scoped global read must equal get_account_performance"
        );
    }

    // GPF-010 — (Some, Some) returns the exact single-account asset-scoped read.
    #[tokio::test]
    async fn account_and_asset_scoped_read_matches_account_performance() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        let account =
            create_account(&account_svc, "Delegated", "EUR", UpdateFrequency::Automatic).await;
        account_svc
            .record_deposit(&account.id, "2024-03-01".to_string(), 12_500_000_000, None)
            .await
            .unwrap();
        let stock = create_stock(&asset_svc, "Delegated Stock", "DLG").await;
        account_svc
            .buy_holding(
                &account.id,
                stock.clone(),
                "2024-03-01".to_string(),
                10_000_000,
                1_000_000_000,
                1_000_000,
                0,
                None,
                None,
            )
            .await
            .unwrap();
        asset_svc
            .record_asset_price(&stock, "2024-03-31", 1350.0)
            .await
            .unwrap();

        let account_uc = AccountPerformanceUseCase::new(
            account_svc.clone(),
            asset_svc.clone(),
            make_currency_service_with_no_rate(),
        );
        let global_uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );

        let expected = account_uc
            .get_account_performance(&account.id, Some(&stock))
            .await
            .unwrap();
        let actual = global_uc
            .get_global_performance(Some(&account.id), Some(&stock))
            .await
            .unwrap();
        assert_eq!(
            serde_json::to_value(&actual).unwrap(),
            serde_json::to_value(&expected).unwrap(),
            "asset-scoped global read must equal get_account_performance"
        );
    }

    // GPF-020/030 (asset scope) — the scoped read sums the asset's positions
    // across every account holding it.
    #[tokio::test]
    async fn asset_scope_sums_positions_across_accounts() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        let first = create_account(&account_svc, "First", "EUR", UpdateFrequency::Automatic).await;
        let second =
            create_account(&account_svc, "Second", "EUR", UpdateFrequency::Automatic).await;
        let stock = create_stock(&asset_svc, "Shared Stock", "SHR").await;
        for account_id in [&first.id, &second.id] {
            account_svc
                .record_deposit(account_id, "2024-03-01".to_string(), 2_000_000_000, None)
                .await
                .unwrap();
            account_svc
                .buy_holding(
                    account_id,
                    stock.clone(),
                    "2024-03-01".to_string(),
                    10_000_000,
                    100_000_000,
                    1_000_000,
                    0,
                    None,
                    None,
                )
                .await
                .unwrap();
        }
        asset_svc
            .record_asset_price(&stock, "2024-06-30", 120.0)
            .await
            .unwrap();

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let response = uc.get_global_performance(None, Some(&stock)).await.unwrap();

        let row = year_row(&response, 2024);
        assert_eq!(
            row.end_value, 2_400_000_000,
            "2 positions × 10 units × 120 EUR"
        );
        assert_eq!(
            row.cash_flow, 2_000_000_000,
            "both purchases are position inflows (PRF-084)"
        );
        assert_eq!(row.pnl, 400_000_000, "price appreciation across positions");
        assert_eq!(
            row.end_value,
            row.previous_value + row.cash_flow + row.asset_flow + row.pnl,
            "scoped bridge identity (dividends outside, PRF-084)"
        );
        let since_inception = row.since_inception.as_ref().expect("since_inception");
        assert_eq!(since_inception.gain, 400_000_000);
    }

    // GPF-015 — an empty portfolio produces the PRF-043-shaped empty response.
    #[tokio::test]
    async fn empty_portfolio_returns_empty_reference_currency_response() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let response = uc.get_global_performance(None, None).await.unwrap();

        assert_eq!(response.account_name, "");
        assert_eq!(response.currency, "EUR");
        assert!(!response.month_view_available);
        assert!(response.yearly.is_empty());
        assert!(response.monthly.is_empty());
    }

    // GPF-012/015 — an asset scope no account holds excludes every account and
    // yields the empty response.
    #[tokio::test]
    async fn unknown_asset_scope_returns_empty_response() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        let account =
            create_account(&account_svc, "Holder", "EUR", UpdateFrequency::Automatic).await;
        account_svc
            .record_deposit(&account.id, "2024-03-01".to_string(), 100_000_000, None)
            .await
            .unwrap();

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let response = uc
            .get_global_performance(None, Some("missing-asset"))
            .await
            .unwrap();

        assert!(response.yearly.is_empty());
        assert!(response.monthly.is_empty());
        assert_eq!(response.currency, "EUR");
    }

    // GPF-013 — the aggregation spans from the earliest transaction across all
    // included accounts; an account contributes 0 to every period before its
    // own first transaction.
    #[tokio::test]
    async fn later_starting_account_contributes_zero_before_its_first_transaction() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        let older = create_account(&account_svc, "Older", "EUR", UpdateFrequency::Automatic).await;
        let newer = create_account(&account_svc, "Newer", "EUR", UpdateFrequency::Automatic).await;
        account_svc
            .record_deposit(&older.id, "2023-03-01".to_string(), 500_000_000, None)
            .await
            .unwrap();
        account_svc
            .record_deposit(&newer.id, "2025-02-01".to_string(), 1_000_000_000, None)
            .await
            .unwrap();

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let response = uc.get_global_performance(None, None).await.unwrap();

        let row_2023 = year_row(&response, 2023);
        assert_eq!(
            row_2023.end_value, 500_000_000,
            "before the newer account's start, the older account's value stands alone"
        );
        assert_eq!(row_2023.cash_flow, 500_000_000);
        let row_2024 = year_row(&response, 2024);
        assert_eq!(
            row_2024.end_value, 500_000_000,
            "the newer account still contributes 0 in 2024"
        );
        assert_eq!(row_2024.cash_flow, 0, "no flows dated 2024");
        let row_2025 = year_row(&response, 2025);
        assert_eq!(
            row_2025.end_value, 1_500_000_000,
            "both accounts contribute from the newer account's first transaction on"
        );
        assert_eq!(row_2025.cash_flow, 1_000_000_000);
    }

    // GPF-030/040 — the reference-rate date set only contains flow-relevant
    // dates per scope: Deposit/Withdrawal convert only unscoped, Purchase/Sell
    // only scoped, ManagementFee never.
    #[test]
    fn reference_rate_dates_narrow_to_flow_relevant_types_per_scope() {
        let tx = |transaction_type: TransactionType, date: &str| {
            Transaction::new(
                "acc".to_string(),
                "asset".to_string(),
                transaction_type,
                date.to_string(),
                1_000_000,
                1_000_000,
                1_000_000,
                0,
                1_000_000,
                None,
                None,
            )
            .unwrap()
        };
        let transactions = vec![
            tx(TransactionType::Deposit, "2024-01-01"),
            tx(TransactionType::Purchase, "2024-02-01"),
            tx(TransactionType::Sell, "2024-03-01"),
            tx(TransactionType::Dividend, "2024-04-01"),
            tx(TransactionType::ManagementFee, "2024-05-01"),
            tx(TransactionType::OpeningBalance, "2024-06-01"),
        ];
        let date = |raw: &str| parse_date(raw).unwrap();

        let unscoped = reference_rate_dates(&transactions, None);
        assert!(unscoped.contains(&date("2024-01-01")), "deposit converts");
        assert!(
            !unscoped.contains(&date("2024-02-01")) && !unscoped.contains(&date("2024-03-01")),
            "unscoped trades convert nothing"
        );
        assert!(unscoped.contains(&date("2024-04-01")), "dividend converts");
        assert!(
            !unscoped.contains(&date("2024-05-01")),
            "a management fee is never a flow"
        );
        assert!(
            unscoped.contains(&date("2024-06-01")),
            "opening balance converts at its own date"
        );

        let scoped = reference_rate_dates(&transactions, Some("asset"));
        assert!(
            !scoped.contains(&date("2024-01-01")),
            "deposits never touch a position"
        );
        assert!(
            scoped.contains(&date("2024-02-01")) && scoped.contains(&date("2024-03-01")),
            "scoped trade flows convert at their own date"
        );
    }

    // GPF-040/PRF-071 — a foreign account's zero-cost in-kind credit (FreeShares)
    // lands in asset_flow valued at its GRANT date and converted at the
    // grant-date rate (0.80), consistent with GPF-030's flow-at-its-own-date
    // conversion; the period-end rate (0.90) applies only to end values.
    #[tokio::test]
    async fn foreign_in_kind_credit_converts_at_grant_date_rate() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("USD").await.unwrap();
        let account =
            create_account(&account_svc, "Foreign", "USD", UpdateFrequency::Automatic).await;
        // A USD-currency stock inside the USD account: no asset→account
        // conversion, so the only FX leg is USD→EUR (GPF-020/030/040).
        let stock = asset_svc
            .create_asset(CreateAssetDTO {
                name: "US Stock".to_string(),
                reference: "UST".to_string(),
                isin: None,
                class: crate::context::asset::AssetClass::Stocks,
                currency: "USD".to_string(),
                risk_level: 1,
                category_id: SYSTEM_CATEGORY_ID.to_string(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .expect("stock created")
            .id;
        account_svc
            .record_deposit(&account.id, "2024-03-15".to_string(), 2_000_000_000, None)
            .await
            .unwrap();
        account_svc
            .buy_holding(
                &account.id,
                stock.clone(),
                "2024-03-15".to_string(),
                10_000_000,
                100_000_000,
                1_000_000,
                0,
                None,
                None,
            )
            .await
            .unwrap();
        asset_svc
            .record_asset_price(&stock, "2024-03-20", 100.0)
            .await
            .unwrap();
        account_svc
            .record_free_shares(
                &account.id,
                stock.clone(),
                "2024-03-20".to_string(),
                5_000_000,
                None,
            )
            .await
            .unwrap();
        asset_svc
            .record_asset_price(&stock, "2024-03-31", 100.0)
            .await
            .unwrap();

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_usd_rate_by_date(),
        );
        let response = uc.get_global_performance(None, None).await.unwrap();

        let row = year_row(&response, 2024);
        // 5 free shares × $100 (grant-date price, 2024-03-20) × 0.80 (USD→EUR
        // rate as of the grant date) = 400 EUR. A period-end (0.90) conversion
        // would read 450 EUR.
        assert_eq!(
            row.asset_flow, 400_000_000,
            "in-kind credit valued and converted at its grant date"
        );
        // 2000 USD deposit × 0.80 (rate as of 2024-03-15).
        assert_eq!(row.cash_flow, 1_600_000_000);
        // Cash 1000 USD + 15 shares × $100 = 2500 USD × 0.90 (period-end rate).
        assert_eq!(row.end_value, 2_250_000_000);
        assert_eq!(row.dividends, 0);
        assert_eq!(
            row.end_value,
            row.previous_value + row.cash_flow + row.asset_flow + row.dividends + row.pnl,
            "GPF-041 bridge identity balances"
        );
    }
}
