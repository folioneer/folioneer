//! Single-account performance series engine (PRF spec, ADR-013): per-period end
//! values, Global Value bridge terms, and Simple Dietz metrics, recomputed on
//! read. Composed by both the `account_performance` and `global_performance` use
//! cases, so neither imports from the other (B18).

use crate::context::account::{
    AccountError, AccountServiceContract, Transaction, TransactionType, UpdateFrequency,
};
use crate::context::asset::{AssetClass, AssetServiceContract};
use crate::context::currency::CurrencyService;
use crate::use_cases::shared::valuation::{
    end_value_as_of, holding_close_date_as_of, holding_end_value_as_of,
    holding_performance_for_span, holding_performance_for_span_windowed, load_priced_assets,
    load_rate_map, metric_for_span, metric_for_span_windowed, month_periods,
    opening_balance_flow_value, parse_date, year_periods, MonthPeriod, PerformanceMetric,
    PricedAsset, RateMap, YearPeriod, MICRO, PERCENT_SCALE,
};
use chrono::{Datelike, Local, NaiveDate};
use serde::Serialize;
use specta::Type;
use std::collections::HashMap;
use std::result::Result as StdResult;

/// Average days per calendar year (leap-year aware) used to convert an elapsed
/// span into fractional years for the annualized-yield calculation.
const DAYS_PER_YEAR: f64 = 365.25;

/// One calendar period row (PRF-020, PRF-040). In an asset-scoped read
/// (PRF-080) every value describes the scoped position instead of the whole
/// account: `end_value` per PRF-082, the metrics per PRF-083, the bridge terms
/// per PRF-084 (where `dividends` sits outside the bridge identity).
#[derive(Debug, Serialize, Clone, Type)]
pub struct PerformancePeriod {
    /// Calendar year of this row.
    pub year: i32,
    /// Some(1..=12) for month rows; None for year rows (PRF-011).
    pub month: Option<u8>,
    /// Global Value at period end in account-currency micros (PRF-020).
    /// Bridge identity: `end_value = previous_value + cash_flow + asset_flow + dividends + pnl` (PRF-074).
    pub end_value: i64,
    /// Global Value at the previous period end — the bridge baseline (PRF-074). 0 for the first period.
    pub previous_value: i64,
    /// Net external cash flow within the period: deposits − withdrawals, account-currency micros (PRF-070).
    pub cash_flow: i64,
    /// In-kind asset contributions within the period: opening-balance cost + zero-cost credits at grant-date market value (PRF-071).
    pub asset_flow: i64,
    /// Dividend income received within the period, account-currency micros (PRF-072).
    pub dividends: i64,
    /// Investment P&L vs the previous period (realized gains + price movement); the bridge residual (PRF-073).
    pub pnl: i64,
    /// Performance vs the preceding period of the same granularity (PRF-033).
    /// None when no preceding period exists (PRF-042).
    pub period_over_period: Option<PerformanceMetric>,
    /// Performance from the start of the calendar year to this period end (PRF-034).
    /// None for year rows (PRF-037) or when the year-start baseline is absent (PRF-034).
    pub year_to_date: Option<PerformanceMetric>,
    /// Performance from inception to this period end, vs net invested (PRF-035).
    pub since_inception: Option<PerformanceMetric>,
    /// Annualized cumulative since-inception return (CAGR) — populated only for
    /// year rows; None for month rows. `pct` is the annualized rate; `gain`
    /// reuses the cumulative since-inception gain. None when the since-inception
    /// percentage is absent or the cumulative is a total loss (root undefined).
    pub annualized_yield: Option<PerformanceMetric>,
}

/// Top-level response for `get_account_performance` — recomputed on read
/// (ADR-013). Also returned by `get_global_performance`, whose cross-account
/// aggregation reports in the reference currency with an empty `account_name`
/// the frontend resolves to a display label (GPF-011).
#[derive(Debug, Serialize, Clone, Type)]
pub struct AccountPerformanceResponse {
    /// Display name of the account; empty for a cross-account aggregation (GPF-011).
    pub account_name: String,
    /// ISO 4217 currency code every figure is reported in.
    pub currency: String,
    /// True only for Automatic/ManualDay/ManualWeek (PRF-013).
    pub month_view_available: bool,
    /// One row per year, most-recent first (PRF-041). month is None for each row.
    pub yearly: Vec<PerformancePeriod>,
    /// One row per month over the full span, most-recent first.
    /// Empty when month_view_available is false (PRF-013, PRF-015).
    pub monthly: Vec<PerformancePeriod>,
}

/// Computes per-period performance for a single account (PRF-016, PRF-020–035,
/// PRF-040–043), optionally scoped to one asset's position (PRF-080–084).
/// Orchestrates a cross-context read of account transactions and asset price
/// history (ADR-003, ADR-013).
pub(crate) async fn account_performance_series(
    account_service: &dyn AccountServiceContract,
    asset_service: &dyn AssetServiceContract,
    currency_service: &CurrencyService,
    account_id: &str,
    asset_id: Option<&str>,
) -> StdResult<AccountPerformanceResponse, AccountError> {
    let account = account_service
        .get_by_id(account_id)
        .await?
        .ok_or_else(|| AccountError::AccountNotFound {
            account_id: account_id.to_string(),
        })?;

    let month_view_available = matches!(
        account.update_frequency,
        UpdateFrequency::Automatic | UpdateFrequency::ManualDay | UpdateFrequency::ManualWeek
    );

    let transactions = account_service
        .get_all_transactions_for_account(account_id)
        .await?;
    // PRF-080/081 — an asset scope narrows the whole computation to that one
    // asset's transactions; every downstream figure derives from this set.
    let transactions: Vec<Transaction> = match asset_id {
        None => transactions,
        Some(asset_id) => transactions
            .into_iter()
            .filter(|transaction| transaction.asset_id == asset_id)
            .collect(),
    };

    // PRF-043 / PRF-081 — no transactions in scope means no data span and an
    // empty result.
    let earliest_date = match transactions
        .iter()
        .filter_map(|t| parse_date(&t.date))
        .min()
    {
        Some(date) => date,
        None => {
            return Ok(AccountPerformanceResponse {
                account_name: account.name,
                currency: account.currency,
                month_view_available,
                yearly: Vec::new(),
                monthly: Vec::new(),
            })
        }
    };

    let priced_assets = load_priced_assets(asset_service, &transactions).await?;

    let today = Local::now().date_naive();
    // FXR-042/035 — pre-resolve FX rates for every foreign holding currency at
    // each period-end the synchronous valuation loop will visit.
    let rate_map = load_rate_map(
        currency_service,
        &priced_assets,
        &account.currency,
        &transactions,
        month_view_available,
        earliest_date,
        today,
    )
    .await?;
    let yearly = build_yearly(
        &transactions,
        &priced_assets,
        &rate_map,
        &account.currency,
        earliest_date,
        today,
        asset_id,
    );
    let monthly = if month_view_available {
        build_monthly(
            &transactions,
            &priced_assets,
            &rate_map,
            &account.currency,
            earliest_date,
            today,
            asset_id,
        )
    } else {
        Vec::new()
    };

    Ok(AccountPerformanceResponse {
        account_name: account.name,
        currency: account.currency,
        month_view_available,
        yearly,
        monthly,
    })
}

/// Builds the yearly series, most-recent first (PRF-012, PRF-040, PRF-041).
#[allow(clippy::too_many_arguments)]
fn build_yearly(
    transactions: &[Transaction],
    priced_assets: &HashMap<String, PricedAsset>,
    rate_map: &RateMap,
    account_currency: &str,
    earliest_date: NaiveDate,
    today: NaiveDate,
    asset_scope: Option<&str>,
) -> Vec<PerformancePeriod> {
    // PRF-040 — the span opens on the period containing the first transaction.
    // PRF-042 — that earliest row has no preceding period, so its
    // period_over_period is None; its performance is carried by since_inception.
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
        let end_value = end_value_for_scope(
            transactions,
            priced_assets,
            rate_map,
            account_currency,
            period_end,
            asset_scope,
        );

        let period_over_period = if year == first_year {
            None
        } else {
            Some(windowed_metric_for_scope(
                transactions,
                priced_assets,
                rate_map,
                account_currency,
                previous_end_value,
                end_value,
                period_start,
                period_end,
                asset_scope,
            ))
        };
        // PRF-085 — a closed scoped position freezes its cumulative metrics at
        // the close date, so the Dietz weights stop shifting after the close.
        let cumulative_end = cumulative_metric_end(transactions, period_end, asset_scope);
        let since_inception = Some(since_inception_metric(
            transactions,
            end_value,
            earliest_date,
            cumulative_end,
            asset_scope,
        ));
        // Annualize the cumulative since-inception return over the elapsed years.
        let annualized_yield = since_inception
            .as_ref()
            .and_then(|metric| annualized_yield_metric(metric, earliest_date, cumulative_end));
        let bridge = bridge_for_scope(
            transactions,
            priced_assets,
            rate_map,
            account_currency,
            period_start,
            period_end,
            asset_scope,
        );
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

/// Builds the monthly series over the full span, most-recent first (PRF-040, PRF-041).
#[allow(clippy::too_many_arguments)]
fn build_monthly(
    transactions: &[Transaction],
    priced_assets: &HashMap<String, PricedAsset>,
    rate_map: &RateMap,
    account_currency: &str,
    earliest_date: NaiveDate,
    today: NaiveDate,
    asset_scope: Option<&str>,
) -> Vec<PerformancePeriod> {
    // PRF-040 — the span opens on the month containing the first transaction.
    // PRF-042 — that earliest row has no preceding period, so its
    // period_over_period is None; its performance is carried by since_inception.
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
        let end_value = end_value_for_scope(
            transactions,
            priced_assets,
            rate_map,
            account_currency,
            period_end,
            asset_scope,
        );

        let is_first_period = year == first_year && month == first_month;
        let period_over_period = if is_first_period {
            None
        } else {
            Some(windowed_metric_for_scope(
                transactions,
                priced_assets,
                rate_map,
                account_currency,
                previous_end_value,
                end_value,
                period_start,
                period_end,
                asset_scope,
            ))
        };

        // PRF-085 — a closed scoped position freezes its cumulative metrics at
        // the close date, so the Dietz weights stop shifting after the close.
        let cumulative_end = cumulative_metric_end(transactions, period_end, asset_scope);

        // PRF-034 — year-to-date baseline is the prior 31 December end value.
        let year_start_baseline_value = end_value_for_scope(
            transactions,
            priced_assets,
            rate_map,
            account_currency,
            year_start_baseline,
            asset_scope,
        );
        let year_to_date = Some(windowed_metric_for_scope(
            transactions,
            priced_assets,
            rate_map,
            account_currency,
            year_start_baseline_value,
            end_value,
            year_start,
            cumulative_end,
            asset_scope,
        ));

        let since_inception = Some(since_inception_metric(
            transactions,
            end_value,
            earliest_date,
            cumulative_end,
            asset_scope,
        ));
        let bridge = bridge_for_scope(
            transactions,
            priced_assets,
            rate_map,
            account_currency,
            period_start,
            period_end,
            asset_scope,
        );
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

/// The flow terms of the per-period Global Value bridge (PRF-070–072), in
/// account-currency micros. The remaining term, `pnl`, is the residual the caller
/// derives so the bridge `end_value = previous_value + cash_flow + asset_flow +
/// dividends + pnl` balances exactly (PRF-073/074).
pub(crate) struct PeriodBridge {
    /// Deposits − withdrawals within the period (PRF-070).
    pub(crate) cash_flow: i64,
    /// Opening-balance cost + zero-cost credits at grant-date market value within the period (PRF-071).
    pub(crate) asset_flow: i64,
    /// Dividend income received within the period (PRF-072).
    pub(crate) dividends: i64,
}

/// PRF-070/071/072 — the cash, in-kind-asset and dividend flows occurring within
/// `[period_start, period_end]`. Purchases and sells are cash↔asset swaps and carry
/// no bridge flow — a sell's only lasting Global Value effect (its realized gain)
/// surfaces in the residual `pnl` instead.
fn period_bridge(
    transactions: &[Transaction],
    priced_assets: &HashMap<String, PricedAsset>,
    rate_map: &RateMap,
    account_currency: &str,
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> PeriodBridge {
    let mut cash_flow: i128 = 0;
    let mut asset_flow: i128 = 0;
    let mut dividends: i128 = 0;

    for transaction in transactions {
        let within = matches!(
            parse_date(&transaction.date),
            Some(date) if date >= period_start && date <= period_end
        );
        if !within {
            continue;
        }
        match transaction.transaction_type {
            TransactionType::Deposit => cash_flow += transaction.total_amount as i128,
            TransactionType::Withdrawal => cash_flow -= transaction.total_amount as i128,
            // PRF-086 — an opening balance contributes its entry-date market
            // value (fallback typed cost), keeping the transfer pnl-neutral.
            TransactionType::OpeningBalance => {
                asset_flow += opening_balance_flow_value(
                    transaction,
                    priced_assets,
                    rate_map,
                    account_currency,
                ) as i128;
            }
            // FSD-070/PRF-071 — free shares carry no cost, so their grant-date
            // market value is the in-kind contribution (valued like `end_value_as_of`).
            TransactionType::FreeShares => {
                asset_flow +=
                    zero_cost_credit_value(transaction, priced_assets, rate_map, account_currency);
            }
            // INT-024 — interest on a non-cash asset is an in-kind credit valued like
            // free shares (FSD-070). INT-023 — interest on the cash line IS a cash
            // credit of `quantity` (`zero_cost_credit_value` cannot value the Cash Asset —
            // it returns 0 for the Cash class).
            TransactionType::Interest => {
                if crate::core::cash::is_cash_asset(&transaction.asset_id) {
                    cash_flow += transaction.quantity as i128;
                } else {
                    asset_flow += zero_cost_credit_value(
                        transaction,
                        priced_assets,
                        rate_map,
                        account_currency,
                    );
                }
            }
            // DIV-023 — a dividend credits cash income from a holding.
            TransactionType::Dividend => dividends += transaction.total_amount as i128,
            // Cash↔asset swaps; net zero to Global Value (a sell's realized gain → residual pnl).
            // FEE-071 — a management fee is not a flow; its drag surfaces via the reduced
            // position value at period end, not as a flow adjustment.
            // SPL-050 — a split is not a flow.
            TransactionType::Purchase
            | TransactionType::Sell
            | TransactionType::ManagementFee
            | TransactionType::Split => {}
        }
    }

    debug_assert!(
        [cash_flow, asset_flow, dividends]
            .iter()
            .all(|v| *v <= i64::MAX as i128 && *v >= i64::MIN as i128),
        "period_bridge i64 overflow: cash_flow={cash_flow} asset_flow={asset_flow} dividends={dividends}"
    );
    PeriodBridge {
        cash_flow: cash_flow as i64,
        asset_flow: asset_flow as i64,
        dividends: dividends as i64,
    }
}

/// PRF-071 — market value of a zero-cost in-kind credit (free shares, or interest
/// on a non-cash asset per INT-024) as of its **grant date**, in account-currency
/// micros, using the same carry-forward price + FX rules as `end_value_as_of`.
/// Post-grant price movement therefore surfaces in the residual pnl, and the
/// decomposition stays intact when the credit is disposed of within the same
/// period. Contributes 0 when the asset has no usable price or rate as of the
/// grant date (PRF-022 / FXR-034) — that value then surfaces via the residual pnl.
pub(crate) fn zero_cost_credit_value(
    transaction: &Transaction,
    priced_assets: &HashMap<String, PricedAsset>,
    rate_map: &RateMap,
    account_currency: &str,
) -> i128 {
    let Some(grant_date) = parse_date(&transaction.date) else {
        return 0;
    };
    let Some(priced) = priced_assets.get(transaction.asset_id.as_str()) else {
        return 0;
    };
    if priced.class == AssetClass::Cash {
        return 0;
    }
    let Some(price) = priced.price_as_of(grant_date).map(|p| p as i128) else {
        return 0;
    };
    let quantity = transaction.quantity as i128;
    if priced.currency == account_currency {
        quantity * price / MICRO
    } else if let Some(rate) = rate_map.get(&(priced.currency.clone(), grant_date)) {
        let converted_price = price * (*rate as i128) / MICRO;
        quantity * converted_price / MICRO
    } else {
        0
    }
}

/// PRF-020 (account scope) / PRF-082 (asset scope) — the value a period row
/// reports as of `period_end`: the account's Global Value, or the scoped asset's
/// position market value.
fn end_value_for_scope(
    transactions: &[Transaction],
    priced_assets: &HashMap<String, PricedAsset>,
    rate_map: &RateMap,
    account_currency: &str,
    period_end: NaiveDate,
    asset_scope: Option<&str>,
) -> i64 {
    match asset_scope {
        None => end_value_as_of(
            transactions,
            priced_assets,
            rate_map,
            account_currency,
            period_end,
        ),
        Some(asset_id) => holding_end_value_as_of(
            transactions,
            asset_id,
            priced_assets,
            rate_map,
            account_currency,
            period_end,
        ),
    }
}

/// PRF-031/032 (account scope) / PRF-083 (asset scope) — the calendar-window
/// span metric with the flow set matching the scope: Deposit/Withdrawal/
/// OpeningBalance for the account, the asset's own Purchase/Sell/OpeningBalance
/// (dividends added to gain) for a scoped position. Windowed metrics value an
/// opening balance at its entry-date market value (PRF-086).
#[allow(clippy::too_many_arguments)]
fn windowed_metric_for_scope(
    transactions: &[Transaction],
    priced_assets: &HashMap<String, PricedAsset>,
    rate_map: &RateMap,
    account_currency: &str,
    start_value: i64,
    end_value: i64,
    period_start: NaiveDate,
    period_end: NaiveDate,
    asset_scope: Option<&str>,
) -> PerformanceMetric {
    match asset_scope {
        None => metric_for_span_windowed(
            transactions,
            priced_assets,
            rate_map,
            account_currency,
            start_value,
            end_value,
            period_start,
            period_end,
        ),
        Some(_) => {
            let asset_transactions: Vec<&Transaction> = transactions.iter().collect();
            holding_performance_for_span_windowed(
                &asset_transactions,
                priced_assets,
                rate_map,
                account_currency,
                start_value,
                end_value,
                period_start,
                period_end,
            )
        }
    }
}

/// PRF-035 — the lifetime span metric with the flow set matching the scope.
/// Unlike the windowed variant (PRF-086), an opening balance keeps its typed
/// cost here, so the pre-account gain stays in since-inception performance.
fn lifetime_metric_for_scope(
    transactions: &[Transaction],
    start_value: i64,
    end_value: i64,
    period_start: NaiveDate,
    period_end: NaiveDate,
    asset_scope: Option<&str>,
) -> PerformanceMetric {
    match asset_scope {
        None => metric_for_span(
            transactions,
            start_value,
            end_value,
            period_start,
            period_end,
        ),
        Some(_) => {
            let asset_transactions: Vec<&Transaction> = transactions.iter().collect();
            holding_performance_for_span(
                &asset_transactions,
                start_value,
                end_value,
                period_start,
                period_end,
            )
        }
    }
}

/// PRF-070–072 (account scope) / PRF-084 (asset scope) — the bridge flow terms
/// for the scope the series describes.
#[allow(clippy::too_many_arguments)]
fn bridge_for_scope(
    transactions: &[Transaction],
    priced_assets: &HashMap<String, PricedAsset>,
    rate_map: &RateMap,
    account_currency: &str,
    period_start: NaiveDate,
    period_end: NaiveDate,
    asset_scope: Option<&str>,
) -> PeriodBridge {
    match asset_scope {
        None => period_bridge(
            transactions,
            priced_assets,
            rate_map,
            account_currency,
            period_start,
            period_end,
        ),
        Some(_) => holding_period_bridge(
            transactions,
            priced_assets,
            rate_map,
            account_currency,
            period_start,
            period_end,
        ),
    }
}

/// PRF-084 — the bridge flow terms for one asset's position within
/// `[period_start, period_end]`. `cash_flow` is the net money the position
/// absorbed or released through trades: Purchase (`+total_amount`),
/// OpeningBalance (`+total_amount`), Sell (`−total_amount`). `asset_flow` is the
/// zero-cost in-kind credits (free shares, non-cash interest) at their
/// grant-date market value (PRF-071 valuation). `dividends` is the asset's
/// dividend income within the period — income that accrues to the account's
/// cash, not to the scoped position value, so `residual_pnl` leaves it outside
/// the scoped bridge identity.
///
/// `transactions` must be pre-filtered to the scoped asset.
fn holding_period_bridge(
    transactions: &[Transaction],
    priced_assets: &HashMap<String, PricedAsset>,
    rate_map: &RateMap,
    account_currency: &str,
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> PeriodBridge {
    let mut cash_flow: i128 = 0;
    let mut asset_flow: i128 = 0;
    let mut dividends: i128 = 0;

    for transaction in transactions {
        let within = matches!(
            parse_date(&transaction.date),
            Some(date) if date >= period_start && date <= period_end
        );
        if !within {
            continue;
        }
        match transaction.transaction_type {
            TransactionType::Purchase => {
                cash_flow += transaction.total_amount as i128;
            }
            // PRF-086 — an opening balance has no cash leg: it is an in-kind
            // contribution at its entry-date market value (fallback typed cost).
            TransactionType::OpeningBalance => {
                asset_flow += opening_balance_flow_value(
                    transaction,
                    priced_assets,
                    rate_map,
                    account_currency,
                ) as i128;
            }
            TransactionType::Sell => cash_flow -= transaction.total_amount as i128,
            TransactionType::Dividend => dividends += transaction.total_amount as i128,
            // FSD-070 / INT-024 — zero-cost in-kind credits valued at the
            // grant-date carry-forward market price (0 for the Cash class).
            TransactionType::FreeShares | TransactionType::Interest => {
                asset_flow +=
                    zero_cost_credit_value(transaction, priced_assets, rate_map, account_currency);
            }
            // Deposit/Withdrawal move the account's cash, never a position;
            // FEE-071 — a management fee's drag surfaces via the reduced
            // position value at period end, not as a flow. SPL-050 — a split
            // is not a flow.
            TransactionType::Deposit
            | TransactionType::Withdrawal
            | TransactionType::ManagementFee
            | TransactionType::Split => {}
        }
    }

    debug_assert!(
        [cash_flow, asset_flow, dividends]
            .iter()
            .all(|v| *v <= i64::MAX as i128 && *v >= i64::MIN as i128),
        "holding_period_bridge i64 overflow: cash_flow={cash_flow} asset_flow={asset_flow} dividends={dividends}"
    );
    PeriodBridge {
        cash_flow: cash_flow as i64,
        asset_flow: asset_flow as i64,
        dividends: dividends as i64,
    }
}

/// PRF-073 — pnl as the bridge residual: makes the bridge balance exactly and
/// equals realized gains + price movement by the value decomposition (PRF-074).
/// In asset scope (PRF-084) the dividend income accrues to the account's cash,
/// outside the scoped position value, so it stays out of the residual and the
/// scoped identity is `end_value = previous_value + cash_flow + asset_flow + pnl`.
pub(crate) fn residual_pnl(
    end_value: i64,
    previous_value: i64,
    bridge: &PeriodBridge,
    asset_scope: Option<&str>,
) -> i64 {
    let dividends: i128 = if asset_scope.is_some() {
        0
    } else {
        bridge.dividends as i128
    };
    let pnl: i128 = end_value as i128
        - previous_value as i128
        - bridge.cash_flow as i128
        - bridge.asset_flow as i128
        - dividends;
    debug_assert!(
        pnl <= i64::MAX as i128 && pnl >= i64::MIN as i128,
        "pnl residual i64 overflow"
    );
    pnl as i64
}

/// PRF-085 — the span end for a row's cumulative metrics (since-inception,
/// year-to-date, annualized yield): the close date when the scoped position is
/// closed as of `period_end`, otherwise `period_end` itself. Account scope never
/// freezes (its since-inception flows are deposits/withdrawals, not trades).
fn cumulative_metric_end(
    transactions: &[Transaction],
    period_end: NaiveDate,
    asset_scope: Option<&str>,
) -> NaiveDate {
    match asset_scope {
        None => period_end,
        Some(_) => holding_close_date_as_of(transactions, period_end).unwrap_or(period_end),
    }
}

/// PRF-035 — since-inception metric: start value is 0 and the flow is the total
/// net invested over the lifetime span `[inception_start, period_end]`.
fn since_inception_metric(
    transactions: &[Transaction],
    end_value: i64,
    earliest_date: NaiveDate,
    period_end: NaiveDate,
    asset_scope: Option<&str>,
) -> PerformanceMetric {
    lifetime_metric_for_scope(
        transactions,
        0,
        end_value,
        earliest_date,
        period_end,
        asset_scope,
    )
}

/// Annualized cumulative since-inception return (CAGR) for a year row. Annualizes
/// the cash-flow-adjusted cumulative return carried by `since_inception` over the
/// elapsed years from inception (`earliest_date`) to `period_end`. The headline
/// `pct` is the annualized rate; `gain` reuses the cumulative since-inception gain.
///
/// `f64` is used because this is a derived percentage, not a money value (money
/// stays integer micro-units elsewhere).
///
/// Returns None when the since-inception percentage is absent (PRF-032 denominator
/// 0) or when the cumulative is a total loss (`1 + cumulative <= 0`, root undefined).
/// A sub-1-year first period is not annualized — extrapolating a fraction of a year
/// would overstate the return — so the cumulative is reported as-is.
pub(crate) fn annualized_yield_metric(
    since_inception: &PerformanceMetric,
    earliest_date: NaiveDate,
    period_end: NaiveDate,
) -> Option<PerformanceMetric> {
    // Cumulative since-inception return as a fraction (micro-percent → ratio).
    let cumulative = since_inception.pct? as f64 / PERCENT_SCALE as f64;
    let base = 1.0 + cumulative;
    if base <= 0.0 {
        return None;
    }
    let years = (period_end - earliest_date).num_days() as f64 / DAYS_PER_YEAR;
    let cagr = if years >= 1.0 {
        base.powf(1.0 / years) - 1.0
    } else {
        cumulative
    };
    Some(PerformanceMetric {
        gain: since_inception.gain,
        pct: Some((cagr * PERCENT_SCALE as f64).round() as i64),
    })
}
