//! Shared Global Value arithmetic (PMV-023) — the same computation behind
//! `AccountSummary.total_global_value`, extracted so `account_summary` and the
//! Price Movement report (`asset_price_fetch`) never diverge into two valuation
//! paths. `account_details::orchestrator` keeps its own inlined accumulator by
//! design (price-movement plan §2.1e) — not reached from here.

use crate::context::account::Holding;
use crate::context::asset::{AssetClass, AssetPrice};
use std::collections::HashMap;

/// Fixed reference currency every cross-account figure is reported in
/// (GPF-011). Moved here from `use_cases::global_performance::orchestrator` so
/// the Price Movement report can use it without importing another use case
/// (B18).
pub const REFERENCE_CURRENCY: &str = "EUR";

/// Per-asset facts a Global Value computation needs, independent of price
/// (PMV-023).
pub struct AssetValuationFacts {
    /// ISO 4217 currency code the asset's prices are denominated in.
    pub currency: String,
    /// Asset class — cash contributes its holding quantity verbatim (CSH-094).
    pub class: AssetClass,
}

/// A frozen set of prices and rates a Global Value computation is run over
/// (PMV-020/021/022). `prices` carries each asset's whole latest `AssetPrice`
/// — micros AND observation date — so the PMV-022 overlay guard (in
/// `price_movement::build_report`) can compare dates; a bare
/// `HashMap<String, i64>` could not express that guard. `rates` is keyed
/// `(from_currency, to_currency) → rate micros`; identity pairs are present as
/// `1_000_000`; a missing key means "no usable rate" (FXR-034).
pub struct ValuationSnapshot {
    /// Per-asset facts, keyed by `asset_id`.
    pub assets: HashMap<String, AssetValuationFacts>,
    /// Each asset's whole latest recorded price — micros and date — keyed by `asset_id`.
    pub prices: HashMap<String, AssetPrice>,
    /// Conversion rate micros, keyed `(from_currency, to_currency)`.
    pub rates: HashMap<(String, String), i64>,
}

/// CSH-094 / PMV-021/022/023 — an account's Global Value over `holdings`,
/// valued with the prices and rates carried in `snapshot`. Mirrors
/// `AccountSummaryUseCase::compute_global_value` verbatim, including its
/// two-step truncation: the converted price is narrowed to i64 micros before
/// being multiplied by quantity, never fused into a single
/// `quantity × price × rate / MICRO²` division (PMV-023).
pub fn account_global_value(
    account_currency: &str,
    holdings: &[Holding],
    snapshot: &ValuationSnapshot,
) -> i64 {
    let mut total: i64 = 0;
    for holding in holdings.iter().filter(|holding| holding.quantity > 0) {
        let Some(asset) = snapshot.assets.get(&holding.asset_id) else {
            continue;
        };
        if asset.class == AssetClass::Cash {
            total = total.saturating_add(holding.quantity);
            continue;
        }
        let Some(rate) = snapshot
            .rates
            .get(&(asset.currency.clone(), account_currency.to_string()))
            .copied()
        else {
            continue;
        };
        let Some(latest) = snapshot.prices.get(&holding.asset_id) else {
            continue;
        };
        let converted_price = (latest.price as i128 * rate as i128 / 1_000_000) as i64;
        let market_value = (holding.quantity as i128 * converted_price as i128 / 1_000_000) as i64;
        total = total.saturating_add(market_value);
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::asset::AssetPriceSource;

    fn holding(asset_id: &str, quantity: i64) -> Holding {
        Holding::restore(
            "holding-1".to_string(),
            "account-1".to_string(),
            asset_id.to_string(),
            quantity,
            0,
            0,
            None,
        )
    }

    fn price(asset_id: &str, date: &str, price_micros: i64) -> AssetPrice {
        AssetPrice::restore(
            asset_id.to_string(),
            date.to_string(),
            price_micros,
            AssetPriceSource::Manual,
        )
    }

    fn empty_snapshot() -> ValuationSnapshot {
        ValuationSnapshot {
            assets: HashMap::new(),
            prices: HashMap::new(),
            rates: HashMap::new(),
        }
    }

    // PMV-023 — golden test: the two-step truncation (converted price narrowed
    // to i64 micros BEFORE the second multiplication) must be preserved
    // verbatim. For this triple a fused `quantity × price × rate / MICRO²`
    // computes a DIFFERENT, wrong number: 1_111_109_888. The correct two-step
    // result is 1_111_109_000. Any future fusion of the two divisions fails
    // this test loudly.
    #[test]
    fn two_step_truncation_matches_the_established_arithmetic_not_a_fused_calculation() {
        let mut snapshot = empty_snapshot();
        snapshot.assets.insert(
            "asset-1".to_string(),
            AssetValuationFacts {
                currency: "USD".to_string(),
                class: AssetClass::Stocks,
            },
        );
        snapshot.prices.insert(
            "asset-1".to_string(),
            price("asset-1", "2026-09-10", 3_333_333),
        );
        snapshot
            .rates
            .insert(("USD".to_string(), "EUR".to_string()), 333_333);

        let holdings = vec![holding("asset-1", 1_000_000_000)];
        let value = account_global_value("EUR", &holdings, &snapshot);

        assert_eq!(
            value, 1_111_109_000,
            "expected the two-step truncation result (1_111_109_000); a fused \
             quantity×price×rate/MICRO² calculation would instead yield 1_111_109_888 — got {value}"
        );
    }

    // CSH-094 — a cash holding adds its quantity verbatim; no price/rate lookup happens.
    #[test]
    fn cash_holding_contributes_quantity_verbatim() {
        let mut snapshot = empty_snapshot();
        snapshot.assets.insert(
            "system-cash-eur".to_string(),
            AssetValuationFacts {
                currency: "EUR".to_string(),
                class: AssetClass::Cash,
            },
        );
        let holdings = vec![holding("system-cash-eur", 250_000_000)];
        let value = account_global_value("EUR", &holdings, &snapshot);
        assert_eq!(value, 250_000_000);
    }

    // FXR-034 — a non-cash holding with no recorded price contributes 0.
    #[test]
    fn non_cash_holding_with_no_recorded_price_contributes_zero() {
        let mut snapshot = empty_snapshot();
        snapshot.assets.insert(
            "asset-1".to_string(),
            AssetValuationFacts {
                currency: "EUR".to_string(),
                class: AssetClass::Stocks,
            },
        );
        let holdings = vec![holding("asset-1", 1_000_000)];
        let value = account_global_value("EUR", &holdings, &snapshot);
        assert_eq!(value, 0);
    }

    // FXR-034 — a foreign-currency holding with no usable rate contributes 0.
    #[test]
    fn non_cash_holding_with_no_usable_rate_contributes_zero() {
        let mut snapshot = empty_snapshot();
        snapshot.assets.insert(
            "asset-1".to_string(),
            AssetValuationFacts {
                currency: "USD".to_string(),
                class: AssetClass::Stocks,
            },
        );
        snapshot.prices.insert(
            "asset-1".to_string(),
            price("asset-1", "2026-09-10", 100_000_000),
        );
        // No ("USD", "EUR") rate inserted.
        let holdings = vec![holding("asset-1", 1_000_000)];
        let value = account_global_value("EUR", &holdings, &snapshot);
        assert_eq!(value, 0);
    }

    // A closed / non-positive quantity holding is excluded entirely, even when priced.
    #[test]
    fn non_positive_quantity_holding_is_skipped() {
        let mut snapshot = empty_snapshot();
        snapshot.assets.insert(
            "asset-1".to_string(),
            AssetValuationFacts {
                currency: "EUR".to_string(),
                class: AssetClass::Stocks,
            },
        );
        snapshot.prices.insert(
            "asset-1".to_string(),
            price("asset-1", "2026-09-10", 100_000_000),
        );
        snapshot
            .rates
            .insert(("EUR".to_string(), "EUR".to_string()), 1_000_000);
        let holdings = vec![holding("asset-1", 0), holding("asset-1", -5)];
        let value = account_global_value("EUR", &holdings, &snapshot);
        assert_eq!(value, 0);
    }

    // Cash and a priced non-cash holding accumulate together (saturating_add).
    #[test]
    fn accumulates_cash_and_priced_holdings_together() {
        let mut snapshot = empty_snapshot();
        snapshot.assets.insert(
            "system-cash-eur".to_string(),
            AssetValuationFacts {
                currency: "EUR".to_string(),
                class: AssetClass::Cash,
            },
        );
        snapshot.assets.insert(
            "asset-1".to_string(),
            AssetValuationFacts {
                currency: "EUR".to_string(),
                class: AssetClass::Stocks,
            },
        );
        snapshot.prices.insert(
            "asset-1".to_string(),
            price("asset-1", "2026-09-10", 110_000_000),
        );
        snapshot
            .rates
            .insert(("EUR".to_string(), "EUR".to_string()), 1_000_000);
        let holdings = vec![
            holding("system-cash-eur", 250_000_000),
            holding("asset-1", 2_000_000),
        ];
        let value = account_global_value("EUR", &holdings, &snapshot);
        // 250 EUR cash + 2 units × 110 EUR = 470 EUR.
        assert_eq!(value, 470_000_000);
    }
}
