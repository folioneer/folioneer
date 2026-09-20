//! Price Movement report arithmetic (PMV spec) — pure, synchronous, no I/O, no
//! injected services. Consumes the "before" snapshot captured at refresh start
//! (`asset_price_fetch::movement_capture::PriceMovementCapture::capture`) plus
//! what the refresh itself wrote, and produces the `PriceMovementReport` event
//! payload. Never re-reads the database (PMV-026).

use crate::context::account::{Account, Holding};
use crate::context::asset::AssetPrice;
use crate::core::event_bus::{PriceMovementReport, PriceMovementRow};
use std::collections::{HashMap, HashSet};

use super::global_value::{account_global_value, ValuationSnapshot, REFERENCE_CURRENCY};

/// The "before" reading plus everything `build_report` needs to compute the
/// "after" reading without touching the database again (PMV-020/021/026).
pub struct PriceMovementBaseline {
    /// Every account in scope for the report (PMV-030) — one row per account.
    pub accounts: Vec<Account>,
    /// Each account's active holdings, keyed by `account_id`, as they stood at
    /// refresh start — the SAME holdings feed both readings (PMV-020).
    pub holdings_by_account: HashMap<String, Vec<Holding>>,
    /// Prices and rates frozen at refresh start (PMV-020/021). `build_report`
    /// overlays `fetched` on top of `snapshot.prices` — guarded — to produce
    /// the "after" reading; `snapshot.rates` is reused unchanged for both
    /// readings.
    pub snapshot: ValuationSnapshot,
    /// Each account's Global Value computed over `snapshot` before the fetch
    /// (PMV-021), keyed by `account_id`.
    pub before_by_account: HashMap<String, i64>,
    /// The observation date the portfolio carried before this fetch (PMV-050)
    /// — the max `AssetPrice.date` over the in-scope assets at refresh start;
    /// `None` when none carried a price (PMV-052).
    pub observed_from: Option<String>,
    /// Accounts with an active non-cash holding whose currency cannot be
    /// converted — to the holding's account currency, or the account's own
    /// currency to the reference currency (PMV-032 second condition, PMV-042).
    pub rate_missing_accounts: HashSet<String>,
}

/// PMV-011/022/030..052/060 — builds the Price Movement report from the
/// frozen `baseline`, the prices this refresh actually wrote (`fetched`,
/// keyed by `asset_id`), and the MKT-171 skip set (`unpriced_asset_ids`).
pub fn build_report(
    baseline: PriceMovementBaseline,
    fetched: &HashMap<String, AssetPrice>,
    unpriced_asset_ids: &HashSet<String>,
) -> PriceMovementReport {
    let PriceMovementBaseline {
        accounts,
        holdings_by_account,
        snapshot,
        before_by_account,
        observed_from,
        rate_missing_accounts,
    } = baseline;

    // PMV-022 — the "after" prices are the ones THIS refresh left in place, not a
    // re-read (PMV-026). A fetched row only supersedes the baseline when it is at
    // least as recent: `get_latest` orders by date, so a past-dated write leaves
    // the previous price in force (ADR-012). An equal date IS applied — the upsert
    // replaced that very row (MKT-025).
    let mut after_prices = snapshot.prices.clone();
    let mut applied_dates: Vec<&str> = Vec::new();
    for (asset_id, fetched_price) in fetched {
        let supersedes = match snapshot.prices.get(asset_id) {
            None => true,
            Some(baseline_price) => fetched_price.date >= baseline_price.date,
        };
        if supersedes {
            applied_dates.push(&fetched_price.date);
            after_prices.insert(asset_id.clone(), fetched_price.clone());
        }
    }
    let after_snapshot = ValuationSnapshot {
        assets: snapshot.assets,
        prices: after_prices,
        rates: snapshot.rates,
    };

    // PMV-033 — account name ascending, the accounts list's own default (ACC-007).
    let mut accounts = accounts;
    accounts.sort_by(|left, right| left.name.cmp(&right.name));

    let mut rows = Vec::with_capacity(accounts.len());
    let mut total_before: i64 = 0;
    let mut total_after: i64 = 0;
    for account in accounts {
        let holdings = holdings_by_account
            .get(&account.id)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let before = before_by_account.get(&account.id).copied().unwrap_or(0);
        let after = account_global_value(&account.currency, holdings, &after_snapshot);

        // PMV-032 — only a holding that was MEANT to be read at its current price
        // counts. The MKT-171 skip set already excludes system cash (MKT-116) and
        // refresh-locked holdings (MKT-151), so no further filtering is owed here.
        let incomplete = rate_missing_accounts.contains(&account.id)
            || holdings
                .iter()
                .filter(|holding| holding.quantity > 0)
                .any(|holding| unpriced_asset_ids.contains(&holding.asset_id));

        // PMV-040/042 — both totals in the reference currency; an account with no
        // usable rate contributes 0 to each (FXR-034/GPF) while keeping its own row.
        let to_reference = after_snapshot
            .rates
            .get(&(account.currency.clone(), REFERENCE_CURRENCY.to_string()))
            .copied();
        if let Some(rate) = to_reference {
            total_before = total_before.saturating_add(to_reference_currency(before, rate));
            total_after = total_after.saturating_add(to_reference_currency(after, rate));
        }

        rows.push(PriceMovementRow {
            account_id: account.id,
            name: account.name,
            currency: account.currency,
            before,
            after,
            movement_pct: movement_pct(before, after),
            movement_amount: movement_amount(before, after),
            incomplete,
        });
    }

    // PMV-051/052 — the later date is the most recent one this refresh actually
    // applied. It is carried whenever it advances the portfolio, including when
    // nothing was priced before; the two dates are otherwise independent.
    let observed_to = applied_dates
        .into_iter()
        .max()
        .filter(|applied| match observed_from.as_deref() {
            Some(from) => *applied > from,
            None => true,
        })
        .map(str::to_string);

    let incomplete = rows.iter().any(|row| row.incomplete);
    PriceMovementReport {
        total_movement_pct: movement_pct(total_before, total_after),
        total_movement_amount: movement_amount(total_before, total_after),
        rows,
        total_before,
        total_after,
        total_currency: REFERENCE_CURRENCY.to_string(),
        observed_from,
        observed_to,
        incomplete,
    }
}

/// PMV-040 — an account-currency amount in the reference currency.
fn to_reference_currency(amount: i64, rate: i64) -> i64 {
    (amount as i128 * rate as i128 / 1_000_000) as i64
}

/// PMV-024/041 — movement as micro-percent of the earlier value. Absent when the
/// earlier value is not positive (PMV-025/044) or the two are equal (PMV-031/045).
fn movement_pct(before: i64, after: i64) -> Option<i64> {
    if before <= 0 || after == before {
        return None;
    }
    Some(((after as i128 - before as i128) * 100_000_000 / before as i128) as i64)
}

/// PMV-027/046 — movement as a signed amount in the currency of the two values. Absent
/// when they are equal (PMV-031/045); a non-positive earlier value still has one.
fn movement_amount(before: i64, after: i64) -> Option<i64> {
    (after != before).then(|| after.saturating_sub(before))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::UpdateFrequency;
    use crate::context::asset::{AssetClass, AssetPriceSource};
    use crate::use_cases::shared::global_value::AssetValuationFacts;

    fn account(id: &str, name: &str, currency: &str) -> Account {
        Account::restore(
            id.to_string(),
            name.to_string(),
            String::new(),
            currency.to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
    }

    fn holding(asset_id: &str, quantity: i64) -> Holding {
        Holding::restore(
            "holding-1".to_string(),
            "acc-1".to_string(),
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
            AssetPriceSource::YahooFinance,
        )
    }

    fn empty_snapshot() -> ValuationSnapshot {
        ValuationSnapshot {
            assets: HashMap::new(),
            prices: HashMap::new(),
            rates: HashMap::new(),
        }
    }

    // ── PMV-022 — the latest-wins overlay guard ─────────────────────────────

    // PMV-022 — with no baseline price at all, the fetched price is applied.
    #[test]
    fn overlay_applies_when_no_baseline_price_exists() {
        let mut snapshot = empty_snapshot();
        snapshot.assets.insert(
            "asset-1".to_string(),
            AssetValuationFacts {
                currency: "EUR".to_string(),
                class: AssetClass::Stocks,
            },
        );
        snapshot
            .rates
            .insert(("EUR".to_string(), "EUR".to_string()), 1_000_000);

        let baseline = PriceMovementBaseline {
            accounts: vec![account("acc-1", "Alpha", "EUR")],
            holdings_by_account: HashMap::from([(
                "acc-1".to_string(),
                vec![holding("asset-1", 1_000_000)],
            )]),
            snapshot,
            before_by_account: HashMap::from([("acc-1".to_string(), 0)]),
            observed_from: None,
            rate_missing_accounts: HashSet::new(),
        };
        let fetched = HashMap::from([(
            "asset-1".to_string(),
            price("asset-1", "2026-09-11", 100_000_000),
        )]);

        let report = build_report(baseline, &fetched, &HashSet::new());

        assert_eq!(
            report.rows[0].after, 100_000_000,
            "never-priced asset must take the fetched price"
        );
    }

    // PMV-022 / ADR-012 / MKT-118 — a fetched price dated BEFORE the baseline
    // price must be invisible to the "after" reading: get_latest is
    // ORDER BY date DESC LIMIT 1, so a past-dated write changes nothing the
    // application would read.
    #[test]
    fn overlay_rejects_a_fetched_price_dated_before_the_baseline() {
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

        let baseline = PriceMovementBaseline {
            accounts: vec![account("acc-1", "Alpha", "EUR")],
            holdings_by_account: HashMap::from([(
                "acc-1".to_string(),
                vec![holding("asset-1", 1_000_000)],
            )]),
            snapshot,
            before_by_account: HashMap::from([("acc-1".to_string(), 100_000_000)]),
            observed_from: Some("2026-09-10".to_string()),
            rate_missing_accounts: HashSet::new(),
        };
        // A manual entry recorded a stale Friday close, dated BEFORE the stored latest (MKT-118).
        let fetched = HashMap::from([(
            "asset-1".to_string(),
            price("asset-1", "2026-09-05", 50_000_000),
        )]);

        let report = build_report(baseline, &fetched, &HashSet::new());

        assert_eq!(
            report.rows[0].after, 100_000_000,
            "a fetched price dated before the baseline must leave the after-reading unchanged"
        );
        assert_eq!(
            report.rows[0].movement_pct, None,
            "the account did not move once the stale write is ignored (PMV-031)"
        );
    }

    // PMV-022 — a fetched price dated AFTER the baseline is applied (latest wins).
    #[test]
    fn overlay_applies_a_fetched_price_dated_after_the_baseline() {
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

        let baseline = PriceMovementBaseline {
            accounts: vec![account("acc-1", "Alpha", "EUR")],
            holdings_by_account: HashMap::from([(
                "acc-1".to_string(),
                vec![holding("asset-1", 1_000_000)],
            )]),
            snapshot,
            before_by_account: HashMap::from([("acc-1".to_string(), 100_000_000)]),
            observed_from: Some("2026-09-10".to_string()),
            rate_missing_accounts: HashSet::new(),
        };
        let fetched = HashMap::from([(
            "asset-1".to_string(),
            price("asset-1", "2026-09-11", 110_000_000),
        )]);

        let report = build_report(baseline, &fetched, &HashSet::new());

        assert_eq!(report.rows[0].after, 110_000_000);
        assert_eq!(report.rows[0].movement_pct, Some(10_000_000), "10.00% move");
    }

    // PMV-022 / MKT-025 — a fetched price sharing the baseline's own date is
    // applied: the upsert replaced that row, so it IS the latest recorded price.
    #[test]
    fn overlay_applies_a_fetched_price_sharing_the_baseline_date() {
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

        let baseline = PriceMovementBaseline {
            accounts: vec![account("acc-1", "Alpha", "EUR")],
            holdings_by_account: HashMap::from([(
                "acc-1".to_string(),
                vec![holding("asset-1", 1_000_000)],
            )]),
            snapshot,
            before_by_account: HashMap::from([("acc-1".to_string(), 100_000_000)]),
            observed_from: Some("2026-09-10".to_string()),
            rate_missing_accounts: HashSet::new(),
        };
        let fetched = HashMap::from([(
            "asset-1".to_string(),
            price("asset-1", "2026-09-10", 105_000_000),
        )]);

        let report = build_report(baseline, &fetched, &HashSet::new());

        assert_eq!(
            report.rows[0].after, 105_000_000,
            "an equal-date upsert must be applied"
        );
        assert_eq!(report.rows[0].movement_pct, Some(5_000_000), "5.00% move");
    }

    // ── PMV-020 — rates are frozen at refresh start ─────────────────────────

    // PMV-020 — both readings use the rate resolved at refresh start. The
    // "after" reading here can ONLY see `baseline.snapshot.rates` — there is no
    // other rate source in this function's signature — so a later FX refresh
    // (FXR-075) cannot leak into it. This pins the exact converted value.
    #[test]
    fn after_reading_uses_the_rate_frozen_in_the_baseline_snapshot() {
        let mut snapshot = empty_snapshot();
        snapshot.assets.insert(
            "asset-usd".to_string(),
            AssetValuationFacts {
                currency: "USD".to_string(),
                class: AssetClass::Stocks,
            },
        );
        snapshot.prices.insert(
            "asset-usd".to_string(),
            price("asset-usd", "2026-09-01", 100_000_000),
        );
        // Frozen FX rate at refresh start: 0.90 USD→EUR.
        snapshot
            .rates
            .insert(("USD".to_string(), "EUR".to_string()), 900_000);

        let baseline = PriceMovementBaseline {
            accounts: vec![account("acc-1", "Alpha", "EUR")],
            holdings_by_account: HashMap::from([(
                "acc-1".to_string(),
                vec![holding("asset-usd", 1_000_000)],
            )]),
            snapshot,
            before_by_account: HashMap::from([("acc-1".to_string(), 90_000_000)]),
            observed_from: Some("2026-09-01".to_string()),
            rate_missing_accounts: HashSet::new(),
        };
        let fetched = HashMap::from([(
            "asset-usd".to_string(),
            price("asset-usd", "2026-09-11", 120_000_000),
        )]);

        let report = build_report(baseline, &fetched, &HashSet::new());

        // converted_price = 120 USD × 0.90 = 108 EUR; after = 1 unit × 108 EUR = 108_000_000.
        assert_eq!(
            report.rows[0].after, 108_000_000,
            "the after-reading must use the frozen 0.90 rate, not a different one"
        );
        assert_eq!(report.rows[0].movement_pct, Some(20_000_000), "20.00% move");
    }

    // ── PMV-032 — incomplete population ──────────────────────────────────────

    // PMV-032 — the population is the account's ACTIVE holdings. A closed position
    // (quantity <= 0) contributes nothing to either reading, so an unpriced asset it
    // once held must not mark the row incomplete — otherwise an account that sold out
    // of a delisted ticker carries the flag for good.
    #[test]
    fn a_closed_position_in_the_skip_set_does_not_mark_a_row_incomplete() {
        let mut snapshot = empty_snapshot();
        snapshot.assets.insert(
            "asset-x".to_string(),
            AssetValuationFacts {
                currency: "EUR".to_string(),
                class: AssetClass::Stocks,
            },
        );
        let baseline = PriceMovementBaseline {
            accounts: vec![account("acc-1", "Alpha", "EUR")],
            holdings_by_account: HashMap::from([(
                "acc-1".to_string(),
                vec![holding("asset-x", 0)],
            )]),
            snapshot,
            before_by_account: HashMap::from([("acc-1".to_string(), 0)]),
            observed_from: None,
            rate_missing_accounts: HashSet::new(),
        };
        let unpriced = HashSet::from(["asset-x".to_string()]);

        let report = build_report(baseline, &HashMap::new(), &unpriced);

        assert!(
            !report.rows[0].incomplete,
            "a closed position must not mark the row incomplete (PMV-032 population)"
        );
    }

    // PMV-032 — an active holding in the MKT-171 skip set marks its row incomplete.
    #[test]
    fn row_is_incomplete_when_a_holding_is_in_the_unpriced_skip_set() {
        let mut snapshot = empty_snapshot();
        snapshot.assets.insert(
            "asset-x".to_string(),
            AssetValuationFacts {
                currency: "EUR".to_string(),
                class: AssetClass::Stocks,
            },
        );
        let baseline = PriceMovementBaseline {
            accounts: vec![account("acc-1", "Alpha", "EUR")],
            holdings_by_account: HashMap::from([(
                "acc-1".to_string(),
                vec![holding("asset-x", 1_000_000)],
            )]),
            snapshot,
            before_by_account: HashMap::from([("acc-1".to_string(), 0)]),
            observed_from: None,
            rate_missing_accounts: HashSet::new(),
        };
        let unpriced = HashSet::from(["asset-x".to_string()]);

        let report = build_report(baseline, &HashMap::new(), &unpriced);

        assert!(
            report.rows[0].incomplete,
            "an attempted-but-unpriced holding must mark the row incomplete"
        );
    }

    // PMV-032 second condition / PMV-042 — an account with no usable
    // account→EUR rate marks its row incomplete, contributes 0 to both totals,
    // but its OWN row value is unaffected.
    #[test]
    fn unconvertible_account_contributes_zero_to_totals_but_keeps_its_own_row_value_and_marks_incomplete(
    ) {
        let baseline = PriceMovementBaseline {
            accounts: vec![account("acc-1", "Alpha", "GBP")],
            holdings_by_account: HashMap::from([("acc-1".to_string(), vec![])]),
            snapshot: empty_snapshot(),
            before_by_account: HashMap::from([("acc-1".to_string(), 500_000_000)]),
            observed_from: None,
            rate_missing_accounts: HashSet::from(["acc-1".to_string()]),
        };

        let report = build_report(baseline, &HashMap::new(), &HashSet::new());

        assert_eq!(
            report.rows[0].before, 500_000_000,
            "the row's own value is unaffected"
        );
        assert!(report.rows[0].incomplete);
        assert_eq!(
            report.total_before, 0,
            "an unconvertible account contributes 0 to the total (PMV-042)"
        );
        assert_eq!(report.total_after, 0);
        assert!(
            report.incomplete,
            "PMV-043 — the report is incomplete when any row is"
        );
    }

    // PMV-032 — a system cash holding (MKT-116) and a refresh-locked holding
    // (MKT-151) never appear in the skip set, so a row holding only those never
    // marks incomplete — their stale price is the point of the exclusion.
    #[test]
    fn system_cash_and_locked_holdings_never_mark_a_row_incomplete() {
        let mut snapshot = empty_snapshot();
        snapshot.assets.insert(
            "system-cash-eur".to_string(),
            AssetValuationFacts {
                currency: "EUR".to_string(),
                class: AssetClass::Cash,
            },
        );
        snapshot.assets.insert(
            "asset-locked".to_string(),
            AssetValuationFacts {
                currency: "EUR".to_string(),
                class: AssetClass::Stocks,
            },
        );
        let baseline = PriceMovementBaseline {
            accounts: vec![account("acc-1", "Alpha", "EUR")],
            holdings_by_account: HashMap::from([(
                "acc-1".to_string(),
                vec![
                    holding("system-cash-eur", 100_000_000),
                    holding("asset-locked", 1_000_000),
                ],
            )]),
            snapshot,
            before_by_account: HashMap::from([("acc-1".to_string(), 100_000_000)]),
            observed_from: None,
            rate_missing_accounts: HashSet::new(),
        };
        // Neither the cash asset nor the locked asset ever enters the fetch scope,
        // so neither ever appears in the skip set (MKT-116 / MKT-151 / ADR-014).
        let unpriced = HashSet::new();

        let report = build_report(baseline, &HashMap::new(), &unpriced);

        assert!(!report.rows[0].incomplete);
    }

    // PMV-043 — the report is incomplete when at least one row is.
    #[test]
    fn report_incomplete_is_true_when_any_row_is_incomplete() {
        let baseline = PriceMovementBaseline {
            accounts: vec![
                account("acc-1", "Alpha", "EUR"),
                account("acc-2", "Beta", "GBP"),
            ],
            holdings_by_account: HashMap::from([
                ("acc-1".to_string(), vec![]),
                ("acc-2".to_string(), vec![]),
            ]),
            snapshot: empty_snapshot(),
            before_by_account: HashMap::from([("acc-1".to_string(), 0), ("acc-2".to_string(), 0)]),
            observed_from: None,
            rate_missing_accounts: HashSet::from(["acc-2".to_string()]),
        };

        let report = build_report(baseline, &HashMap::new(), &HashSet::new());

        assert!(report.incomplete);
    }

    // PMV-043 — the report is complete when no row is incomplete.
    #[test]
    fn report_incomplete_is_false_when_no_row_is_incomplete() {
        let baseline = PriceMovementBaseline {
            accounts: vec![account("acc-1", "Alpha", "EUR")],
            holdings_by_account: HashMap::from([("acc-1".to_string(), vec![])]),
            snapshot: empty_snapshot(),
            before_by_account: HashMap::from([("acc-1".to_string(), 0)]),
            observed_from: None,
            rate_missing_accounts: HashSet::new(),
        };

        let report = build_report(baseline, &HashMap::new(), &HashSet::new());

        assert!(!report.incomplete);
    }

    // ── PMV-025 / PMV-031 — absence conditions ──────────────────────────────

    // PMV-025 — no proportion is reported when the earlier value is zero or
    // negative; the two values are still shown.
    #[test]
    fn movement_pct_is_none_when_before_is_not_positive() {
        let baseline = PriceMovementBaseline {
            accounts: vec![
                account("acc-1", "Zero", "EUR"),
                account("acc-2", "Negative", "EUR"),
            ],
            holdings_by_account: HashMap::from([
                ("acc-1".to_string(), vec![]),
                ("acc-2".to_string(), vec![]),
            ]),
            snapshot: empty_snapshot(),
            before_by_account: HashMap::from([
                ("acc-1".to_string(), 0),
                ("acc-2".to_string(), -100),
            ]),
            observed_from: None,
            rate_missing_accounts: HashSet::new(),
        };

        let report = build_report(baseline, &HashMap::new(), &HashSet::new());

        for row in &report.rows {
            assert_eq!(
                row.movement_pct, None,
                "row {} must carry no proportion",
                row.name
            );
        }
    }

    // PMV-031 — an unmoved account (before == after, both positive) carries no
    // proportion; the entry stays distinguishable from PMV-025 because both
    // values are shown and both are positive.
    #[test]
    fn movement_pct_is_none_when_unmoved_but_positive() {
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

        let baseline = PriceMovementBaseline {
            accounts: vec![account("acc-1", "Alpha", "EUR")],
            holdings_by_account: HashMap::from([(
                "acc-1".to_string(),
                vec![holding("asset-1", 1_000_000)],
            )]),
            snapshot,
            before_by_account: HashMap::from([("acc-1".to_string(), 100_000_000)]),
            observed_from: Some("2026-09-10".to_string()),
            rate_missing_accounts: HashSet::new(),
        };
        // Nothing fetched for asset-1 this refresh — the after-reading stays at the baseline price.
        let report = build_report(baseline, &HashMap::new(), &HashSet::new());

        assert_eq!(report.rows[0].before, 100_000_000);
        assert_eq!(report.rows[0].after, 100_000_000);
        assert_eq!(report.rows[0].movement_pct, None);
    }

    // ── PMV-027 / PMV-046 — movement as an amount ─────────────────────────────────────

    fn cash_snapshot() -> ValuationSnapshot {
        let mut snapshot = empty_snapshot();
        snapshot.assets.insert(
            "system-cash-eur".to_string(),
            AssetValuationFacts {
                currency: "EUR".to_string(),
                class: AssetClass::Cash,
            },
        );
        snapshot
            .rates
            .insert(("EUR".to_string(), "EUR".to_string()), 1_000_000);
        snapshot
    }

    // PMV-027/046 — each entry carries after minus before, signed, in its own
    // currency; the total carries the difference of the two totals.
    #[test]
    fn movement_amount_is_the_signed_difference_per_entry_and_for_the_total() {
        let baseline = PriceMovementBaseline {
            accounts: vec![
                account("acc-1", "Alpha", "EUR"),
                account("acc-2", "Beta", "EUR"),
            ],
            holdings_by_account: HashMap::from([
                (
                    "acc-1".to_string(),
                    vec![holding("system-cash-eur", 150_000_000)],
                ),
                (
                    "acc-2".to_string(),
                    vec![holding("system-cash-eur", 50_000_000)],
                ),
            ]),
            snapshot: cash_snapshot(),
            before_by_account: HashMap::from([
                ("acc-1".to_string(), 100_000_000),
                ("acc-2".to_string(), 200_000_000),
            ]),
            observed_from: None,
            rate_missing_accounts: HashSet::new(),
        };

        let report = build_report(baseline, &HashMap::new(), &HashSet::new());

        let amount_of = |id: &str| {
            report
                .rows
                .iter()
                .find(|row| row.account_id == id)
                .unwrap()
                .movement_amount
        };
        assert_eq!(amount_of("acc-1"), Some(50_000_000));
        assert_eq!(amount_of("acc-2"), Some(-150_000_000));
        assert_eq!(report.total_movement_amount, Some(-100_000_000));
    }

    // PMV-027 — the amount needs no positive earlier value: it is carried where
    // PMV-025 withholds the proportion.
    #[test]
    fn movement_amount_is_carried_when_the_earlier_value_is_not_positive() {
        let baseline = PriceMovementBaseline {
            accounts: vec![account("acc-1", "Negative", "EUR")],
            holdings_by_account: HashMap::from([("acc-1".to_string(), vec![])]),
            snapshot: cash_snapshot(),
            before_by_account: HashMap::from([("acc-1".to_string(), -100)]),
            observed_from: None,
            rate_missing_accounts: HashSet::new(),
        };

        let report = build_report(baseline, &HashMap::new(), &HashSet::new());

        assert_eq!(report.rows[0].movement_pct, None);
        assert_eq!(report.rows[0].movement_amount, Some(100));
        assert_eq!(report.total_movement_amount, Some(100));
    }

    // PMV-031/045 — equal values carry no amount: an unmoved entry, and a total whose
    // entries moved in opposite directions that cancel out.
    #[test]
    fn movement_amount_is_absent_for_an_unmoved_entry_and_an_unmoved_total() {
        let baseline = PriceMovementBaseline {
            accounts: vec![
                account("acc-1", "Alpha", "EUR"),
                account("acc-2", "Beta", "EUR"),
                account("acc-3", "Gamma", "EUR"),
            ],
            holdings_by_account: HashMap::from([
                (
                    "acc-1".to_string(),
                    vec![holding("system-cash-eur", 150_000_000)],
                ),
                (
                    "acc-2".to_string(),
                    vec![holding("system-cash-eur", 50_000_000)],
                ),
                (
                    "acc-3".to_string(),
                    vec![holding("system-cash-eur", 100_000_000)],
                ),
            ]),
            snapshot: cash_snapshot(),
            before_by_account: HashMap::from([
                ("acc-1".to_string(), 100_000_000),
                ("acc-2".to_string(), 100_000_000),
                ("acc-3".to_string(), 100_000_000),
            ]),
            observed_from: None,
            rate_missing_accounts: HashSet::new(),
        };

        let report = build_report(baseline, &HashMap::new(), &HashSet::new());

        let amounts: Vec<Option<i64>> = report.rows.iter().map(|row| row.movement_amount).collect();
        assert_eq!(amounts, vec![Some(50_000_000), Some(-50_000_000), None]);
        assert_eq!(report.total_before, 300_000_000);
        assert_eq!(report.total_after, 300_000_000);
        assert_eq!(report.total_movement_amount, None);
    }

    // ── PMV-030 / PMV-033 / PMV-034 — population, order, currency ──────────

    // PMV-030/033 — every account gets exactly one row, ordered by name
    // ascending regardless of input order.
    #[test]
    fn rows_cover_every_account_sorted_by_name_ascending() {
        let baseline = PriceMovementBaseline {
            accounts: vec![
                account("acc-z", "Zeta", "EUR"),
                account("acc-a", "Alpha", "EUR"),
                account("acc-m", "Mid", "EUR"),
            ],
            holdings_by_account: HashMap::from([
                ("acc-z".to_string(), vec![]),
                ("acc-a".to_string(), vec![]),
                ("acc-m".to_string(), vec![]),
            ]),
            snapshot: empty_snapshot(),
            before_by_account: HashMap::from([
                ("acc-z".to_string(), 10),
                ("acc-a".to_string(), 0),
                ("acc-m".to_string(), 20),
            ]),
            observed_from: None,
            rate_missing_accounts: HashSet::new(),
        };

        let report = build_report(baseline, &HashMap::new(), &HashSet::new());

        assert_eq!(
            report.rows.len(),
            3,
            "every account gets one row, including price-free ones"
        );
        let names: Vec<&str> = report.rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["Alpha", "Mid", "Zeta"]);
    }

    // PMV-034 — each row is expressed in its OWN account's currency, never converted.
    #[test]
    fn each_row_states_its_own_account_currency_not_converted() {
        let baseline = PriceMovementBaseline {
            accounts: vec![
                account("acc-eur", "Euro Acct", "EUR"),
                account("acc-usd", "Dollar Acct", "USD"),
            ],
            holdings_by_account: HashMap::from([
                ("acc-eur".to_string(), vec![]),
                ("acc-usd".to_string(), vec![]),
            ]),
            snapshot: empty_snapshot(),
            before_by_account: HashMap::from([
                ("acc-eur".to_string(), 100_000_000),
                ("acc-usd".to_string(), 100_000_000),
            ]),
            observed_from: None,
            rate_missing_accounts: HashSet::new(),
        };

        let report = build_report(baseline, &HashMap::new(), &HashSet::new());

        let eur_row = report
            .rows
            .iter()
            .find(|r| r.account_id == "acc-eur")
            .unwrap();
        let usd_row = report
            .rows
            .iter()
            .find(|r| r.account_id == "acc-usd")
            .unwrap();
        assert_eq!(eur_row.currency, "EUR");
        assert_eq!(usd_row.currency, "USD");
        assert_eq!(
            usd_row.before, 100_000_000,
            "no conversion applied to the row's own value"
        );
    }

    // ── PMV-040 / PMV-041 — portfolio total, derived from the totals ───────

    // PMV-041 — the total's movement is derived from the two converted totals,
    // never by combining the per-account proportions. Here the naive average
    // of the two row percentages (+100% and -75%) is -87.5%; the total-based
    // figure is a different number, proving the totals — not the row
    // percentages — feed the calculation.
    #[test]
    fn total_movement_pct_is_derived_from_the_totals_not_from_row_percentages() {
        let mut snapshot = empty_snapshot();
        snapshot.assets.insert(
            "system-cash-eur".to_string(),
            AssetValuationFacts {
                currency: "EUR".to_string(),
                class: AssetClass::Cash,
            },
        );
        snapshot
            .rates
            .insert(("EUR".to_string(), "EUR".to_string()), 1_000_000);

        let baseline = PriceMovementBaseline {
            accounts: vec![
                account("acc-1", "Alpha", "EUR"),
                account("acc-2", "Beta", "EUR"),
            ],
            holdings_by_account: HashMap::from([
                ("acc-1".to_string(), vec![]), // no holdings → after = 0 (row pct = -100%)
                (
                    "acc-2".to_string(),
                    vec![holding("system-cash-eur", 50_000_000)],
                ), // after = 50M (row pct = -75%)
            ]),
            snapshot,
            before_by_account: HashMap::from([
                ("acc-1".to_string(), 100_000_000),
                ("acc-2".to_string(), 200_000_000),
            ]),
            observed_from: None,
            rate_missing_accounts: HashSet::new(),
        };

        let report = build_report(baseline, &HashMap::new(), &HashSet::new());

        assert_eq!(report.total_currency, "EUR");
        assert_eq!(report.total_before, 300_000_000);
        assert_eq!(report.total_after, 50_000_000);
        let naive_row_average = -87_500_000; // (-100% + -75%) / 2, NOT the expected result
        assert_ne!(
            report.total_movement_pct,
            Some(naive_row_average),
            "the total percentage must not equal the naive average of the row percentages"
        );
        // (50M − 300M) × 100_000_000 / 300M, truncated toward zero.
        assert_eq!(report.total_movement_pct, Some(-83_333_333));
    }

    // PMV-044 — no portfolio proportion when the earlier total is zero or negative.
    #[test]
    fn total_movement_pct_is_none_when_total_before_is_not_positive() {
        let baseline = PriceMovementBaseline {
            accounts: vec![account("acc-1", "Alpha", "EUR")],
            holdings_by_account: HashMap::from([("acc-1".to_string(), vec![])]),
            snapshot: empty_snapshot(),
            before_by_account: HashMap::from([("acc-1".to_string(), 0)]),
            observed_from: None,
            rate_missing_accounts: HashSet::new(),
        };

        let report = build_report(baseline, &HashMap::new(), &HashSet::new());

        assert_eq!(report.total_before, 0);
        assert_eq!(report.total_movement_pct, None);
    }

    // PMV-045 — an unmoved portfolio total carries no proportion, even though
    // individual accounts moved in opposite directions that cancel out; the
    // per-account entries still carry their own movement.
    #[test]
    fn total_movement_pct_is_none_when_totals_are_equal_despite_offsetting_row_movements() {
        let mut snapshot = empty_snapshot();
        snapshot.assets.insert(
            "system-cash-eur".to_string(),
            AssetValuationFacts {
                currency: "EUR".to_string(),
                class: AssetClass::Cash,
            },
        );
        snapshot
            .rates
            .insert(("EUR".to_string(), "EUR".to_string()), 1_000_000);

        let baseline = PriceMovementBaseline {
            accounts: vec![
                account("acc-1", "Alpha", "EUR"),
                account("acc-2", "Beta", "EUR"),
            ],
            holdings_by_account: HashMap::from([
                (
                    "acc-1".to_string(),
                    vec![holding("system-cash-eur", 150_000_000)],
                ), // +50%
                (
                    "acc-2".to_string(),
                    vec![holding("system-cash-eur", 100_000_000)],
                ), // -33.33%
            ]),
            snapshot,
            before_by_account: HashMap::from([
                ("acc-1".to_string(), 100_000_000),
                ("acc-2".to_string(), 150_000_000),
            ]),
            observed_from: None,
            rate_missing_accounts: HashSet::new(),
        };

        let report = build_report(baseline, &HashMap::new(), &HashSet::new());

        assert_eq!(report.total_before, 250_000_000);
        assert_eq!(report.total_after, 250_000_000);
        assert_eq!(
            report.total_movement_pct, None,
            "equal totals report no portfolio proportion"
        );
        assert_eq!(
            report
                .rows
                .iter()
                .find(|r| r.account_id == "acc-1")
                .unwrap()
                .movement_pct,
            Some(50_000_000)
        );
        assert!(report
            .rows
            .iter()
            .find(|r| r.account_id == "acc-2")
            .unwrap()
            .movement_pct
            .is_some());
    }

    // ── PMV-050 / PMV-051 / PMV-052 — observation dates ─────────────────────

    // PMV-050 — both dates present when the fetch produced a date strictly
    // later than what the portfolio carried before.
    #[test]
    fn both_dates_are_stated_when_the_fetch_produced_a_later_date() {
        let mut snapshot = empty_snapshot();
        snapshot.assets.insert(
            "asset-1".to_string(),
            AssetValuationFacts {
                currency: "EUR".to_string(),
                class: AssetClass::Stocks,
            },
        );
        let baseline = PriceMovementBaseline {
            accounts: vec![account("acc-1", "Alpha", "EUR")],
            holdings_by_account: HashMap::from([("acc-1".to_string(), vec![])]),
            snapshot,
            before_by_account: HashMap::from([("acc-1".to_string(), 0)]),
            observed_from: Some("2026-09-05".to_string()),
            rate_missing_accounts: HashSet::new(),
        };
        let fetched = HashMap::from([(
            "asset-1".to_string(),
            price("asset-1", "2026-09-11", 100_000_000),
        )]);

        let report = build_report(baseline, &fetched, &HashSet::new());

        assert_eq!(report.observed_from, Some("2026-09-05".to_string()));
        assert_eq!(report.observed_to, Some("2026-09-11".to_string()));
    }

    // PMV-051 — the refresh obtained a date, but not later than the one the
    // portfolio already carried: only the single earlier date is shown. This
    // says nothing about whether values moved (PMV-060 decides that).
    #[test]
    fn observed_to_is_absent_when_the_fetch_produced_no_later_date() {
        let mut snapshot = empty_snapshot();
        snapshot.assets.insert(
            "asset-1".to_string(),
            AssetValuationFacts {
                currency: "EUR".to_string(),
                class: AssetClass::Stocks,
            },
        );
        let baseline = PriceMovementBaseline {
            accounts: vec![account("acc-1", "Alpha", "EUR")],
            holdings_by_account: HashMap::from([("acc-1".to_string(), vec![])]),
            snapshot,
            // The portfolio's newest baseline observation is 2026-09-10 (from a
            // different asset than the one fetched below).
            observed_from: Some("2026-09-10".to_string()),
            before_by_account: HashMap::from([("acc-1".to_string(), 0)]),
            rate_missing_accounts: HashSet::new(),
        };
        // asset-1 has no baseline price, so the overlay guard applies this fetched
        // entry — but its date (09-08) is not later than the portfolio's observed_from.
        let fetched = HashMap::from([(
            "asset-1".to_string(),
            price("asset-1", "2026-09-08", 100_000_000),
        )]);

        let report = build_report(baseline, &fetched, &HashSet::new());

        assert_eq!(report.observed_from, Some("2026-09-10".to_string()));
        assert_eq!(report.observed_to, None);
    }

    // PMV-052 (amended) — no prior price anywhere in scope: observed_from is
    // absent, but a date the refresh DID produce is still stated.
    #[test]
    fn observed_to_is_stated_even_when_observed_from_is_absent() {
        let mut snapshot = empty_snapshot();
        snapshot.assets.insert(
            "asset-1".to_string(),
            AssetValuationFacts {
                currency: "EUR".to_string(),
                class: AssetClass::Stocks,
            },
        );
        let baseline = PriceMovementBaseline {
            accounts: vec![account("acc-1", "Alpha", "EUR")],
            holdings_by_account: HashMap::from([("acc-1".to_string(), vec![])]),
            snapshot,
            observed_from: None,
            before_by_account: HashMap::from([("acc-1".to_string(), 0)]),
            rate_missing_accounts: HashSet::new(),
        };
        let fetched = HashMap::from([(
            "asset-1".to_string(),
            price("asset-1", "2026-09-11", 100_000_000),
        )]);

        let report = build_report(baseline, &fetched, &HashSet::new());

        assert_eq!(report.observed_from, None);
        assert_eq!(report.observed_to, Some("2026-09-11".to_string()));
    }

    // PMV-052 — when neither the baseline nor the fetch carries any dated
    // price, the report carries no date at all.
    #[test]
    fn no_dates_are_stated_when_neither_baseline_nor_fetch_carries_a_price() {
        let baseline = PriceMovementBaseline {
            accounts: vec![account("acc-1", "Alpha", "EUR")],
            holdings_by_account: HashMap::from([("acc-1".to_string(), vec![])]),
            snapshot: empty_snapshot(),
            observed_from: None,
            before_by_account: HashMap::from([("acc-1".to_string(), 0)]),
            rate_missing_accounts: HashSet::new(),
        };

        let report = build_report(baseline, &HashMap::new(), &HashSet::new());

        assert_eq!(report.observed_from, None);
        assert_eq!(report.observed_to, None);
    }
}
