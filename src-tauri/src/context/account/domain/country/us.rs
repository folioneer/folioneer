//! The United States (ADR-025): the calendar tax year. Its short and long term split and its
//! lot identification are its own, left until planned (#395), so it implies no cost basis yet.

use super::CountryRules;

/// The United States' rules.
pub struct UnitedStates;

impl CountryRules for UnitedStates {
    fn code(&self) -> &'static str {
        "US"
    }
}
