//! What differs by country (ADR-025): one file per country states only what differs from the
//! generic defaults of [`CountryRules`]; adding a country is its file and one line in
//! [`COUNTRY_RULES`].

use super::wrapper::Wrapper;

mod de;
mod es;
mod fr;
mod it;
mod pl;
mod uk;
mod us;

/// How the cost of the units sold is chosen (#514): their weighted average, or the first
/// bought first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostBasisMethod {
    /// The average cost of every unit held (VWAP).
    WeightedAverage,
    /// The units bought first are sold first.
    Fifo,
}

/// The rules a country's file states; each method's default is the generic one.
pub trait CountryRules: Sync {
    /// Its ISO 3166-1 alpha-2 code.
    fn code(&self) -> &'static str;

    /// The day its tax year starts, as (month, day): 1 January unless it says otherwise.
    fn tax_year_start(&self) -> (u32, u32) {
        (1, 1)
    }

    /// The cost basis method its realized gains follow by default; none where its own method
    /// is not planned yet (#395: weighted average and FIFO only).
    fn default_cost_basis(&self) -> Option<CostBasisMethod> {
        None
    }

    /// Its own wrappers (ACC-040); none unless it lists some.
    fn wrappers(&self) -> &'static [Wrapper] {
        &[]
    }
}

/// Every country with rules of its own, in the order its wrappers are listed (ACC-045).
pub static COUNTRY_RULES: &[&dyn CountryRules] = &[
    &fr::France,
    &de::Germany,
    &es::Spain,
    &it::Italy,
    &pl::Poland,
    &uk::UnitedKingdom,
    &us::UnitedStates,
];

/// The rules of `code`, when it has rules of its own.
pub fn rules_of(code: &str) -> Option<&'static dyn CountryRules> {
    COUNTRY_RULES
        .iter()
        .copied()
        .find(|rules| rules.code() == code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::domain::wrapper::COUNTRIES;
    use chrono::NaiveDate;

    // ADR-025 — a country is its file and one line of the list: every one has a real code,
    // once; its wrappers are its own; its tax year starts on a real day.
    #[test]
    fn every_country_of_the_list_is_whole() {
        let mut codes: Vec<&str> = COUNTRY_RULES.iter().map(|rules| rules.code()).collect();
        for rules in COUNTRY_RULES {
            let code = rules.code();
            assert!(COUNTRIES.contains(&code), "{code} is no ISO 3166-1 code");
            let (month, day) = rules.tax_year_start();
            assert!(
                NaiveDate::from_ymd_opt(2024, month, day).is_some(),
                "{code}"
            );
            for wrapper in rules.wrappers() {
                assert_eq!(wrapper.country, Some(code), "{}", wrapper.id);
            }
            assert_eq!(rules_of(code).map(|found| found.code()), Some(code));
        }
        let count = codes.len();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), count, "a country is listed twice");
        assert!(rules_of("ZW").is_none());
    }

    // #764 — the cost basis each country implies: France and Italy the weighted average,
    // Germany, Spain and Poland FIFO; the UK and the US none yet.
    #[test]
    fn each_country_implies_its_cost_basis() {
        let method = |code| rules_of(code).and_then(|rules| rules.default_cost_basis());
        use CostBasisMethod::*;
        assert_eq!(
            ["FR", "IT", "DE", "ES", "PL", "GB", "US"].map(method),
            [
                Some(WeightedAverage),
                Some(WeightedAverage),
                Some(Fifo),
                Some(Fifo),
                Some(Fifo),
                None,
                None
            ]
        );
    }

    // The UK's tax year starts on 6 April; the others on 1 January.
    #[test]
    fn the_uk_tax_year_starts_in_april() {
        assert_eq!(
            rules_of("GB").map(|rules| rules.tax_year_start()),
            Some((4, 6))
        );
        assert_eq!(
            rules_of("FR").map(|rules| rules.tax_year_start()),
            Some((1, 1))
        );
    }
}
