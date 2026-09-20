//! Stateless cross-context valuation primitives shared by the account
//! performance, account summary and global performance use cases
//! (PRF / GPF / FXR / ADR-013). Owned by no use case.

use crate::context::account::{Account, AccountError, Transaction, TransactionType};
use crate::context::asset::{AssetClass, AssetPrice, AssetServiceContract};
use crate::context::currency::CurrencyService;
use crate::core::logger::BACKEND;
use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use specta::Type;
use std::collections::{BTreeSet, HashMap};
use std::result::Result as StdResult;

/// Micro-unit scale shared by every monetary field (ADR-001).
pub(crate) const MICRO: i128 = 1_000_000;

/// FX conversion rates pre-resolved per `(asset_currency, valuation_date)` in
/// account-currency micros (FXR-035/042). Only foreign pairs with a usable rate
/// on or before the date appear; a missing entry means "no usable rate" → the
/// holding contributes 0 (FXR-034). Pre-resolved up front because the per-period
/// valuation runs in a synchronous loop.
pub(crate) type RateMap = HashMap<(String, NaiveDate), i64>;

/// Percentage scale applied to the Simple Dietz numerator (PRF-032): `× 100`
/// turns a ratio into percent, `× 1_000_000` into micro-percent.
pub(crate) const PERCENT_SCALE: i128 = 100_000_000;

/// Non-cash asset metadata plus its full recorded price history, preloaded once
/// per asset so the per-period valuation loop never re-queries the asset context.
pub(crate) struct PricedAsset {
    pub(crate) currency: String,
    pub(crate) class: AssetClass,
    /// Recorded prices sorted ascending by date (PRF-022 carry-forward lookup).
    prices: Vec<AssetPrice>,
}

impl PricedAsset {
    /// PRF-022 carry-forward: the most recent recorded price (asset-currency micros)
    /// dated on or before `date`, or `None` when no recorded price qualifies. Prices
    /// are stored ascending, so the reverse scan yields the latest qualifying match.
    pub(crate) fn price_as_of(&self, date: NaiveDate) -> Option<i64> {
        self.prices
            .iter()
            .rev()
            .find(|p| parse_date(&p.date).is_some_and(|d| d <= date))
            .map(|p| p.price)
    }
}

/// Net-of-flows performance figures for one period (PRF-031, PRF-032).
#[derive(Debug, Serialize, Clone, Type)]
pub struct PerformanceMetric {
    /// Net-of-flows gain in account-currency micros (PRF-031).
    pub gain: i64,
    /// Simple Dietz percentage as micro-percent (8.00% = 8_000_000).
    /// None when the Dietz denominator is not positive (PRF-032).
    pub pct: Option<i64>,
}

/// Preloads metadata + price history for every distinct non-cash asset in the
/// transaction set, so the per-period valuation never re-queries the asset BC.
/// Shared with `account_summary` so the YTD computation reuses one loading pass
/// (ADR-004 service-level reuse).
pub(crate) async fn load_priced_assets(
    asset_service: &dyn AssetServiceContract,
    transactions: &[Transaction],
) -> StdResult<HashMap<String, PricedAsset>, AccountError> {
    let mut priced_assets: HashMap<String, PricedAsset> = HashMap::new();
    for transaction in transactions {
        if priced_assets.contains_key(&transaction.asset_id) {
            continue;
        }
        let asset = asset_service
            .get_asset_by_id(&transaction.asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, asset_id = %transaction.asset_id, err = ?e, "load_priced_assets: get_asset_by_id failed");
                AccountError::DatabaseError
            })?
            .ok_or_else(|| {
                tracing::error!(target: BACKEND, asset_id = %transaction.asset_id, "load_priced_assets: transaction references missing asset");
                AccountError::DatabaseError
            })?;

        if asset.class == AssetClass::Cash {
            priced_assets.insert(
                transaction.asset_id.clone(),
                PricedAsset {
                    currency: asset.currency,
                    class: asset.class,
                    prices: Vec::new(),
                },
            );
            continue;
        }

        // get_asset_by_id above already confirmed the asset exists, so AssetError's AssetNotFound arm is unreachable here.
        let mut prices = asset_service
            .get_asset_prices(&transaction.asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, asset_id = %transaction.asset_id, err = ?e, "load_priced_assets: get_asset_prices failed");
                AccountError::DatabaseError
            })?;
        prices.sort_by(|a, b| a.date.cmp(&b.date));
        priced_assets.insert(
            transaction.asset_id.clone(),
            PricedAsset {
                currency: asset.currency,
                class: asset.class,
                prices,
            },
        );
    }
    Ok(priced_assets)
}

/// Pre-resolves FX rates for each foreign holding currency at every period-end
/// the synchronous valuation loop will visit (FXR-035/042). Identity pairs are
/// excluded — same-currency holdings need no conversion.
pub(crate) async fn load_rate_map(
    currency_service: &CurrencyService,
    priced_assets: &HashMap<String, PricedAsset>,
    account_currency: &str,
    transactions: &[Transaction],
    month_view_available: bool,
    earliest_date: NaiveDate,
    today: NaiveDate,
) -> StdResult<RateMap, AccountError> {
    let mut dates = period_end_dates(month_view_available, earliest_date, today);
    // PRF-071/PRF-086 — zero-cost credits and opening balances are valued at
    // their own transaction date, so those dates need a pre-resolved rate too.
    dates.extend(market_valued_flow_dates(transactions));
    let dates: Vec<NaiveDate> = dates.into_iter().collect();
    load_rate_map_for_dates(currency_service, priced_assets, account_currency, &dates).await
}

/// PRF-071/PRF-086 — the transaction dates of the flows valued at their own-date
/// market price: zero-cost in-kind credits (free shares, non-cash interest, the
/// dates `zero_cost_credit_value` resolves) and opening balances (the dates
/// `opening_balance_flow_value` resolves). Cash-line interest is a plain cash
/// credit and needs no valuation date.
pub(crate) fn market_valued_flow_dates(transactions: &[Transaction]) -> BTreeSet<NaiveDate> {
    transactions
        .iter()
        .filter(|t| match t.transaction_type {
            TransactionType::FreeShares | TransactionType::OpeningBalance => true,
            TransactionType::Interest => !crate::core::cash::is_cash_asset(&t.asset_id),
            _ => false,
        })
        .filter_map(|t| parse_date(&t.date))
        .collect()
}

/// PRF-086 — the flow value of an opening balance in a **windowed** metric or
/// bridge: the position's market value as of its entry date (carry-forward price
/// PRF-022, FX rate FXR-042), falling back to the typed book cost when no usable
/// price or rate exists as of that date. Valuing the transfer at market keeps the
/// entry pnl-neutral in the window containing it — gains earned before the
/// position joined the account belong to no tracked period. Lifetime metrics
/// (PRF-035) keep the typed cost so the pre-account gain stays in since-inception
/// performance.
pub(crate) fn opening_balance_flow_value(
    transaction: &Transaction,
    priced_assets: &HashMap<String, PricedAsset>,
    rate_map: &RateMap,
    account_currency: &str,
) -> i64 {
    let market_value = || -> Option<i64> {
        let entry_date = parse_date(&transaction.date)?;
        let priced = priced_assets.get(transaction.asset_id.as_str())?;
        if priced.class == AssetClass::Cash {
            return None;
        }
        let price = priced.price_as_of(entry_date)? as i128;
        let converted_price = if priced.currency == account_currency {
            price
        } else {
            let rate = *rate_map.get(&(priced.currency.clone(), entry_date))? as i128;
            price * rate / MICRO
        };
        let value = transaction.quantity as i128 * converted_price / MICRO;
        i64::try_from(value).ok()
    };
    market_value().unwrap_or(transaction.total_amount)
}

/// Pre-resolves FX rates for each foreign holding currency at the caller-supplied
/// dates only (FXR-035/042). Identity pairs are excluded — same-currency holdings
/// need no conversion. Shared with `account_summary` (ADR-004 service-level reuse).
pub(crate) async fn load_rate_map_for_dates(
    currency_service: &CurrencyService,
    priced_assets: &HashMap<String, PricedAsset>,
    account_currency: &str,
    dates: &[NaiveDate],
) -> StdResult<RateMap, AccountError> {
    let mut foreign_currencies: Vec<String> = priced_assets
        .values()
        .filter(|p| p.class != AssetClass::Cash && p.currency != account_currency)
        .map(|p| p.currency.clone())
        .collect();
    foreign_currencies.sort();
    foreign_currencies.dedup();

    let mut rate_map: RateMap = HashMap::new();
    if foreign_currencies.is_empty() {
        return Ok(rate_map);
    }

    for date in dates {
        let as_of = date.format("%Y-%m-%d").to_string();
        for currency in &foreign_currencies {
            if let Some(rate) = currency_service
                .resolve_rate_micros(currency, account_currency, &as_of)
                .await
                .map_err(|e| {
                    tracing::error!(target: BACKEND, currency = %currency, as_of = %as_of, err = ?e, "load_rate_map_for_dates: resolve_rate_micros failed");
                    AccountError::DatabaseError
                })?
            {
                rate_map.insert((currency.clone(), *date), rate);
            }
        }
    }
    Ok(rate_map)
}

/// ACC-024 — current calendar-year YTD performance percentage (PRF-034) for the
/// span `[Jan 1 of the current year, today]`, in micro-percent. Composes the same
/// Simple-Dietz machinery that `build_monthly` uses for the current-month row, so
/// `AccountSummary.ytd_performance_pct` agrees with the `AccountPerformance`
/// latest-month `year_to_date.pct`.
///
/// Returns `None` when there is no data span (no transactions) or when the Dietz
/// denominator is not positive (PRF-032). A first-calendar-year account uses a year-start
/// baseline of 0 and is present (denominator is the weighted current-year flow).
pub(crate) async fn compute_current_ytd_pct(
    account_currency: &str,
    asset_service: &dyn AssetServiceContract,
    currency_service: &CurrencyService,
    transactions: &[Transaction],
    today: NaiveDate,
) -> StdResult<Option<i64>, AccountError> {
    // PRF-043 — no transactions means no data span and no YTD period.
    if !transactions.iter().any(|t| parse_date(&t.date).is_some()) {
        return Ok(None);
    }

    let priced_assets = load_priced_assets(asset_service, transactions).await?;
    // The current-year YTD valuation values the prior 31 December (year-start
    // baseline), today (period end), and the market-valued flow dates (PRF-086).
    let year_start_baseline_date = last_day_of_year(today.year() - 1);
    let mut dates: BTreeSet<NaiveDate> = [year_start_baseline_date, today].into_iter().collect();
    dates.extend(market_valued_flow_dates(transactions));
    let dates: Vec<NaiveDate> = dates.into_iter().collect();
    let rate_map =
        load_rate_map_for_dates(currency_service, &priced_assets, account_currency, &dates).await?;

    let end_value = end_value_as_of(
        transactions,
        &priced_assets,
        &rate_map,
        account_currency,
        today,
    );
    // PRF-034 — year-start baseline is the prior 31 December end value (0 for a
    // first-calendar-year account, whose data starts in the current year).
    let year_start_baseline = end_value_as_of(
        transactions,
        &priced_assets,
        &rate_map,
        account_currency,
        year_start_baseline_date,
    );
    let metric = metric_for_span_windowed(
        transactions,
        &priced_assets,
        &rate_map,
        account_currency,
        year_start_baseline,
        end_value,
        first_day_of_year(today.year()),
        today,
    );
    Ok(metric.pct)
}

/// Parses an ISO `YYYY-MM-DD` date, returning None on malformed input.
pub(crate) fn parse_date(raw: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok()
}

/// One yearly valuation period (PRF-040, PRF-041). The current year clamps its
/// end to `today`.
pub(crate) struct YearPeriod {
    pub(crate) year: i32,
    pub(crate) period_start: NaiveDate,
    pub(crate) period_end: NaiveDate,
}

/// One monthly valuation period (PRF-040, PRF-041). The current month clamps its
/// end to `today`; `year_start` opens the calendar year and `year_start_baseline`
/// is the prior 31 December the YTD metric values against (PRF-034).
pub(crate) struct MonthPeriod {
    pub(crate) year: i32,
    pub(crate) month: u32,
    pub(crate) period_start: NaiveDate,
    pub(crate) period_end: NaiveDate,
    pub(crate) year_start: NaiveDate,
    pub(crate) year_start_baseline: NaiveDate,
}

/// The single source of truth for the yearly period iteration, from the first
/// transaction year through the current year, oldest first. `build_yearly` and
/// `period_end_dates` both derive their dates from this so they cannot drift.
pub(crate) fn year_periods(earliest_date: NaiveDate, today: NaiveDate) -> Vec<YearPeriod> {
    (earliest_date.year()..=today.year())
        .map(|year| YearPeriod {
            year,
            period_start: first_day_of_year(year),
            period_end: if year == today.year() {
                today
            } else {
                last_day_of_year(year)
            },
        })
        .collect()
}

/// The single source of truth for the monthly period iteration, from the month of
/// the first transaction through the current month, oldest first. `build_monthly`
/// and `period_end_dates` both derive their dates from this so they cannot drift.
pub(crate) fn month_periods(earliest_date: NaiveDate, today: NaiveDate) -> Vec<MonthPeriod> {
    // A bounded range of month indices: a first date after today yields no
    // period, and no arithmetic slip can turn the walk into an endless loop.
    let first_month = earliest_date.year() * 12 + earliest_date.month0() as i32;
    let last_month = today.year() * 12 + today.month0() as i32;
    (first_month..=last_month)
        .map(|index| {
            let (year, month) = (index.div_euclid(12), index.rem_euclid(12) as u32 + 1);
            let last_day = last_day_of_month(year, month);
            MonthPeriod {
                year,
                month,
                period_start: first_day_of_month(year, month),
                period_end: if last_day > today { today } else { last_day },
                year_start: first_day_of_year(year),
                year_start_baseline: last_day_of_year(year - 1),
            }
        })
        .collect()
}

/// Enumerates every period-end date the valuation loop visits, collected from the
/// same `year_periods` / `month_periods` the `build_*` methods iterate (including
/// the prior-year-end YTD baseline) so FX rates can be pre-resolved for the
/// synchronous valuation (FXR-035/042) without drifting from the series.
fn period_end_dates(
    month_view_available: bool,
    earliest_date: NaiveDate,
    today: NaiveDate,
) -> BTreeSet<NaiveDate> {
    // A set deduplicates the prior-year-end baselines (one per month in a year)
    // and any overlap between the yearly and monthly series.
    let mut dates = BTreeSet::new();
    for period in year_periods(earliest_date, today) {
        dates.insert(period.period_end);
    }
    if month_view_available {
        for period in month_periods(earliest_date, today) {
            dates.insert(period.period_end);
            dates.insert(period.year_start_baseline);
        }
    }
    dates
}

fn first_day_of_year(year: i32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, 1, 1).unwrap_or_else(|| {
        tracing::error!(target: BACKEND, year, "first_day_of_year: unreachable invalid date");
        NaiveDate::MIN
    })
}

fn last_day_of_year(year: i32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, 12, 31).unwrap_or_else(|| {
        tracing::error!(target: BACKEND, year, "last_day_of_year: unreachable invalid date");
        NaiveDate::MAX
    })
}

fn first_day_of_month(year: i32, month: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, 1).unwrap_or_else(|| {
        tracing::error!(target: BACKEND, year, month, "first_day_of_month: unreachable invalid date");
        NaiveDate::MIN
    })
}

fn last_day_of_month(year: i32, month: u32) -> NaiveDate {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    first_day_of_month(next_year, next_month)
        .pred_opt()
        .unwrap_or_else(|| {
            tracing::error!(target: BACKEND, year, month, "last_day_of_month: unreachable invalid date");
            NaiveDate::MAX
        })
}

/// PRF-020/021/022/023/024 — Global Value as of `period_end`: cash at face value
/// plus the carry-forward market value of each same-currency non-cash holding.
pub(crate) fn end_value_as_of(
    transactions: &[Transaction],
    priced_assets: &HashMap<String, PricedAsset>,
    rate_map: &RateMap,
    account_currency: &str,
    period_end: NaiveDate,
) -> i64 {
    // Quantity per asset and cash balance reconstructed from transactions ≤ period_end.
    let mut quantity_by_asset: HashMap<&str, i128> = HashMap::new();
    let mut cash_balance: i128 = 0;
    for transaction in transactions {
        match parse_date(&transaction.date) {
            Some(date) if date <= period_end => {}
            _ => continue,
        }
        match transaction.transaction_type {
            TransactionType::Deposit | TransactionType::Sell | TransactionType::Dividend => {
                cash_balance += transaction.total_amount as i128;
            }
            TransactionType::Withdrawal | TransactionType::Purchase => {
                cash_balance -= transaction.total_amount as i128;
            }
            // INT-023 — interest on the cash line credits the balance by `quantity`
            // (its total_amount is 0); interest on a non-cash asset never touches cash.
            TransactionType::Interest => {
                if crate::core::cash::is_cash_asset(&transaction.asset_id) {
                    cash_balance += transaction.quantity as i128;
                }
            }
            // FSD-022d / FEE-022d / SPL-010 — free-share, management-fee and
            // split events have no cash leg.
            TransactionType::OpeningBalance
            | TransactionType::FreeShares
            | TransactionType::ManagementFee
            | TransactionType::Split => {}
        }
        match transaction.transaction_type {
            // FSD-070 — free shares enter the as-of-date unit reconstruction like a
            // purchase (quantity rises); they carry no cash or flow effect.
            // INT-024 — interest credits quantity at zero cost exactly like free shares
            // (the cash-line case is excluded from the priced loop by its Cash class).
            TransactionType::Purchase
            | TransactionType::OpeningBalance
            | TransactionType::FreeShares
            | TransactionType::Interest => {
                *quantity_by_asset
                    .entry(transaction.asset_id.as_str())
                    .or_insert(0) += transaction.quantity as i128;
            }
            // FEE-046/050 — a management fee reduces the position quantity like a sell.
            TransactionType::Sell | TransactionType::ManagementFee => {
                *quantity_by_asset
                    .entry(transaction.asset_id.as_str())
                    .or_insert(0) -= transaction.quantity as i128;
            }
            // SPL-020 — a split rescales the running quantity (factor in `quantity`).
            TransactionType::Split => {
                let quantity = quantity_by_asset
                    .entry(transaction.asset_id.as_str())
                    .or_insert(0);
                *quantity = *quantity * transaction.quantity as i128 / 1_000_000;
            }
            TransactionType::Deposit | TransactionType::Withdrawal | TransactionType::Dividend => {}
        }
    }

    let mut total: i128 = cash_balance;
    for (asset_id, quantity) in quantity_by_asset {
        if quantity <= 0 {
            continue;
        }
        let Some(priced) = priced_assets.get(asset_id) else {
            continue;
        };
        if priced.class == AssetClass::Cash {
            continue;
        }
        // PRF-022 — carry-forward: most recent recorded price with date ≤ period_end.
        let Some(price) = priced.price_as_of(period_end).map(|p| p as i128) else {
            continue;
        };
        if priced.currency == account_currency {
            // Same-currency holding — no conversion (PRF-020/024 pre-FXR behaviour).
            total += quantity * price / MICRO;
        } else if let Some(rate) = rate_map.get(&(priced.currency.clone(), period_end)) {
            // FXR-042 — value the foreign holding using the rate as of period_end.
            let converted_price = price * (*rate as i128) / MICRO;
            total += quantity * converted_price / MICRO;
        }
        // FXR-034 — a foreign holding with no usable rate as-of period_end contributes 0.
    }
    debug_assert!(
        total <= i64::MAX as i128 && total >= i64::MIN as i128,
        "end_value_as_of i64 overflow: {total}"
    );
    total as i64
}

/// One signed dated flow feeding a Simple Dietz computation, in the metric's
/// reporting-currency micros. Inflows are positive, outflows negative.
#[derive(Clone, Copy)]
pub(crate) struct DatedFlow {
    pub(crate) date: NaiveDate,
    pub(crate) amount: i64,
}

/// PRF-030 — the account-level external flows of a transaction set as signed
/// dated amounts: `Deposit` (+), `Withdrawal` (−), `OpeningBalance` cost (+).
/// Dividends, interest, free shares and management fees are internal events;
/// purchases and sells are cash↔asset conversions — none is an external flow
/// (DIV-023, FSD-070, FEE-071, INT-024). Transactions with malformed dates are
/// skipped. This is the **lifetime** flow set (PRF-035): opening balances carry
/// their typed cost; windowed metrics use `external_cash_flows_windowed`.
pub(crate) fn external_cash_flows(transactions: &[Transaction]) -> Vec<DatedFlow> {
    let mut flows = Vec::new();
    for transaction in transactions {
        let Some(date) = parse_date(&transaction.date) else {
            continue;
        };
        let amount = match transaction.transaction_type {
            TransactionType::Deposit | TransactionType::OpeningBalance => transaction.total_amount,
            TransactionType::Withdrawal => -transaction.total_amount,
            // SPL-050 — a split is not a flow.
            TransactionType::Purchase
            | TransactionType::Sell
            | TransactionType::Dividend
            | TransactionType::FreeShares
            | TransactionType::ManagementFee
            | TransactionType::Interest
            | TransactionType::Split => continue,
        };
        flows.push(DatedFlow { date, amount });
    }
    flows
}

/// PRF-086 — the **windowed** variant of `external_cash_flows`: identical except
/// that an `OpeningBalance` flow carries its entry-date market value
/// (`opening_balance_flow_value`, fallback typed cost) so the transfer is
/// pnl-neutral inside the window containing it.
pub(crate) fn external_cash_flows_windowed(
    transactions: &[Transaction],
    priced_assets: &HashMap<String, PricedAsset>,
    rate_map: &RateMap,
    account_currency: &str,
) -> Vec<DatedFlow> {
    let mut flows = Vec::new();
    for transaction in transactions {
        let Some(date) = parse_date(&transaction.date) else {
            continue;
        };
        let amount = match transaction.transaction_type {
            TransactionType::Deposit => transaction.total_amount,
            TransactionType::OpeningBalance => {
                opening_balance_flow_value(transaction, priced_assets, rate_map, account_currency)
            }
            TransactionType::Withdrawal => -transaction.total_amount,
            // SPL-050 — a split is not a flow.
            TransactionType::Purchase
            | TransactionType::Sell
            | TransactionType::Dividend
            | TransactionType::FreeShares
            | TransactionType::ManagementFee
            | TransactionType::Interest
            | TransactionType::Split => continue,
        };
        flows.push(DatedFlow { date, amount });
    }
    flows
}

/// PRF-031/032 — gain and Simple Dietz percentage for a span ending at `period_end`,
/// whose calendar window is `[period_start, period_end]` and whose start value is
/// `start_value`. Flows are netted within the window and weighted by the fraction
/// of the window remaining after each flow's date.
pub(crate) fn metric_for_span(
    transactions: &[Transaction],
    start_value: i64,
    end_value: i64,
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> PerformanceMetric {
    metric_for_span_over_flows(
        &external_cash_flows(transactions),
        start_value,
        end_value,
        period_start,
        period_end,
    )
}

/// PRF-086 — `metric_for_span` over the windowed flow set (opening balances at
/// their entry-date market value). Used by every calendar-window metric
/// (period-over-period, year-to-date); lifetime metrics keep `metric_for_span`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn metric_for_span_windowed(
    transactions: &[Transaction],
    priced_assets: &HashMap<String, PricedAsset>,
    rate_map: &RateMap,
    account_currency: &str,
    start_value: i64,
    end_value: i64,
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> PerformanceMetric {
    metric_for_span_over_flows(
        &external_cash_flows_windowed(transactions, priced_assets, rate_map, account_currency),
        start_value,
        end_value,
        period_start,
        period_end,
    )
}

/// PRF-031/032 over pre-extracted flows: gain and Simple Dietz percentage for a
/// span whose external flows are already signed dated amounts in the reporting
/// currency (`external_cash_flows`, or currency-converted flows in a
/// cross-account aggregation, GPF-030). Flows dated outside
/// `[period_start, period_end]` are ignored.
pub(crate) fn metric_for_span_over_flows(
    flows: &[DatedFlow],
    start_value: i64,
    end_value: i64,
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> PerformanceMetric {
    let days_in_period = (period_end - period_start).num_days();
    let mut net_flow: i128 = 0;
    let mut weighted_flow: i128 = 0;
    for flow in flows {
        if flow.date < period_start || flow.date > period_end {
            continue;
        }
        net_flow += flow.amount as i128;
        if days_in_period > 0 {
            let days_remaining = (period_end - flow.date).num_days() as i128;
            weighted_flow += flow.amount as i128 * days_remaining / days_in_period as i128;
        }
    }

    let gain = end_value as i128 - start_value as i128 - net_flow;
    debug_assert!(
        gain <= i64::MAX as i128 && gain >= i64::MIN as i128,
        "metric_for_span_over_flows gain i64 overflow: {gain}"
    );
    let denominator = start_value as i128 + weighted_flow;
    // PRF-032 — a non-positive average capital base (drained early in the window)
    // would sign-flip the ratio, so the percentage is absent, not negative.
    let pct = if denominator <= 0 {
        None
    } else {
        // PRF-032 — scale the numerator by 100_000_000 before dividing; truncate toward zero.
        Some((gain * PERCENT_SCALE / denominator) as i64)
    };

    PerformanceMetric {
        gain: gain as i64,
        pct,
    }
}

/// ACD-056/057 / PRF-083 — Simple Dietz position performance for ONE asset over
/// the window `[period_start, period_end]`: net-of-flows gain plus micro-percent.
/// Unlike the account-level `metric_for_span`, the external flows of a position
/// are its own Purchase (`+total_amount`), Sell (`−total_amount`) and
/// OpeningBalance (`+total_amount`) transactions — Deposit/Withdrawal move the
/// account's cash, not the position — and dividends received inside the window
/// are added to the gain (DIV-071 semantics):
/// `gain = end_value − start_value − net_flow + dividends`.
///
/// `asset_transactions` must be pre-filtered to the asset being measured.
/// `pct` is `None` when the Dietz denominator
/// (`start_value + Σ flow × days_remaining / days_in_period`) is not positive —
/// which covers the not-held-and-no-flows case.
pub(crate) fn holding_performance_for_span(
    asset_transactions: &[&Transaction],
    start_value: i64,
    end_value: i64,
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> PerformanceMetric {
    let flows = position_flows(asset_transactions);
    holding_performance_for_span_over_flows(
        &flows.trades,
        &flows.dividends,
        start_value,
        end_value,
        period_start,
        period_end,
    )
}

/// The dated flows of ONE asset's position (PRF-083): its trade flows —
/// `Purchase` (+), `OpeningBalance` (+), `Sell` (−) at `total_amount` — and its
/// dividend income, kept separate because a dividend is added to the position
/// gain rather than netted as a flow.
pub(crate) struct PositionFlows {
    pub(crate) trades: Vec<DatedFlow>,
    pub(crate) dividends: Vec<DatedFlow>,
}

/// PRF-083 — extracts the position flows of a transaction set pre-filtered to
/// one asset. Deposit/Withdrawal touch the cash line, never a position;
/// FreeShares, ManagementFee and Interest are zero-cash position events
/// (FSD-070 / FEE-071 / INT-024) whose effect surfaces through the endpoint
/// values. Transactions with malformed dates are skipped. This is the
/// **lifetime** flow set (PRF-035): opening balances carry their typed cost;
/// windowed metrics use `position_flows_windowed`.
pub(crate) fn position_flows(asset_transactions: &[&Transaction]) -> PositionFlows {
    let mut trades = Vec::new();
    let mut dividends = Vec::new();
    for transaction in asset_transactions {
        let Some(date) = parse_date(&transaction.date) else {
            continue;
        };
        match transaction.transaction_type {
            TransactionType::Purchase | TransactionType::OpeningBalance => trades.push(DatedFlow {
                date,
                amount: transaction.total_amount,
            }),
            TransactionType::Sell => trades.push(DatedFlow {
                date,
                amount: -transaction.total_amount,
            }),
            TransactionType::Dividend => dividends.push(DatedFlow {
                date,
                amount: transaction.total_amount,
            }),
            // SPL-050 — a split is not a flow.
            TransactionType::Deposit
            | TransactionType::Withdrawal
            | TransactionType::FreeShares
            | TransactionType::ManagementFee
            | TransactionType::Interest
            | TransactionType::Split => {}
        }
    }
    PositionFlows { trades, dividends }
}

/// PRF-086 — the **windowed** variant of `position_flows`: identical except that
/// an `OpeningBalance` trade flow carries its entry-date market value
/// (`opening_balance_flow_value`, fallback typed cost).
pub(crate) fn position_flows_windowed(
    asset_transactions: &[&Transaction],
    priced_assets: &HashMap<String, PricedAsset>,
    rate_map: &RateMap,
    account_currency: &str,
) -> PositionFlows {
    let mut trades = Vec::new();
    let mut dividends = Vec::new();
    for transaction in asset_transactions {
        let Some(date) = parse_date(&transaction.date) else {
            continue;
        };
        match transaction.transaction_type {
            TransactionType::Purchase => trades.push(DatedFlow {
                date,
                amount: transaction.total_amount,
            }),
            TransactionType::OpeningBalance => trades.push(DatedFlow {
                date,
                amount: opening_balance_flow_value(
                    transaction,
                    priced_assets,
                    rate_map,
                    account_currency,
                ),
            }),
            TransactionType::Sell => trades.push(DatedFlow {
                date,
                amount: -transaction.total_amount,
            }),
            TransactionType::Dividend => dividends.push(DatedFlow {
                date,
                amount: transaction.total_amount,
            }),
            // SPL-050 — a split is not a flow.
            TransactionType::Deposit
            | TransactionType::Withdrawal
            | TransactionType::FreeShares
            | TransactionType::ManagementFee
            | TransactionType::Interest
            | TransactionType::Split => {}
        }
    }
    PositionFlows { trades, dividends }
}

/// PRF-083 over pre-extracted position flows: the position Simple Dietz for a
/// span whose trade flows and dividends are already signed dated amounts in the
/// reporting currency (`position_flows`, or currency-converted flows in a
/// cross-account aggregation, GPF-030). Flows dated outside
/// `[period_start, period_end]` are ignored; dividends within the window are
/// added to the gain: `gain = end_value − start_value − net_flow + dividends`.
/// `pct` is `None` when the Dietz denominator is not positive.
pub(crate) fn holding_performance_for_span_over_flows(
    trades: &[DatedFlow],
    dividends: &[DatedFlow],
    start_value: i64,
    end_value: i64,
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> PerformanceMetric {
    let days_in_period = (period_end - period_start).num_days();
    let mut net_flow: i128 = 0;
    let mut weighted_flow: i128 = 0;
    for flow in trades {
        if flow.date < period_start || flow.date > period_end {
            continue;
        }
        net_flow += flow.amount as i128;
        if days_in_period > 0 {
            let days_remaining = (period_end - flow.date).num_days() as i128;
            weighted_flow += flow.amount as i128 * days_remaining / days_in_period as i128;
        }
    }
    let dividends_in_window: i128 = dividends
        .iter()
        .filter(|flow| flow.date >= period_start && flow.date <= period_end)
        .map(|flow| flow.amount as i128)
        .sum();

    let gain = end_value as i128 - start_value as i128 - net_flow + dividends_in_window;
    debug_assert!(
        gain <= i64::MAX as i128 && gain >= i64::MIN as i128,
        "holding_performance_for_span_over_flows gain i64 overflow: {gain}"
    );
    let denominator = start_value as i128 + weighted_flow;
    let pct = if denominator <= 0 {
        None
    } else {
        Some((gain * PERCENT_SCALE / denominator) as i64)
    };
    PerformanceMetric {
        gain: gain as i64,
        pct,
    }
}

/// PRF-086 — the **windowed** variant of `holding_performance_for_span`:
/// identical except that `OpeningBalance` flows carry their entry-date market
/// value. Used by every calendar-window position metric; lifetime metrics keep
/// `holding_performance_for_span`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn holding_performance_for_span_windowed(
    asset_transactions: &[&Transaction],
    priced_assets: &HashMap<String, PricedAsset>,
    rate_map: &RateMap,
    account_currency: &str,
    start_value: i64,
    end_value: i64,
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> PerformanceMetric {
    let flows = position_flows_windowed(
        asset_transactions,
        priced_assets,
        rate_map,
        account_currency,
    );
    holding_performance_for_span_over_flows(
        &flows.trades,
        &flows.dividends,
        start_value,
        end_value,
        period_start,
        period_end,
    )
}

/// ACD-056/057 — the percentage of `holding_performance_for_span_windowed`
/// (per-line window returns are calendar-window metrics per PRF-086), for
/// callers that only consume the micro-percent.
#[allow(clippy::too_many_arguments)]
pub(crate) fn holding_metric_for_span(
    asset_transactions: &[&Transaction],
    priced_assets: &HashMap<String, PricedAsset>,
    rate_map: &RateMap,
    account_currency: &str,
    start_value: i64,
    end_value: i64,
    period_start: NaiveDate,
    period_end: NaiveDate,
) -> Option<i64> {
    holding_performance_for_span_windowed(
        asset_transactions,
        priced_assets,
        rate_map,
        account_currency,
        start_value,
        end_value,
        period_start,
        period_end,
    )
    .pct
}

/// PRF-082 — market value of ONE asset's position as of `period_end`, in
/// account-currency micros: the quantity reconstructed from the asset's
/// transaction replay (PRF-021) valued at the carry-forward price (PRF-022) and
/// the FX rate at `period_end` (FXR-042). Contributes 0 when the position is not
/// held, when the asset has no usable price or rate as of the date (PRF-022 /
/// FXR-034), or when the scoped asset is the Cash class — mirroring the
/// priced-holding path of `end_value_as_of`, which never values the cash line as
/// a holding.
pub(crate) fn holding_end_value_as_of(
    asset_transactions: &[Transaction],
    asset_id: &str,
    priced_assets: &HashMap<String, PricedAsset>,
    rate_map: &RateMap,
    account_currency: &str,
    period_end: NaiveDate,
) -> i64 {
    let quantity = Account::reconstruct_holding_as_of(
        asset_transactions,
        asset_id,
        &period_end.format("%Y-%m-%d").to_string(),
    )
    .quantity;
    if quantity <= 0 {
        return 0;
    }
    let Some(priced) = priced_assets.get(asset_id) else {
        return 0;
    };
    if priced.class == AssetClass::Cash {
        return 0;
    }
    let Some(price) = priced.price_as_of(period_end).map(|p| p as i128) else {
        return 0;
    };
    let quantity = quantity as i128;
    let value = if priced.currency == account_currency {
        quantity * price / MICRO
    } else if let Some(rate) = rate_map.get(&(priced.currency.clone(), period_end)) {
        let converted_price = price * (*rate as i128) / MICRO;
        quantity * converted_price / MICRO
    } else {
        0
    };
    debug_assert!(
        value <= i64::MAX as i128 && value >= i64::MIN as i128,
        "holding_end_value_as_of i64 overflow: {value}"
    );
    value as i64
}

/// PRF-085 — the close date of ONE asset's position as of `period_end`: the date
/// of the last transaction that brought the replayed quantity to zero, when the
/// position is closed (quantity ≤ 0) as of `period_end`. `None` while the
/// position is open or was never held. A later purchase reopens the position and
/// clears any earlier close. Quantity semantics mirror
/// `Account::reconstruct_holding_as_of` (same type arms, same ordering).
///
/// `asset_transactions` must be pre-filtered to the asset being measured.
pub(crate) fn holding_close_date_as_of(
    asset_transactions: &[Transaction],
    period_end: NaiveDate,
) -> Option<NaiveDate> {
    let mut dated: Vec<&Transaction> = asset_transactions
        .iter()
        .filter(|t| parse_date(&t.date).is_some_and(|d| d <= period_end))
        .collect();
    dated.sort_by(|a, b| {
        a.date
            .cmp(&b.date)
            .then_with(|| a.created_at.cmp(&b.created_at))
    });

    let mut quantity: i128 = 0;
    let mut close_date: Option<NaiveDate> = None;
    for transaction in dated {
        let before = quantity;
        match transaction.transaction_type {
            TransactionType::Purchase
            | TransactionType::OpeningBalance
            | TransactionType::Deposit
            | TransactionType::FreeShares
            | TransactionType::Interest => quantity += transaction.quantity as i128,
            TransactionType::Sell
            | TransactionType::Withdrawal
            | TransactionType::ManagementFee => quantity -= transaction.quantity as i128,
            // SPL-020 — a split rescales the running quantity (factor in `quantity`).
            TransactionType::Split => {
                quantity = quantity * transaction.quantity as i128 / 1_000_000
            }
            TransactionType::Dividend => {}
        }
        if before > 0 && quantity <= 0 {
            close_date = parse_date(&transaction.date);
        } else if quantity > 0 {
            close_date = None;
        }
    }
    if quantity <= 0 {
        close_date
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // FXR-035/042 — period_end_dates must pre-resolve a rate for every date the
    // build_* methods actually value (end_value_as_of), or a foreign holding
    // silently degrades to 0 (FXR-034). Lock that the FX pre-resolution set is a
    // superset of every valued date the shared period series exposes.
    #[test]
    fn period_end_dates_cover_every_valued_date() {
        let earliest = NaiveDate::from_ymd_opt(2022, 3, 15).expect("valid date");
        let today = NaiveDate::from_ymd_opt(2024, 6, 10).expect("valid date");

        // month_view_available = true: yearly ends + monthly ends + YTD baselines.
        let dates = period_end_dates(true, earliest, today);
        for period in year_periods(earliest, today) {
            assert!(
                dates.contains(&period.period_end),
                "yearly period_end {} not pre-resolved",
                period.period_end
            );
        }
        for period in month_periods(earliest, today) {
            assert!(
                dates.contains(&period.period_end),
                "monthly period_end {} not pre-resolved",
                period.period_end
            );
            assert!(
                dates.contains(&period.year_start_baseline),
                "YTD baseline {} not pre-resolved",
                period.year_start_baseline
            );
        }

        // month_view_available = false: only the yearly ends are pre-resolved.
        let yearly_only = period_end_dates(false, earliest, today);
        for period in year_periods(earliest, today) {
            assert!(
                yearly_only.contains(&period.period_end),
                "yearly-only period_end {} not pre-resolved",
                period.period_end
            );
        }
    }

    // The month walk is a bounded range: a first date in a month after today's
    // yields no period at all, and the span from the first month to today's is
    // exact, ending on today.
    #[test]
    fn month_periods_are_a_bounded_range_ending_today() {
        let today = NaiveDate::from_ymd_opt(2026, 9, 14).expect("valid date");
        let next_month = NaiveDate::from_ymd_opt(2026, 10, 1).expect("valid date");
        assert!(month_periods(next_month, today).is_empty());

        let earliest = NaiveDate::from_ymd_opt(2025, 11, 20).expect("valid date");
        let periods = month_periods(earliest, today);
        let months: Vec<(i32, u32)> = periods.iter().map(|p| (p.year, p.month)).collect();
        assert_eq!(
            months,
            vec![
                (2025, 11),
                (2025, 12),
                (2026, 1),
                (2026, 2),
                (2026, 3),
                (2026, 4),
                (2026, 5),
                (2026, 6),
                (2026, 7),
                (2026, 8),
                (2026, 9),
            ]
        );
        assert_eq!(
            periods[0].period_start,
            NaiveDate::from_ymd_opt(2025, 11, 1).expect("valid date")
        );
        assert_eq!(periods.last().map(|p| p.period_end), Some(today));
    }

    // PRF-032 — a negative Dietz denominator (weighted outflows exceeding the
    // start value plus weighted inflows) must yield an absent percentage, never a
    // sign-flipped one: an account holding 10_000 that liquidates for 12_000 and
    // withdraws it early in the period gained 2_000 but has a negative average
    // capital base for the rest of the window.
    #[test]
    fn negative_dietz_denominator_yields_absent_pct_not_sign_flip() {
        let period_start = NaiveDate::from_ymd_opt(2026, 1, 1).expect("valid date");
        let period_end = NaiveDate::from_ymd_opt(2026, 12, 31).expect("valid date");
        let withdrawal = DatedFlow {
            date: NaiveDate::from_ymd_opt(2026, 1, 5).expect("valid date"),
            amount: -12_000_000_000,
        };

        let metric =
            metric_for_span_over_flows(&[withdrawal], 10_000_000_000, 0, period_start, period_end);

        assert_eq!(metric.gain, 2_000_000_000, "realized gain must be reported");
        assert_eq!(
            metric.pct, None,
            "negative denominator must suppress pct, not flip its sign"
        );
    }

    // PRF-032 — the lifetime variant of the same breakdown: weighted withdrawals
    // exceed weighted deposits over a since-inception span (start value 0).
    #[test]
    fn lifetime_weighted_withdrawals_above_deposits_yield_absent_pct() {
        let inception = NaiveDate::from_ymd_opt(2020, 1, 1).expect("valid date");
        let period_end = NaiveDate::from_ymd_opt(2026, 12, 31).expect("valid date");
        // Deposit on day one (weight 1), withdraw more shortly after (weight ≈ 1):
        // net weighted flow is negative while the account still gained overall.
        let flows = [
            DatedFlow {
                date: inception,
                amount: 10_000_000_000,
            },
            DatedFlow {
                date: NaiveDate::from_ymd_opt(2020, 1, 10).expect("valid date"),
                amount: -12_000_000_000,
            },
        ];

        let metric = metric_for_span_over_flows(&flows, 0, 500_000_000, inception, period_end);

        assert_eq!(metric.gain, 2_500_000_000);
        assert_eq!(metric.pct, None);
    }

    // PRF-085 — the close-date probe's reopen/re-close state machine, pinned
    // DB-free: only the LAST transition to zero counts, and any state with
    // quantity > 0 as of the probe date reports the position open (None).
    #[test]
    fn holding_close_date_tracks_the_last_transition_to_zero() {
        let tx = |transaction_type: TransactionType, date: &str, quantity: i64| {
            Transaction::new(
                "acc".to_string(),
                "asset".to_string(),
                transaction_type,
                date.to_string(),
                quantity,
                1_000_000,
                1_000_000,
                0,
                quantity,
                None,
                None,
            )
            .unwrap()
        };
        let date = |raw: &str| parse_date(raw).unwrap();
        let transactions = vec![
            tx(TransactionType::Purchase, "2024-01-10", 10_000_000),
            tx(TransactionType::Sell, "2024-03-01", 10_000_000),
            tx(TransactionType::Purchase, "2024-06-01", 5_000_000),
            tx(TransactionType::Sell, "2024-08-01", 2_000_000),
            tx(TransactionType::Sell, "2024-10-01", 3_000_000),
        ];

        assert_eq!(
            holding_close_date_as_of(&transactions, date("2024-02-01")),
            None,
            "open position before any sell"
        );
        assert_eq!(
            holding_close_date_as_of(&transactions, date("2024-04-01")),
            Some(date("2024-03-01")),
            "closed by the sell-to-zero"
        );
        assert_eq!(
            holding_close_date_as_of(&transactions, date("2024-07-01")),
            None,
            "the re-buy reopens the position"
        );
        assert_eq!(
            holding_close_date_as_of(&transactions, date("2024-09-01")),
            None,
            "a partial sell leaves the position open"
        );
        assert_eq!(
            holding_close_date_as_of(&transactions, date("2024-12-31")),
            Some(date("2024-10-01")),
            "re-closed: only the LAST transition to zero counts"
        );
        assert_eq!(
            holding_close_date_as_of(&[], date("2024-12-31")),
            None,
            "never held"
        );
    }

    fn price_at(date: &str, price: i64) -> AssetPrice {
        AssetPrice {
            asset_id: "asset-1".to_string(),
            date: date.to_string(),
            price,
            source: crate::context::asset::AssetPriceSource::Manual,
        }
    }

    fn priced_with(prices: Vec<AssetPrice>) -> PricedAsset {
        PricedAsset {
            currency: "USD".to_string(),
            class: AssetClass::Stocks,
            prices,
        }
    }

    // PRF-022 carry-forward: the reverse scan over an ascending-sorted price vec
    // returns the latest price dated on or before the query date.
    #[test]
    fn price_as_of_returns_latest_price_on_or_before_date() {
        let priced = priced_with(vec![
            price_at("2024-01-10", 100_000_000),
            price_at("2024-03-15", 120_000_000),
            price_at("2024-06-01", 130_000_000),
        ]);

        // Between two observations — carries the earlier one forward.
        let mid = NaiveDate::from_ymd_opt(2024, 4, 1).expect("valid date");
        assert_eq!(priced.price_as_of(mid), Some(120_000_000));

        // Exact-match date qualifies (boundary is inclusive).
        let exact = NaiveDate::from_ymd_opt(2024, 6, 1).expect("valid date");
        assert_eq!(priced.price_as_of(exact), Some(130_000_000));
    }

    #[test]
    fn price_as_of_returns_none_when_all_prices_postdate() {
        let priced = priced_with(vec![price_at("2024-03-15", 120_000_000)]);
        let before = NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date");
        assert_eq!(priced.price_as_of(before), None);
    }

    #[test]
    fn price_as_of_returns_none_on_empty_price_list() {
        let priced = priced_with(Vec::new());
        let date = NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date");
        assert_eq!(priced.price_as_of(date), None);
    }

    use crate::context::asset::{
        Asset, AssetCategory, AssetService, MockAssetCategoryRepository, MockAssetPriceRepository,
        MockAssetRepository, SYSTEM_CATEGORY_ID,
    };
    use crate::context::currency::domain::{
        CurrencyRate, CurrencyRateSource, MockCurrencyPairRepository, MockCurrencyRateRepository,
    };

    fn priced_asset(currency: &str, class: AssetClass) -> PricedAsset {
        PricedAsset {
            currency: currency.to_string(),
            class,
            prices: Vec::new(),
        }
    }

    fn currency_service_with_fixed_rate(
        rate_micros: i64,
        expected_resolutions: usize,
    ) -> CurrencyService {
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_latest_rate_on_or_before()
            .times(expected_resolutions)
            .returning(move |from_currency, to_currency, as_of| {
                Ok(Some(CurrencyRate::from_storage(
                    from_currency.to_string(),
                    to_currency.to_string(),
                    as_of.to_string(),
                    rate_micros,
                    CurrencyRateSource::Manual,
                )))
            });
        CurrencyService::new(
            Box::new(MockCurrencyPairRepository::new()),
            Box::new(rate_repo),
        )
    }

    // FXR-035/042 — the caller-supplied date list bounds the FX pre-resolution:
    // exactly one repository lookup per (foreign currency × requested date) pair,
    // with cash and account-currency holdings excluded. The `.times(2)` mock
    // expectation fails the test if any other date is resolved.
    #[tokio::test]
    async fn load_rate_map_for_dates_resolves_only_the_requested_pairs() {
        let currency_service = currency_service_with_fixed_rate(900_000, 2);
        let mut priced_assets = HashMap::new();
        priced_assets.insert(
            "usd-stock".to_string(),
            priced_asset("USD", AssetClass::Stocks),
        );
        priced_assets.insert(
            "eur-stock".to_string(),
            priced_asset("EUR", AssetClass::Stocks),
        );
        priced_assets.insert(
            "system-cash-eur".to_string(),
            priced_asset("EUR", AssetClass::Cash),
        );

        let baseline = NaiveDate::from_ymd_opt(2025, 12, 31).expect("valid date");
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).expect("valid date");
        let rate_map =
            load_rate_map_for_dates(&currency_service, &priced_assets, "EUR", &[baseline, today])
                .await
                .expect("rate map");

        assert_eq!(rate_map.len(), 2);
        assert_eq!(rate_map.get(&("USD".to_string(), baseline)), Some(&900_000));
        assert_eq!(rate_map.get(&("USD".to_string(), today)), Some(&900_000));
    }

    // FXR-035 — without foreign holding currencies the map is empty and the
    // currency repository is never queried (`.times(0)` on the mock).
    #[tokio::test]
    async fn load_rate_map_for_dates_returns_empty_map_without_foreign_currencies() {
        let currency_service = currency_service_with_fixed_rate(900_000, 0);
        let mut priced_assets = HashMap::new();
        priced_assets.insert(
            "eur-stock".to_string(),
            priced_asset("EUR", AssetClass::Stocks),
        );
        priced_assets.insert(
            "system-cash-eur".to_string(),
            priced_asset("EUR", AssetClass::Cash),
        );

        let today = NaiveDate::from_ymd_opt(2026, 6, 15).expect("valid date");
        let rate_map = load_rate_map_for_dates(&currency_service, &priced_assets, "EUR", &[today])
            .await
            .expect("rate map");

        assert!(rate_map.is_empty());
    }

    fn asset_category() -> AssetCategory {
        AssetCategory::from_storage(
            SYSTEM_CATEGORY_ID.to_string(),
            "generic.uncategorized".to_string(),
        )
    }

    fn asset_service_with_usd_stock_and_eur_cash() -> AssetService {
        let mut asset_repo = MockAssetRepository::new();
        asset_repo.expect_get_by_id().returning(|asset_id| {
            Ok(Some(match asset_id {
                "system-cash-eur" => Asset::restore(
                    "system-cash-eur".to_string(),
                    "Cash".to_string(),
                    AssetClass::Cash,
                    asset_category(),
                    "EUR".to_string(),
                    1,
                    "EUR".to_string(),
                    None,
                    false,
                    None,
                    false,
                    false,
                ),
                _ => Asset::restore(
                    "usd-stock".to_string(),
                    "US Stock".to_string(),
                    AssetClass::Stocks,
                    asset_category(),
                    "USD".to_string(),
                    1,
                    "USTK".to_string(),
                    None,
                    false,
                    None,
                    false,
                    false,
                ),
            }))
        });
        let mut price_repo = MockAssetPriceRepository::new();
        price_repo.expect_get_all_for_asset().returning(|_| {
            Ok(vec![
                price_at("2025-12-15", 100_000_000),
                price_at("2026-06-01", 120_000_000),
            ])
        });
        AssetService::new(
            Box::new(asset_repo),
            Box::new(MockAssetCategoryRepository::new()),
            Box::new(price_repo),
        )
    }

    // PRF-034/FXR-042 — foreign-holding YTD fixture: EUR account, 1000 EUR
    // deposited and 10 units of a 100-USD stock bought in the prior year at
    // rate 0.9 USD→EUR. Baseline (prior 31 Dec) = 100 cash + 10 × 90 = 1000 EUR;
    // today = 100 cash + 10 × 108 = 1180 EUR; no current-year flows, so YTD
    // pct = 180 / 1000 = 18% (18_000_000 micro-percent). The `.times(2)` mock
    // expectation locks the FX pre-resolution to the two consumed dates.
    #[tokio::test]
    async fn compute_current_ytd_pct_values_foreign_holding_at_baseline_and_today_only() {
        let asset_service = asset_service_with_usd_stock_and_eur_cash();
        let currency_service = currency_service_with_fixed_rate(900_000, 2);
        let transactions = vec![
            Transaction::new_deposit(
                "account-1".to_string(),
                "system-cash-eur".to_string(),
                "2025-01-10".to_string(),
                1_000_000_000,
                None,
            )
            .expect("valid deposit"),
            Transaction::new(
                "account-1".to_string(),
                "usd-stock".to_string(),
                TransactionType::Purchase,
                "2025-02-01".to_string(),
                10_000_000,
                100_000_000,
                900_000,
                0,
                900_000_000,
                None,
                None,
            )
            .expect("valid purchase"),
        ];
        let today = NaiveDate::from_ymd_opt(2026, 6, 15).expect("valid date");

        let ytd_pct = compute_current_ytd_pct(
            "EUR",
            &asset_service,
            &currency_service,
            &transactions,
            today,
        )
        .await
        .expect("ytd computation");

        assert_eq!(ytd_pct, Some(18_000_000));
    }
}
