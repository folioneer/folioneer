use chrono::{Datelike, Duration, NaiveDate, Weekday};

use super::asset::{AssetClass, AssetKind};

/// MKT-205 — whether a held asset's price, observed on `price_date`, is outdated on `today`:
/// more business days (Monday to Friday) have passed since it than the asset's threshold
/// allows. Whatever recorded the price, the judgement is the same.
pub fn is_price_outdated(
    kind: &AssetKind,
    class: &AssetClass,
    price_date: NaiveDate,
    today: NaiveDate,
) -> bool {
    match outdated_after(kind, class) {
        Some(threshold) => business_days_after(price_date, today) > threshold,
        None => false,
    }
}

/// MKT-205 — the business days a price stays current: 10 for a listed fund, 5 for any other
/// listed or crypto asset; `None` for what is never outdated — a custom or cash asset, and
/// real estate.
fn outdated_after(kind: &AssetKind, class: &AssetClass) -> Option<i64> {
    match (kind, class) {
        (AssetKind::Custom | AssetKind::Cash, _) | (_, AssetClass::RealEstate) => None,
        (_, AssetClass::MutualFunds) => Some(10),
        _ => Some(5),
    }
}

/// The business days after `from` up to `to` included; 0 when `to` is not after `from`.
fn business_days_after(from: NaiveDate, to: NaiveDate) -> i64 {
    let days = (to - from).num_days();
    if days <= 0 {
        return 0;
    }
    let weeks = days / 7;
    let rest = (1..=days % 7)
        .map(|offset| from + Duration::days(weeks * 7 + offset))
        .filter(|day| !matches!(day.weekday(), Weekday::Sat | Weekday::Sun))
        .count() as i64;
    weeks * 5 + rest
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").expect("a test date")
    }

    // MKT-205 — a Friday's price is current the next Friday and outdated the Monday after.
    #[test]
    fn mkt_205_a_listed_price_is_outdated_past_five_business_days() {
        let friday = date("2026-10-09");
        let listed = (AssetKind::Listed, AssetClass::Stocks);
        assert!(!is_price_outdated(
            &listed.0,
            &listed.1,
            friday,
            date("2026-10-16")
        ));
        assert!(is_price_outdated(
            &listed.0,
            &listed.1,
            friday,
            date("2026-10-19")
        ));
        assert!(!is_price_outdated(
            &AssetKind::Listed,
            &AssetClass::ETF,
            friday,
            friday
        ));
        assert!(is_price_outdated(
            &AssetKind::Crypto,
            &AssetClass::DigitalAsset,
            friday,
            date("2026-10-19")
        ));
    }

    // MKT-205 — a listed fund publishes less often: ten business days.
    #[test]
    fn mkt_205_a_fund_price_is_outdated_past_ten_business_days() {
        let friday = date("2026-10-09");
        let fund = (AssetKind::Listed, AssetClass::MutualFunds);
        assert!(!is_price_outdated(
            &fund.0,
            &fund.1,
            friday,
            date("2026-10-23")
        ));
        assert!(is_price_outdated(
            &fund.0,
            &fund.1,
            friday,
            date("2026-10-26")
        ));
    }

    // MKT-205 — custom, cash and real estate are never outdated, however old the price.
    #[test]
    fn mkt_205_custom_cash_and_real_estate_are_never_outdated() {
        let old = date("2024-01-02");
        let today = date("2026-10-19");
        assert!(!is_price_outdated(
            &AssetKind::Custom,
            &AssetClass::MutualFunds,
            old,
            today
        ));
        assert!(!is_price_outdated(
            &AssetKind::Cash,
            &AssetClass::Cash,
            old,
            today
        ));
        assert!(!is_price_outdated(
            &AssetKind::Listed,
            &AssetClass::RealEstate,
            old,
            today
        ));
    }

    // MKT-205 — counting business days over long spans and a price dated after today.
    #[test]
    fn mkt_205_business_days_are_counted_week_by_week() {
        assert_eq!(
            business_days_after(date("2026-10-09"), date("2026-10-09")),
            0
        );
        assert_eq!(
            business_days_after(date("2026-10-09"), date("2026-10-12")),
            1
        );
        assert_eq!(
            business_days_after(date("2026-10-09"), date("2026-10-30")),
            15
        );
        assert_eq!(
            business_days_after(date("2026-10-10"), date("2026-10-11")),
            0
        );
        assert_eq!(
            business_days_after(date("2026-10-19"), date("2026-10-09")),
            0
        );
    }
}
