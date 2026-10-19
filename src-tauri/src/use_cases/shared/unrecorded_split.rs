//! MKT-231 — which reported splits an account holding the asset has not recorded. Pure:
//! the account details view and the agent connection both read it, so it lives here.

use chrono::NaiveDate;
use serde::Serialize;
use specta::Type;

use crate::context::account::{Account, Transaction, TransactionType};
use crate::context::asset::ReportedSplit;

/// A reported split an account holding the asset has not recorded (MKT-231). A read
/// model, never persisted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UnrecordedSplit {
    /// The reported split's date, ISO `yyyy-mm-dd`.
    pub date: String,
    /// New shares of its split factor (SPL-062).
    pub new: i64,
    /// Old shares of its split factor (SPL-062).
    pub old: i64,
}

/// MKT-231 — the reported splits of `asset_id` that the account whose `transactions`
/// these are has not recorded, oldest first: dated on or before `today`, held the day
/// before (its transactions dated before the split replayed to a quantity above zero),
/// and no split of the asset recorded on that date, whatever its factor.
pub fn unrecorded_splits(
    transactions: &[Transaction],
    asset_id: &str,
    reported: &[ReportedSplit],
    today: NaiveDate,
) -> Vec<UnrecordedSplit> {
    let mut unrecorded: Vec<UnrecordedSplit> = reported
        .iter()
        .filter(|split| split.asset_id == asset_id)
        .filter_map(|split| {
            let date = NaiveDate::parse_from_str(&split.date, "%Y-%m-%d").ok()?;
            let day_before = date.pred_opt()?.format("%Y-%m-%d").to_string();
            let recorded = transactions.iter().any(|transaction| {
                transaction.asset_id == asset_id
                    && transaction.transaction_type == TransactionType::Split
                    && transaction.date == split.date
            });
            let held = Account::reconstruct_holding_as_of(transactions, asset_id, &day_before)
                .quantity
                > 0;
            (date <= today && held && !recorded).then(|| UnrecordedSplit {
                date: split.date.clone(),
                new: split.new,
                old: split.old,
            })
        })
        .collect();
    unrecorded.sort_by(|left, right| left.date.cmp(&right.date));
    unrecorded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::use_cases::shared::valuation::test_fixtures::transaction_on;

    const ASSET: &str = "asset-1";

    fn day(raw: &str) -> NaiveDate {
        NaiveDate::parse_from_str(raw, "%Y-%m-%d").expect("date")
    }

    fn reported(date: &str, new: i64, old: i64) -> ReportedSplit {
        ReportedSplit {
            asset_id: ASSET.to_string(),
            date: date.to_string(),
            new,
            old,
        }
    }

    fn bought_on(date: &str) -> Transaction {
        transaction_on(ASSET, TransactionType::Purchase, date, 10_000_000)
    }

    // MKT-231 — a split reported while the account held the asset, and not recorded, is
    // unrecorded, with its date and factor.
    #[test]
    fn a_reported_split_the_account_held_through_and_did_not_record_is_unrecorded() {
        let found = unrecorded_splits(
            &[bought_on("2019-03-01")],
            ASSET,
            &[reported("2022-07-18", 20, 1)],
            day("2026-10-19"),
        );
        assert_eq!(
            found,
            vec![UnrecordedSplit {
                date: "2022-07-18".into(),
                new: 20,
                old: 1
            }]
        );
    }

    // MKT-231 — a split recorded on that date, whatever its factor, ends it.
    #[test]
    fn a_split_recorded_on_its_date_whatever_its_factor_is_not_unrecorded() {
        let transactions = vec![
            bought_on("2019-03-01"),
            transaction_on(ASSET, TransactionType::Split, "2022-07-18", 10_000_000),
        ];
        let found = unrecorded_splits(
            &transactions,
            ASSET,
            &[reported("2022-07-18", 20, 1)],
            day("2026-10-19"),
        );
        assert!(found.is_empty());
    }

    // MKT-231 — not held the day before: bought on the split's date or later, or not yet.
    #[test]
    fn a_split_before_the_holding_or_on_its_first_day_is_not_unrecorded() {
        let found = unrecorded_splits(
            &[bought_on("2022-07-18")],
            ASSET,
            &[reported("2020-01-10", 2, 1), reported("2022-07-18", 20, 1)],
            day("2026-10-19"),
        );
        assert!(found.is_empty());
    }

    // MKT-231 — a split dated after today is not unrecorded yet.
    #[test]
    fn a_split_dated_after_today_is_not_unrecorded() {
        let found = unrecorded_splits(
            &[bought_on("2019-03-01")],
            ASSET,
            &[reported("2026-10-20", 2, 1)],
            day("2026-10-19"),
        );
        assert!(found.is_empty());
    }

    // MKT-231 — oldest first; another asset's splits are not this one's.
    #[test]
    fn unrecorded_splits_come_oldest_first_and_only_for_the_asset() {
        let mut other = reported("2021-01-04", 3, 1);
        other.asset_id = "asset-2".into();
        let found = unrecorded_splits(
            &[bought_on("2019-03-01")],
            ASSET,
            &[
                reported("2024-03-04", 1, 10),
                other,
                reported("2022-07-18", 20, 1),
            ],
            day("2026-10-19"),
        );
        let dates: Vec<&str> = found.iter().map(|split| split.date.as_str()).collect();
        assert_eq!(dates, vec!["2022-07-18", "2024-03-04"]);
    }
}
