//! ACC-021 / ACC-027 — the account summaries and their portfolio total over a set of accounts:
//! the computation the account summaries and the profiles overview (PFL-055) share (B18).

use crate::context::account::{Account, AccountError, AccountServiceContract, AgentRecording};
use crate::context::asset::{is_price_outdated, AssetClass, AssetServiceContract};
use crate::context::currency::CurrencyService;
use crate::core::logger::BACKEND;
use crate::shared::domain::quantity::QUANTITY_UNIT;
use chrono::NaiveDate;
use std::collections::HashMap;

use crate::use_cases::shared::global_value::{
    account_global_value, AssetValuationFacts, ValuationSnapshot,
};
use crate::use_cases::shared::inconsistency::holding_inconsistency;
use crate::use_cases::shared::valuation::{compute_current_ytd_pct, convert_at_rate};
use serde::Serialize;
use specta::Type;
use std::result::Result as StdResult;
use std::sync::Arc;

/// Row returned by `get_account_summaries` (ACC-021). Pairs each `Account` with its
/// computed `total_global_value` so the Accounts list can render the value column
/// without each row calling `get_account_details` separately.
#[derive(Debug, Serialize, Clone, Type)]
pub struct AccountSummary {
    /// Account identifier (UUID).
    pub id: String,
    /// User-defined display name.
    pub name: String,
    /// ISO 4217 currency code; matches `Account.currency`.
    pub currency: String,
    /// ACC-046 — the identifier of the account's tax wrapper (ACC-043), for the list's column
    /// and grouping; none when unset.
    pub wrapper: Option<String>,
    /// Total economic value in account-currency micros (CSH-094 / FXR-041): cash
    /// quantity plus the sum of `quantity × latest_price` over priced active non-cash
    /// holdings, with foreign holdings converted to account currency. Unpriced holdings,
    /// or foreign holdings with no usable rate (FXR-034), contribute 0.
    pub total_global_value: i64,
    /// Account-wide unrealized P&L in account-currency micros (ACC-023, ADR-001):
    /// the sum of per-holding unrealized P&L (current value − cost basis) over
    /// priced, computable, active non-cash holdings, with foreign holdings
    /// converted to account currency (MKT-040, FXR-040). `None` when no holding
    /// qualifies (no price, or a foreign holding with no usable rate).
    pub total_unrealized_pnl: Option<i64>,
    /// Year-to-date performance for the current calendar year as micro-percent
    /// (ACC-024, ADR-001): the Simple-Dietz return over `[Jan 1, today]` (PRF-034).
    /// `None` when the account has no transactions or the Dietz denominator is
    /// not positive (PRF-032). A first-calendar-year account uses a year-start
    /// baseline of 0 and is present.
    pub ytd_performance_pct: Option<i64>,
    /// Whether any holding of this account carries a derived inconsistency (SYN-040,
    /// CFR-042) — marks the account row in the Accounts list.
    pub has_inconsistent_holding: bool,
    /// MKT-206 — how many of the account's active holdings have an outdated price (MKT-205).
    pub outdated_price_count: i64,
    /// AGT-069 — the agent and the session that created the account on this computer; none
    /// for an account the owner created, or one created on another computer.
    pub created_by: Option<AgentRecording>,
}

/// ACC-027/028 — the portfolio total over the accounts of the profile in use (PFL-050), in the
/// reference currency.
#[derive(Debug, Serialize, Clone, Type)]
pub struct PortfolioTotal {
    /// Σ every account's Global Value, converted at the read date's rate, in
    /// reference-currency micros (ADR-001).
    pub total_global_value: i64,
    /// Σ every account's Unrealized P&L, converted the same way; `None` when no
    /// account carries one.
    pub total_unrealized_pnl: Option<i64>,
    /// The reference currency both figures are expressed in (GPF-011).
    pub currency: String,
    /// An account whose currency has no usable rate to the reference currency
    /// (FXR-034) held a non-zero Global Value or Unrealized P&L, which the total
    /// therefore leaves out (ACC-028).
    pub incomplete: bool,
    /// MKT-206 — how many holdings have an outdated price, over the accounts of the profile in
    /// use (PFL-050).
    pub outdated_price_count: i64,
}

/// Response of `get_account_summaries` (ACC-021, ACC-027): the rows and their
/// portfolio total, read together so the list and its total never disagree.
#[derive(Debug, Serialize, Clone, Type)]
pub struct AccountSummaries {
    /// One row per non-deleted account.
    pub summaries: Vec<AccountSummary>,
    /// The portfolio total over those rows.
    pub total: PortfolioTotal,
}

/// ACC-021 / ACC-027 — reads the summaries of a set of accounts over the account, asset and
/// currency services; held by each use case that needs them.
pub(crate) struct SummaryReader {
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
    currency_service: Arc<CurrencyService>,
}

impl SummaryReader {
    /// The reader over the three services. The currency service is the valuation read port
    /// for foreign-currency holdings (FXR-041/035).
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

    /// ACC-021 / ACC-027 — a summary row for each of `accounts`, and their total.
    pub(crate) async fn summaries_of(
        &self,
        accounts: Vec<Account>,
    ) -> StdResult<AccountSummaries, AccountError> {
        let mut creators = self.account_service.agent_creations().await?;
        let mut summaries = Vec::with_capacity(accounts.len());
        let today = chrono::Local::now().date_naive();
        for account in accounts {
            let total_global_value = self.compute_global_value(&account).await?;
            // ACC-023 — account-wide unrealized P&L, mirroring the MKT-040/FXR-040
            // valuation pass over the account's active non-cash holdings.
            let total_unrealized_pnl = self.compute_total_unrealized_pnl(&account, today).await?;
            // ACC-024 — current calendar-year YTD performance, reusing the
            // account-performance Simple-Dietz machinery (PRF-034) over the
            // account's transactions. Per-account degradation to None mirrors the
            // global-value path; it does not abort the list.
            let transactions = self
                .account_service
                .get_all_transactions_for_account(&account.id)
                .await?;
            let ytd_performance_pct = compute_current_ytd_pct(
                &account.currency,
                self.asset_service.as_ref(),
                &self.currency_service,
                &transactions,
                today,
            )
            .await?;
            // CFR-042/SYN-040 — derived from the replayed holdings, never stored.
            let has_inconsistent_holding = self
                .account_service
                .get_holdings_for_account(&account.id)
                .await?
                .iter()
                .any(|holding| holding_inconsistency(holding).is_some());
            let outdated_price_count = self.count_outdated_prices(&account, today).await?;
            let created_by = creators.remove(&account.id);
            summaries.push(AccountSummary {
                id: account.id,
                name: account.name,
                currency: account.currency,
                wrapper: account.wrapper,
                total_global_value,
                total_unrealized_pnl,
                ytd_performance_pct,
                has_inconsistent_holding,
                outdated_price_count,
                created_by,
            });
        }
        // ACC-027/028 — the portfolio total in the reference currency, each account
        // converted at the read date's rate; a currency with no usable rate counts
        // as zero (FXR-034).
        let as_of = today.format("%Y-%m-%d").to_string();
        let reference = self.currency_service.reference_currency().await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "get_account_summaries: reference currency not read");
            AccountError::DatabaseError
        })?;
        let mut rates: HashMap<String, Option<i64>> = HashMap::new();
        for summary in &summaries {
            if rates.contains_key(&summary.currency) {
                continue;
            }
            let rate = self
                .currency_service
                .resolve_rate_micros(&summary.currency, &reference, &as_of)
                .await
                .map_err(|e| {
                    tracing::error!(target: BACKEND, currency = %summary.currency, err = ?e, "get_account_summaries: resolve_rate_micros failed (portfolio total)");
                    AccountError::DatabaseError
                })?;
            rates.insert(summary.currency.clone(), rate);
        }
        let total = portfolio_total(&summaries, &rates, &reference);
        Ok(AccountSummaries { summaries, total })
    }

    /// ACC-023 / MKT-040 — account-wide unrealized P&L in account currency: the
    /// sum of per-holding unrealized P&L over priced, computable, active non-cash
    /// holdings. Mirrors the per-holding computation in
    /// `account_details::orchestrator::get_account_details` (current value − cost
    /// basis, with foreign holdings converted via the FX rate). `None` when no
    /// holding qualifies — no price, or a foreign holding with no usable rate
    /// (FXR-034). Cash holdings carry no P&L and never qualify.
    async fn compute_total_unrealized_pnl(
        &self,
        account: &Account,
        today: chrono::NaiveDate,
    ) -> StdResult<Option<i64>, AccountError> {
        let holdings = self
            .account_service
            .get_holdings_for_account(&account.id)
            .await?;
        // FXR-035 — valuation date is "today" (snapshotted once by the caller);
        // future-dated rates are forbidden (FXR-022), so the latest rate on or
        // before today is the latest rate.
        let today = today.format("%Y-%m-%d").to_string();
        let mut total: i64 = 0;
        let mut any_qualified = false;
        for holding in holdings.into_iter().filter(|holding| holding.is_active()) {
            let asset = self
                .asset_service
                .get_asset_by_id(&holding.asset_id)
                .await
                .map_err(|e| {
                    tracing::error!(target: BACKEND, asset_id = %holding.asset_id, err = ?e, "get_account_summaries: get_asset_by_id failed (unrealized_pnl)");
                    AccountError::DatabaseError
                })?
                .ok_or_else(|| {
                    tracing::error!(target: BACKEND, asset_id = %holding.asset_id, "get_account_summaries: holding references missing asset (unrealized_pnl)");
                    AccountError::DatabaseError
                })?;

            // MKT-040 — cash holdings carry no unrealized P&L; they never qualify.
            if asset.class == AssetClass::Cash {
                continue;
            }
            // FXR-040 — resolve the conversion rate (identity → 1.0). A foreign
            // pair with no usable rate makes the holding non-computable (FXR-034).
            let Some(rate) = self
                .currency_service
                .resolve_rate_micros(&asset.currency, &account.currency, &today)
                .await
                .map_err(|e| {
                    tracing::error!(target: BACKEND, asset_id = %holding.asset_id, err = ?e, "get_account_summaries: resolve_rate_micros failed (unrealized_pnl)");
                    AccountError::DatabaseError
                })?
            else {
                continue;
            };
            // MKT-031 — an unpriced holding is non-computable. A price-lookup error
            // is a deliberate degradation (the holding drops from the P&L total),
            // but log it server-side so a real failure leaves a trace.
            let latest_price = self.asset_service.get_latest_price(&holding.asset_id).await;
            if let Err(ref e) = latest_price {
                tracing::warn!(target: BACKEND, asset_id = %holding.asset_id, err = ?e, "get_account_summaries: get_latest_price failed; holding excluded from unrealized P&L (MKT-031)");
            }
            let Some(latest) = latest_price.ok().flatten() else {
                continue;
            };
            // MKT-040/FXR-040 — (converted_price − average_price) × quantity, with
            // i128 intermediates, matching the per-holding detail computation.
            let converted_price = convert_at_rate(latest.price, rate);
            let unrealized_pnl = ((converted_price as i128 - holding.average_price as i128)
                * holding.quantity as i128
                / QUANTITY_UNIT) as i64;
            total = total.saturating_add(unrealized_pnl);
            any_qualified = true;
        }
        Ok(any_qualified.then_some(total))
    }

    /// MKT-206 — how many of the account's active holdings have an outdated price (MKT-205):
    /// each judged on its latest price's observation date. A price that cannot be read is
    /// not counted, as it is left out of the value (MKT-031).
    async fn count_outdated_prices(
        &self,
        account: &Account,
        today: NaiveDate,
    ) -> StdResult<i64, AccountError> {
        let holdings = self
            .account_service
            .get_holdings_for_account(&account.id)
            .await?;
        let mut count = 0;
        for holding in holdings.iter().filter(|holding| holding.is_active()) {
            let Some(asset) = self
                .asset_service
                .get_asset_by_id(&holding.asset_id)
                .await
                .map_err(|e| {
                    tracing::error!(target: BACKEND, asset_id = %holding.asset_id, err = ?e, "get_account_summaries: get_asset_by_id failed (outdated prices)");
                    AccountError::DatabaseError
                })?
            else {
                continue;
            };
            let latest = match self.asset_service.get_latest_price(&holding.asset_id).await {
                Ok(Some(latest)) => latest,
                Ok(None) => continue,
                Err(e) => {
                    tracing::error!(target: BACKEND, asset_id = %holding.asset_id, err = ?e, "get_account_summaries: get_latest_price failed; holding left out of the outdated count (MKT-031)");
                    continue;
                }
            };
            let outdated = NaiveDate::parse_from_str(&latest.date, "%Y-%m-%d")
                .is_ok_and(|date| is_price_outdated(&asset.kind, &asset.class, date, today));
            if outdated {
                count += 1;
            }
        }
        Ok(count)
    }

    /// CSH-094 — per-account economic value in account currency. Mirrors the
    /// inlined accumulator in `account_details::orchestrator::get_account_details`;
    /// kept duplicated to avoid double-iterating the holdings loop on the detail
    /// path. Tracked as tech debt for a future shared helper.
    async fn compute_global_value(&self, account: &Account) -> StdResult<i64, AccountError> {
        let holdings = self
            .account_service
            .get_holdings_for_account(&account.id)
            .await?;
        // FXR-035 — valuation date is "today"; future-dated rates are forbidden
        // (FXR-022), so the latest rate on or before today is the latest rate.
        let today = chrono::Local::now()
            .date_naive()
            .format("%Y-%m-%d")
            .to_string();

        let mut assets = HashMap::new();
        let mut prices = HashMap::new();
        let mut rates = HashMap::new();
        for holding in holdings.iter().filter(|holding| holding.is_active()) {
            let asset = self
                .asset_service
                .get_asset_by_id(&holding.asset_id)
                .await
                .map_err(|e| {
                    tracing::error!(target: BACKEND, asset_id = %holding.asset_id, err = ?e, "get_account_summaries: get_asset_by_id failed");
                    AccountError::DatabaseError
                })?
                .ok_or_else(|| {
                    tracing::error!(target: BACKEND, asset_id = %holding.asset_id, "get_account_summaries: holding references missing asset");
                    AccountError::DatabaseError
                })?;

            if asset.class != AssetClass::Cash {
                let key = (asset.currency.clone(), account.currency.clone());
                if let std::collections::hash_map::Entry::Vacant(e) = rates.entry(key) {
                    // FXR-041/035 — identity resolves to 1.0; a foreign pair with no
                    // usable rate is simply absent, and contributes 0 (FXR-034).
                    if let Some(rate) = self
                        .currency_service
                        .resolve_rate_micros(&asset.currency, &account.currency, &today)
                        .await
                        .map_err(|e| {
                            tracing::error!(target: BACKEND, asset_id = %holding.asset_id, err = ?e, "get_account_summaries: resolve_rate_micros failed");
                            AccountError::DatabaseError
                        })?
                    {
                        e.insert(rate);
                    }
                }
                if let Ok(Some(latest)) =
                    self.asset_service.get_latest_price(&holding.asset_id).await
                {
                    prices.insert(holding.asset_id.clone(), latest);
                }
            }

            assets.insert(
                holding.asset_id.clone(),
                AssetValuationFacts {
                    currency: asset.currency.clone(),
                    class: asset.class,
                },
            );
        }

        let snapshot = ValuationSnapshot {
            assets,
            prices,
            rates,
        };
        Ok(account_global_value(
            &account.currency,
            &holdings,
            &snapshot,
        ))
    }
}

/// ACC-027/028/029 — sums every account's Global Value and Unrealized P&L in the
/// reference currency. `rate_to_reference` maps an account currency to its rate
/// micros, `None` when no usable rate exists: that account contributes zero, and
/// the total is incomplete when that zero hides a non-zero figure.
pub(crate) fn portfolio_total(
    summaries: &[AccountSummary],
    rate_to_reference: &HashMap<String, Option<i64>>,
    reference_currency: &str,
) -> PortfolioTotal {
    let mut total_global_value: i64 = 0;
    let mut total_unrealized_pnl: Option<i64> = None;
    let mut incomplete = false;
    // MKT-206 — a count needs no conversion: every account's adds up.
    let outdated_price_count = summaries
        .iter()
        .map(|summary| summary.outdated_price_count)
        .sum();
    for summary in summaries {
        let Some(rate) = rate_to_reference.get(&summary.currency).copied().flatten() else {
            // ACC-028 — the unconvertible account counts as zero; it leaves the total
            // incomplete only when that zero hides a figure.
            if summary.total_global_value != 0
                || summary.total_unrealized_pnl.is_some_and(|pnl| pnl != 0)
            {
                incomplete = true;
            }
            continue;
        };
        total_global_value =
            total_global_value.saturating_add(convert_at_rate(summary.total_global_value, rate));
        if let Some(pnl) = summary.total_unrealized_pnl {
            total_unrealized_pnl = Some(
                total_unrealized_pnl
                    .unwrap_or(0)
                    .saturating_add(convert_at_rate(pnl, rate)),
            );
        }
    }
    PortfolioTotal {
        total_global_value,
        total_unrealized_pnl,
        currency: reference_currency.to_string(),
        incomplete,
        outdated_price_count,
    }
}
